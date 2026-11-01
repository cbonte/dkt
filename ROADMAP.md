# Roadmap

## 0.3.0 Site groups, vector diagrams, data files, versions and replayed sessions

| Area | Work | Priority | State |
| ---- | ---- | -------- | ----- |
| `site` | Split the menu of a section into titled groups, link to a document of another tree with `[[mirror]]`, leave files out of an asset directory with patterns, and refuse a link to a file that the site does not serve. | 1 | Done |
| `site` | Load the widgets and their style in the pages of any theme, and keep the widgets file of a partial build. | 1 | Done |
| widgets | Allow steps and checklist inside tabs, read a table with `table src` and a session with `replay src` from a file. | 1 | Done |
| `html` | Draw Mermaid diagrams as vector figures that fit the column, tinted by role with custom properties a theme may set. | 1 | Done |
| `cat` | Draw Mermaid diagrams with fewer crossings, joined edges and colours by role. | 1 | Done |
| `replay` | Play a full screen session as a series of SVG snapshots, 1 per step. | 1 | Done |
| widgets | Light the parts of a figure step by step, the steps of a page playing a scene on it. | 1 | Done |
| `site` | Serve several versions of the documentation, with a selector of versions. | 1 | Done |
| `site` | Build a table from a JSON or a YAML file with `table src`. | 1 | Done |

## 0.4.0 More diagrams, more formats

| Area | Work | Priority | State |
| ---- | ---- | -------- | ----- |
| `html` | Draw a Graphviz DOT graph as a diagram, with the layered layout of the Mermaid drawer. | 1 | Planned |
| formats | Read an OpenAPI description as a reference page. | 1 | Planned |
| formats | Read an AsciiDoc document. | 1 | Planned |
| `convert` | Write a manual page, in roff, from a document. | 2 | Planned |
| `replay` | Play a full screen session through a terminal emulator in the browser. | 3 | Planned |

## 0.5.0 Export a site to Confluence

| Area | Work | Priority | State |
| ---- | ---- | -------- | ----- |
| `confluence` | Render pages as storage format: blocks, inlines, links, anchors, images and attachments. | 1 | Planned |
| `confluence` | Render widgets, Mermaid, `diagram` and `ansi` blocks, with a degraded form where Confluence has no equivalent. | 1 | Planned |
| `confluence` | Write the pages, their attachments and a manifest to a directory, with no network, to check them before a push. | 1 | Planned |
| `http` | Reach a Cloud site over native HTTPS with proxy, redirects, timeouts, rate limits and redaction of secrets. | 1 | Planned |
| `confluence` | Read credentials from the decided sources, prompt without echo, and never write or log a secret. | 1 | Planned |
| `confluence` | Probe a site for its account, space and permissions before the first push. | 2 | Planned |

## 0.6.0 Publish a site to Confluence Cloud

| Area | Work | Priority | State |
| ---- | ---- | -------- | ----- |
| `confluence` | Print the plan of a push as a diff of created, updated, unchanged and orphan pages, without writing. | 1 | Planned |
| `confluence` | Create the pages of a space under a root page, mark their ownership and upload their attachments. | 1 | Planned |
| `confluence` | Update only the pages, attachments, titles, parents and sibling order that changed. | 1 | Planned |
| `confluence` | Detect an edit made by a person, skip that page and fail the run, or overwrite on request. | 1 | Planned |
| `confluence` | Resume after an interrupted run and recover lost responses. | 2 | Planned |
| `confluence` | Report the pages that the site no longer holds, and leave them in place. | 2 | Planned |

## 0.7.0 Existing pages and Data Center

| Area | Work | Priority | State |
| ---- | ---- | -------- | ----- |
| `confluence` | Adopt existing pages by title or by ID, with the consent of the author of the push. | 1 | Planned |
| `confluence` | Archive or delete the pages that the site no longer holds, per the decided policy. | 1 | Planned |
| `confluence` | Push to Confluence Data Center over REST v1 with personal access tokens. | 1 | Planned |
| `confluence` | Push a part of a site with `--only`. | 2 | Planned |
