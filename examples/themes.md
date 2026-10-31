# Themes

A theme is a set of templates and files that sites share. 3 come with the binary, and a site
picks one with `theme` in its properties file. A site can replace any template of a theme and keep
the others.

Each link below opens the same 2 pages in another theme. They are built from the files in
`examples/themes` when the site is published.

| Theme                       | Look                                                          | Demo                                                       |
| --------------------------- | ------------------------------------------------------------- | ---------------------------------------------------------- |
| `docs`                      | the pages of a section on the left, the headings on the right | [open](https://cbonte.github.io/dkt/demos/themes/docs/)    |
| `paper`                     | one column of text, in a serif                                | [open](https://cbonte.github.io/dkt/demos/themes/paper/)   |
| `plain`                     | the system fonts, a few lines of style                        | [open](https://cbonte.github.io/dkt/demos/themes/plain/)   |
| `docs` with its own colours | the same theme, with a green accent for the 2 appearances     | [open](https://cbonte.github.io/dkt/demos/themes/colours/) |

All of them follow the system between the light and the dark appearance. `docs` and `paper` have a
switch in the header, and each one has the search palette, opened with the slash key.

## The same pages

<!-- site: widget tabs id=themes -->

### Properties

The only change from one demo to the next is the `theme` line.

```
[site]
title = "Sample"
theme = "paper"
base_url = "https://cbonte.github.io/dkt/demos/themes/paper"

[[section]]
name = "guide"
sidebar = true

[[page]]
glob = "pages/*.md"
section = "guide"
```

### Colours

`[colors.light]` and `[colors.dark]` give the colours of the 2 appearances, by the name of a
variable of the theme.

```
[colors.light]
accent = "#15803d"
accent-bg = "#e6f4ea"

[colors.dark]
accent = "#6ee7a0"
accent-bg = "#10281a"
```

### Templates

A template of the site is looked up before the one of the theme. A site that wants a logo in the
header writes `partials/brand.html`, and nothing else.

```
templates/
  partials/
    brand.html
```

<!-- site: end -->

See [dkt site](../docs/site.md#theme) for the templates, the variables and the files of a theme.
