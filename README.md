# dkt

Doc Kit (dk:t): document tools in one binary, sharing one parser and one model.

| Command                               | Purpose                                                  |
| ------------------------------------- | -------------------------------------------------------- |
| [dkt cat](docs/cat.md)                | render a document as styled text                         |


```
cargo build --release
cargo install --path .
```

Each command reads files or stdin and writes to stdout.

## Shell integration

```
dkt --completions bash > ~/.local/share/bash-completion/completions/dkt
dkt --completions zsh > ~/.local/share/zsh/site-functions/_dkt
dkt --man > dkt.1
```

The manual holds one page for `dkt` and one for each command.

## Why

Renderers reflow a paragraph the way CommonMark specifies, which shows every line break of a
hand-wrapped source as a break on screen. `cat` joins them by default and keeps them on request,
with continuation lines aligned under their list item or quote. `fmt` solves the same problem at
the source: it rewraps the file so the breaks land where you want them.

## Licence

Dual licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE), at your option.
