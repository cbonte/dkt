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

`scripts/notice.py` rewrites `NOTICE` from the crates that the binary links.
`scripts/notice.py --check` exits 1 when `NOTICE` is stale, and runs in the CI.

## Continuous integration

The checks do not run on a push to a branch. They run in 3 cases:

| Case | What runs |
| --- | --- |
| a pull request | the tests on Linux and Windows, clippy and the notice |
| a tag `v*` | the same checks, the release archive and the site |
| a manual run | the checks, or the site, from the Actions tab |

## The library

The crate also holds the parser, the block model, the terminal probe, the ANSI writer, GitHub
anchors, and a ranged syntax tree that keeps the byte range of every construct for the commands that
rewrite or locate Markdown rather than render it. Each other format is a converter to Markdown, in
`src/rst` and in `src/haproxy`, chosen by `src/format.rs`.

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

## Rendering to HTML

`dkt::page::render` returns the page for a document, and takes an optional function that sees the
target of every link and image, to point a `.md` link at the page it becomes.
