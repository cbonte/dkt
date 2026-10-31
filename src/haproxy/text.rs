//! The lines of a documentation text, ready to be read.

/// One line of the text.
#[derive(Clone, Copy, Debug)]
pub struct Line<'a> {
    /// The line without its leading spaces and its trailing white space.
    pub text: &'a str,
    /// The number of leading spaces.
    pub indent: usize,
}

impl Line<'_> {
    pub fn blank(&self) -> bool {
        self.text.is_empty()
    }
}

/// The text with tabs as 8 spaces, no carriage return and no trailing white space.
pub fn normalise(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        let mut col = 0;
        for c in line.chars() {
            if c == '\t' {
                let to = (col / 8 + 1) * 8;
                out.extend(std::iter::repeat_n(' ', to - col));
                col = to;
            } else {
                out.push(c);
                col += 1;
            }
        }
        while out.ends_with(' ') {
            out.pop();
        }
        out.push('\n');
    }
    out
}

/// The lines of a normalised text.
pub fn lines(text: &str) -> Vec<Line<'_>> {
    text.lines()
        .map(|raw| {
            let text = raw.trim_start_matches(' ');
            Line {
                text,
                indent: raw.len() - text.len(),
            }
        })
        .collect()
}

/// The lines without the comments of the file, such as the editor settings at its end: a comment
/// starts with `/*` in the first column and ends with the line that ends with `*/`.
pub fn without_comments(ls: Vec<Line<'_>>) -> Vec<Line<'_>> {
    let mut out = Vec::with_capacity(ls.len());
    let mut inside = false;
    for l in ls {
        if !inside && l.indent == 0 && l.text.starts_with("/*") {
            inside = true;
        }
        if inside {
            if l.text.ends_with("*/") {
                inside = false;
            }
            continue;
        }
        out.push(l);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_comment_in_the_first_column_is_dropped_and_an_indented_one_is_kept() {
        let text =
            "keep\n\n/*\n * Local variables:\n *  fill-column: 79\n */\n\n  /* kept */\nend\n";
        let kept: Vec<&str> = without_comments(lines(text))
            .iter()
            .map(|l| l.text)
            .collect();
        assert_eq!(kept, ["keep", "", "", "/* kept */", "end"]);
    }

    #[test]
    fn tabs_become_the_spaces_up_to_the_next_multiple_of_8() {
        assert_eq!(
            normalise("a\tb\n  \tc\r\nd  \n"),
            "a       b\n        c\nd\n"
        );
    }

    #[test]
    fn a_line_knows_its_indent() {
        let text = normalise("  one\n\nzero\n");
        let l = lines(&text);
        assert_eq!((l[0].indent, l[0].text), (2, "one"));
        assert!(l[1].blank());
        assert_eq!(l[2].indent, 0);
    }
}
