# dkt

Doc Kit (dk:t): document tools in one binary, sharing one parser and one model. A document in
Markdown, in reStructuredText or in another format is read as Markdown first.

| Command                               | Purpose                                                  |
| ------------------------------------- | -------------------------------------------------------- |
| [dkt cat](docs/cat.md)                | render a document as styled text                         |
| [dkt fmt](docs/fmt.md)                | rewrap prose and normalise block syntax                  |
| [dkt toc](docs/toc.md)                | insert or refresh a table of contents                    |
| [dkt lint](docs/lint.md)              | check links, anchors and heading structure               |
| [dkt convert](docs/convert.md)        | convert a document to Markdown                           |
| [dkt html](docs/html.md)              | render a document as HTML, in a template                 |
| [dkt site](docs/site.md)              | build a static site from documents and a properties file |
| [the rst format](docs/rst.md)         | read reStructuredText, as Sphinx documents are written   |
| [the haproxy format](docs/haproxy.md) | read the documentation texts of HAProxy                  |

`cat`, `convert`, `html` and `site` read Markdown, reStructuredText and the documentation texts of
HAProxy, from the extension of the file or from a format that is named. `fmt`, `lint` and `toc` work
on Markdown.

```
cargo build --release
cargo install --path .
```

Each command reads files or stdin and writes to stdout.

## Shell integration

```
dkt --completions bash > ~/.local/share/bash-completion/completions/dkt
dkt --completions zsh > ~/.local/share/zsh/site-functions/_dkt
dkt --man > dkt.1
```

The manual holds one page for `dkt` and one for each command.

## Why

Renderers reflow a paragraph the way CommonMark specifies, which shows every line break of a
hand-wrapped source as a break on screen. `cat` joins them by default and keeps them on request,
with continuation lines aligned under their list item or quote. `fmt` solves the same problem at
the source: it rewraps the file so the breaks land where you want them.

## Licence

Dual licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE), at your option.
