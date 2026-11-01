# dkt cat

Renders a document as styled text on the terminal. The document is Markdown, reStructuredText or a
documentation text of HAProxy.

```
dkt cat README.md
dkt cat docs/*.md | less -R
cat notes.md | dkt cat
dkt cat --soft-breaks preserve notes.md | less -R
dkt cat api.rst
dkt cat --format haproxy --section tune.bufsize configuration.txt
```

The format comes from the extension, `.md` giving Markdown and `.rst` giving reStructuredText, and
it is Markdown for the standard input. `--format markdown|rst|haproxy` names it. A file with any
other extension, a `.txt` file included, is written as it is, as the standard `cat` does, with a
warning on stderr that says to give the format. The document is read as Markdown first, so every
option below applies to each format.

With several files, each document starts with its file name and a rule (`--no-header` hides it). An
unreadable file is reported on stderr, the others are still rendered, and the exit code is non-zero.

## Behaviour

| Feature                           | Default                                                                                                                                                       | Option                                           |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------ |
| Single newline inside a paragraph | joined, paragraph reflowed (CommonMark)                                                                                                                       | `--soft-breaks preserve` keeps each source line  |
| Line width                        | terminal width, else 80                                                                                                                                       | `--width N`                                      |
| Colours                           | on when stdout is a terminal or a pipe from an interactive shell (`\| less -R`), off when redirected to a file or run without a terminal, off with `NO_COLOR` | `--color auto`, `always`, `never`                |
| Links                             | OSC 8 hyperlink on terminals that support it, else `text (url)`, and the text alone for a link to the same document                                           | `--links auto`, `osc8`, `inline`, `none`         |
| Bullets, rules, table borders     | Unicode in a UTF-8 locale, else ASCII                                                                                                                         | `--borders auto`, `unicode`, `ascii`             |
| Code blocks                       | syntax highlighted (syntect), tabs expanded                                                                                                                   | `--theme NAME`, `--list-themes`, `--tab-width N` |
| Mermaid blocks                    | drawn as text when supported                                                                                                                                  | `--no-diagrams`                                  |
| Tables                            | column widths fitted to the line, cells wrapped                                                                                                               | `--table-overflow wrap`, `truncate`              |
| Table rows                        | a rule between rows                                                                                                                                           | `--table-rules always`, `never`                  |
| Headings                          | `#` markers kept                                                                                                                                              | `--no-heading-marker`                            |
| Front matter                      | YAML or TOML metadata block hidden                                                                                                                            | `--front-matter show`                            |
| Outline                           | off                                                                                                                                                           | `--toc`, `--toc-depth N`                         |
| Section                           | whole document                                                                                                                                                | `--section HEADING` by text or by anchor         |
| Pager                             | off. `-p` sends the output to `DKT_PAGER`, `PAGER`, else `less -R -F`                                                                                         | `-p`, `--pager`                                  |

Continuation lines inside lists and block quotes stay aligned under the item text in both soft-break
modes.

Supported: headings, paragraphs, emphasis, strong, strikethrough, inline code, block quotes, GitHub
alerts, ordered and unordered lists, task lists, fenced and indented code blocks, GFM tables,
thematic breaks, hard breaks, links, images (alt text and URL), footnotes, Mermaid flowcharts,
sequence diagrams, state diagrams, class diagrams, entity-relationship diagrams and pie charts.

A `mermaid` code block holding a `graph`, a `flowchart`, a `sequenceDiagram`, a `stateDiagram`, a
`classDiagram`, an `erDiagram` or a `pie` is drawn with box characters, in the same repertoire as
table borders. A diagram that is not drawn is shown as a plain code block: another diagram type,
syntax outside the supported set, or a drawing wider than the line. A left-to-right flowchart too
wide for the line is stacked top-down first.

With colours, the outline of a node takes the colour of its role, from its shape: a step, an end, a
decision or a store. The main path and the edge labels take colours of their own. Edges of one line
style that reach the same node join into one last line, with one head. In a sequence diagram, the
participants take the colour of a step and the notes the colour of a decision. A call takes the
colour of the main path, a return a softer one, and the text of a message the colour of a label. The
bars of a pie chart take the colours in turn.

Flowcharts: nodes may be rectangles `[text]`, rounded `(text)`, `([text])`, `((text))` or diamonds
`{text}`. The other shapes of a page, such as `{{text}}` or `[(text)]`, are drawn as rectangles.
`<br/>` in a label starts a new line. Edges may be `-->`, `---`, dotted `-.->`, `-.-` or thick
`==>`, `===`, longer or with a head at both ends, with a label as `-->|text|` or `-- text -->`.
`A & B --> C` links each node on one side to each node on the other. Styling lines (`style`,
`classDef`, `class`, `linkStyle`, `click`) are ignored. The nodes of a subgraph are drawn without
its box. A node linked to itself and a link to a subgraph are not drawn.

Sequence diagrams: `participant` and `actor` declarations, messages `->>`, `-->>`, `->`, `-->`,
`-)`, `--)`, notes `left of`, `right of` and `over`, and `autonumber`. The bars of `activate` are
left out. A diagram with a block (`loop`, `alt`, `opt`, `par`, `critical`, `break`, `rect`), a
message to oneself, a cross arrow `-x` or a message with 2 heads is not drawn in the terminal. A
page draws all of them.

State diagrams: transitions `A --> B` with an optional `: label`, the pseudo-state `[*]` as start
and end, `state "Name" as id`, `id : description` and `direction`. Composite states, forks, choices,
notes and concurrency are not drawn.

Class diagrams: `class Name { ... }` blocks, `Name : member` lines, `<<annotation>>` lines, and
relations with the heads `<|`, `*`, `o`, `<` and `>` on `--` or `..` lines, with an optional
`: label`. Members are split into fields and methods by the presence of a parenthesis. Quoted
cardinalities are not drawn, nor are namespaces, notes and links.

Entity-relationship diagrams: `NAME { type name PK }` blocks and relations `A ||--o{ B : label`,
with the cardinalities `||`, `|o`, `o|`, `}o`, `o{`, `}|`, `|{` and solid `--` or dotted `..` lines.
The cardinalities are written in the label as `(1 to 0..*)`.

Pie charts: an optional `showData` and `title`, and one `"label" : value` line per slice. Each slice
is a bar scaled to the largest one, with its share and, with `showData`, its value.

Footnote references are numbered in order of first appearance. The bodies are printed as a numbered
list under a rule at the end of the document.

HTML is rendered the way GitHub displays it, for the tags GitHub keeps: `b`, `strong`, `i`, `em`,
`u`, `s`, `del`, `code`, `kbd`, `sub`, `sup`, `br`, `a`, `img`, `p`, `div`, `center`, `h1` to `h6`,
`blockquote`, `ul`, `ol`, `li`, `details`, `summary`, `table`, `pre`, `hr`. Comments, `script` and
`style` are dropped, entities are decoded, other tags are ignored and their text kept. `align`
attributes are honoured on table cells only.
