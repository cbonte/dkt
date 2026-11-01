# Unreleased

## Added

- site: load the widgets and their style in the pages of any theme.
- site: refuse a link to a file that the site does not serve, as link-unserved.
- site: leave files out of an asset directory with patterns, at any depth.

## Changed

- haproxy: break the line where the text breaks it after a full stop, a colon or a question mark.
- site: refuse repo and branch in the site table, and read the repository link from data.repo instead.

## Fixed

- site: keep the widgets file of the pages a partial build does not build.

# 0.2.0 - 2026-10-31

## Added

- widget: wrap blocks in a widget annotation to make tabs, a checklist, steps or a quiz.
- widget: scroll a carousel like a gallery, with several slides in view, a loop and auto-play.
- widget: enlarge the pictures of a carousel, and load the ones out of view when needed.
- widget: group tabs, so that a choice made in one set is made in the others.
- widget: filter and sort a table, with the filter in the address.
- widget: compare 2 pictures with a slider, and read a diff on 1 side or on 2.
- widget: draw a table as bars or lines, and turn a table into flashcards.
- widget: put numbered pins on a picture, tied to the items of a list.
- widget: write a command from a form, and play a terminal session back.
- widget: show only the versions of a changelog that come after the reader's.
- widget: show what the reader has done in the whole site, page by page.
- widget: document every widget, its options, what it remembers and its fallback.
- lint: report a widget that is unknown, nested or never closed.
- lint: let the tabs of a widget repeat their names.
- html: draw Mermaid blocks as SVG figures, and pie charts as pies.
- html: draw ansi blocks with the colours of a terminal.
- site: show examples of formats, themes, widgets and terminal output.
- site: show a card with the title and the description of a page on a link to it.
- site: end the pages of a section with links to the previous and to the next page.
- theme: add a copy button to the code blocks.
- theme: style the alerts and the footnotes of a document.
- theme: close the gaps between the lines of a drawing made of box characters.
- rst: read tables, definition lists, line blocks, footnotes and substitutions.
- rst: read included files, the entries of a toctree, list tables and csv tables.
- rst: read version notes, topics, rubrics, math, option lists and doctests.
- rst: read named and anonymous references, and the common inline roles.

## Changed

- site: convert each document once and build the pages on several threads.
- lint: read the anchors of a file as a stream, with a fraction of the memory.
- ci: run the checks on a pull request, a tag or a manual start, not on every push.

## Fixed

- fmt: keep the marker of an alert alone on its line.
- rst: read the address of a target, which was cut at its last colon.
- rst: read a directive with a space before its colons.

# 0.1.0 - 2026-10-31

## Added

- cat: show a link to the same document as its text.
- cat: draw Mermaid flowcharts as text.
- cat: draw Mermaid rounded and diamond nodes and multi-line labels.
- cat: draw Mermaid edge labels and dotted and thick edges.
- cat: draw Mermaid sequence diagrams as text.
- cat: show Mermaid blocks as code on request.
- cat: draw Mermaid pie charts as bars.
- cat: draw Mermaid state diagrams as text.
- cat: draw Mermaid class diagrams as text.
- cat: draw Mermaid entity-relationship diagrams as text.
- cat: stack a Mermaid flowchart top-down when left-to-right is too wide.
- cat: draw a rule between table rows, or a blank line with --table-rules never.
- cat: render a Markdown file as styled text on the terminal.
- cat: render several files in order with a file name header on each.
- cat: join the lines of a paragraph by default and keep them on request.
- cat: highlight the syntax of code blocks.
- cat: fit table columns to the line width.
- cat: render GitHub alerts such as NOTE and WARNING.
- cat: render footnotes as a numbered list at the end of the document.
- cat: hide the YAML or TOML front matter unless asked to show it.
- cat: send the output to a pager on request.
- dkt: print a completion script for a given shell.
- dkt: print the manual as one roff page per command.
- cat: list the outline of a document with the anchor of each heading.
- cat: render one section chosen by its heading text or its anchor.
- toc: insert or refresh a table of contents between two HTML comment markers.
- toc: report the files whose table of contents is stale.
- lint: report a site annotation that does not hold, with the rule site-annotation.
- lint: report relative links and images that point to a missing file.
- lint: report anchors that match no heading in the file or in the linked one.
- lint: report skipped heading levels and duplicate headings.
- fmt: rewrap paragraphs to a target width without touching inline markup.
- fmt: normalise list markers and heading syntax and realign tables.
- fmt: report the files that would change or print a diff of the changes.
- rst: read reStructuredText, as Sphinx documents are written.
- cat: read a document in any format, from its extension or with --format.
- cat: write a file that no format claims as it is, with a warning.
- haproxy: convert a HAProxy documentation text to Markdown, as a format of a page of a site.
- haproxy: read tables drawn as a grid, with headers that group columns.
- haproxy: keep drawings as diagrams and split a paragraph before the list it introduces.
- haproxy: show a command with marks under it as a diagram.
- haproxy: read ordered lists, definitions that line up, columns lined up with spaces and usable in lines.
- haproxy: read paragraphs, examples and lists inside the items of a list, and notes of every kind.
- haproxy: link the keywords that start an item or a row of the matrix, and flag deprecated keywords.
- haproxy: read chapters whose number is closed with a parenthesis.
- convert: convert a document in any format to Markdown.
- html: read a document in any format, from its extension or with --format.
- theme: cut the long entries of the menus with an ellipsis and a title.
- theme: make every heading a link to itself, and show a link to copy on hover.
- theme: fold the titles of level 3 in the menu on the right of the docs theme.
- html: render Markdown as an HTML body or a page in a template, with GitHub anchors.
- html: act on site annotations written as comments: a page title and weight, a figure, a replaced region.
- html: draw a diagram block as an SVG figure.
- site: name the format of a page, markdown, rst or haproxy.
- site: show a document on the home page, and let a template add a stylesheet and change the footer.
- site: give the colours of a site for the light and the dark appearance.
- site: build a part of a site, the home page, a section or a page.
- site: give the title of a page in the properties and tell its section in the search.
- site: build a static site from Markdown documents, a properties file and templates.
- site: share templates and files between sites through a theme that each site can override.
- site: build with the docs, paper or plain theme that comes with the binary, docs by default.
- site: write an index of the titles of the pages and of their sections, unless search is off.
- site: search those titles from a palette in the docs, paper and plain themes.
- site: find a section by words given in an alias annotation, and by one slip of the keys.
- site: show the section above and the description of a page in the results.
- site: open the palette on the pages opened from it lately.
- site: leave a page out of the search with the flag no-search.
- site: let a site add a block at the top of the menu of the docs theme.
- site: let the menu and the list of headings of the docs theme scroll, and keep the current heading in view.
- site: look at sections only or pages only with a hash or a greater-than sign.
- site: highlight the section that a search result leads to.
