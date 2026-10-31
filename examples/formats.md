# Formats

A site mixes documents of several formats. Each one is converted to Markdown when the site is built,
so the pages look alike and the same search, menus and widgets apply.

The properties file does not change from one format to another. The extension gives the format of a
`.md` or a `.rst` file, and any other file names it with `format`.

```
[[page]]
src = "examples/rst/api.rst"
url = "/examples/rst-sample/"

[[page]]
src = "examples/haproxy/excerpt.txt"
format = "haproxy"
url = "/examples/haproxy-sample/"
```

## The formats

<!-- site: widget tabs id=formats -->

### Markdown

The format of this page. It stays as GitHub shows it, with the notes of the site in comments.

```
# A page

A [link](other.md) to another document, a table and an alert.

> [!NOTE]
> The page reads the same on GitHub.
```

### reStructuredText

Read as Sphinx documents are. Tables, definition lists, footnotes, substitutions, options, included
files and the common directives all come out as Markdown.

```
Options
-------

-v, --verbose   Print what is done.

draw
    Put a component on a page.
```

See the [whole page](rst/api.rst), served from a `.rst` file.

### HAProxy

The documentation text of HAProxy, which has no markup of its own. The sections, the keywords, the
tables drawn with characters and the examples are read from the layout of the text.

```
balance <algorithm> [ <arguments> ]
  Define the load balancing algorithm to be used in a backend.

  Arguments :
    <algorithm> is the algorithm used to select a server.
```

See the [whole page](haproxy/excerpt.txt), served from the text of the manual. The [HAProxy
demo](https://cbonte.github.io/dkt/demos/haproxy/) builds 20 versions of it.

<!-- site: end -->

## Before it is served

The converters are also commands, so a document can be checked before it is served.

```
dkt convert examples/rst/api.rst
dkt html --format haproxy configuration.txt
```

See [the rst format](../docs/rst.md) and [the haproxy format](../docs/haproxy.md).
