# The haproxy format

A page of a site can be a documentation text of HAProxy, such as `doc/configuration.txt`,
`doc/intro.txt` or `doc/management.txt`. The site converts it to Markdown when it reads it, for the
check and for the build, so the text stays as HAProxy writes it.

```
[[page]]
src = "3.4/configuration.txt"
format = "haproxy"
section = "3.4"
```

Every command that reads a format reads it with `--format haproxy`: `dkt convert` gives its
Markdown, `dkt cat` shows it on the terminal, `dkt html` renders it and a `[[page]]` of a site
publishes it. `dkt cat --section NAME` shows one keyword or one chapter.

The `format` has to be written, since a `.txt` file is not a format of its own. The pages of a site
are listed in its properties file, so a version that has no management guide does not list one.

## What the conversion gives

The text is read in one pass. Anything that is not recognised stays as preformatted text, so the
conversion never loses a line.

| In the text                                   | In the Markdown                                             |
| --------------------------------------------- | ----------------------------------------------------------- |
| the banner                                    | the title, with the version after its first word            |
| `Summary` and its numbered chapters           | a list of links                                             |
| a numbered title underlined with dashes       | a heading, one level per number                             |
| a keyword and its syntax lines                | a heading with the name, then the syntax as code            |
| a keyword defined in several chapters         | the name for the keyword reference, then the chapter title  |
| `May be used in the following contexts`       | a line of contexts                                          |
| `May be used in sections`                     | a line of the sections that have `yes`                      |
| `Arguments`                                   | a list of the arguments                                     |
| `Example` and `Examples`                      | the example as code                                         |
| `See also`                                    | a line                                                      |
| `Note:` and its hanging text                  | a GitHub alert                                              |
| a table drawn with dashes and pluses          | a table, a header that groups columns naming each of them   |
| a table drawn as a grid of borders and pipes  | a table, a cell on several lines joined                     |
| a drawing, with arrows, the borders of boxes or marks under a command | a diagram block, drawn as SVG on a site |
| a list                                        | a list, with the paragraphs, examples and lists under an item |
| numbered items                                | an ordered list, from the first number                      |
| names with descriptions that line up          | a list of definitions                                       |
| columns lined up with spaces                  | a table, a cell on several lines joined                     |
| `Usable in:` and its marks                    | a line of the places that have a mark                       |
| a title underlined with dashes in a chapter   | a bold line                                                 |
| `Note:`, `Important note:`, `Warning:`        | an alert of the matching kind                               |
| a keyword marked `(deprecated)`               | a warning under its heading                                 |
| a keyword at the start of an item or of a row of the matrix | a link to its definition                      |
| `section 7.3.6`, and a quoted keyword         | a link to the heading                                       |
| an address that starts with `http`            | a link to itself                                            |
| a comment that opens with a slash and a star in the first column | left out                       |
