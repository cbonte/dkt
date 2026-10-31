# The rst format

A document written in reStructuredText, as Sphinx documents are, is read as Markdown by every
command that takes a format: `dkt html api.rst`, or a page of a site with `format = "rst"`, which
the extension `.rst` gives.

```
[[page]]
src = "api/index.rst"
title = "API"
section = "api"
```

The text is read as blocks. A directive that has no meaning for a reader of Markdown, such as one
that the writer defined, stays as preformatted text, so the conversion does not lose a line.

## What the conversion gives

| In the text                                                                                        | In the Markdown                                                                                                                                |
| -------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| a title with an underline, and maybe an overline                                                   | a heading, one level per adornment in the order it came                                                                                        |
| a title that is alone at its level                                                                 | the title of the document, and the others move up                                                                                              |
| a paragraph, and `::` that ends it before an indented block                                        | a paragraph, and a code block                                                                                                                  |
| `>>>` lines of a doctest                                                                           | a code block in `pycon`                                                                                                                        |
| `.. code-block:: lua`, `.. code::`, `.. parsed-literal::`                                          | a code block with its language                                                                                                                 |
| `.. math::` and `:math:`                                                                           | a `math` block and inline math                                                                                                                 |
| a bullet list, a numbered list, `(1)` and `1)` numbers, and what is under an item                  | a list, with the blocks under each item                                                                                                        |
| a definition list, with `term : classifier`                                                        | a list with the term in bold and the definition under it                                                                                       |
| a field list such as `:param str name: text`                                                       | a list of the fields, with the name and the type                                                                                               |
| an option list such as `-a, --all  text`                                                           | a list with the options in code                                                                                                                |
| a line block with `\|`                                                                             | lines that keep their breaks and their indent                                                                                                  |
| a transition, a line of punctuation alone                                                          | a rule                                                                                                                                         |
| a grid table, a simple table, `.. list-table::`, `.. csv-table::`                                  | a table, with the first row as header when the text gives none                                                                                 |
| `.. note::`, `.. warning::`, `.. important::`, `.. tip::`, `.. caution::`, `.. seealso::`          | an alert of the matching kind                                                                                                                  |
| `.. versionadded::`, `.. versionchanged::`, `.. deprecated::`                                      | an alert that says it, with the version                                                                                                        |
| `.. topic::`, `.. sidebar::`, `.. admonition::`                                                    | an alert with the title in bold                                                                                                                |
| `.. rubric::`, `.. centered::`                                                                     | a line in bold                                                                                                                                 |
| `.. epigraph::`, `.. pull-quote::`, `.. highlights::`                                              | a quote                                                                                                                                        |
| `.. only::`, `.. container::`, `.. glossary::` and the like                                        | their content                                                                                                                                  |
| `.. image::` and `.. figure::`                                                                     | an image, named by its file when it has no `:alt:`                                                                                             |
| `.. raw:: html`                                                                                    | the HTML, and other raw formats are left out                                                                                                   |
| `.. include::`                                                                                     | the text of the file, found next to the document, with `:start-after:`, `:end-before:`, `:start-line:`, `:end-line:`, `:literal:` and `:code:` |
| `.. function::`, `.. class::`, `.. attribute::` and the like, with a domain such as `py:` or `js:` | a heading with the signature, then what describes it                                                                                           |
| `.. _label:` above a title                                                                         | the anchor that `:ref:` leads to                                                                                                               |
| `.. _name: address`, `.. __: address` and `name_`, `` `text`__ ``                                  | a link                                                                                                                                         |
| `.. [1]`, `.. [#]`, `.. [#name]`, `.. [*]`, `.. [CIT]` and `[1]_`                                  | a footnote, numbered as the text numbers it                                                                                                    |
| `.. \|name\| replace::`, `image::` and `unicode::`, and `\|name\|`                                 | the text, the image or the character where the name is used                                                                                    |
| `:ref:`, `:func:`, `:class:`, `:attr:` and the roles of a domain                                   | a link to the title or to the object                                                                                                           |
| `:kbd:`, `:sup:`, `:sub:`, `:abbr:`, `:guilabel:`, `:menuselection:`                               | the matching HTML or bold text                                                                                                                 |
| `` `text <address>`_ `` and `` `Title`_ ``                                                         | a link                                                                                                                                         |
| `**strong**`, `*emphasis*`, ` ``code`` ` and `` `text` ``                                          | the same, and code for the text in single backticks                                                                                            |
| an address that starts with `http`                                                                 | a link to itself                                                                                                                               |
| `.. toctree::`                                                                                     | the list of its entries, unless it is `:hidden:`                                                                                               |
| a comment, `.. contents::`, `.. index::` and the directives that only steer a builder              | left out                                                                                                                                       |
| a directive that none of the above names                                                           | its text as a code block                                                                                                                       |

A file that an `include` names and that cannot be read leaves the directive as it was written. The
files are looked for next to the document, so `dkt html` and `dkt convert` take the folder of their
file, and `dkt site` the folder of each page.

The anchors are the ones GitHub gives, so 2 headings with the same text get the numbers that a page
gives them. A check that stops on repeated headings is turned off for such a site with `[lint]` and
`disable = ["heading-dup"]`.
