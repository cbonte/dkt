//! Sequence diagrams: parsing, column layout and drawing.

use std::collections::HashMap;

use super::Drawing;
use super::canvas::{Canvas, Dir, E, N, S, W, Weight, tint};
use crate::options::RenderOptions;
use crate::wrap::{display_width, segments_width};

/// Border plus one blank cell on each side of a name or note text.
const BOX_PAD: usize = 4;
/// Rows of a participant box.
const HEADER_ROWS: usize = 3;
/// Rows of a message: its label, its line and a blank row.
const MESSAGE_ROWS: usize = 3;
/// Blank cells kept between two participant boxes.
const BOX_GAP: usize = 2;
/// Cells a message line needs on top of its label.
const LABEL_ROOM: usize = 4;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Place {
    Left(usize),
    Right(usize),
    Over(usize, usize),
}

/// End of a message line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tip {
    None,
    /// Filled triangle of `->>`.
    Arrow,
    /// Open arrow of the asynchronous `-)`.
    Open,
    /// Cross of `-x`.
    Cross,
}

/// Kind of a block that frames messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Frame {
    Loop,
    Alt,
    Opt,
    Par,
    Critical,
    Break,
    /// `rect`: a shaded band without a title.
    Rect,
}

impl Frame {
    pub(super) fn keyword(self) -> &'static str {
        match self {
            Frame::Loop => "loop",
            Frame::Alt => "alt",
            Frame::Opt => "opt",
            Frame::Par => "par",
            Frame::Critical => "critical",
            Frame::Break => "break",
            Frame::Rect => "rect",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Event {
    Message {
        from: usize,
        to: usize,
        /// The text, a line break of the source written as `\n`.
        text: String,
        dashed: bool,
        tip: Tip,
        /// `<<->>`: a head at both ends.
        both: bool,
    },
    Note {
        place: Place,
        lines: Vec<String>,
    },
    /// The bar of a participant starts, by `activate` or a `+` on a message.
    Activate(usize),
    Deactivate(usize),
    /// A block opens: `loop`, `alt`, `opt`, `par`, `critical`, `break` or `rect`.
    Open {
        frame: Frame,
        label: String,
    },
    /// A new section of the open block: `else`, `and` or `option`.
    Section {
        label: String,
    },
    Close,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Sequence {
    pub(super) names: Vec<String>,
    /// Whether each participant is an `actor`.
    pub(super) actors: Vec<bool>,
    pub(super) events: Vec<Event>,
}

/// Message operators with their line style and head, longest first where one starts another.
const OPERATORS: [(&str, bool, Tip); 8] = [
    ("-->>", true, Tip::Arrow),
    ("->>", false, Tip::Arrow),
    ("-->", true, Tip::None),
    ("->", false, Tip::None),
    ("--)", true, Tip::Open),
    ("-)", false, Tip::Open),
    ("--x", true, Tip::Cross),
    ("-x", false, Tip::Cross),
];

/// Keywords of features that are not drawn.
const UNSUPPORTED: [&str; 7] = [
    "box",
    "create",
    "destroy",
    "link",
    "links",
    "properties",
    "details",
];

struct Parser {
    seq: Sequence,
    ids: HashMap<String, usize>,
    numbered: bool,
    count: usize,
    /// Blocks open, so that an `end` closes one and a section has a block.
    depth: usize,
}

/// The lines of a text, `<br>` starting a new one.
fn split_lines(text: &str) -> Vec<String> {
    text.replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("<br>", "\n")
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

impl Parser {
    fn participant(&mut self, id: &str) -> usize {
        *self.ids.entry(id.to_string()).or_insert_with(|| {
            self.seq.names.push(id.to_string());
            self.seq.actors.push(false);
            self.seq.names.len() - 1
        })
    }

    fn declare(&mut self, rest: &str, actor: bool) {
        let (id, name) = match rest.split_once(" as ") {
            Some((id, name)) => (id.trim(), name.trim()),
            None => (rest.trim(), rest.trim()),
        };
        let index = self.participant(id);
        self.seq.names[index] = name.to_string();
        self.seq.actors[index] = actor;
    }

    fn note(&mut self, rest: &str) -> Option<()> {
        let (head, text) = rest.split_once(':')?;
        let head = head.trim();
        let place = if let Some(who) = head.strip_prefix("right of ") {
            Place::Right(self.participant(who.trim()))
        } else if let Some(who) = head.strip_prefix("left of ") {
            Place::Left(self.participant(who.trim()))
        } else {
            let who = head.strip_prefix("over ")?;
            match who.split_once(',') {
                Some((a, b)) => {
                    let (a, b) = (self.participant(a.trim()), self.participant(b.trim()));
                    Place::Over(a.min(b), a.max(b))
                }
                None => {
                    let a = self.participant(who.trim());
                    Place::Over(a, a)
                }
            }
        };
        let lines = split_lines(text);
        if lines.is_empty() {
            return None;
        }
        self.seq.events.push(Event::Note { place, lines });
        Some(())
    }

    fn message(&mut self, line: &str) -> Option<()> {
        let (head, text) = match line.split_once(':') {
            Some((head, text)) => (head, text.trim()),
            None => (line, ""),
        };
        let (both, head) = match head.find("<<") {
            Some(at) => (true, format!("{}{}", &head[..at], &head[at + 2..])),
            None => (false, head.to_string()),
        };
        let (at, op, dashed, tip) = OPERATORS
            .iter()
            .filter_map(|&(op, dashed, tip)| head.find(op).map(|at| (at, op, dashed, tip)))
            .min_by_key(|&(at, op, ..)| (at, std::cmp::Reverse(op.len())))?;
        if both && tip != Tip::Arrow {
            return None;
        }
        let from = head[..at].trim();
        let target = head[at + op.len()..].trim();
        let (activate, deactivate) = (target.starts_with('+'), target.starts_with('-'));
        let to = target.trim_start_matches(['+', '-']).trim();
        if from.is_empty() || to.is_empty() || to.contains(char::is_whitespace) {
            return None;
        }
        let (from, to) = (self.participant(from), self.participant(to));
        self.count += 1;
        let lines = split_lines(text);
        let mut text = lines.join("\n");
        if self.numbered {
            text = format!("{}. {text}", self.count);
        }
        self.seq.events.push(Event::Message {
            from,
            to,
            text: text.trim().to_string(),
            dashed,
            tip,
            both,
        });
        if activate {
            self.seq.events.push(Event::Activate(to));
        } else if deactivate {
            self.seq.events.push(Event::Deactivate(from));
        }
        Some(())
    }

    fn open(&mut self, frame: Frame, rest: &str) {
        self.depth += 1;
        let label = if frame == Frame::Rect {
            String::new()
        } else {
            rest.trim().to_string()
        };
        self.seq.events.push(Event::Open { frame, label });
    }
}

/// Parses a `sequenceDiagram` block. `None` for another diagram type and for syntax that is not
/// drawn.
pub(super) fn parse(src: &str) -> Option<Sequence> {
    let mut lines = src
        .lines()
        .map(|l| l.trim().trim_end_matches(';').trim_end())
        .filter(|l| !l.is_empty() && !l.starts_with("%%"));
    if lines.next()? != "sequenceDiagram" {
        return None;
    }
    let mut parser = Parser {
        seq: Sequence::default(),
        ids: HashMap::new(),
        numbered: false,
        count: 0,
        depth: 0,
    };
    for line in lines {
        let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let frame = match word {
            "loop" => Some(Frame::Loop),
            "alt" => Some(Frame::Alt),
            "opt" => Some(Frame::Opt),
            "par" => Some(Frame::Par),
            "critical" => Some(Frame::Critical),
            "break" => Some(Frame::Break),
            "rect" => Some(Frame::Rect),
            _ => None,
        };
        match word {
            _ if frame.is_some() => parser.open(frame?, rest),
            "else" | "and" | "option" if parser.depth > 0 => {
                parser.seq.events.push(Event::Section {
                    label: rest.trim().to_string(),
                });
            }
            "end" if parser.depth > 0 => {
                parser.depth -= 1;
                parser.seq.events.push(Event::Close);
            }
            "participant" => parser.declare(rest, false),
            "actor" => parser.declare(rest, true),
            "autonumber" => parser.numbered = true,
            "activate" => {
                let who = parser.participant(rest.trim());
                parser.seq.events.push(Event::Activate(who));
            }
            "deactivate" => {
                let who = parser.participant(rest.trim());
                parser.seq.events.push(Event::Deactivate(who));
            }
            "title" | "accTitle" | "accDescr" => {}
            _ if word.starts_with("title:")
                || word.starts_with("accTitle:")
                || word.starts_with("accDescr:") => {}
            "Note" | "note" => parser.note(rest)?,
            "else" | "and" | "option" | "end" => return None,
            _ if UNSUPPORTED.contains(&word) => return None,
            _ => parser.message(line)?,
        }
    }
    (parser.depth == 0 && !parser.seq.names.is_empty()).then_some(parser.seq)
}

/// Whether the grid drawing draws every event: it draws no block, no message to oneself, no
/// cross and no message with 2 heads, and it leaves out the bars of `activate`.
fn grid_draws(seq: &Sequence) -> bool {
    seq.events.iter().all(|e| match e {
        Event::Message {
            from,
            to,
            tip,
            both,
            ..
        } => from != to && *tip != Tip::Cross && !both,
        Event::Note { .. } | Event::Activate(_) | Event::Deactivate(_) => true,
        Event::Open { .. } | Event::Section { .. } | Event::Close => false,
    })
}

/// A message text on one line, as the grid drawing writes it.
fn one_line(text: &str) -> String {
    text.replace('\n', " ")
}

fn text_width(lines: &[String]) -> usize {
    lines.iter().map(|l| display_width(l)).max().unwrap_or(0)
}

/// Raises the distances between the centers `lo..hi` so that they add up to at least `need`.
fn widen(dist: &mut [usize], lo: usize, hi: usize, need: usize) {
    let have: usize = dist[lo..hi].iter().sum();
    if have < need {
        dist[hi - 1] += need - have;
    }
}

/// Box of a note: left column and width, with the participant centers `c`.
fn note_box(place: &Place, lines: &[String], c: &[i64]) -> (i64, usize) {
    let text = text_width(lines) + BOX_PAD;
    let width = |w: usize| i64::try_from(w).unwrap_or(i64::MAX);
    match *place {
        Place::Right(a) => (c[a] + 2, text),
        Place::Left(a) => (c[a] - 2 - width(text), text),
        Place::Over(a, b) => {
            let w = text.max(usize::try_from(c[b] - c[a]).unwrap_or(0) + 5);
            ((c[a] + c[b]) / 2 - width(w) / 2, w)
        }
    }
}

fn draw(seq: &Sequence, width: usize, opts: &RenderOptions) -> Option<Drawing> {
    if !grid_draws(seq) {
        return None;
    }
    let n = seq.names.len();
    let boxes: Vec<usize> = seq
        .names
        .iter()
        .map(|s| display_width(s) + BOX_PAD)
        .collect();
    let mut dist: Vec<usize> = (1..n)
        .map(|i| (boxes[i - 1] + boxes[i]).div_ceil(2) + BOX_GAP)
        .collect();
    for event in &seq.events {
        match event {
            Event::Message { from, to, text, .. } => {
                let (lo, hi) = (*from.min(to), *from.max(to));
                widen(
                    &mut dist,
                    lo,
                    hi,
                    display_width(&one_line(text)) + LABEL_ROOM,
                );
            }
            Event::Note { place, lines } => {
                let need = text_width(lines) + BOX_PAD + 3;
                match *place {
                    Place::Right(a) if a + 1 < n => widen(&mut dist, a, a + 1, need),
                    Place::Left(a) if a > 0 => widen(&mut dist, a - 1, a, need),
                    Place::Over(a, b) if a == b => {
                        let half = need / 2 + 1;
                        if a > 0 {
                            widen(&mut dist, a - 1, a, half);
                        }
                        if a + 1 < n {
                            widen(&mut dist, a, a + 1, half);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    let mut centers = vec![0i64; n];
    for i in 1..n {
        centers[i] = centers[i - 1] + i64::try_from(dist[i - 1]).ok()?;
    }
    let half = |i: usize| i64::try_from(boxes[i] / 2).unwrap_or(0);
    let mut left = (0..n).map(|i| centers[i] - half(i)).min()?;
    let mut right = (0..n)
        .map(|i| centers[i] - half(i) + i64::try_from(boxes[i]).unwrap_or(0))
        .max()?;
    for event in &seq.events {
        if let Event::Note { place, lines } = event {
            let (x, w) = note_box(place, lines, &centers);
            left = left.min(x);
            right = right.max(x + i64::try_from(w).ok()?);
        }
    }
    let shift = |x: i64| usize::try_from(x - left).ok();
    let rows: usize = seq
        .events
        .iter()
        .map(|e| match e {
            Event::Message { .. } => MESSAGE_ROWS,
            Event::Note { lines, .. } => lines.len() + 3,
            _ => 0,
        })
        .sum();
    let end = HEADER_ROWS + 1 + rows;
    let mut canvas = Canvas::new(usize::try_from(right - left).ok()?, end + HEADER_ROWS);
    let columns: Vec<usize> = centers.iter().map(|&c| shift(c)).collect::<Option<_>>()?;
    for (i, name) in seq.names.iter().enumerate() {
        let x = columns[i] - boxes[i] / 2;
        for y in [0, end] {
            canvas.draw_box(x, y, boxes[i], HEADER_ROWS);
            canvas.text(x + BOX_PAD / 2, y + 1, name);
            canvas.tint_lines(x, y, boxes[i], HEADER_ROWS, tint::STEP);
        }
        canvas.add_lines(columns[i], HEADER_ROWS - 1, S, Weight::Normal);
        for y in HEADER_ROWS..end {
            canvas.add_lines(columns[i], y, N | S, Weight::Dotted);
        }
        canvas.add_lines(columns[i], end, N, Weight::Normal);
    }
    let mut y = HEADER_ROWS + 1;
    for event in &seq.events {
        match event {
            Event::Message {
                from,
                to,
                text,
                dashed,
                tip: head,
                ..
            } => {
                let text = &one_line(text);
                let weight = if *dashed {
                    Weight::Dotted
                } else {
                    Weight::Normal
                };
                let (a, b) = (columns[*from], columns[*to]);
                let (lo, hi) = (a.min(b), a.max(b));
                let label = display_width(text);
                let start = lo + 1 + (hi - lo - 1 - label) / 2;
                canvas.text(start, y, text);
                let cells: Vec<(usize, usize)> = (start..start + label).map(|x| (x, y)).collect();
                canvas.tint(&cells, tint::LABEL);
                let (dir, near) = if a < b { (Dir::E, a) } else { (Dir::W, a) };
                let far = b;
                canvas.add_lines(near, y + 1, dir.bit(), Weight::Normal);
                for x in lo + 1..hi {
                    canvas.add_lines(x, y + 1, E | W, weight);
                }
                let tip = if a < b { far - 1 } else { far + 1 };
                if *head != Tip::None {
                    canvas.set_glyph(tip, y + 1, opts.glyphs.arrows[dir.index()]);
                } else {
                    canvas.add_lines(far, y + 1, dir.opposite().bit(), Weight::Normal);
                }
                // A call takes the colour of the main path, a return a softer one.
                let line: Vec<(usize, usize)> = (lo + 1..hi).map(|x| (x, y + 1)).collect();
                canvas.tint(&line, if *dashed { tint::RETURN } else { tint::MAIN });
                y += MESSAGE_ROWS;
            }
            Event::Note { place, lines } => {
                let (x, w) = note_box(place, lines, &centers);
                let (x, h) = (shift(x)?, lines.len() + 2);
                canvas.clear_rect(x, y, w, h);
                canvas.draw_box(x, y, w, h);
                canvas.tint_lines(x, y, w, h, tint::DECISION);
                for (i, line) in lines.iter().enumerate() {
                    canvas.text(x + BOX_PAD / 2, y + 1 + i, line);
                }
                y += h + 1;
            }
            _ => {}
        }
    }
    let lines = canvas.render_lines(
        &opts.glyphs,
        opts.theme.table_border,
        opts.theme.base,
        &opts.theme.diagram,
    );
    if lines.iter().any(|l| segments_width(l) > width) {
        return None;
    }
    Some(Drawing {
        lines,
        canvas,
        routes: Vec::new(),
        boxes: Vec::new(),
        links: Vec::new(),
        horizontal: false,
    })
}

/// Draws a `sequenceDiagram` block within `width` columns.
pub fn render(code: &str, width: usize, opts: &RenderOptions) -> Option<Drawing> {
    draw(&parse(code)?, width, opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draw_plain(src: &str, width: usize) -> Option<String> {
        let lines = render(src, width, &RenderOptions::plain(width))?.lines;
        Some(
            lines
                .iter()
                .map(|l| l.iter().map(|s| s.text.as_str()).collect::<String>() + "\n")
                .collect(),
        )
    }

    #[test]
    fn parses_participants_messages_and_notes() {
        let seq = parse(
            "sequenceDiagram\n participant A as Alice\n A->>B: hi\n B-->>A: ok\n Note over A,B: n\n",
        )
        .unwrap();
        assert_eq!(seq.names, ["Alice", "B"]);
        assert_eq!(seq.events.len(), 3);
        assert!(matches!(
            &seq.events[1],
            Event::Message {
                dashed: true,
                tip: Tip::Arrow,
                ..
            }
        ));
    }

    #[test]
    fn parses_blocks_self_messages_and_activations() {
        let src = "sequenceDiagram\n A->>+B: go\n alt ok\n  B->>B: think\n else no\n  B--xA: fail\n \
                   end\n deactivate B\n";
        let seq = parse(src).unwrap();
        assert_eq!(seq.events[1], Event::Activate(1));
        assert_eq!(
            seq.events[2],
            Event::Open {
                frame: Frame::Alt,
                label: "ok".to_string()
            }
        );
        assert!(matches!(
            &seq.events[3],
            Event::Message { from: 1, to: 1, .. }
        ));
        assert!(matches!(&seq.events[4], Event::Section { label } if label == "no"));
        assert!(matches!(
            &seq.events[5],
            Event::Message {
                tip: Tip::Cross,
                dashed: true,
                ..
            }
        ));
        assert_eq!(seq.events[6], Event::Close);
        assert_eq!(seq.events[7], Event::Deactivate(1));
    }

    #[test]
    fn rejects_unclosed_blocks_and_unknown_features() {
        assert!(parse("sequenceDiagram\n loop x\n A->>B: y\n").is_none());
        assert!(parse("sequenceDiagram\n A->>B: y\n end").is_none());
        assert!(parse("sequenceDiagram\n box Blue\n A->>B: y\n end").is_none());
        assert!(parse("graph TD\n A --> B").is_none());
    }

    #[test]
    fn the_grid_leaves_blocks_and_self_messages_out() {
        assert!(draw_plain("sequenceDiagram\n loop x\n A->>B: y\n end", 40).is_none());
        assert!(draw_plain("sequenceDiagram\n A->>A: y", 40).is_none());
        assert!(draw_plain("sequenceDiagram\n A-xB: y", 40).is_none());
        assert!(draw_plain("sequenceDiagram\n activate A\n A->>B: y\n deactivate A", 40).is_some());
    }

    #[test]
    fn autonumber_prefixes_labels() {
        let seq = parse("sequenceDiagram\n autonumber\n A->>B: hi\n").unwrap();
        assert!(matches!(&seq.events[0], Event::Message { text, .. } if text == "1. hi"));
    }

    #[test]
    fn two_participants() {
        let art = draw_plain("sequenceDiagram\n A->>B: hello\n B-->>A: ok\n", 40).unwrap();
        let expected = "\
+---+    +---+
| A |    | B |
+-+-+    +-+-+
  :        :
  : hello  :
  +------->:
  :        :
  :   ok   :
  :<.......+
  :        :
+-+-+    +-+-+
| A |    | B |
+---+    +---+
";
        assert_eq!(art, expected);
    }

    #[test]
    fn too_wide_falls_back() {
        assert!(draw_plain("sequenceDiagram\n A->>B: a long message here\n", 10).is_none());
    }
}
