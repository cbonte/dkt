# dkt convert

Converts a document to Markdown, from the format it is written in.

```
dkt convert api.rst                          # to stdout
dkt convert api.rst -o api.md                # to a file
dkt convert --format haproxy configuration.txt --width 80
dkt convert --format rst < api.txt
```

| Option           | Default           | Effect                                                    |
| ---------------- | ----------------- | --------------------------------------------------------- |
| `--format NAME`  | from the extension | `markdown`, `rst` or `haproxy`                            |
| `--output`, `-o` | stdout            | write to a file                                           |
| `--width N`      | none              | wrap the prose at N columns, 0 for one line per paragraph |

The format comes from the extension, `.md` giving Markdown and `.rst` giving reStructuredText, and
it is Markdown for the standard input. A file with any other extension, a `.txt` file included,
needs `--format`, and `dkt convert` stops without it.

The text is written as the conversion gives it: each paragraph on one line, as the source of the
format puts it. `--width` runs the result through [`dkt fmt`](fmt.md), which wraps the prose and
normalises the blocks. A Markdown document comes out unchanged, or formatted with `--width`.

Every other command that reads a format, `dkt cat`, `dkt html` and `dkt site`, reads the same
Markdown. The conversion of each format is described with it: [rst](rst.md) and
[haproxy](haproxy.md).
