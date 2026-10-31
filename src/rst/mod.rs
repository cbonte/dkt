//! Converts reStructuredText, as Sphinx documents are written, into Markdown.
//!
//! The text is read as blocks: sections, paragraphs, lists, literal blocks, field lists, the notes
//! and warnings, the images and the objects that the `js` domain documents with `js:function`,
//! `js:class` and `js:attribute`. What is not understood stays as preformatted text, so the
//! conversion does not lose a line.

pub mod blocks;
pub mod inline;
pub mod render;

/// The result of a conversion.
pub struct Converted {
    pub markdown: String,
    /// The title of the document when its first title is the only one of its level.
    pub title: Option<String>,
}

/// Converts a reStructuredText document to Markdown.
pub fn convert(source: &str) -> Converted {
    let lines = blocks::lines(source);
    let mut parsed = blocks::parse(&lines);
    let title = blocks::promote_title(&mut parsed);
    Converted {
        markdown: render::render(&parsed, title.as_deref()),
        title,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn md(text: &str) -> String {
        convert(text).markdown
    }

    #[test]
    fn titles_take_their_level_from_the_order_of_their_adornments() {
        let out = md("One\n===\n\ntext\n\nTwo\n---\n\nmore\n\nThree\n=====\n\nend\n");
        assert!(out.contains("## One\n"), "{out}");
        assert!(out.contains("### Two\n"), "{out}");
        assert!(out.contains("## Three\n"), "{out}");
    }

    #[test]
    fn a_title_that_is_alone_at_its_level_is_the_title_of_the_document() {
        let c = convert("Doc\n===\n\nlede\n\nPart\n----\n\nbody\n");
        assert_eq!(c.title.as_deref(), Some("Doc"));
        assert!(c.markdown.starts_with("# Doc\n"), "{}", c.markdown);
        assert!(c.markdown.contains("## Part"), "{}", c.markdown);
    }

    #[test]
    fn a_double_colon_opens_a_literal_block_and_a_directive_a_code_block() {
        let out = md(
            "Example::\n\n    one\n      two\n\nthen\n\n.. code-block:: lua\n\n    local a = 1\n",
        );
        assert!(
            out.contains("Example:\n\n```text\none\n  two\n```"),
            "{out}"
        );
        assert!(out.contains("```lua\nlocal a = 1\n```"), "{out}");
    }

    #[test]
    fn lists_keep_their_items_their_numbers_and_what_is_under_them() {
        let out = md("* one\n  more\n\n* two\n\n  .. note::\n     careful\n\n3. three\n4. four\n");
        assert!(out.contains("- one more\n- two"), "{out}");
        assert!(out.contains("  > [!NOTE]\n  > careful"), "{out}");
        assert!(out.contains("3. three\n4. four"), "{out}");
    }

    #[test]
    fn admonitions_become_alerts_and_comments_disappear() {
        let out = md(".. warning::\n   Take care.\n\n.. a comment\n   on 2 lines\n\ntext\n");
        assert!(out.contains("> [!WARNING]\n> Take care."), "{out}");
        assert!(!out.contains("comment"), "{out}");
        assert!(out.contains("text"), "{out}");
    }

    #[test]
    fn a_domain_object_is_a_heading_with_its_fields_as_a_list() {
        let out = md(
            "Other\n=====\n\ntext\n\nPart\n====\n\n.. py:function:: run(cmd, n)\n\n   Runs it.\n\n   :param str cmd: the command\n   :param n: the count\n   :returns: nothing\n",
        );
        assert!(out.contains("## Part\n\n### `run(cmd, n)`"), "{out}");
        assert!(out.contains("- **cmd** (*str*): the command"), "{out}");
        assert!(out.contains("- **n**: the count"), "{out}");
        assert!(out.contains("- **Returns**: nothing"), "{out}");
    }

    #[test]
    fn roles_and_references_link_to_labels_titles_and_objects() {
        let out = md(
            "Top\n===\n\ntext\n\n.. _where:\n\nHere\n====\n\nSee :ref:`where`, :ref:`the place <where>`, `Here`_ and :py:func:`run`.\n\n.. py:function:: run()\n\n   x\n\nAlso `a link <https://example.org/x>`_ and ``code``, *em* and **strong**.\n",
        );
        assert!(
            out.contains("[Here](#here), [the place](#here), [Here](#here)"),
            "{out}"
        );
        assert!(out.contains("[`run`](#run)"), "{out}");
        assert!(out.contains("[a link](https://example.org/x)"), "{out}");
        assert!(out.contains("`code`, *em* and **strong**"), "{out}");
    }

    #[test]
    fn what_is_not_understood_stays_as_text() {
        let out = md(".. unknown-thing:: arg\n   :opt: 1\n\n   body\n");
        assert!(out.contains("```text\n.. unknown-thing:: arg"), "{out}");
        assert!(out.contains("body"), "{out}");
    }
}
