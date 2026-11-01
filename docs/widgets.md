# Widgets

A widget is a component of a page, such as tabs, a carousel or a quiz, that a document asks for with
2 comments around ordinary Markdown.

The comments are invisible on GitHub and in the terminal, which show the Markdown as it is. The site
makes a component of the blocks between them. [The examples](../examples/widgets.md) show each
widget at work.

## Syntax

```
<!-- site: widget NAME key=value flag -->
the blocks of the widget
<!-- site: end -->
```

The name of the widget comes first, then its options. A key is written `key=value`, and a value with
spaces goes in double quotes. A flag is a word with no value.

| Option    | Applies to   | Effect                                   |
| --------- | ------------ | ---------------------------------------- |
| `id=NAME` | every widget | names the widget for what the reader did |

`id` is made of letters, digits, hyphens and underscores. A widget with no `id` is named by its
place on the page, as `w1`, `w2` and so on. Give an `id` to any widget whose state matters, because
an edit that adds a widget above it would rename it.

## Rules

- A widget is closed by an `end`, and cannot hold another widget, except `tabs`, which may hold
  `steps` and `checklist`, each closed by its own `end`.
- A `replace` cannot start inside a widget, nor a widget inside a `replace`.
- The blocks of a widget go through the same rendering as the rest of the page, so headings keep
  their anchors and links keep their targets.
- An annotation that does not hold is left as a comment, and `dkt lint` reports it: an unknown
  widget, an unknown key or flag, a value out of range, a widget inside another one and a widget
  that no `end` closes.
- The tabs of a widget may repeat the names of other tabs, so the rule on repeated headings leaves
  them alone.

Write the blocks so that they read well in order, since GitHub shows them without the component. The
tabs come one after the other, the slides of a carousel are separated by rules, and a table stays a
table.

## How it works

The build wraps the blocks in a `div` with the class `dkt-widget`, the name of the widget in
`data-widget` and its options in other `data-` attributes. A script of dkt builds the
component from what the `div` holds.

Without the script, the blocks stay in place and the page reads the same. The carousel is the
exception that is better than plain: its slides are a row that scrolls, with no script.

| File               | Holds                                                                         |
| ------------------ | ----------------------------------------------------------------------------- |
| `widgets.js`       | the copy button of the code blocks, tabs, checklist, steps, quiz and carousel |
| `widgets-extra.js` | the other widgets, which a page loads only when it holds one                  |
| `widgets.css`      | the style of the widgets                                                      |
| `content.css`      | the style of the alerts, the footnotes, the pager and the drawings            |

`dkt site` writes these 4 files to every site, whatever its theme.
It adds the 2 style sheets to the head and the script to the end of each page that does not load them, so a theme of your own needs nothing.
A theme that loads them itself is left as it is, and a file of the same name in the `static` folder of a theme or of the site replaces the one of dkt.

`widgets.js` loads `widgets-extra.js` from the same folder. The style reads the variables `--bg`,
`--surface`, `--line`, `--fg`, `--muted`, `--accent` and `--accent-bg`, and has a fallback for each.
Every widget works with the appearance of the system and with the switch of the theme.

## What stays in the browser

The state of a widget is kept in the browser of the reader, in `localStorage`, and nothing is sent
anywhere. A reader with the storage blocked gets the widgets with no memory.

| What                      | Key                                                          | Shared        |
| ------------------------- | ------------------------------------------------------------ | ------------- |
| the state of a widget     | `dkt:` and the path of the page, then the `id`               | the widget    |
| the tab chosen in a group | `dkt-group:` and the name of the group                       | all the pages |
| the view of a diff        | `dkt:diff-view`                                              | all the diffs |
| the filter of a table     | `filter-ID` in the address, as in `?filter-providers=ollama` | the link      |

A change of `id`, or a move of the page to another address, forgets the state.

## The widgets

| Widget                      | Blocks it holds                                            | Options                            |
| --------------------------- | ---------------------------------------------------------- | ---------------------------------- |
| [`tabs`](#tabs)             | a heading for each tab, and what follows                   | `group`                            |
| [`checklist`](#checklist)   | a task list                                                |                                    |
| [`steps`](#steps)           | an ordered list                                            |                                    |
| [`quiz`](#quiz)             | a question, a list of choices and what explains the answer | `answer`                           |
| [`carousel`](#carousel)     | slides, separated by thematic breaks                       | `show`, `autoplay`, `loop`, `zoom` |
| [`table`](#table)           | a table                                                    |                                    |
| [`compare`](#compare)       | 2 pictures                                                 |                                    |
| [`diff`](#diff)             | a code block of the language `diff`                        |                                    |
| [`chart`](#chart)           | a table of numbers                                         | `type`                             |
| [`flashcards`](#flashcards) | a table of 2 columns                                       |                                    |
| [`annotated`](#annotated)   | a picture and a list                                       | `pins`                             |
| [`builder`](#builder)       | a table of fields and a code block                         |                                    |
| [`replay`](#replay)         | a code block of a terminal session                         | `speed`                            |
| [`since`](#since)           | headings that start with a version                         |                                    |
| [`progress`](#progress)     | nothing                                                    |                                    |

Every widget also takes `id`.

## Tabs

```
<!-- site: widget tabs group=system id=install -->
### Linux

...

### macOS

...
<!-- site: end -->
```

The shallowest level of heading among the blocks gives the tabs, and the text under each heading is
its panel. What comes before the first heading stays above the tabs.

| Option  | Value  | Effect                                         |
| ------- | ------ | ---------------------------------------------- |
| `group` | a name | the tabs with the same group follow each other |

A group matches the tabs by their text. A reader who picks `macOS` in one group gets `macOS` in all
the others of the page and of the next pages, and a group that has no such tab stays as it is.

The tab shown is, by priority, the one that holds the heading of the address, the one the group
chose and the one the reader left open. The arrows, `Home` and `End` move between the tabs.

## Checklist

A list of tasks, written as a Markdown task list. The boxes can be ticked, and a bar counts them as
`done/total`. The boxes ticked are remembered.

## Steps

An ordered list. Each item has a button, `Mark as done` or `Undo`, and the first item not done is
highlighted. Marking a step done takes the focus to the next one, and a bar counts the steps done.

## Quiz

```
<!-- site: widget quiz answer=2 id=format -->
The question.

1. A choice
2. The right choice
3. Another choice

What follows the list, shown once the reader has answered.
<!-- site: end -->
```

The first list holds the choices. `answer` is required and is the position of the right choice,
from 1. A click, `Enter` or `Space` answers, the right choice is marked and the result says
`Correct.` or `Not quite.`. A reader answers once, then `Try again` clears it.

The blocks after the list are hidden until the reader has answered, which suits an explanation. The
answer is remembered.

## Carousel

```
<!-- site: widget carousel id=screens show="1 2 3" loop autoplay=4500 zoom -->
<!-- site: figure name=first alt="The first screen" caption="The first screen" -->

---

<!-- site: figure name=second alt="The second screen" caption="The second screen" -->
<!-- site: end -->
```

A thematic break, 3 hyphens on a line, cuts the slides. The slides are a row that the page scrolls,
so touch, the arrow keys and the wheel work with no script, and the script adds the buttons, the
dots and the options.

| Option     | Value                                | Effect                                                                   |
| ---------- | ------------------------------------ | ------------------------------------------------------------------------ |
| `show`     | 1 to 3 counts, each from 1 to 6      | the slides in view on a narrow page, from 640 pixels and from 960 pixels |
| `loop`     | a flag                               | the row never ends                                                       |
| `autoplay` | a delay in milliseconds, 500 or more | scrolls by itself, with a button to pause                                |
| `zoom`     | a flag                               | a click on a picture shows it larger in a dialog                         |

A `show` with fewer than 3 counts repeats its last count for the wider pages. `show="1 2 3"` shows 1
slide on a phone, 2 on a tablet and 3 on a wide screen.

The automatic scroll stops when the pointer or the focus is in the carousel, when the tab is hidden
and for good once the reader acts on the carousel. It does not start for a reader whose system asks
for less motion.

A picture that comes from a `figure` annotation and lies after the slides that the widest width
shows is loaded when the reader gets near it.

## Table

A Markdown table, which gets a filter and headings that sort.

| Option    | Value                | Effect                                                         |
| --------- | -------------------- | -------------------------------------------------------------- |
| `src`     | a path               | a file of values, JSON or YAML that gives the table            |
| `columns` | headings, with commas | the columns to keep, in that order, with `src`                |

With `src`, the site builds the table from a file written relative to the document, and it takes the place of the blocks the widget holds.
The extension of the file gives its format.

| Extension         | Format                     | Headings                                  |
| ----------------- | -------------------------- | ----------------------------------------- |
| `.csv`            | values separated by commas | the first row                             |
| `.json`           | JSON                       | the keys of the objects, or the first row |
| `.yaml` or `.yml` | YAML                       | the keys of the objects, or the first row |
| any other         | values separated by tabs   | the first row                             |

A JSON or a YAML file holds a list of objects or a list of rows.
The keys of the objects give the headings, in the order they first appear.
A key that an object lacks gives an empty cell.
In a list of rows, the first row gives the headings.
A file whose top is an object stands for the only list that the object holds, and an object with several lists is refused.
A text, a number or a boolean gives its cell, and null gives an empty cell.
A cell that holds a list or an object stops the build, and the error names the item and the key that hold it.

The file is the one a test of the project writes, so that the table follows the code.
A missing file, a missing column and a file that does not read stop the build, and an error in the file gives its line and its column.

The filter keeps the rows that hold every word typed, whatever the case. The count says `n of N`,
and the filter is written in the address as `?filter-ID=words`, so a link can open a filtered table.
A click on a heading sorts by its column, a second click sorts the other way. The order is the one
of numbers inside the text, so `9` comes before `10` and `1.2` before `1.10`.

## Compare

```
<!-- site: widget compare id=restyle -->
![Before](before.png)

![After](after.png)
<!-- site: end -->
```

The first 2 pictures are laid one over the other with a slider. The first is shown on the left of
the handle and the second on its right. The name of a side is the caption of its figure, or else the
alternative text of its picture. The slider is a range, so the arrow keys move it.

Give the pictures the same size and the same proportions.

## Diff

A block of code of the language `diff`. The added lines, the removed lines, the headers of the hunks
and the lines of the files are told apart by their first characters, and drawn with their colours.

2 views are offered, `Unified` and `Side by side`. In the second, a run of removed lines meets the
run of added lines that follows it. The choice of the reader is kept for all the diffs.

## Chart

```
<!-- site: widget chart type=line id=speed -->
| Pages | Before | After |
| ----- | ------ | ----- |
| 10    | 0.8    | 0.1   |
<!-- site: end -->
```

| Option | Value           | Effect                                                         |
| ------ | --------------- | -------------------------------------------------------------- |
| `type` | `bar` or `line` | bars side by side, or a line for each series, `bar` by default |

The first column names the points and each other column is a series, named by its heading. In a
cell, what is not a digit, a point or a minus sign is left out, so `12%` and `3.5 s` are numbers,
and a cell with no number counts as 0.

The axis starts at 0 and ends at a round figure above the highest value, so the chart suits values
from 0 up. The legend names the series, a bar or a point shows its value when the pointer rests on
it, and a button goes back to the table.

## Flashcards

```
<!-- site: widget flashcards id=terms -->
| Term | Meaning |
| ---- | ------- |
| Widget | blocks between 2 comments |
<!-- site: end -->
```

A table of 2 columns, the front and the back. The row of headings is not a card, and the formatting
of a cell is kept.

A click, `Enter` or `Space` turns the card. `I know it` sets a card aside, and `Again` puts it at
the end of the pile. A bar counts the cards known, and `Start again` forgets them. What is known is
remembered.

## Annotated

```
<!-- site: widget annotated pins="50,9 16,50 62,37" id=anatomy -->
![A page](page.png)

1. The header.
2. The menu.
3. The text.
<!-- site: end -->
```

| Option | Value                       | Effect                                                                                  |
| ------ | --------------------------- | --------------------------------------------------------------------------------------- |
| `pins` | up to 50 positions as `x,y` | where each pin stands on the picture, in percent of its width and height, from 0 to 100 |

The first picture gets a numbered pin for each position, and the first list explains them in the
same order. There has to be as many positions as items, or the widget stays plain.

Pointing at a pin or at an item lights both, and the focus does the same. A click on a pin takes the
reader to its item.

## Builder

~~~
<!-- site: widget builder id=command -->
| Field | Label   | Values              |
| ----- | ------- | ------------------- |
| model | Model   | small, medium, large |
| root  | Project | .                   |

```
tool --model {model} --root {root}
```
<!-- site: end -->
~~~

The first table gives the fields, with 3 columns in this order: the name, the label and the values
separated by commas. The first code block gives the model, where `{name}` stands for the value of a
field. A name that no field has stays as written.

A field with several values is a choice that starts on the first one. A field with 1 value is a text
that starts as it, and one with none is an empty text. The code block that the reader sees follows
the form, with a button to copy it, and the form is remembered.

## Replay

```
<!-- site: widget replay speed=1.5 -->
$ dkt site dkt.toml -o public
dkt site: 19 pages written to public
<!-- site: end -->
```

| Option  | Value                   | Effect                                |
| ------- | ----------------------- | ------------------------------------- |
| `speed` | a number from 0.25 to 8 | the speed of the replay, 1 by default |
| `src`   | a path                  | a file that holds the session         |

The first code block is a terminal session. A line that starts with `$ ` is typed one character
after the other, and the other lines appear one after the other. The session starts when it comes
into view, and `Pause`, `Restart` and `Replay` are its buttons. A reader whose system asks for less
motion gets the whole session at once.

With `src`, the site builds the session from a file written relative to the document, and it takes the place of the code block the widget holds.
The file is plain text, or a recording of version 2 of asciinema.
A recording that keeps to the lines of a shell gives its output without the escape sequences and without its timing.
A file that cannot be read stops the build, and an error in a recording gives its line.

### A full screen session

A recording that takes the whole screen, as a program built on curses does, becomes a series of pictures of the screen.
A recording takes the whole screen when it places the cursor, clears the screen or shows the alternate screen.
Each picture is a step.
The events less than 50 milliseconds apart make 1 step, and a step that changes nothing on the screen is left out.

The steps follow each other at the pace of the recording, and `speed` sets how fast.
A pause is cut to the `idle_time_limit` of the recording, or to 2 seconds when the recording sets none.
`Pause`, `Restart` and `Replay` are the buttons, as for a session of lines.
Without the script, and for a reader whose system asks for less motion, the last picture stays on show.

A picture keeps the colours of the text and of its ground, bold, dim, italic, underlined and struck text, the wide characters and the lines of the line drawing set.
A new size of the screen in the recording gives the next pictures that size.
The pictures are drawn on the dark ground of a terminal, as an `ansi` block is.
Their 16 colours come from the variables `--a0` to `--a15`, with a fallback for each, so a theme that sets them sets both.

A row is drawn once, and the pictures that show it again use it, which keeps the page light.
A session holds at most 200 steps, on a screen of at most 300 columns by 100 rows.
A longer session or a larger screen stops the build, and the recording has to be cut.

## Since

```
<!-- site: widget since id=news -->
### 0.3.0

- A change.

### 0.2.0

- Another one.
<!-- site: end -->
```

The headings whose text starts with a version, with or without brackets or a `v`, cut the document
in sections. The shallowest level among them is the one that cuts. What comes before the first
version stays on show.

The reader tells the version they run, and only the sections of the versions that come after it
stay. The versions are compared as numbers, segment by segment. A count says how many versions
remain, and the choice is remembered. It suits a changelog.

## Progress

```
<!-- site: widget progress id=course -->
The progress of the reader appears here.
<!-- site: end -->
```

The widget shows what the reader has done in the whole site, page by page, with a bar for each page
and one for all. It counts the boxes of the checklists, the steps, the quizzes and the cards, which
are the widgets that a reader goes through.

The build writes the file `widgets.json` at the root of the site, with the address, the title and
the widgets of each page that holds some, as
`{"u": "/guides/a/", "t": "A", "w": [{"i": "start", "k": "checklist", "n": 3}]}`. The widget reads
it and looks in the browser for what each widget remembers. A build of a part of the site keeps
the entries of the pages it does not build, and writes again those of the pages it builds and
those under the address of a section it builds. A page that leaves the address of its section
keeps its old entry. A site with no such widget has no file.

Give an `id` to the widgets that the page counts, since a widget with none changes name when another
comes above it.

## Beyond widgets

Some features of a page need no widget, because the generator does them as it renders:

| Feature                                        | Where                                                                    |
| ---------------------------------------------- | ------------------------------------------------------------------------ |
| the copy button of the code blocks             | every code block, with `widgets.js`                                      |
| a Mermaid block as an SVG figure               | [dkt html](html.md#diagrams)                                             |
| an `ansi` block with the colours of a terminal | [dkt html](html.md#diagrams)                                             |
| a card for the page that a link leads to       | [dkt site](site.md#previews)                                             |
| links to the previous and the next page        | [dkt site](site.md#the-properties-file), the option `pager` of a section |
