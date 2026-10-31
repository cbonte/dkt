# dkt html

Renders a document as HTML, as a body or as a page set in a template. The document is Markdown,
reStructuredText or a documentation text of HAProxy.

```
dkt html README.md                          # the body, on stdout
dkt html guide.md -o guide.html             # the body, in a file
dkt html guide.md --template page.html      # a page
dkt html guide.md --template page.html --set site=lokode
dkt html api.rst                            # reStructuredText, from the extension
dkt html --format haproxy configuration.txt # a format that the extension does not give
```

The body is what CommonMark and the GitHub extensions give: tables, task lists, strikethrough,
footnotes, alerts and raw HTML. A metadata block at the top of the file is left out.

Every heading carries the anchor GitHub gives it, duplicates numbered the same way, so a link
written for GitHub holds on the page. Every heading below the title is a link to itself. A table is
in a `div` with the class `table-wrap`, so that a wide table scrolls. An alert becomes a
`blockquote` with the class `markdown-alert-note`, `markdown-alert-tip`, `markdown-alert-important`,
`markdown-alert-warning` or `markdown-alert-caution`.

## Formats

| Format     | Extension | The document is                                     |
| ---------- | --------- | --------------------------------------------------- |
| `markdown` | `.md`     | Markdown, the default for the standard input        |
| `rst`      | `.rst`    | [reStructuredText](rst.md), as Sphinx documents are |
| `haproxy`  | none      | a [documentation text of HAProxy](haproxy.md)       |

`--format` names the format. A file whose extension is not `.md` or `.rst` needs it, a `.txt` file
included, and so does the format `haproxy`. Every other part of this page, the anchors, the diagrams
and the template, works the same for each format, since each one is read as Markdown first.

## Template

The template is a Jinja file. Another file it extends or includes is looked up next to it. The
content is HTML already, so it is not escaped, and everything else is.

| Name               | Value                                                      |
| ------------------ | ---------------------------------------------------------- |
| `page.title`       | text of the first level one heading, or nothing            |
| `page.description` | text of the paragraph that directly follows it, or nothing |
| `page.content`     | the body                                                   |
| `page.headings`    | each heading, as `level`, `id` and `text`                  |
| `page.nav_title`   | the short title from a `page` annotation, or nothing       |
| `page.weight`      | the position from a `page` annotation, or nothing          |
| `page.hidden`      | true when a `page` annotation says `hide`                  |
| `page.path`        | the path the document was read from, empty for stdin       |
| `vars.KEY`         | the value given with `--set KEY=VALUE`                     |

```
<title>{{ page.title }} - {{ vars.site }}</title>
<nav>{% for h in page.headings if h.level == 2 %}<a href="#{{ h.id }}">{{ h.text }}</a>{% endfor %}</nav>
{{ page.content }}
```

| Option                | Effect                                                    |
| --------------------- | --------------------------------------------------------- |
| `--template FILE`     | set the page in this template instead of writing the body |
| `--set KEY=VALUE`     | a variable for the template, repeatable                   |
| `-o`, `--output FILE` | write to this file instead of stdout                      |

Without a file, or for `-`, `dkt html` reads stdin.

## Diagrams

A fenced block whose language is `diagram` is drawn as an SVG figure. The block stays text on GitHub
and in the terminal, and the figure holds that text for readers who cannot see the drawing.

````
```diagram
cmd arg
|---|
  a _/
```
````

The dashes, bars, pluses, slashes, underscores, equal signs, colons and angle brackets that draw
lines become strokes and arrow heads. A bar in the middle of a word, a dash between letters and an
underscore in a name stay text. An underscore and the slashes that follow it are one stroke, and a
stroke that rises to a span of dashes between 2 bars, or to a run of dashes, ends at the middle of
that span. A backtick under a bar, or under the end of a span, followed by a dash is the foot of a
line that turns right toward a label, and a row of `<--->` is a double arrow for each span of the
text above it. The strokes and the text take the colour of the text of the page.

## Annotations

A document can carry notes for the site in HTML comments, which GitHub does not show. A comment is
an annotation when it starts with `site:` and a kind. The kind sets what it applies to.

| Kind               | Applies to                       | Keys and flags                                                              |
| ------------------ | -------------------------------- | --------------------------------------------------------------------------- |
| `page`             | the page, wherever it is written | `title`, `description`, `nav-title`, `weight`, flags `hide` and `no-search` |
| `figure`           | the place where it is written    | `src` or `name`, `dark`, `alt`, `caption`                                   |
| `replace` to `end` | the blocks between them          | the keys of `figure`, and the blocks give way to its figure                 |
| `alias`            | the section it is written in     | `words`, the words that find the section in a search                        |

```
<!-- site: page title="The tour" nav-title="Tour" weight=2 -->

<!-- site: figure src=img/a-light.png dark=img/a-dark.png alt="A screen" caption="A screen" -->

<!-- site: alias words="no-lint base-url" -->

<!-- site: replace src=img/b-light.png alt="The same screen" -->
the screen as text, which GitHub shows
<!-- site: end -->
```

A `page` annotation wins over what the text gives: the title is the first level one heading until a
`page` annotation sets another, and the same goes for the description. A value with spaces goes in
double quotes.

`dkt lint` reports an annotation that does not hold: an unknown kind or key, a missing `src` or
`name`, an `alias` with no words, a weight that is not a number, an `end` with no `replace`, a
`replace` never closed. A comment that does not start with `site:` is left alone.
