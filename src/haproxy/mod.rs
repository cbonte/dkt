//! Converts the documentation of HAProxy, written as structured text, into Markdown.
//!
//! The text is read in one pass: a banner, then chapters with numbered titles, keywords with their
//! notes, paragraphs, lists, examples and preformatted text. What is not recognised is kept as
//! preformatted text, so the conversion never loses a line.

pub mod blocks;
pub mod defs;
pub mod grid;
pub mod render;
pub mod text;

/// The result of a conversion.
pub struct Converted {
    pub markdown: String,
    pub title: String,
    pub version: String,
    pub date: String,
}

/// Converts a documentation text to Markdown.
pub fn convert(source: &str) -> Converted {
    let normal = text::normalise(source);
    let lines = text::without_comments(text::lines(&normal));
    let (banner, used) = blocks::banner(&lines);
    let parsed = blocks::parse(&lines[used..]);
    Converted {
        markdown: render::render(&banner, &parsed) + "\n",
        title: banner.title,
        version: banner.version,
        date: banner.date,
    }
}
