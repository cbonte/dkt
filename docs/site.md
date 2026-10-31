# dkt site

Builds a static site from Markdown documents that stay as GitHub shows them, and a properties file.

```
dkt site                                  # reads dkt.toml, writes public/
dkt site docs/dkt.toml -o out            # another file, another directory
dkt site --base-url http://localhost:8000/preview
dkt site --no-lint                        # skip the check that runs first
dkt site --only section:3.4 --only home   # build a part of the site
```

The paths of the properties file are read from the directory that holds it. The output directory,
which `--output` (`-o`) names, is cleared before each build, but only when `dkt site` made it, which
a `.dkt-site` file inside tells. Any other directory that is not empty is left alone, and the build
stops.

The documents are read and rendered on several threads, as many as the cores up to 8. The
variable `DKT_JOBS` sets the number, and `DKT_JOBS=1` builds on one thread with the least memory.

## Building a part of a site

`--only` builds a part of the site and leaves the rest of the output directory as it is. It can be
given several times.

| Value          | What is built                                         |
| -------------- | ----------------------------------------------------- |
| `home`         | the home page                                         |
| `section:NAME` | the pages of a section and the index of the section   |
| `page:SRC`     | one page, by the `src` of its properties              |
| `static`       | the files of the theme, of the site and of the assets |

The documents out of the scope are not read, so a build can run where only the documents that
changed are present. The check runs over the documents that are built, and a link to a page that is
not built goes to the address the properties give it. The search index keeps its entries for the
pages that are not built and replaces the others.

A template that lists the pages of a section shows none for a section that is not built.

```
dkt site --only section:3.4 --only home -o public
```

## What the documents give

| The site needs      | It comes from                                                        |
| ------------------- | -------------------------------------------------------------------- |
| title of a page     | the first level one heading                                          |
| its description     | the paragraph that directly follows it                               |
| anchors             | the ones GitHub gives, so a link written for GitHub holds            |
| links between pages | the relative links of the documents, such as `guides/a.md#start`     |
| a file not served   | a link to it in the repository, at the branch of the properties file |
| anything else       | a [site annotation](html.md#annotations) or the properties file      |

A relative link to a served document becomes a link to its page, with its anchor. A link to a file
of an asset directory points to where the site serves it. A link to any other file that exists goes
to the repository. A target that does not exist is left as written, and the check reports it.

The body of a page starts with its title and its first paragraph, as the document does on GitHub, so
a template does not write them again.

## The properties file

It is a TOML file.

| Table         | What it says                                                                                                       |
| ------------- | ------------------------------------------------------------------------------------------------------------------ |
| `[site]`      | `title`, `description`, `base_url`, `repo`, `branch`, `templates`, `static`, `theme`, `search`                     |
| `[[nav]]`     | a menu entry: `name`, `url`, and `section` to keep it current in a section                                         |
| `[home]`      | the page at the root: its `template`, and a `src` document that the template shows                                 |
| `[[section]]` | a group of pages with an index of its own: `name`, `url`, `title`, `description`, `template`, `sidebar`            |
| `[[page]]`    | documents to serve: a `src` with its `url` and `title`, or a `glob` such as `docs/guides/*.md`, and their `format` |
| `[[asset]]`   | a directory of files the pages point at: `src`, and the `url` it is served at                                      |
| `[figures]`   | where a figure given by name is found: `src` and `dark`, with `{name}` replaced                                    |
| `[colors]`    | `light` and `dark`, tables of colours that replace those of the theme                                              |
| `[lint]`      | `disable` and `enable`, lists of rules for the check that runs first                                               |
| `[data]`      | anything, read by the templates as `data`                                                                          |

For a `glob`, `section` puts the pages in a section and in its address, and `order` lists the file
stems that come first. The pages of a `src` come in the order they are written. A `page` annotation
with a `weight` wins over that order.

The `title` of a `src` page replaces the first level one heading of its document, which is how a
site shortens the title of a document that repeats the name of its section. The title of the section
comes after it in the title of the browser and in the search, so that the same page of 2 sections
can be told apart.

The `format` of a page says how its file is written. It is `markdown`, `rst` for reStructuredText,
or `haproxy` for a documentation text of HAProxy. A document that is not Markdown is converted
before the check and the build. The format comes from the extension when omitted for `.md` and
`.rst`. Any other extension needs a `format`, and so does a `.txt` file, which is not a format of
its own.

```
[site]
title = "lokode"
base_url = "https://loken-ai.github.io/lokode"
repo = "https://github.com/loken-ai/lokode"

[[nav]]
name = "Guides"
url = "/guides/"
section = "guides"

[[section]]
name = "guides"
sidebar = true

[[page]]
glob = "docs/guides/*.md"
section = "guides"
order = ["using", "safety"]

[figures]
src = "/img/{name}-light.png"
dark = "/img/{name}-dark.png"
```

The path of `base_url` prefixes every address of the site, so a site published under a project name
works, and `--base-url` replaces it to look at a build elsewhere.

## Theme

<!-- site: alias words="skin look style" -->

A theme is a set of `templates` and `static` files that several sites share. A site names one with
`theme` in `[site]`, as a directory or as the name of a built-in theme. A directory of that name
wins when it holds `templates` or `static`, so the `docs` folder of a project does not hide the
theme `docs`.

| Built-in theme | What it is                                                                                                              |
| -------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `docs`         | a documentation layout: the pages of a section on the left, the headings on the right, which fold and follow the scroll |
| `paper`        | one column of text in a serif, with as little around it as possible                                                     |
| `plain`        | the system fonts and colours, a few lines of style, and a script only for the search                                    |

A site that names no theme gets `docs`. Its templates and files are looked up last, so a site that
has all its own templates loses nothing, and the files of the theme are the only addition to its
output.

In the `docs` theme, every heading below the title is a link to itself, with a button to copy its
address when the pointer is over it. A wide table scrolls in its own box, and a long entry of a menu
ends with an ellipsis and shows its text as its title.

`docs` and `paper` carry a switch between the light and the dark theme. Without a choice, all 3
follow the system.

The templates of the site are looked up first, then the ones of the theme. The files of the theme
are copied first, then the `static` files of the site, which win. A site can replace any template of
a theme and keep the others.

Every built-in theme has the same templates, so a site can change theme without changing anything
else.

| Template              | Use                                                                                |
| --------------------- | ---------------------------------------------------------------------------------- |
| `base.html`           | the frame of every page: header, menu, footer, with the blocks `head` and `footer` |
| `page.html`           | a document                                                                         |
| `section.html`        | the index of a section                                                             |
| `index.html`          | the home page: the title, the description and the pages by section                 |
| `partials/brand.html` | what the header shows for the site, the title by default                           |

A site that wants a logo in the header writes its own `partials/brand.html`. A template that extends
`base.html` adds a stylesheet in the block `head` and replaces the footer in the block `footer`.

The home template reads the document of `[home]` as `page`, with `page.title`, `page.description`,
`page.content` for all of it and `page.rest` for what follows the title and its paragraph. These
values of `[data]` are read:

| Value           | Use                                    |
| --------------- | -------------------------------------- |
| `install`       | a command shown on the home page       |
| `brand.favicon` | the address of an icon, under the site |

## Colours

`[colors.light]` and `[colors.dark]` give the colours of the 2 appearances, each one by the name of
a variable of the theme. They apply to every page of the site, the home page included.

```
[colors.light]
accent = "#1d4ed8"
accent-bg = "#e8eefc"

[colors.dark]
accent = "#7aa2ff"
accent-bg = "#141d36"
```

| Name        | What it colours                              |
| ----------- | -------------------------------------------- |
| `bg`        | the background of the page                   |
| `surface`   | the background of blocks and tables          |
| `surface2`  | the background of inline code                |
| `line`      | borders and rules                            |
| `fg`        | the text                                     |
| `soft`      | secondary text                               |
| `muted`     | the quietest text, such as the menus         |
| `accent`    | the current item, and what the pointer is on |
| `accent-bg` | the background of the current item           |
| `link`      | links, the accent when it is not given       |

A value is a colour and nothing else. Any other name or value stops the build. The docs theme reads
these variables, and the other themes have their own.

## Search

Every site gets `search.json` at the root, and `search = false` in `[site]` leaves it out. It lists
the title of each page and of each section of each page. Each entry has the address and the anchor
that lead to it, the section above it, the description of a page, and the aliases it was given.

An `alias` annotation gives a section words that find it without being in its title. It applies to
the section it is written in. A `page` annotation with the flag `no-search` leaves the page out.

```
## The check

<!-- site: alias words="no-lint skip" -->
```

The palette of the themes opens with the button of the header, with the slash key or with Ctrl+K. An
address that ends with `?q=words` opens it on those words. Opened with nothing typed, it lists the
pages opened from it lately, or else the pages of the site. The section a result leads to is
highlighted for a moment when the page opens.

The palette matches what is typed against the titles.

| Rule                                                                                       |
| ------------------------------------------------------------------------------------------ |
| case and accents are ignored                                                               |
| each word has to be found, in any order                                                    |
| a word found at the start of a word ranks above one found inside it                        |
| a title ranks above the aliases, then the page and the section above, then the description |
| a long word with one slip of the keys still finds a title, and so do its letters in order  |
| a query that starts with `#` looks at sections only, and one that starts with `>` at pages |
| a page ranks above a section, and a shorter title above a longer one                       |
| a title is also matched without its trailing parenthesis, as `bind` in `bind (Peers)`      |
| a result under the part of the site being read ranks above the same result elsewhere       |

A theme without a palette ignores the file. A template of a site can read it with `fetch`.

## Templates

A template is a Jinja file in the `templates` directory. It can extend and include the others.

| Name       | Value                                                                                          |
| ---------- | ---------------------------------------------------------------------------------------------- |
| `site`     | `title`, `description`, `base_url`, `base_path`, `repo`, `search`, `nav` and `sections`        |
| `sections` | by name: `title`, `description`, `url`, `sidebar` and `pages`, in order                        |
| `nav`      | the menu for the page being drawn, each entry with `name`, `url` and `current`                 |
| `page`     | `title`, `description`, `content`, `headings`, `path`, `url`, `section`, `nav_title`, `weight` |
| `section`  | on an index page, the section it lists                                                         |
| `data`     | the `[data]` table                                                                             |

Each entry of `pages` has `title`, `nav_title`, `description`, `url`, `weight` and `section`. The
content is HTML already, so it is not escaped. Everything else is, except the slash.

## The check

<!-- site: alias words="no-lint skip lint" -->

Before anything is written, the documents are checked as `dkt lint` does, with its default rules and
the changes of `[lint]`. A finding stops the build and is printed as `file:line:column: rule:
message`. A page hidden by a `page` annotation is not served, not listed and not linked to.
