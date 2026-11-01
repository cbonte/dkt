//! Sequence diagrams drawn as vector graphics: participants, lifelines, messages, notes, the bars
//! of `activate` and the frames of the blocks.

use super::chart::FIT;
use super::text::{self, Anchor, SIZE, SMALL, num};
use crate::mermaid::sequence::{Event, Frame, Place, Sequence, Tip, parse};

/// Space between the name of a participant and its border.
const PAD_X: f64 = 14.0;
const PAD_Y: f64 = 9.0;
/// Narrowest participant box.
const MIN_BOX: f64 = 72.0;
/// Space between 2 participant boxes.
const BOX_GAP: f64 = 28.0;
/// Room on each side of a message label.
const LABEL_ROOM: f64 = 14.0;
/// Space between the text of a note and its border.
const NOTE_PAD: f64 = 7.0;
/// Width and height of the loop of a message to oneself.
const LOOP_W: f64 = 30.0;
const LOOP_H: f64 = 16.0;
/// Vertical space after each event.
const STEP: f64 = 14.0;
/// Half width of an activation bar, and the shift of a nested one.
const BAR: f64 = 5.0;
/// Room between a frame and what it holds, and between nested frames.
const FRAME_PAD: f64 = 10.0;
/// Height of the title row of a frame.
const TAB_H: f64 = 18.0;
/// Height of a stick figure above its name.
const ACTOR_H: f64 = 34.0;
/// Margin around the drawing.
const MARGIN: f64 = 6.0;
/// Widest line of a participant name.
const NAME_WRAP: f64 = 180.0;
/// Widest line of a message and of a note, from the roomiest to the most compact.
const WRAPS: [(f64, f64); 4] = [
    (260.0, 220.0),
    (200.0, 170.0),
    (150.0, 130.0),
    (110.0, 100.0),
];

/// Draws a `sequenceDiagram` block. `None` when its syntax is not drawn.
pub fn svg(code: &str) -> Option<String> {
    let seq = parse(code)?;
    let mut best: Option<(String, f64, f64)> = None;
    for &(message, note) in &WRAPS {
        let drawing = draw(&seq, message, note);
        let fits = drawing.1 <= FIT;
        if best.as_ref().is_none_or(|b| drawing.1 < b.1) {
            best = Some(drawing);
        }
        if fits {
            break;
        }
    }
    let (body, w, h) = best?;
    Some(format!("{}{body}</svg>", text::open(w, h, code)))
}

fn lines_of(text: &str) -> Vec<String> {
    text.split('\n')
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Raises the distances between the centres `lo..hi` so that they add up to at least `need`,
/// sharing what is missing among them.
fn widen(dist: &mut [f64], lo: usize, hi: usize, need: f64) {
    if lo >= hi {
        return;
    }
    let have: f64 = dist[lo..hi].iter().sum();
    if have < need {
        let each = (need - have) / (hi - lo) as f64;
        for d in &mut dist[lo..hi] {
            *d += each;
        }
    }
}

/// A rectangle that the lifelines do not cross: left, top, right, bottom.
type Rect = [f64; 4];

/// A block being drawn.
struct Open {
    frame: Frame,
    label: String,
    top: f64,
    /// Sections after the first: where the divider runs and its label.
    sections: Vec<(f64, String)>,
    /// Extent across of what the frame holds, frames inside it included.
    lo: f64,
    hi: f64,
    /// Depth of the frames inside it.
    inner: usize,
}

/// Bars of `activate` open on `p`.
fn level(active: &[(usize, f64, usize)], p: usize) -> usize {
    active.iter().filter(|a| a.0 == p).count()
}

/// Widens the innermost open frame to hold `lo..hi`.
fn hold(open: &mut [Open], lo: f64, hi: f64) {
    if let Some(top) = open.last_mut() {
        top.lo = top.lo.min(lo);
        top.hi = top.hi.max(hi);
    }
}

/// The parts of a drawing, each drawn in its own layer.
#[derive(Default)]
struct Layers {
    words: String,
    hide: Vec<Rect>,
    solid: String,
    dashed: String,
    heads: String,
    shaded: String,
    frames: String,
    notes: String,
}

fn draw(seq: &Sequence, message_wrap: f64, note_wrap: f64) -> (String, f64, f64) {
    let n = seq.names.len();
    let names: Vec<Vec<String>> = seq
        .names
        .iter()
        .map(|s| {
            let s = s
                .replace("<br/>", "\n")
                .replace("<br />", "\n")
                .replace("<br>", "\n");
            text::wrap(&lines_of(&s), SIZE, false, NAME_WRAP)
        })
        .collect();
    let boxes: Vec<f64> = names
        .iter()
        .map(|l| (text::block_width(l, SIZE, false) + 2.0 * PAD_X).max(MIN_BOX))
        .collect();
    let name_h = names
        .iter()
        .map(|l| text::block_height(l.len(), SIZE))
        .fold(0.0, f64::max);
    let head_h = if seq.actors.iter().any(|&a| a) {
        name_h + ACTOR_H + 4.0
    } else {
        name_h + 2.0 * PAD_Y
    };
    let texts: Vec<Vec<String>> = seq
        .events
        .iter()
        .map(|e| match e {
            Event::Message { text, .. } => text::wrap(&lines_of(text), SMALL, false, message_wrap),
            Event::Note { lines, .. } => text::wrap(lines, SMALL, false, note_wrap),
            _ => Vec::new(),
        })
        .collect();
    let note_size = |lines: &[String]| {
        (
            text::block_width(lines, SMALL, false) + 2.0 * NOTE_PAD,
            text::block_height(lines.len(), SMALL) + 2.0 * NOTE_PAD,
        )
    };
    // Distances between the centres of neighbours, and room past the outer ones.
    let mut dist: Vec<f64> = (1..n)
        .map(|i| (boxes[i - 1] + boxes[i]) / 2.0 + BOX_GAP)
        .collect();
    let (mut left_room, mut right_room) = (boxes[0] / 2.0, boxes[n - 1] / 2.0);
    for (e, event) in seq.events.iter().enumerate() {
        let tw = text::block_width(&texts[e], SMALL, false);
        match event {
            Event::Message { from, to, .. } if from != to => {
                let (lo, hi) = (*from.min(to), *from.max(to));
                widen(&mut dist, lo, hi, tw + 2.0 * LABEL_ROOM + 2.0 * BAR);
            }
            Event::Message { from, .. } => {
                let need = (LOOP_W + 8.0).max(tw + 12.0) + BAR + 10.0;
                if *from + 1 < n {
                    widen(&mut dist, *from, from + 1, need + boxes[from + 1] / 2.0);
                } else {
                    right_room = right_room.max(need);
                }
            }
            Event::Note { place, .. } => {
                let (w, _) = note_size(&texts[e]);
                match *place {
                    Place::Right(a) if a + 1 < n => widen(&mut dist, a, a + 1, w + 24.0),
                    Place::Right(_) => right_room = right_room.max(w + 14.0),
                    Place::Left(a) if a > 0 => widen(&mut dist, a - 1, a, w + 24.0),
                    Place::Left(_) => left_room = left_room.max(w + 14.0),
                    Place::Over(a, b) if a == b => {
                        if a > 0 {
                            widen(&mut dist, a - 1, a, w / 2.0 + boxes[a - 1] / 2.0 + 8.0);
                        } else {
                            left_room = left_room.max(w / 2.0);
                        }
                        if a + 1 < n {
                            widen(&mut dist, a, a + 1, w / 2.0 + boxes[a + 1] / 2.0 + 8.0);
                        } else {
                            right_room = right_room.max(w / 2.0);
                        }
                    }
                    Place::Over(a, b) => widen(&mut dist, a, b, w - 40.0),
                }
            }
            _ => {}
        }
    }
    let mut centre = vec![0.0; n];
    for i in 1..n {
        centre[i] = centre[i - 1] + dist[i - 1];
    }

    let mut l = Layers::default();
    // Bars of `activate`: participant, start, level; and the bars closed: x, start, end.
    let mut active: Vec<(usize, f64, usize)> = Vec::new();
    let mut bars: Vec<(f64, f64, f64)> = Vec::new();
    let mut open: Vec<Open> = Vec::new();
    let mut y = MARGIN + head_h + 18.0;
    // The line of the last message: where it runs, its source and its target.
    let mut last_line: Option<(f64, usize, usize)> = None;
    let mut extent = (centre[0] - left_room, centre[n - 1] + right_room);
    for (e, event) in seq.events.iter().enumerate() {
        let lines = &texts[e];
        let th = text::block_height(lines.len(), SMALL);
        let tw = text::block_width(lines, SMALL, false);
        let mut line = None;
        match event {
            Event::Message {
                from,
                to,
                dashed,
                tip,
                both,
                ..
            } => {
                let stroke = if *dashed { &mut l.dashed } else { &mut l.solid };
                let (a, b) = (centre[*from], centre[*to]);
                if from == to {
                    let x0 = a + BAR * level(&active, *from) as f64;
                    l.words.push_str(&text::text(
                        lines,
                        x0 + 8.0,
                        y + th / 2.0,
                        SMALL,
                        Anchor::Start,
                        "",
                    ));
                    l.hide
                        .push([x0 + 6.0, y - 1.0, x0 + 10.0 + tw, y + th + 1.0]);
                    let top = y + th + 4.0;
                    let bottom = top + LOOP_H;
                    stroke.push_str(&format!(
                        "M{} {}H{}V{}H{}",
                        num(x0),
                        num(top),
                        num(x0 + LOOP_W),
                        num(bottom),
                        num(x0 + 9.0)
                    ));
                    l.heads.push_str(&message_head(*tip, (x0, bottom), -1.0));
                    let right = x0 + (LOOP_W + 4.0).max(tw + 12.0);
                    hold(&mut open, a, right);
                    extent.1 = extent.1.max(right);
                    line = Some((bottom, *from, *to));
                    y = bottom + STEP;
                } else {
                    let line_y = y + th + 5.0;
                    let mid = (a + b) / 2.0;
                    l.words.push_str(&text::text(
                        lines,
                        mid,
                        y + th / 2.0,
                        SMALL,
                        Anchor::Middle,
                        "",
                    ));
                    l.hide.push([
                        mid - tw / 2.0 - 2.0,
                        y - 1.0,
                        mid + tw / 2.0 + 2.0,
                        y + th + 1.0,
                    ]);
                    let sign = if b > a { 1.0 } else { -1.0 };
                    // A message starts and ends at the side of a bar when one is open.
                    let side = |p: usize, toward: f64| {
                        let lvl = level(&active, p) as f64;
                        if lvl == 0.0 {
                            centre[p]
                        } else if toward > 0.0 {
                            centre[p] - BAR
                        } else {
                            centre[p] + BAR * lvl
                        }
                    };
                    let start = side(*from, -sign);
                    let end = side(*to, sign);
                    let pull = match tip {
                        Tip::Arrow => 8.0,
                        Tip::Open => 1.0,
                        _ => 0.0,
                    };
                    let x_start = start + if *both { sign * pull } else { 0.0 };
                    stroke.push_str(&format!(
                        "M{} {}H{}",
                        num(x_start),
                        num(line_y),
                        num(end - sign * pull)
                    ));
                    l.heads.push_str(&message_head(*tip, (end, line_y), sign));
                    if *both {
                        l.heads
                            .push_str(&message_head(*tip, (start, line_y), -sign));
                    }
                    hold(&mut open, a.min(b), a.max(b));
                    line = Some((line_y, *from, *to));
                    y = line_y + STEP;
                }
            }
            Event::Note {
                place,
                lines: source,
            } => {
                // A note over several participants takes the width between them for its text.
                let lines = &match *place {
                    Place::Over(p, q) if p != q => {
                        let span = centre[q] - centre[p] + 40.0 - 2.0 * NOTE_PAD;
                        text::wrap(source, SMALL, false, span.max(note_wrap))
                    }
                    _ => lines.clone(),
                };
                let (w, h) = note_size(lines);
                let (x0, w) = match *place {
                    Place::Right(p) => (centre[p] + BAR * level(&active, p) as f64 + 6.0, w),
                    Place::Left(p) => (centre[p] - 6.0 - w, w),
                    Place::Over(p, q) => {
                        let w = if p == q {
                            w
                        } else {
                            w.max(centre[q] - centre[p] + 40.0)
                        };
                        ((centre[p] + centre[q]) / 2.0 - w / 2.0, w)
                    }
                };
                l.notes.push_str(&note(x0, y, w, h));
                l.words.push_str(&text::text(
                    lines,
                    x0 + NOTE_PAD,
                    y + h / 2.0,
                    SMALL,
                    Anchor::Start,
                    "",
                ));
                l.hide.push([x0, y, x0 + w, y + h]);
                hold(&mut open, x0, x0 + w);
                extent = (extent.0.min(x0), extent.1.max(x0 + w));
                y += h + STEP;
            }
            Event::Activate(p) => {
                let start = match last_line {
                    Some((ly, _, to)) if to == *p => ly,
                    _ => y - STEP / 2.0,
                };
                active.push((*p, start, level(&active, *p)));
                line = last_line;
            }
            Event::Deactivate(p) => {
                let end = match last_line {
                    Some((ly, from, _)) if from == *p => ly,
                    _ => y - STEP / 2.0,
                };
                if let Some(k) = active.iter().rposition(|a| a.0 == *p) {
                    let (who, start, lvl) = active.remove(k);
                    bars.push((centre[who] + BAR * lvl as f64, start, end.max(start + 8.0)));
                }
                line = last_line;
            }
            Event::Open { frame, label } => {
                y += 4.0;
                open.push(Open {
                    frame: *frame,
                    label: label.clone(),
                    top: y,
                    sections: Vec::new(),
                    lo: f64::INFINITY,
                    hi: f64::NEG_INFINITY,
                    inner: 0,
                });
                y += if *frame == Frame::Rect {
                    8.0
                } else {
                    TAB_H + 10.0
                };
            }
            Event::Section { label } => {
                y += 4.0;
                if let Some(top) = open.last_mut() {
                    top.sections.push((y, label.clone()));
                }
                y += TAB_H + 6.0;
            }
            Event::Close => {
                y += 2.0;
                if let Some(f) = open.pop() {
                    let (x0, x1) = close(&f, y, centre[0], &mut l);
                    extent = (extent.0.min(x0), extent.1.max(x1));
                    if let Some(parent) = open.last_mut() {
                        parent.lo = parent.lo.min(x0);
                        parent.hi = parent.hi.max(x1);
                        parent.inner = parent.inner.max(f.inner + 1);
                    }
                }
                y += 10.0;
            }
        }
        last_line = line;
    }
    for (who, start, lvl) in active {
        bars.push((centre[who] + BAR * lvl as f64, start, y));
    }
    let foot = y + 6.0;
    let height = foot + head_h + MARGIN;
    // Participants at the top and at the bottom.
    let mut people = String::new();
    for i in 0..n {
        let c = centre[i];
        for top in [MARGIN, foot] {
            if seq.actors[i] {
                people.push_str(&actor(c, top));
                l.words.push_str(&text::text(
                    &names[i],
                    c,
                    top + ACTOR_H + 4.0 + name_h / 2.0,
                    SIZE,
                    Anchor::Middle,
                    "",
                ));
            } else {
                people.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"4\"/>",
                    num(c - boxes[i] / 2.0),
                    num(top),
                    num(boxes[i]),
                    num(head_h)
                ));
                l.words.push_str(&text::text(
                    &names[i],
                    c,
                    top + head_h / 2.0,
                    SIZE,
                    Anchor::Middle,
                    "",
                ));
            }
        }
    }
    // Bars, cut where a text or a note lies across them; then the lifelines keep out of the bars.
    let texts_only = l.hide.clone();
    let mut bars_svg = String::new();
    for &(x, start, end) in &bars {
        for (a, b) in uncut(start, end, x - BAR, x + BAR, &texts_only) {
            bars_svg.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
                num(x - BAR),
                num(a),
                num(2.0 * BAR),
                num(b - a)
            ));
        }
        l.hide.push([x - BAR, start, x + BAR, end]);
    }
    // Lifelines, cut where a text, a note or a bar lies across them.
    let mut lifelines = String::new();
    for &c in &centre {
        for (a, b) in uncut(MARGIN + head_h, foot, c, c, &l.hide) {
            lifelines.push_str(&format!("M{} {}V{}", num(c), num(a), num(b)));
        }
    }
    let shift = MARGIN + FRAME_PAD - extent.0;
    let width = extent.1 - extent.0 + 2.0 * (MARGIN + FRAME_PAD);
    let mut body = format!("<g transform=\"translate({} 0)\">", num(shift));
    if !l.shaded.is_empty() {
        body.push_str(&format!(
            "<g class=\"dkt-pill dkt-group\" fill=\"currentColor\" fill-opacity=\"0.05\" stroke=\"none\">{}</g>",
            l.shaded
        ));
    }
    body.push_str(&format!(
        "<path d=\"{lifelines}\" stroke-width=\"1\" stroke-dasharray=\"3 3\" stroke-opacity=\"0.7\"/>"
    ));
    if !l.frames.is_empty() {
        body.push_str(&format!(
            "<g class=\"dkt-tint dkt-store\" stroke-width=\"1\" stroke-opacity=\"0.8\">{}</g>",
            l.frames
        ));
    }
    body.push_str(&format!(
        "<g fill=\"currentColor\" fill-opacity=\"0.08\" stroke-width=\"1.3\"><g class=\"dkt-fill dkt-step\">{people}</g><g class=\"dkt-fill dkt-accent\">{bars_svg}</g><g class=\"dkt-fill dkt-decision\">{}</g></g>",
        l.notes
    ));
    if !l.solid.is_empty() {
        body.push_str(&format!("<path d=\"{}\" stroke-width=\"1.3\"/>", l.solid));
    }
    if !l.dashed.is_empty() {
        body.push_str(&format!(
            "<path d=\"{}\" stroke-width=\"1.3\" stroke-dasharray=\"5 4\"/>",
            l.dashed
        ));
    }
    if !l.heads.is_empty() {
        body.push_str(&format!(
            "<g stroke-width=\"1.3\" stroke-linejoin=\"round\">{}</g>",
            l.heads
        ));
    }
    body.push_str(&format!(
        "<g fill=\"currentColor\" stroke=\"none\">{}</g></g>",
        l.words
    ));
    (body, width, height)
}

/// The stretches of `from..to` that no rectangle covers, for a band from `x0` to `x1`.
fn uncut(from: f64, to: f64, x0: f64, x1: f64, rects: &[Rect]) -> Vec<(f64, f64)> {
    let mut cuts: Vec<(f64, f64)> = rects
        .iter()
        .filter(|r| r[0] < x1.max(x0 + 0.01) && r[2] > x0)
        .map(|r| (r[1], r[3]))
        .collect();
    cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out = Vec::new();
    let mut at = from;
    for (a, b) in cuts {
        if a > at && at < to {
            out.push((at, a.min(to)));
        }
        at = at.max(b);
    }
    if at < to {
        out.push((at, to));
    }
    out
}

/// Draws the frame of a block that ends at `bottom`. Returns its extent across.
fn close(f: &Open, bottom: f64, first: f64, l: &mut Layers) -> (f64, f64) {
    let (lo, hi) = if f.lo.is_finite() {
        (f.lo, f.hi)
    } else {
        (first, first)
    };
    let inset = FRAME_PAD + 8.0 * f.inner as f64;
    let x0 = lo - inset - BAR;
    let keyword = f.frame.keyword();
    let tab_w = text::width(keyword, SMALL, true) + 16.0;
    let condition = if f.label.is_empty() {
        String::new()
    } else {
        format!("[{}]", f.label)
    };
    let cw = text::width(&condition, SMALL, false);
    let mut x1 = hi + inset + BAR;
    if f.frame != Frame::Rect {
        x1 = x1.max(x0 + tab_w + cw + 16.0);
    }
    for (_, label) in &f.sections {
        x1 = x1.max(x0 + text::width(label, SMALL, false) + 30.0);
    }
    let rect = format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"3\"/>",
        num(x0),
        num(f.top),
        num(x1 - x0),
        num(bottom - f.top)
    );
    if f.frame == Frame::Rect {
        l.shaded.push_str(&rect);
        return (x0, x1);
    }
    l.frames.push_str(&rect);
    l.frames.push_str(&format!(
        "<path d=\"M{x0} {t}H{x1}V{m}L{x2} {b}H{x0}Z\" fill=\"currentColor\" fill-opacity=\"0.1\"/>",
        x0 = num(x0),
        t = num(f.top),
        x1 = num(x0 + tab_w),
        m = num(f.top + TAB_H - 6.0),
        x2 = num(x0 + tab_w - 6.0),
        b = num(f.top + TAB_H)
    ));
    let ty = f.top + TAB_H / 2.0;
    l.words.push_str(&text::text(
        &[keyword.to_string()],
        x0 + 7.0,
        ty,
        SMALL,
        Anchor::Start,
        " font-weight=\"bold\"",
    ));
    l.hide.push([x0, f.top, x0 + tab_w, f.top + TAB_H]);
    if !condition.is_empty() {
        l.words.push_str(&text::text(
            &[condition],
            x0 + tab_w + 8.0,
            ty,
            SMALL,
            Anchor::Start,
            " font-style=\"italic\"",
        ));
        l.hide.push([
            x0 + tab_w + 6.0,
            f.top,
            x0 + tab_w + 10.0 + cw,
            f.top + TAB_H,
        ]);
    }
    for (sy, label) in &f.sections {
        l.frames.push_str(&format!(
            "<path d=\"M{} {}H{}\" stroke-dasharray=\"5 4\"/>",
            num(x0),
            num(*sy),
            num(x1)
        ));
        if !label.is_empty() {
            let label = format!("[{label}]");
            let lw = text::width(&label, SMALL, false);
            l.words.push_str(&text::text(
                &[label],
                x0 + 10.0,
                sy + TAB_H / 2.0 + 2.0,
                SMALL,
                Anchor::Start,
                " font-style=\"italic\"",
            ));
            l.hide
                .push([x0 + 8.0, sy + 2.0, x0 + 12.0 + lw, sy + TAB_H + 2.0]);
        }
    }
    (x0, x1)
}

/// A note with a folded corner, at `(x, y)`, `w` by `h`.
fn note(x: f64, y: f64, w: f64, h: f64) -> String {
    let fold = 7.0;
    format!(
        "<path d=\"M{x0} {y0}H{xf}L{x1} {yf}V{y1}H{x0}Z\"/><path d=\"M{xf} {y0}V{yf}H{x1}\" fill=\"none\"/>",
        x0 = num(x),
        y0 = num(y),
        xf = num(x + w - fold),
        x1 = num(x + w),
        yf = num(y + fold),
        y1 = num(y + h),
    )
}

/// A stick figure whose head sits at the top centre `(c, top)`.
fn actor(c: f64, top: f64) -> String {
    let r = 6.0;
    let neck = top + 2.0 * r;
    format!(
        "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"/><path d=\"M{c} {n}V{hip}M{l} {arm}H{r}M{lf} {foot}L{c} {hip}L{rf} {foot}\" fill=\"none\"/>",
        num(c),
        num(top + r),
        num(r),
        c = num(c),
        n = num(neck),
        hip = num(neck + 12.0),
        l = num(c - 10.0),
        arm = num(neck + 5.0),
        r = num(c + 10.0),
        lf = num(c - 9.0),
        rf = num(c + 9.0),
        foot = num(top + ACTOR_H),
    )
}

/// The head of a message that ends at `tip`, heading right when `sign` is 1, left when -1.
fn message_head(head: Tip, tip: (f64, f64), sign: f64) -> String {
    let (x, y) = tip;
    let back = x - sign * 9.0;
    match head {
        Tip::None => String::new(),
        Tip::Arrow => format!(
            "<path d=\"M{} {}L{} {}L{} {}Z\" fill=\"currentColor\"/>",
            num(x),
            num(y),
            num(back),
            num(y - 4.5),
            num(back),
            num(y + 4.5)
        ),
        Tip::Open => format!(
            "<path d=\"M{} {}L{} {}L{} {}\" fill=\"none\"/>",
            num(back),
            num(y - 5.0),
            num(x),
            num(y),
            num(back),
            num(y + 5.0)
        ),
        Tip::Cross => {
            let c = x - sign * 5.0;
            format!(
                "<path d=\"M{} {}L{} {}M{} {}L{} {}\" fill=\"none\"/>",
                num(c - 4.0),
                num(y - 4.0),
                num(c + 4.0),
                num(y + 4.0),
                num(c - 4.0),
                num(y + 4.0),
                num(c + 4.0),
                num(y - 4.0)
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_messages_notes_blocks_and_actors() {
        let src = "sequenceDiagram\n participant A as Alice\n actor B\n A->>+B: hello\n \
                   alt fine\n  B->>B: think\n else not\n  B--xA: no\n end\n \
                   Note over A,B: done\n deactivate B\n";
        let svg = svg(src).unwrap();
        assert!(svg.starts_with("<svg "), "{svg}");
        for word in [
            "Alice", "hello", "think", "[fine]", "[not]", ">alt<", "done",
        ] {
            assert!(svg.contains(word), "{word} in {svg}");
        }
        assert!(svg.contains("<circle"), "the actor has a head: {svg}");
    }

    #[test]
    fn a_message_to_oneself_loops_right_of_its_lifeline() {
        let seq = parse("sequenceDiagram\n A->>A: again\n").unwrap();
        let (body, w, _) = draw(&seq, 260.0, 220.0);
        assert!(body.contains(&format!("H{}V", num(LOOP_W))), "{body}");
        assert!(w >= MIN_BOX / 2.0 + LOOP_W, "{w}");
    }
}
