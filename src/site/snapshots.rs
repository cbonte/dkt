//! A recording of a program that takes the whole screen, as a series of SVG snapshots of the
//! screen, 1 per step.
//!
//! The events of the recording go through a terminal screen.
//! Events less than `STEP_GAP` apart make 1 step, and a step that changes nothing on the screen
//! is left out.
//! Each snapshot is text in a grid of fixed cells, with the colours of the cells.
//! A row is drawn once, in the definitions of the widget, and each snapshot that shows it uses
//! it again.

use std::collections::HashMap;
use std::fmt::Write;

use anyhow::{Result, bail};

use super::fromfile::{Event, Recording};
use crate::screen::{Cell, Color, Screen, Style};

/// The most steps a session gives, so that a page stays light.
pub const MAX_STEPS: usize = 200;

/// The largest screen a session may have.
pub const MAX_COLUMNS: usize = 300;
pub const MAX_ROWS: usize = 100;

/// Events closer than this, in seconds, make 1 step.
const STEP_GAP: f64 = 0.05;

/// The longest pause between 2 steps, in seconds, when the recording sets none.
const IDLE: f64 = 2.0;

/// The width and the height of a cell, in the units of the drawing.
const CELL_W: usize = 6;
const CELL_H: usize = 12;

/// The distance from the top of a cell to the baseline of its text.
const BASELINE: usize = 9;

/// The size of the drawing on the page, for each unit of it, in tenths of a pixel.
const SCALE: usize = 14;

/// The look of the snapshots.
///
/// The 16 colours come from the custom properties of an `ansi` block, with the same fallbacks.
/// A page that loads no script shows the last snapshot alone.
const STYLE: &str = "<style>\
.dkt-snap{display:block;max-width:100%;height:auto;margin:0 0 8px;border-radius:10px;background:#0b1020;fill:#e8ecf6;font-family:var(--mono,ui-monospace,monospace);font-size:10px}\
.dkt-snaps:not(.dkt-live)>.dkt-snap:not(:last-child),.dkt-snaps.dkt-live>.dkt-snap:not(.dkt-on){display:none}\
.dkt-s0{fill:var(--a0,#8b93a1)}.dkt-s1{fill:var(--a1,#f87171)}.dkt-s2{fill:var(--a2,#4ade80)}.dkt-s3{fill:var(--a3,#fbbf24)}\
.dkt-s4{fill:var(--a4,#60a5fa)}.dkt-s5{fill:var(--a5,#c084fc)}.dkt-s6{fill:var(--a6,#22d3ee)}.dkt-s7{fill:var(--a7,#cbd5e1)}\
.dkt-s8{fill:var(--a8,#9aa3b2)}.dkt-s9{fill:var(--a9,#fca5a5)}.dkt-s10{fill:var(--a10,#86efac)}.dkt-s11{fill:var(--a11,#fde68a)}\
.dkt-s12{fill:var(--a12,#93c5fd)}.dkt-s13{fill:var(--a13,#d8b4fe)}.dkt-s14{fill:var(--a14,#67e8f9)}.dkt-s15{fill:var(--a15,#f8fafc)}\
.dkt-sf{fill:#e8ecf6}.dkt-sk{fill:#0b1020}\
.dkt-sb{font-weight:700}.dkt-sd{fill-opacity:.7}.dkt-si{font-style:italic}\
.dkt-su{text-decoration:underline}.dkt-ss{text-decoration:line-through}.dkt-su.dkt-ss{text-decoration:underline line-through}\
.dkt-sc{fill:#e8ecf6;fill-opacity:.6}\
</style>";

/// What a step shows: the size of the screen, the drawing of each row and the cursor.
#[derive(PartialEq)]
struct Frame {
    cols: usize,
    rows: usize,
    lines: Vec<String>,
    cursor: Option<(usize, usize)>,
}

/// A step: what it shows, and the pause before it in milliseconds.
struct Step {
    frame: Frame,
    delay: u64,
}

/// Whether the recording takes the whole screen: it places the cursor, clears the screen or
/// shows the alternate screen.
pub fn takes_the_screen(recording: &Recording) -> bool {
    // The size is held to the largest one, since only the sequences matter here.
    let mut screen = Screen::new(
        recording.cols.min(MAX_COLUMNS),
        recording.rows.min(MAX_ROWS),
    );
    for event in &recording.events {
        if let Event::Output(_, text) = event {
            screen.feed(text);
            if screen.addressed() {
                return true;
            }
        }
    }
    false
}

/// The snapshots of a recording, as HTML. `prefix` makes the names of the rows unique in the page.
pub fn html(recording: &Recording, prefix: &str) -> Result<String> {
    let steps = steps(recording)?;
    let mut ids: HashMap<&str, usize> = HashMap::new();
    let mut defs = String::new();
    for step in &steps {
        for line in &step.frame.lines {
            if line.is_empty() || ids.contains_key(line.as_str()) {
                continue;
            }
            let id = ids.len();
            ids.insert(line, id);
            let _ = write!(defs, "<g id=\"{prefix}-{id}\">{line}</g>");
        }
    }
    let mut out = format!(
        "<div class=\"dkt-snaps\">\n<svg class=\"dkt-snap-defs\" width=\"0\" height=\"0\" style=\"position:absolute\" aria-hidden=\"true\">{STYLE}<defs>{defs}</defs></svg>\n"
    );
    let count = steps.len();
    for (n, step) in steps.iter().enumerate() {
        let frame = &step.frame;
        let (w, h) = (frame.cols * CELL_W, frame.rows * CELL_H);
        let _ = write!(
            out,
            "<svg class=\"dkt-snap\" viewBox=\"-{CELL_W} -{CELL_W} {} {}\" width=\"{}\" height=\"{}\" role=\"img\" aria-label=\"A terminal screen, step {} of {count}\" data-ms=\"{}\">",
            w + 2 * CELL_W,
            h + 2 * CELL_W,
            tenths((w + 2 * CELL_W) * SCALE),
            tenths((h + 2 * CELL_W) * SCALE),
            n + 1,
            step.delay,
        );
        for (row, line) in frame.lines.iter().enumerate() {
            if let Some(id) = ids.get(line.as_str()) {
                let _ = write!(
                    out,
                    "<use href=\"#{prefix}-{id}\" y=\"{}\"/>",
                    row * CELL_H + BASELINE
                );
            }
        }
        if let Some((row, col)) = frame.cursor {
            let _ = write!(
                out,
                "<rect class=\"dkt-sc\" x=\"{}\" y=\"{}\" width=\"{CELL_W}\" height=\"{CELL_H}\"/>",
                col * CELL_W,
                row * CELL_H
            );
        }
        out.push_str("</svg>\n");
    }
    out.push_str("</div>\n");
    Ok(out)
}

/// A number of tenths, written as a decimal number.
fn tenths(n: usize) -> String {
    if n.is_multiple_of(10) {
        (n / 10).to_string()
    } else {
        format!("{}.{}", n / 10, n % 10)
    }
}

/// The steps of a recording, each with the screen at its end.
fn steps(recording: &Recording) -> Result<Vec<Step>> {
    let idle = recording.idle.unwrap_or(IDLE);
    check_size(recording.cols, recording.rows)?;
    let mut screen = Screen::new(recording.cols, recording.rows);
    let mut steps: Vec<Step> = Vec::new();
    let mut shown = 0.0;
    let events = &recording.events;
    for (i, event) in events.iter().enumerate() {
        match event {
            Event::Output(_, text) => screen.feed(text),
            Event::Resize(_, cols, rows) => {
                check_size(*cols, *rows)?;
                screen.resize(*cols, *rows);
            }
        }
        let time = event.time();
        let last_of_step = events
            .get(i + 1)
            .is_none_or(|next| next.time() - time >= STEP_GAP);
        if !last_of_step {
            continue;
        }
        let frame = frame_of(&screen);
        if steps.last().is_some_and(|s| s.frame == frame) {
            continue;
        }
        if steps.len() == MAX_STEPS {
            bail!(
                "the session has more than {MAX_STEPS} steps, the most a page shows: cut the recording"
            );
        }
        let pause = (time - shown).clamp(0.0, idle);
        steps.push(Step {
            frame,
            delay: (pause * 1000.0).round() as u64,
        });
        shown = time;
    }
    if steps.is_empty() {
        bail!("the recording has no output");
    }
    Ok(steps)
}

fn check_size(cols: usize, rows: usize) -> Result<()> {
    if cols > MAX_COLUMNS || rows > MAX_ROWS {
        bail!(
            "the screen of {cols} by {rows} is larger than {MAX_COLUMNS} columns by {MAX_ROWS} rows"
        );
    }
    Ok(())
}

fn frame_of(screen: &Screen) -> Frame {
    let (cols, rows) = screen.size();
    Frame {
        cols,
        rows,
        lines: screen.lines().iter().map(|l| line_svg(l)).collect(),
        cursor: screen.cursor(),
    }
}

/// A colour as a snapshot paints it: the colour of the text, the colour of the ground, one of
/// the 16 colours or another one.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Paint {
    Text,
    Ground,
    Index(u8),
    Rgb(u8, u8, u8),
}

impl Paint {
    fn of(color: Color, default: Paint) -> Paint {
        match color {
            Color::Default => default,
            Color::Index(n) if n < 16 => Paint::Index(n),
            Color::Index(n) => {
                let (r, g, b) = crate::ansi_html::palette(n);
                Paint::Rgb(r, g, b)
            }
            Color::Rgb(r, g, b) => Paint::Rgb(r, g, b),
        }
    }

    /// The class or the fill of this paint, nothing for `inherited`.
    fn attribute(self, inherited: Paint, classes: &mut Vec<String>, fill: &mut String) {
        match self {
            p if p == inherited => {}
            Paint::Text => classes.push("dkt-sf".to_string()),
            Paint::Ground => classes.push("dkt-sk".to_string()),
            Paint::Index(n) => classes.push(format!("dkt-s{n}")),
            Paint::Rgb(r, g, b) => *fill = format!(" fill=\"#{r:02x}{g:02x}{b:02x}\""),
        }
    }
}

/// The paints of the text and of the ground of a cell, swapped when it is reversed.
fn paints(style: &Style) -> (Paint, Paint) {
    let fg = Paint::of(style.fg, Paint::Text);
    let bg = Paint::of(style.bg, Paint::Ground);
    if style.reverse { (bg, fg) } else { (fg, bg) }
}

/// What a run of text shares: its paint and its flags.
#[derive(PartialEq)]
struct Look {
    paint: Paint,
    flags: [bool; 5],
}

impl Look {
    fn of(style: &Style) -> Look {
        Look {
            paint: paints(style).0,
            flags: [
                style.bold,
                style.dim,
                style.italic,
                style.underline,
                style.strike,
            ],
        }
    }
}

/// A cell that shows no ink, and that a run of text may hold whatever its look.
fn is_blank(cell: &Cell) -> bool {
    cell.style.hidden
        || (cell.ch == ' ' && cell.extra.is_none() && !cell.style.underline && !cell.style.strike)
}

/// The drawing of a row, with its top at `-BASELINE` and the baseline of its text at 0.
///
/// The cells of one ground make 1 rectangle, and the cells of one look make 1 text, which keeps
/// the width of its cells.
/// A row with nothing to draw gives an empty string.
fn line_svg(cells: &[Cell]) -> String {
    let mut out = String::new();
    let mut col = 0;
    while col < cells.len() {
        let ground = paints(&cells[col].style).1;
        let start = col;
        while col < cells.len() && paints(&cells[col].style).1 == ground {
            col += 1;
        }
        if ground == Paint::Ground {
            continue;
        }
        let (mut classes, mut fill) = (Vec::new(), String::new());
        ground.attribute(Paint::Ground, &mut classes, &mut fill);
        let _ = write!(
            out,
            "<rect{}{fill} x=\"{}\" y=\"-{BASELINE}\" width=\"{}\" height=\"{CELL_H}\"/>",
            class_attribute(&classes),
            start * CELL_W,
            (col - start) * CELL_W
        );
    }
    let mut col = 0;
    while col < cells.len() {
        if cells[col].is_continuation() || is_blank(&cells[col]) {
            col += 1;
            continue;
        }
        let look = Look::of(&cells[col].style);
        let start = col;
        let mut end = col + 1;
        let mut next = col + 1;
        while next < cells.len() {
            let cell = &cells[next];
            if !cell.is_continuation() && !is_blank(cell) && Look::of(&cell.style) != look {
                break;
            }
            next += 1;
            if !is_blank(cell) {
                end = next;
            }
        }
        let mut text = String::new();
        for cell in &cells[start..end] {
            if cell.is_continuation() {
                continue;
            }
            if is_blank(cell) {
                // A space that is not a breaking one keeps its place in the text of an SVG.
                text.push('\u{a0}');
                continue;
            }
            crate::escape::push(cell.ch, &mut text);
            if let Some(extra) = &cell.extra {
                text.push_str(&crate::escape::html(extra));
            }
        }
        let (mut classes, mut fill) = (Vec::new(), String::new());
        look.paint.attribute(Paint::Text, &mut classes, &mut fill);
        for (on, class) in look
            .flags
            .iter()
            .zip(["dkt-sb", "dkt-sd", "dkt-si", "dkt-su", "dkt-ss"])
        {
            if *on {
                classes.push(class.to_string());
            }
        }
        let length = if end - start > 1 {
            format!(" textLength=\"{}\"", (end - start) * CELL_W)
        } else {
            String::new()
        };
        let _ = write!(
            out,
            "<text{}{fill} x=\"{}\"{length}>{text}</text>",
            class_attribute(&classes),
            start * CELL_W
        );
        col = end;
    }
    out
}

fn class_attribute(classes: &[String]) -> String {
    if classes.is_empty() {
        String::new()
    } else {
        format!(" class=\"{}\"", classes.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recording(cols: usize, rows: usize, events: &[(f64, &str)]) -> Recording {
        Recording {
            cols,
            rows,
            idle: None,
            events: events
                .iter()
                .map(|(t, text)| Event::Output(*t, text.to_string()))
                .collect(),
        }
    }

    fn line(input: &str) -> String {
        let mut screen = Screen::new(10, 1);
        screen.feed(input);
        line_svg(&screen.lines()[0])
    }

    #[test]
    fn a_run_of_one_look_is_one_text() {
        assert_eq!(
            line("ab cd"),
            "<text x=\"0\" textLength=\"30\">ab\u{a0}cd</text>"
        );
        assert_eq!(line(""), "");
        assert_eq!(
            line("\x1b[1;31mab\x1b[0m  x<"),
            "<text class=\"dkt-s1 dkt-sb\" x=\"0\" textLength=\"12\">ab</text><text x=\"24\" textLength=\"12\">x&lt;</text>"
        );
    }

    #[test]
    fn a_ground_is_one_rectangle() {
        assert_eq!(
            line("\x1b[44m  \x1b[48;2;1;2;3m \x1b[7m \x1b[0m"),
            "<rect class=\"dkt-s4\" x=\"0\" y=\"-9\" width=\"12\" height=\"12\"/><rect fill=\"#010203\" x=\"12\" y=\"-9\" width=\"6\" height=\"12\"/><rect class=\"dkt-sf\" x=\"18\" y=\"-9\" width=\"6\" height=\"12\"/>"
        );
    }

    #[test]
    fn reversed_text_takes_the_colour_of_the_ground() {
        assert!(line("\x1b[7mx").contains("<text class=\"dkt-sk\" x=\"0\">x</text>"));
    }

    #[test]
    fn a_wide_character_counts_its_2_cells() {
        assert_eq!(line("日本"), "<text x=\"0\" textLength=\"24\">日本</text>");
    }

    #[test]
    fn close_events_make_one_step_and_rows_are_shared() {
        let rec = recording(
            10,
            3,
            &[
                (0.0, "\x1b[?1049h\x1b[?25l\x1b[1;1Htop"),
                (0.01, "\x1b[2;1Hrow 1"),
                (1.0, "\x1b[2;1Hrow 2"),
                (1.5, "\x1b[2;1Hrow 2"),
                (9.0, "\x1b[3;1Hend"),
            ],
        );
        assert!(takes_the_screen(&rec));
        let steps = steps(&rec).unwrap();
        assert_eq!(steps.len(), 3);
        // The pause runs from the last event of a step to the last event of the next.
        assert_eq!(steps[1].delay, 990);
        // A long pause is cut to the longest one.
        assert_eq!(steps[2].delay, 2000);
        let html = html(&rec, "p").unwrap();
        // The row "top" is drawn once and used by the 3 steps.
        assert_eq!(html.matches(">top</text>").count(), 1);
        assert_eq!(html.matches("href=\"#p-0\"").count(), 3);
        assert_eq!(html.matches("<svg class=\"dkt-snap\"").count(), 3);
        assert!(!html.contains("<script"));
        assert!(!html.contains("class=\"dkt-sc\""));
    }

    #[test]
    fn a_plain_session_does_not_take_the_screen() {
        let rec = recording(10, 3, &[(0.0, "$ ls\r\n\x1b[32ma\x1b[0m b\r\n")]);
        assert!(!takes_the_screen(&rec));
    }

    #[test]
    fn a_long_session_is_refused() {
        let events: Vec<(f64, String)> = (0..=MAX_STEPS)
            .map(|i| (i as f64, format!("\x1b[1;1H{i}")))
            .collect();
        let events: Vec<(f64, &str)> = events.iter().map(|(t, s)| (*t, s.as_str())).collect();
        let err = steps(&recording(10, 2, &events)).err().unwrap();
        assert!(err.to_string().contains("more than 200 steps"), "{err}");
        let big = recording(MAX_COLUMNS + 1, 2, &[(0.0, "x")]);
        assert!(steps(&big).is_err());
    }

    #[test]
    fn a_resize_changes_the_size_of_the_snapshot() {
        let mut rec = recording(4, 2, &[(0.0, "\x1b[2Jab")]);
        rec.events.push(Event::Resize(1.0, 8, 3));
        rec.events.push(Event::Output(2.0, "cd".to_string()));
        let html = html(&rec, "p").unwrap();
        assert!(html.contains("viewBox=\"-6 -6 36 36\""), "{html}");
        assert!(html.contains("viewBox=\"-6 -6 60 48\""), "{html}");
    }
}
