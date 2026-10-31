# Development

## Build and test

```
cargo build --release
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

`UPDATE_FIXTURES=1 cargo test` rewrites the expected outputs under `tests/fixtures` after an
intentional change.

## The library

The crate also holds the parser, the block model, the terminal probe, the ANSI writer, GitHub
anchors, and a ranged syntax tree that keeps the byte range of every construct for the commands
that rewrite or locate Markdown rather than render it. Each other format is a converter to
Markdown, in `src/rst` and in `src/haproxy`, chosen by `src/format.rs`.

## What the tests hold

The formatter has 3 properties that are tested over the whole fixture corpus: formatting twice gives
the same result, the parser event stream is unchanged apart from line breaks, and no line exceeds
the width unless one unbreakable run does.

The conversion of the documentation of HAProxy is tested on a clone of its repository, when
`HAPROXY_REPO` names one. The test reads the 3 documents of every version from 1.6. It checks that
no more than 3% of the groups of 5 words of a document are lost, that every numbered chapter has its
heading, and that no heading repeats.

```
HAPROXY_REPO=../haproxy cargo test --test haproxy_docs
```
