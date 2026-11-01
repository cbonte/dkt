# dkt

Doc Optimized Components kit (dk:t) is a set of tools for documents, in one binary.

## Commands

| Command                        | Purpose                                                  |
| ------------------------------ | -------------------------------------------------------- |
| [dkt cat](docs/cat.md)         | render a document as styled text                         |
| [dkt fmt](docs/fmt.md)         | rewrap prose and normalise block syntax                  |
| [dkt toc](docs/toc.md)         | insert or refresh a table of contents                    |
| [dkt lint](docs/lint.md)       | check links, anchors and heading structure               |
| [dkt convert](docs/convert.md) | convert a document to Markdown                           |
| [dkt html](docs/html.md)       | render a document as HTML, in a template                 |
| [dkt site](docs/site.md)       | build a static site from documents and a properties file |

The commands share one parser and one model. A command that takes a document reads a file or the
standard input, and writes to the standard output unless an option says otherwise.

## Formats

Every document is read as Markdown first.

| Format                                   | Extension | Read by                             |
| ---------------------------------------- | --------- | ----------------------------------- |
| Markdown                                 | `.md`     | every command                       |
| [reStructuredText](docs/rst.md)          | `.rst`    | `cat`, `convert`, `html` and `site` |
| [HAProxy documentation](docs/haproxy.md) | none      | `cat`, `convert`, `html` and `site` |

- A file with another extension needs `--format`, a `.txt` file included.
- `cat` writes such a file as it is when no format is named.
- `fmt`, `lint` and `toc` work on Markdown only.

## Pages

A site made by `dkt site` is more than text:

| Feature                                                                                                 | Where                                                     |
| ------------------------------------------------------------------------------------------------------- | --------------------------------------------------------- |
| [16 widgets](docs/widgets.md), such as tabs, a carousel, a quiz, a filterable table and a progress page | comments around ordinary Markdown                         |
| Mermaid diagrams, drawn as SVG                                                                          | a `mermaid` block, as on GitHub                           |
| the colours of a terminal                                                                               | an `ansi` block                                           |
| a search palette, cards for the links and pagers                                                        | the [properties file](docs/site.md)                       |
| several versions of a documentation, with a selector in every page                                      | [versions](docs/versions.md)                              |
| examples of each                                                                                        | [the site of dkt](https://cbonte.github.io/dkt/examples/) |

The documents stay readable on GitHub and in the terminal, because the widgets are comments.

## Install

```
cargo build --release
cargo install --path .
```

## Shell integration

```
dkt --completions bash > ~/.local/share/bash-completion/completions/dkt
dkt --completions zsh > ~/.local/share/zsh/site-functions/_dkt
dkt --man > dkt.1
```

The manual has 1 page for `dkt` and 1 page for each command.

## Why

| Problem                                       | Answer                                                          |
| --------------------------------------------- | --------------------------------------------------------------- |
| A renderer shows each hand-wrapped line break | `cat` joins the lines of a paragraph, and keeps them on request |
| A source is wrapped at a width nobody chose   | `fmt` rewraps it to the width you give                          |
| The documents are not all Markdown            | the formats above are read as Markdown                          |
| A site needs its links to be right            | `site` checks the documents before it builds                    |

## Development

The build, the tests and the structure of the code are in [DEVELOPMENT.md](DEVELOPMENT.md).

## Licence

Dual licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE), at your option.
