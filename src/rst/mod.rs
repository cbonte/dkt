//! Converts reStructuredText, as Sphinx documents are written, into Markdown.
//!
//! The text is read as blocks: sections, paragraphs, lists, definition lists, tables, line blocks,
//! literal blocks, field and option lists, footnotes, substitutions, the notes and warnings, the
//! images, the included files and the objects that a domain documents, such as `js:function`.
//! A directive that has no meaning for a reader of Markdown stays as preformatted text, so the
//! conversion does not lose a line.

pub mod blocks;
pub mod include;
pub mod inline;
pub mod render;
pub mod tables;

/// The result of a conversion.
pub struct Converted {
    pub markdown: String,
    /// The title of the document when its first title is the only one of its level.
    pub title: Option<String>,
}

/// Converts a reStructuredText document to Markdown.
pub fn convert(source: &str) -> Converted {
    convert_in(source, None)
}

/// Converts a document to Markdown, the files that it includes being looked for in `dir`.
pub fn convert_in(source: &str, dir: Option<&std::path::Path>) -> Converted {
    let expanded;
    let source = match dir {
        Some(dir) => {
            expanded = include::expand(source, dir);
            expanded.as_str()
        }
        None => source,
    };
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

    #[test]
    fn a_grid_table_is_a_table_with_its_header() {
        let out = md(
            "+-------+-------+\n| Name  | Value |\n+=======+=======+\n| a     | 1     |\n+-------+-------+\n| b     | 2 `x` |\n+-------+-------+\n",
        );
        assert!(
            out.contains("| Name | Value |\n| --- | --- |\n| a | 1 |\n| b | 2 `x` |"),
            "{out}"
        );
    }

    #[test]
    fn a_grid_cell_that_spans_columns_keeps_its_text() {
        let out = md("+-----+-----+\n| a   | b   |\n+=====+=====+\n| both      |\n+-----+-----+\n");
        assert!(out.contains("| both |  |"), "{out}");
    }

    #[test]
    fn a_simple_table_keeps_its_rows_and_the_lines_that_go_on() {
        let out = md(
            "=====  ======\nName   Value\n=====  ======\nfirst  1\n       more\nsecond 2\n=====  ======\n",
        );
        assert!(out.contains("| Name | Value |\n| --- | --- |"), "{out}");
        assert!(
            out.contains("| first | 1<br>more |") || out.contains("| first | 1 more |"),
            "{out}"
        );
        assert!(out.contains("| second | 2 |"), "{out}");
    }

    #[test]
    fn a_table_with_no_header_uses_its_first_row() {
        let out = md("=====  =====\none    two\nthree  four\n=====  =====\n");
        assert!(
            out.contains("| one | two |\n| --- | --- |\n| three | four |"),
            "{out}"
        );
    }

    #[test]
    fn list_tables_and_csv_tables_are_tables() {
        let out = md(
            ".. list-table:: Sizes\n   :header-rows: 1\n\n   * - Name\n     - Size\n   * - a\n     - 1\n\n.. csv-table::\n   :header: \"x\", \"y\"\n\n   1, \"2, 3\"\n",
        );
        assert!(
            out.contains("**Sizes**\n\n| Name | Size |\n| --- | --- |\n| a | 1 |"),
            "{out}"
        );
        assert!(
            out.contains("| x | y |\n| --- | --- |\n| 1 | 2, 3 |"),
            "{out}"
        );
    }

    #[test]
    fn a_pipe_in_a_cell_is_escaped() {
        let out = md("=====  =====\nA      B\n=====  =====\n``a|b``  x\n=====  =====\n");
        assert!(out.contains("`a\\|b`"), "{out}");
    }

    #[test]
    fn definition_lists_keep_terms_and_definitions() {
        let out = md("term one\n    Its meaning.\n\n    More.\n\nterm two : kind\n    Other.\n");
        assert!(
            out.contains("- **term one**\n\n  Its meaning.\n\n  More."),
            "{out}"
        );
        assert!(out.contains("- **term two** (*kind*)\n\n  Other."), "{out}");
    }

    #[test]
    fn a_line_block_keeps_its_breaks() {
        let out = md("| first\n|   indented\n| third\n");
        assert_eq!(out.trim_end(), "first\\\n&nbsp;&nbsp;indented\\\nthird");
    }

    #[test]
    fn footnotes_are_numbered_and_linked() {
        let out = md(
            "Text [1]_ and [#]_ and [#named]_ and [CIT2002]_.\n\n.. [1] One.\n.. [#] Auto.\n.. [#named] Named\n   goes on.\n.. [CIT2002] A citation.\n",
        );
        assert!(
            out.contains("[^1] and [^2] and [^3] and [^cite-cit2002]"),
            "{out}"
        );
        assert!(out.contains("[^1]: One."), "{out}");
        assert!(out.contains("[^2]: Auto."), "{out}");
        assert!(out.contains("[^3]: Named goes on."), "{out}");
        assert!(out.contains("[^cite-cit2002]: A citation."), "{out}");
    }

    #[test]
    fn substitutions_stand_for_their_text() {
        let out = md(
            "The |project| is |logo| |copy|.\n\n.. |project| replace:: **dkt** tool\n.. |logo| image:: img/logo.png\n   :alt: Logo\n.. |copy| unicode:: U+00A9\n",
        );
        assert!(
            out.contains("The **dkt** tool is ![Logo](img/logo.png) \u{a9}."),
            "{out}"
        );
        assert!(!out.contains(".. |"), "{out}");
    }

    #[test]
    fn named_and_anonymous_references_lead_to_their_targets() {
        let out = md(
            "See Python_, `the docs`__ and `a page <https://a.test/>`__.\n\n.. _Python: https://python.org\n.. __: https://docs.test/\n",
        );
        assert!(out.contains("[Python](https://python.org)"), "{out}");
        assert!(out.contains("[the docs](https://docs.test/)"), "{out}");
        assert!(out.contains("[a page](https://a.test/)"), "{out}");
    }

    #[test]
    fn option_lists_and_transitions_and_doctests() {
        let out = md(
            "-a             Everything.\n--output=FILE  Where to write\n               to.\n\n----------\n\n>>> print(1)\n1\n",
        );
        assert!(
            out.contains("- `-a`: Everything.\n- `--output=FILE`: Where to write to."),
            "{out}"
        );
        assert!(out.contains("\n---\n"), "{out}");
        assert!(out.contains("```pycon\n>>> print(1)\n1\n```"), "{out}");
    }

    #[test]
    fn version_notes_topics_and_the_like_keep_their_text() {
        let out = md(
            ".. versionadded:: 2.1\n   Now it can.\n\n.. deprecated:: 3.0 use other\n\n.. rubric:: A rubric\n\n.. topic:: Title\n\n   Body.\n\n.. math::\n\n   a^2 + b^2\n",
        );
        assert!(
            out.contains("> [!NOTE]\n> **New in version 2.1.**\n>\n> Now it can."),
            "{out}"
        );
        assert!(
            out.contains("> [!WARNING]\n> **Deprecated since version 3.0.** use other"),
            "{out}"
        );
        assert!(out.contains("**A rubric**"), "{out}");
        assert!(out.contains("> **Title**\n>\n> Body."), "{out}");
        assert!(out.contains("```math\na^2 + b^2\n```"), "{out}");
    }

    #[test]
    fn more_roles_and_numbers_between_parentheses() {
        let out = md(
            "Press :kbd:`Ctrl+C`, x\\ :sup:`2`, :math:`a+b` and :guilabel:`OK`.\n\n(1) one\n(2) two\n",
        );
        assert!(out.contains("<kbd>Ctrl+C</kbd>"), "{out}");
        assert!(
            out.contains("<sup>2</sup>") && out.contains("$`a+b`$") && out.contains("**OK**"),
            "{out}"
        );
        assert!(out.contains("1. one\n2. two"), "{out}");
    }

    #[test]
    fn a_toctree_lists_its_entries_unless_it_is_hidden() {
        let out = md(
            ".. toctree::\n   :maxdepth: 1\n\n   intro\n   guide/start\n   The tour <tour>\n\n.. toctree::\n   :hidden:\n\n   secret\n",
        );
        assert!(out.contains("- intro\n- start\n- The tour"), "{out}");
        assert!(!out.contains("secret"), "{out}");
    }

    #[test]
    fn a_space_may_come_before_the_colons_of_a_directive() {
        let out = md(".. note ::\n   Careful.\n");
        assert!(out.contains("> [!NOTE]\n> Careful."), "{out}");
    }
}
