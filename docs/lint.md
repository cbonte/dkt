# dkt lint

Checks Markdown links, anchors and heading structure. No network access: only what is in the file
tree is verified.

```
dkt lint README.md
dkt lint docs/                 # walks the directory for .md files
dkt lint --format json docs/   # one JSON object per finding
```

Findings are printed as `file:line:column: rule: message` and the exit code is 1 when there is any.
`--list-rules` prints the rules with their default state.

| Rule                | Default | Check                                                                                                                      |
| ------------------- | ------- | -------------------------------------------------------------------------------------------------------------------------- |
| `link-file`         | on      | a relative link or image target, or a widget file, that does not exist                                                     |
| `link-anchor`       | on      | a `#anchor` that matches no heading, in this file or in a linked one                                                       |
| `link-ref`          | on      | a reference link with no definition, and a definition nobody uses                                                          |
| `heading-skip`      | on      | a heading level that jumps by more than one                                                                                |
| `heading-dup`       | on      | 2 headings that produce the same anchor                                                                                    |
| `image-alt`         | on      | an image with no alt text                                                                                                  |
| `site-annotation`   | on      | a `<!-- site: ... -->` comment that does not hold, a `replace` or `widget` with no `end`, a widget file that does not read, a scene step that names an id its figure lacks |
| `heading-single-h1` | off     | more than one level one heading                                                                                            |
| `fence-lang`        | off     | a fenced code block with no language                                                                                       |
| `trailing-space`    | off     | trailing whitespace that is not a hard break                                                                               |
| `line-length`       | off     | a line over `--line-length`, code and URLs excluded                                                                        |

Turn rules off with `--disable RULE` and on with `--enable RULE`, repeating either.

A widget file is the file that a `table` or a `replay` widget names with `src`.
The check reads it as the site reads it, so it finds what would stop the build.

Anchors are the ones GitHub computes, so a link that passes here works on GitHub. Explicit `id` and
`name` attributes in HTML count as anchors too.
