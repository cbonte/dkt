//! The formats a document can be written in. Each one is read as Markdown, so that everything that
//! renders Markdown renders them.

use std::path::Path;

use anyhow::{Result, bail};

/// How a document is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Markdown,
    Haproxy,
    Rst,
}

/// The names of the formats, as a properties file and the command line give them.
pub const NAMES: &str = "markdown, haproxy or rst";

impl Format {
    /// The format that is named, or the one the extension of the file gives when the extension is
    /// the one of a format and of nothing else.
    pub fn of(named: Option<&str>, src: &str) -> Result<Format> {
        match named {
            Some("markdown") => Ok(Format::Markdown),
            Some("haproxy") => Ok(Format::Haproxy),
            Some("rst") => Ok(Format::Rst),
            Some(other) => bail!("unknown format '{other}' for {src}, expected {NAMES}"),
            None => match src
                .rsplit_once('.')
                .map(|(_, e)| e.to_lowercase())
                .as_deref()
            {
                Some("md" | "markdown") => Ok(Format::Markdown),
                Some("rst") => Ok(Format::Rst),
                _ => bail!(
                    "cannot tell the format of {src} from its extension, give it as one of {NAMES}"
                ),
            },
        }
    }

    /// The Markdown of a text written in this format.
    pub fn markdown(self, text: &str) -> String {
        self.markdown_in(text, None)
    }

    /// The Markdown of a text read from a file in `dir`, which the files it includes are found in.
    pub fn markdown_in(self, text: &str, dir: Option<&Path>) -> String {
        match self {
            Format::Markdown => text.to_string(),
            Format::Haproxy => crate::haproxy::convert(text).markdown,
            Format::Rst => crate::rst::convert_in(text, dir).markdown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_extension_gives_the_format_unless_it_is_named() {
        assert_eq!(Format::of(None, "a/b.md").ok(), Some(Format::Markdown));
        assert_eq!(Format::of(None, "a/b.RST").ok(), Some(Format::Rst));
        // A text file is not a format of its own: it has to be named.
        assert!(Format::of(None, "a.txt").is_err());
        assert_eq!(
            Format::of(Some("haproxy"), "a.txt").ok(),
            Some(Format::Haproxy)
        );
        assert_eq!(Format::of(Some("rst"), "a.txt").ok(), Some(Format::Rst));
        assert!(Format::of(None, "a.html").is_err());
        assert!(Format::of(Some("html"), "a.md").is_err());
    }
}
