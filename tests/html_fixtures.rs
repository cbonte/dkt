//! Golden tests: every `tests/fixtures/html/*.md` rendered as a page body.
//! Run with `UPDATE_FIXTURES=1` to rewrite the expected files.

use std::fs;
use std::path::{Path, PathBuf};

use dkt::page;

fn fixtures() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/html");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("fixtures dir")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .collect();
    files.sort();
    assert!(!files.is_empty());
    files
}

#[test]
fn golden_bodies() {
    let updating = std::env::var_os("UPDATE_FIXTURES").is_some();
    for md in fixtures() {
        let src = fs::read_to_string(&md).expect("read fixture");
        let out = page::render(&src, None).content;
        let expected_path = md.with_extension("html");
        if updating {
            fs::write(&expected_path, &out).expect("write expectation");
            continue;
        }
        let expected = fs::read_to_string(&expected_path).unwrap_or_else(|_| {
            panic!(
                "missing {}, run with UPDATE_FIXTURES=1",
                expected_path.display()
            )
        });
        assert_eq!(out, expected, "{}", md.display());
    }
}

#[test]
fn title_description_and_headings() {
    let src = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/html/blocks.md"),
    )
    .expect("read fixture");
    let page = page::render(&src, None);
    assert_eq!(page.title.as_deref(), Some("The page title"));
    assert_eq!(
        page.description.as_deref(),
        Some("The paragraph that directly follows the title is its description.")
    );
    let ids: Vec<&str> = page.headings.iter().map(|h| h.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "the-page-title",
            "inline",
            "lists",
            "tables",
            "quotes-and-alerts",
            "code",
            "raw-html",
            "notes",
            "duplicates",
            "same",
            "same-1",
            "same-2"
        ]
    );
}

#[test]
fn a_rewrite_sees_links_and_images() {
    let rewrite = |dest: &str| dest.strip_suffix(".md").map(|d| format!("/{d}/"));
    let page = page::render("[a](x.md) ![b](y.png) [c](z.html)", Some(&rewrite));
    assert!(page.content.contains(r#"<a href="/x/">a</a>"#));
    assert!(page.content.contains(r#"src="y.png""#));
    assert!(page.content.contains(r#"<a href="z.html">c</a>"#));
}

#[test]
fn a_page_annotation_wins_over_the_text() {
    let page = page::render(
        "# Long title\n\nThe text.\n\n<!-- site: page title=Short description=\"About it\" nav-title=S weight=3 hide -->\n",
        None,
    );
    assert_eq!(page.title.as_deref(), Some("Short"));
    assert_eq!(page.description.as_deref(), Some("About it"));
    assert_eq!(page.nav_title.as_deref(), Some("S"));
    assert_eq!(page.weight, Some(3));
    assert!(page.hidden);
    assert!(!page.content.contains("site:"), "{}", page.content);
}

#[test]
fn without_an_annotation_the_text_gives_the_title() {
    let page = page::render("# From the text\n\nIts lede.\n", None);
    assert_eq!(page.title.as_deref(), Some("From the text"));
    assert_eq!(page.description.as_deref(), Some("Its lede."));
    assert_eq!(page.weight, None);
    assert!(!page.hidden);
}

#[test]
fn a_replace_stands_in_for_the_blocks_up_to_its_end() {
    let page = page::render(
        "before\n\n<!-- site: replace src=a.png -->\n\n```\nold\n```\n\n<!-- site: end -->\n\nafter\n",
        None,
    );
    assert!(page.content.contains("<img src=\"a.png\" alt=\"\">"));
    assert!(!page.content.contains("old"));
    assert!(page.content.contains("<p>before</p>") && page.content.contains("<p>after</p>"));
}

#[test]
fn an_annotation_that_does_not_hold_is_left_as_a_comment() {
    let page = page::render("<!-- site: nope -->\n\ntext\n", None);
    assert!(page.content.contains("<!-- site: nope -->"));
}
