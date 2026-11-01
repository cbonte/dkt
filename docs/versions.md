# Versions of a documentation

Serves several versions of a documentation under 1 address, with a selector of versions in every page.

```
dkt site v1.1/dkt.toml -o public --docs-version 1.1   # build the version 1.1 into public/1.1
dkt site dkt.toml -o public --only versions            # refresh the root from the declared list
```

Each version is built alone, from its own sources and its own properties file, and is kept as it was built.
A release builds its version, then refreshes the root.
An old version is never built again by a newer release.
The directory `examples/versions` of the repository holds a sample of 2 versions to build this way.

## The tree

```
public/
  index.html       leads to the latest release and lists every version
  versions.json    the declared list, read by the selector in the browser
  latest/          redirect pages to the same page in the latest release
  1.1/             a version, with its own pages, index and files
  1.0/
  2.0/
```

A version is an ordinary site whose base address ends with its name.
With `base_url = "https://example.com/docs"`, the version 1.1 is served at `https://example.com/docs/1.1/`.
Its links, its search and its widgets stay inside that address.

## Declaring the versions

The versions are declared in the properties file of the default branch, newest first.

```
[[version]]
name = "2.0"
title = "2.0-dev"
status = "dev"

[[version]]
name = "1.1"

[[version]]
name = "1.0"
status = "deprecated"
```

| Key      | Meaning                                                  |
| -------- | -------------------------------------------------------- |
| `name`   | the directory of the version and the segment of its address |
| `title`  | the label of the version in the selector, the name when omitted |
| `status` | `release` when omitted, `dev` or `deprecated`            |

The latest version is the first `release` in the order of the declaration.
dkt does not compare version names, so `2026.10` or `v3` need no rule.

| Status            | Selector                  | Warning above the content                              |
| ----------------- | ------------------------- | ------------------------------------------------------ |
| the latest release | marked `(latest)`         | none                                                   |
| an older release  | its title                 | a newer version exists, with a link to the latest      |
| `dev`             | marked `(development)`    | a development version that is not released             |
| `deprecated`      | marked `(deprecated)`     | a deprecated version, with a link to the latest        |

A name is made of letters, digits, `.`, `_` and `-`, and does not start with `.`.
The names `latest`, `index.html` and `versions.json` belong to the root and are refused.

## Building a version

`--docs-version NAME` builds the version NAME into the directory `NAME` of the output directory.
Its base address is the base address of the site followed by `/NAME`, and `--base-url` sets the base of the root.
The build leaves the other versions and the files of the root as they are.

The build reads `versions.json` of the output directory, when it exists, for the selector and for the status of its version.
The list of the maintenance branch of an old version is never read, so it cannot publish a stale list.
A version that `versions.json` does not name yet is built as a release, with a warning, until the root is refreshed.

A version is built whole, so `--only` does not go with `--docs-version`.
A version cannot be built into the directory of a site without versions, and a site without versions cannot be built over a root of versions.

## Refreshing the root

`--only versions` reads the `[[version]]` tables and writes the root, and nothing else.

| File            | Content                                                                      |
| --------------- | ---------------------------------------------------------------------------- |
| `versions.json` | each version with its `name`, `title`, `status` and `url`, the `latest` release and a `format` number |
| `index.html`    | a refresh to the latest release, and the list of the versions as plain links |
| `latest/`       | 1 redirect page per page of the latest release, which keeps the anchor       |

The refresh stops before it writes anything in 2 cases.

- A declared version has no directory, or a directory that no version build made.
- A directory holds a version that the properties do not declare, so a forgotten draft is not published.

The root files are derived again on each refresh, in the order of the declaration.
2 refreshes give the same bytes, whatever order the versions were built in.
Each file is written next to its place, then renamed over the old one.

When no version is a release, the root leads to the first version declared and `latest/` is removed.

A newer format of `versions.json` adds fields and never renames one, so the selector of an old version still reads it.

## The selector

The built-in themes show the selector in the header, after the menu, and the warning above the content.
The selector is a disclosure whose links lead to the same page in each version.
A version whose directory does not hold that page gets a link to its home.

| Property         | How                                                              |
| ---------------- | ---------------------------------------------------------------- |
| keyboard         | the summary takes the focus, Enter and Space open it, Tab walks the links, Escape closes it |
| current version  | the label of the summary and `aria-current` on its link          |
| state            | written as text in the label, never as a colour alone            |
| without JavaScript | the links work, for the versions known when the page was built |

The script of the versions reads `versions.json` when the page opens.
It adds the versions built after the page, and shows the warning when the version is no longer the latest or became deprecated.
A page built as the latest release thus learns of a newer one without being built again.

A choice in the selector leads to the same page and the same anchor in the target version.
The script reads the pages of the target and their anchors from `version.json` in its directory.

| The target has              | The reader lands on               |
| --------------------------- | --------------------------------- |
| the page and the anchor     | the same page at the same anchor  |
| the page and not the anchor | the same page, with a note        |
| not the page                | the home of the target, with a note |

The note names the heading that the target lacks, and closes with its button or with Escape.
It travels to the next page in the storage of the browser tab, so the address stays clean.

## Templates

A version gives its templates 2 more values.

| Name           | Value                                                                                   |
| -------------- | --------------------------------------------------------------------------------------- |
| `site.version` | `name`, `title`, `status`, `latest`, `root` the address of the root, and `note` the warning: `dev`, `deprecated`, `newer` or empty |
| `versions`     | 1 entry per version, each with `name`, `title`, `status`, `url`, `current` and `latest` |

`site.version` is undefined in a site without versions, so a template tests it before it reads a field.
`versions` is empty there.
A site without versions is built as it was, byte for byte.

Each built-in theme has `partials/versions.html` for the selector and `partials/version-note.html` for the warning.
A site that writes its own `base.html` can include them.

A theme of its own gets the files of the versions added to each page of a version, as the widgets are.
It gets the meta that names `versions.json`, `versions.css` and `versions.js`.
The script inserts the warning at the start of `main`.
The selector stays the choice of the theme, through `versions`.

## Updating a version from CI

The built tree is kept between runs, such as on a branch served by GitHub Pages.
A run builds the version that changed, refreshes the root, and commits what changed.

```
dkt site src/dkt.toml -o tree --docs-version 1.1
dkt site main/dkt.toml -o tree --only versions
git add tree/1.1 tree/versions.json tree/index.html tree/latest
git diff --cached --quiet || git commit -m "docs: build 1.1"
```

The marker `.dkt-site` of a version directory is its manifest.
It names the dkt that built the version, the version, and every file the build wrote.

| Case                                              | What the next build of the version does        |
| ------------------------------------------------- | ---------------------------------------------- |
| a file the manifest lists and the build no longer writes | removes it                                |
| a file the manifest does not list, such as `CNAME` | keeps it as it is                             |
| a file the manifest does not list and the build writes | replaces it, with a warning                |
| a version built by another dkt                    | builds it, with a warning that names both versions |
| an empty marker of an earlier dkt                 | takes every file as written by dkt             |

A version is written into the hidden sibling `.NAME.dkt-new`, then put in place of `NAME`.
A build that fails leaves the version directory as it was.
A sibling left by a run that was killed is removed by the next build of that version.

The same sources, the same dkt and the same options give the same bytes.
A run whose build changed nothing leaves nothing to commit.
A maintenance branch should pin the dkt that builds its documentation, since a newer dkt changes the look of a version.

## Search

Each version has its own index, so the palette of 1.0 finds the titles of 1.0 only.
The selector leads to the same page in another version.

## An example with HAProxy

Each version of the documentation of HAProxy has its own directory of sources, filled from its release tag.

```
[[page]]
src = "configuration.txt"
format = "haproxy"
url = "/configuration/"
title = "Configuration Manual"
```

```
dkt site src/3.4/dkt.toml -o public --docs-version 3.4
dkt site src/3.2/dkt.toml -o public --docs-version 3.2
dkt site dkt.toml -o public --only versions
```

A reader of `/3.4/configuration/#timeout-client` who picks 3.2 lands on the same keyword in 3.2, when 3.2 has it.
