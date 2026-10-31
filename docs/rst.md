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

The text is read as blocks. What is not understood stays as preformatted text, so the conversion
does not lose a line.

## What the conversion gives

| In the text                                                                                        | In the Markdown                                         |
| -------------------------------------------------------------------------------------------------- | ------------------------------------------------------- |
| a title with an underline, and maybe an overline                                                   | a heading, one level per adornment in the order it came |
| a title that is alone at its level                                                                 | the title of the document, and the others move up       |
| a paragraph, and `::` that ends it before an indented block                                        | a paragraph, and a code block                           |
| `.. code-block:: lua`                                                                              | a code block with its language                          |
| a bullet list, a numbered list, and what is under an item                                          | a list, with the blocks under each item                 |
| a field list such as `:param str name: text`                                                       | a list of the fields, with the name and the type        |
| `.. note::`, `.. warning::`, `.. important::`, `.. tip::`, `.. caution::`                          | an alert of the matching kind                           |
| `.. image::` and `.. figure::`                                                                     | an image, named by its file when it has no `:alt:`      |
| `.. function::`, `.. class::`, `.. attribute::` and the like, with a domain such as `py:` or `js:` | a heading with the signature, then what describes it    |
| `.. _label:` above a title                                                                         | the anchor that `:ref:` leads to                        |
| `:ref:`, `:func:`, `:class:`, `:attr:` and the roles of a domain                                   | a link to the title or to the object                    |
| `` `text <address>`_ `` and `` `Title`_ ``                                                         | a link                                                  |
| `**strong**`, `*emphasis*`, ` ``code`` ` and `` `text` ``                                          | the same, and code for the text in single backticks     |
| an address that starts with `http`                                                                 | a link to itself                                        |
| a comment, and `.. toctree::`                                                                      | left out                                                |
| any other directive                                                                                | its text as a code block                                |

The anchors are the ones GitHub gives, so 2 headings with the same text get the numbers that a page
gives them. A check that stops on repeated headings is turned off for such a site with `[lint]` and
`disable = ["heading-dup"]`.

## What is not read yet

Tables, definition lists, line blocks, footnotes and substitutions are read as paragraphs.
