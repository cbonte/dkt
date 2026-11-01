//! Text with the escape sequences of a terminal, as HTML.
//!
//! The colours, the weight, the slant, the underline and the strike of a run of text are the
//! classes of a `span`. The 16 colours of a terminal are classes, and the other ones are written in
//! the style of the span, so that a theme sets the 16 colours it prefers.

use std::fmt::Write;

/// The escape character, and the spellings of it that a document written by hand can hold.
const ESC: char = '\u{1b}';
const SPELLINGS: [&str; 3] = ["\\x1b[", "\\u001b[", "\\e["];

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Color {
    #[default]
    Default,
    Index(u8),
    Rgb(u8, u8, u8),
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct State {
    fg: Color,
    bg: Color,
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    reverse: bool,
}

/// The colour of one of the 256 colours of a terminal, when it is not one of the first 16.
pub(crate) fn palette(n: u8) -> (u8, u8, u8) {
    if n >= 232 {
        let level = 8 + (n - 232) * 10;
        return (level, level, level);
    }
    let n = n - 16;
    let step = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
    (step(n / 36), step(n / 6 % 6), step(n % 6))
}

impl State {
    /// The opening tag of the span for this state, empty for the plain state.
    fn open(&self) -> String {
        if *self == State::default() {
            return String::new();
        }
        let mut classes: Vec<String> = Vec::new();
        let mut style = String::new();
        for (flag, name) in [
            (self.bold, "a-b"),
            (self.dim, "a-d"),
            (self.italic, "a-i"),
            (self.underline, "a-u"),
            (self.strike, "a-s"),
            (self.reverse, "a-r"),
        ] {
            if flag {
                classes.push(name.to_string());
            }
        }
        for (color, kind, property) in [(self.fg, "fg", "color"), (self.bg, "bg", "background")] {
            match color {
                Color::Default => {}
                Color::Index(n) if n < 16 => classes.push(format!("a-{kind}{n}")),
                Color::Index(n) => {
                    let (r, g, b) = palette(n);
                    let _ = write!(style, "{property}:#{r:02x}{g:02x}{b:02x};");
                }
                Color::Rgb(r, g, b) => {
                    let _ = write!(style, "{property}:#{r:02x}{g:02x}{b:02x};");
                }
            }
        }
        let style = if style.is_empty() {
            String::new()
        } else {
            format!(" style=\"{style}\"")
        };
        let class = if classes.is_empty() {
            String::new()
        } else {
            format!(" class=\"{}\"", classes.join(" "))
        };
        format!("<span{class}{style}>")
    }

    /// Applies the parameters of one `m` sequence.
    fn apply(&mut self, params: &[u32]) {
        let mut i = 0;
        if params.is_empty() {
            *self = State::default();
        }
        while i < params.len() {
            let p = params[i];
            match p {
                0 => *self = State::default(),
                1 => self.bold = true,
                2 => self.dim = true,
                3 => self.italic = true,
                4 => self.underline = true,
                7 => self.reverse = true,
                9 => self.strike = true,
                22 => {
                    self.bold = false;
                    self.dim = false;
                }
                23 => self.italic = false,
                24 => self.underline = false,
                27 => self.reverse = false,
                29 => self.strike = false,
                30..=37 => self.fg = Color::Index((p - 30) as u8),
                39 => self.fg = Color::Default,
                40..=47 => self.bg = Color::Index((p - 40) as u8),
                49 => self.bg = Color::Default,
                90..=97 => self.fg = Color::Index((p - 90 + 8) as u8),
                100..=107 => self.bg = Color::Index((p - 100 + 8) as u8),
                38 | 48 => {
                    let color = match params.get(i + 1) {
                        Some(5) => params.get(i + 2).map(|n| {
                            i += 2;
                            Color::Index((*n).min(255) as u8)
                        }),
                        Some(2) if params.len() > i + 4 => {
                            let c = Color::Rgb(
                                params[i + 2].min(255) as u8,
                                params[i + 3].min(255) as u8,
                                params[i + 4].min(255) as u8,
                            );
                            i += 4;
                            Some(c)
                        }
                        _ => None,
                    };
                    if let Some(color) = color {
                        if p == 38 {
                            self.fg = color;
                        } else {
                            self.bg = color;
                        }
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }
}

/// The text as HTML, where each run of the same look is a `span`. The sequences that are not
/// about the look of the text, such as the ones that move the cursor, are dropped.
pub fn to_html(text: &str) -> String {
    let mut text = text.to_string();
    for spelling in SPELLINGS {
        text = text.replace(spelling, &format!("{ESC}["));
    }
    let mut out = String::new();
    let mut state = State::default();
    let mut open = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != ESC || chars.peek() != Some(&'[') {
            crate::escape::push(c, &mut out);
            continue;
        }
        chars.next();
        let mut params = String::new();
        let mut last = None;
        for next in chars.by_ref() {
            if ('@'..='~').contains(&next) {
                last = Some(next);
                break;
            }
            params.push(next);
        }
        if last != Some('m') {
            continue;
        }
        let numbers: Vec<u32> = params
            .split([';', ':'])
            .map(|p| p.parse().unwrap_or(0))
            .collect();
        let before = state;
        state.apply(if params.is_empty() { &[] } else { &numbers });
        if state != before {
            if open {
                out.push_str("</span>");
                open = false;
            }
            let tag = state.open();
            if !tag.is_empty() {
                out.push_str(&tag);
                open = true;
            }
        }
    }
    if open {
        out.push_str("</span>");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_colour_and_a_reset_make_a_span() {
        assert_eq!(
            to_html("\u{1b}[1;31mred\u{1b}[0m plain"),
            "<span class=\"a-b a-fg1\">red</span> plain"
        );
    }

    #[test]
    fn the_bright_colours_and_the_background_have_classes() {
        assert_eq!(
            to_html("\u{1b}[92;44mx\u{1b}[m"),
            "<span class=\"a-fg10 a-bg4\">x</span>"
        );
    }

    #[test]
    fn the_other_colours_are_in_the_style() {
        assert_eq!(
            to_html("\u{1b}[38;5;196mhot\u{1b}[48;2;1;2;3mx"),
            "<span style=\"color:#ff0000;\">hot</span><span style=\"color:#ff0000;background:#010203;\">x</span>"
        );
    }

    #[test]
    fn text_is_escaped_and_other_sequences_are_dropped() {
        assert_eq!(to_html("a<b>&\u{1b}[2K\u{1b}[1Ac"), "a&lt;b&gt;&amp;c");
    }

    #[test]
    fn a_spelled_escape_is_one_too() {
        assert_eq!(to_html("\\x1b[4mu\\x1b[0m"), "<span class=\"a-u\">u</span>");
    }
}
