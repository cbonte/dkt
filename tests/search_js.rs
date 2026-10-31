//! The script of the search palette: its matching is checked with node when it is installed, and
//! every theme carries the same copy of it.

use std::process::Command;

#[test]
fn the_matching_of_the_palette_holds() {
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("node is not installed: the matching of the palette is not checked");
        return;
    }
    let out = Command::new("node")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/js/search_core.js"
        ))
        .output()
        .expect("run node");
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn the_themes_carry_the_same_script() {
    let docs = include_str!("../themes/docs/static/search.js");
    assert_eq!(docs, include_str!("../themes/paper/static/search.js"));
    assert_eq!(docs, include_str!("../themes/plain/static/search.js"));
}
