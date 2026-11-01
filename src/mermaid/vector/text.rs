//! Width of text in the sans-serif font of the drawings, and the SVG writing helpers.

use unicode_width::UnicodeWidthChar;

/// Fonts whose letters have the widths of `ADVANCE`, the generic family last.
pub const FONT: &str = "Helvetica, Arial, 'Liberation Sans', 'Nimbus Sans', sans-serif";
/// Size of the text of a node, in the units of the `viewBox`.
pub const SIZE: f64 = 14.0;
/// Size of the text of an edge label, a note and a block title.
pub const SMALL: f64 = 12.5;
/// Distance between 2 base lines, as a share of the font size.
pub const LEADING: f64 = 1.3;
/// Width that a bold letter takes, as a share of its regular width.
const BOLD: f64 = 1.07;

/// Advance of the printable ASCII letters, from the space to the tilde, in thousandths of the font
/// size.
const ADVANCE: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278,
    278, // space to /
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, // digits
    278, 278, 584, 584, 584, 556, 1015, // : to @
    667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, // A to M
    722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, // N to Z
    278, 278, 278, 469, 556, 333, // [ to `
    556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, // a to m
    556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, // n to z
    334, 260, 334, 584, // { to ~
];

/// Advance of one letter, in thousandths of the font size.
fn advance(c: char) -> f64 {
    let code = c as u32;
    if (32..127).contains(&code) {
        return f64::from(ADVANCE[(code - 32) as usize]);
    }
    match c {
        '\u{2014}' | '\u{2026}' => return 1000.0,
        '\u{2013}' | '\u{2022}' | '\u{20ac}' => return 556.0,
        '\u{2018}' | '\u{2019}' => return 222.0,
        '\u{201c}' | '\u{201d}' => return 333.0,
        '\u{2190}'..='\u{21ff}' => return 1000.0,
        _ => {}
    }
    match c.width() {
        Some(0) | None => 0.0,
        Some(2) => 1000.0,
        _ if c.is_uppercase() => 667.0,
        _ => 556.0,
    }
}

/// Width of `text` at `size`, bold or regular.
pub fn width(text: &str, size: f64, bold: bool) -> f64 {
    let sum: f64 = text.chars().map(advance).sum();
    let scale = if bold { BOLD } else { 1.0 };
    sum * size / 1000.0 * scale
}

/// The lines of `lines`, each broken between words so that none is wider than `max`, in lines of
/// even widths. A word wider than `max` stays whole on its own line.
pub fn wrap(lines: &[String], size: f64, bold: bool, max: f64) -> Vec<String> {
    let mut out = Vec::new();
    for line in lines {
        if width(line, size, bold) <= max {
            out.push(line.clone());
            continue;
        }
        let count = greedy(line, size, bold, max).len();
        // The narrowest width that still needs no more lines.
        let (mut lo, mut hi) = (max / 4.0, max);
        for _ in 0..14 {
            let mid = (lo + hi) / 2.0;
            if greedy(line, size, bold, mid).len() <= count {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        out.extend(greedy(line, size, bold, hi));
    }
    out
}

/// `line` broken before each word that would make a line wider than `max`. A word wider than
/// `max` is broken after its punctuation.
fn greedy(line: &str, size: f64, bold: bool, max: f64) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for word in line.split_whitespace() {
        for (k, piece) in pieces(word, size, bold, max).into_iter().enumerate() {
            let candidate = if current.is_empty() {
                piece.clone()
            } else if k == 0 {
                format!("{current} {piece}")
            } else {
                format!("{current}{piece}")
            };
            if !current.is_empty() && width(&candidate, size, bold) > max {
                out.push(std::mem::replace(&mut current, piece));
            } else {
                current = candidate;
            }
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// `word` whole, or cut after the punctuation inside it when it is wider than `max`.
fn pieces(word: &str, size: f64, bold: bool, max: f64) -> Vec<String> {
    if width(word, size, bold) <= max {
        return vec![word.to_string()];
    }
    let mut out = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = word.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        current.push(c);
        let breaks = matches!(c, '_' | '/' | '(' | ',' | '-' | '=' | ':' | '&');
        if breaks && i + 1 < chars.len() {
            out.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// Widest of `lines`.
pub fn block_width(lines: &[String], size: f64, bold: bool) -> f64 {
    lines
        .iter()
        .map(|l| width(l, size, bold))
        .fold(0.0, f64::max)
}

/// Height of `count` lines of text at `size`.
pub fn block_height(count: usize, size: f64) -> f64 {
    count as f64 * size * LEADING
}

/// A coordinate with at most 1 decimal, without a trailing `.0`.
pub fn num(x: f64) -> String {
    let x = (x * 10.0).round() / 10.0;
    if x == 0.0 {
        return "0".to_string();
    }
    let s = format!("{x:.1}");
    s.strip_suffix(".0").map_or(s.clone(), str::to_string)
}

/// Where the text goes within its line.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    Start,
    Middle,
}

/// A `<text>` element of `lines` at `size`, the block of lines centred on `cy`, each line starting
/// at `x` or centred on it.
pub fn text(lines: &[String], x: f64, cy: f64, size: f64, anchor: Anchor, extra: &str) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let step = size * LEADING;
    let top = cy - step * (lines.len() as f64) / 2.0;
    // The base line sits under the middle of the line box by a third of the font size.
    let first = top + step / 2.0 + size * 0.35;
    let mut out = format!("<text x=\"{}\" y=\"{}\"", num(x), num(first));
    if anchor == Anchor::Middle {
        out.push_str(" text-anchor=\"middle\"");
    }
    if (size - SIZE).abs() > f64::EPSILON {
        out.push_str(&format!(" font-size=\"{}\"", num(size)));
    }
    out.push_str(extra);
    out.push('>');
    if lines.len() == 1 {
        out.push_str(&crate::escape::html(&lines[0]));
    } else {
        for (i, line) in lines.iter().enumerate() {
            let dy = if i == 0 { "0".to_string() } else { num(step) };
            out.push_str(&format!(
                "<tspan x=\"{}\" dy=\"{dy}\">{}</tspan>",
                num(x),
                crate::escape::html(line)
            ));
        }
    }
    out.push_str("</text>");
    out
}

/// The opening of the `<svg>` element of a drawing `w` by `h`, scaled down to fit its column, with
/// its title and the source in its description.
/// The colours of a drawing, by the role of each part. Each comes from a custom property that a
/// page may set, with a fallback that reads on a light and on a dark background. A part keeps
/// its plain look where the style does not apply.
///
/// | Class | Property and fallback |
/// |---|---|
/// | `dkt-step` | `--dkt-diagram-step`, blue |
/// | `dkt-terminal` | `--dkt-diagram-terminal`, green |
/// | `dkt-decision` | `--dkt-diagram-decision`, amber |
/// | `dkt-store` | `--dkt-diagram-store`, violet |
/// | `dkt-group` | `--muted`, slate |
/// | `dkt-accent` | `--accent`, orange |
///
/// `dkt-fill` tints a shape with its colour, `dkt-tint` tints it lightly, `dkt-line` strokes a
/// line with it, `dkt-head` fills a head with it, and `dkt-pill` lays it under a label.
pub const STYLE: &str = "<style>\
.dkt-step{--dkt-c:var(--dkt-diagram-step,#3b82f6)}\
.dkt-terminal{--dkt-c:var(--dkt-diagram-terminal,#16a34a)}\
.dkt-decision{--dkt-c:var(--dkt-diagram-decision,#d97706)}\
.dkt-store{--dkt-c:var(--dkt-diagram-store,#8b5cf6)}\
.dkt-group{--dkt-c:var(--muted,#64748b)}\
.dkt-accent{--dkt-c:var(--accent,#d9622b)}\
.dkt-fill{fill:var(--dkt-c);fill-opacity:.14;stroke:var(--dkt-c)}\
.dkt-tint{fill:var(--dkt-c);fill-opacity:.07;stroke:var(--dkt-c);stroke-opacity:.7}\
.dkt-line{stroke:var(--dkt-c)}\
.dkt-head{fill:var(--dkt-c);stroke:var(--dkt-c)}\
.dkt-pill{fill:var(--dkt-c);fill-opacity:.12;stroke:none}\
</style>";

pub fn open(w: f64, h: f64, source: &str) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w} {h}\" width=\"{w}\" height=\"{h}\" style=\"max-width:100%;height:auto\" role=\"img\" aria-label=\"Diagram\" font-family=\"{FONT}\" font-size=\"{}\" fill=\"none\" stroke=\"currentColor\"><title>Diagram</title><desc>{}</desc>{STYLE}",
        num(SIZE),
        crate::escape::html(source.trim_end()),
        w = num(w),
        h = num(h),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_follow_the_letters() {
        assert!(width("iii", SIZE, false) < width("MMM", SIZE, false));
        assert!((width("a", 10.0, false) - 5.56).abs() < 1e-9);
        assert!(width("ab", SIZE, true) > width("ab", SIZE, false));
    }

    #[test]
    fn wrap_breaks_between_words() {
        let lines = wrap(&["one two three four".to_string()], 10.0, false, 40.0);
        assert!(lines.len() > 1, "{lines:?}");
        assert!(
            lines
                .iter()
                .all(|l| width(l, 10.0, false) <= 40.0 || !l.contains(' '))
        );
        assert_eq!(lines.join(" "), "one two three four");
    }

    #[test]
    fn numbers_have_one_decimal_at_most() {
        assert_eq!(num(1.0), "1");
        assert_eq!(num(1.25), "1.3");
        assert_eq!(num(-0.01), "0");
    }
}
