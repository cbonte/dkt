# Widgets

Components for a page, written as ordinary Markdown between 2 comments. GitHub and the terminal show
the Markdown as it is, and the site makes a component of it.

```
<!-- site: widget NAME key=value -->
the blocks of the widget
<!-- site: end -->
```

The blocks go through the same rendering as the rest of the page, so headings keep their anchors and
links keep their targets. Without JavaScript the blocks stay in place and the page reads the same.

| Widget      | Blocks it holds                          | Keys           |
| ----------- | ---------------------------------------- | -------------- |
| `tabs`      | a heading for each tab, and what follows | `id`           |
| `checklist` | a list of tasks                          | `id`           |
| `steps`     | an ordered list                          | `id`           |
| `quiz`      | a question, a list of choices, an answer | `id`, `answer` |
| `carousel`  | slides, separated by thematic breaks     | `id`           |

`id` names the widget for what the reader did, such as the box ticked or the tab chosen. It is
letters, digits, hyphens and underscores. A widget with no `id` is named by its place on the page,
so give one to any widget whose state matters when the page changes. The state stays in the browser
of the reader, under the address of the page.

A widget cannot hold another one, and a `widget` or a `replace` cannot start inside one. `dkt lint`
reports an unknown widget or key, a nested one, and a widget that no `end` closes.

## Tabs

The shallowest heading level inside the widget gives the tabs. A block before the first heading
stays above them.

<!-- site: widget tabs id=install -->

### cargo

```
cargo install --path .
```

### From the source

```
git clone https://github.com/cbonte/dkt
cd dkt
cargo build --release
```

<!-- site: end -->

```
<!-- site: widget tabs id=install -->
### cargo

...

### From the source

...
<!-- site: end -->
```

## Checklist

The boxes of the list can be ticked, and a bar counts them.

<!-- site: widget checklist id=start -->

- [ ] Write a `dkt.toml`
- [ ] Add a `[[page]]` for each document
- [ ] Run `dkt site`

<!-- site: end -->

## Steps

Each step has a button to mark it done. The first step not done is highlighted.

<!-- site: widget steps id=publish -->

1. Build the site with `dkt site`.
2. Read the pages in the output directory.
3. Publish the directory.

<!-- site: end -->

## Quiz

`answer` is the position of the right choice, from 1. What follows the list shows once the reader
has chosen.

<!-- site: widget quiz answer=2 id=format -->

Which format does `dkt fmt` read?

1. reStructuredText
2. Markdown
3. A documentation text of HAProxy

It reads Markdown only. The other commands read the 3 formats.

<!-- site: end -->

## Carousel

A thematic break (3 hyphens on a line) cuts the slides.

<!-- site: widget carousel id=tour -->

**Write.** The documents stay as GitHub shows them.

---

**Check.** `dkt lint` reports the links and anchors that do not hold.

---

**Publish.** `dkt site` builds the pages, the search and the menus.

<!-- site: end -->

## Copy button

Every code block of the site gets a button that copies it, with or without a widget.

## In a theme of your own

A theme that is not built in loads the 2 files the built-in themes carry, `widgets.css` and
`widgets.js`, from its `base.html`.

```
<link rel="stylesheet" href="{{ site.base_path }}/widgets.css">
<script src="{{ site.base_path }}/widgets.js" defer></script>
```

The style reads the variables `--bg`, `--surface`, `--line`, `--fg`, `--muted`, `--accent` and
`--accent-bg`, and has a fallback for each.
