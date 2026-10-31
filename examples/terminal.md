# In the terminal

`dkt cat` draws a document as styled text, with colours, table borders, alerts and diagrams. The
source is the one the site shows on its pages.

The source of the example:

~~~
# Release notes

A short document, read in the terminal.

> [!TIP]
> Pipe the output to a pager with `dkt cat -p`.

| Step    | State   |
| ------- | ------- |
| build   | done    |
| publish | pending |

- [x] Write the notes
- [ ] Tag the release

```rust
fn main() {
    println!("hello");
}
```

```mermaid
graph LR
    A[Write] --> B[Check]
    B --> C[Publish]
```
~~~

Run in a terminal that draws 24-bit colours, `COLORTERM=truecolor dkt cat --color always --width 72 --borders unicode examples/terminal/sample.md` writes this. The block holds the escape sequences of the terminal, and the site draws their colours. A terminal adds the links, and a pager keeps the colours with `less -R`.

```ansi
[0;2m# [0;1;4;38;2;85;85;255mRelease notes[0m

A short document, read in the terminal.

[0;38;2;85;255;85m│ [0;1;38;2;85;255;85mTIP[0m
[0;38;2;85;255;85m│[0m
[0;38;2;85;255;85m│ [0mPipe the output to a pager with [0;38;2;255;255;85mdkt cat -p[0m.

[0;38;2;85;85;85m┌─────────┬─────────┐[0m
[0;38;2;85;85;85m│ [0;1mStep[0m   [0;38;2;85;85;85m │ [0;1mState[0m  [0;38;2;85;85;85m │[0m
[0;38;2;85;85;85m├─────────┼─────────┤[0m
[0;38;2;85;85;85m│ [0mbuild  [0;38;2;85;85;85m │ [0mdone   [0;38;2;85;85;85m │[0m
[0;38;2;85;85;85m├─────────┼─────────┤[0m
[0;38;2;85;85;85m│ [0mpublish[0;38;2;85;85;85m │ [0mpending[0;38;2;85;85;85m │[0m
[0;38;2;85;85;85m└─────────┴─────────┘[0m

[0;38;2;85;85;255m• [0;38;2;85;255;85m[✓] [0mWrite the notes
[0;38;2;85;85;255m• [ ] [0mTag the release

  [0;38;2;180;142;173mfn[0;38;2;192;197;206m [0;38;2;143;161;179mmain[0;38;2;192;197;206m() {[0m
  [0;38;2;192;197;206m    println!("[0;38;2;163;190;140mhello[0;38;2;192;197;206m");[0m
  [0;38;2;192;197;206m}[0m

  [0;38;2;85;85;85m┌───────┐[0m     [0;38;2;85;85;85m┌───────┐[0m     [0;38;2;85;85;85m┌─────────┐[0m
  [0;38;2;85;85;85m│[0m Write [0;38;2;85;85;85m├────►│[0m Check [0;38;2;85;85;85m├────►│[0m Publish [0;38;2;85;85;85m│[0m
  [0;38;2;85;85;85m└───────┘[0m     [0;38;2;85;85;85m└───────┘[0m     [0;38;2;85;85;85m└─────────┘[0m
```

A Mermaid block is drawn with box characters, as above, for flowcharts, sequence diagrams, state
diagrams, class diagrams, entity-relationship diagrams and pie charts.

Other options of the command:

| Option                   | Effect                                            |
| ------------------------ | ------------------------------------------------- |
| `--soft-breaks preserve` | keep each line of the source                      |
| `--toc`                  | list the outline of the document with its anchors |
| `--section HEADING`      | draw one section                                  |
| `-p`                     | send the output to a pager                        |

See [dkt cat](../docs/cat.md) for all of them.
