# dkt toc

Inserts or refreshes a table of contents in a Markdown file.

```
dkt toc README.md              # print the table, or the updated document
dkt toc --write README.md      # update the file in place
dkt toc --check docs/*.md      # exit 1 if a table is stale
```

The table is written between 2 HTML comments, which you add once where it belongs:

```
<!-- toc -->
<!-- /toc -->
```

Without those markers, `dkt toc` prints the table alone so you can paste it.

| Option          | Default | Effect                                                   |
| --------------- | ------- | -------------------------------------------------------- |
| `--min-depth N` | 2       | shallowest heading level listed, so the title is skipped |
| `--max-depth N` | 4       | deepest heading level listed                             |
| `--bullet C`    | `-`     | list marker                                              |
| `--marker NAME` | `toc`   | name used in the comment markers                         |
| `--write`       | off     | update the files in place                                |
| `--check`       | off     | print the stale files and exit 1                         |

Anchors are the ones GitHub computes, duplicates numbered the same way, so the links work on GitHub
and with `dkt cat --section`.
