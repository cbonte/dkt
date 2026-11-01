//! The index and the widgets file of a partial build of `dkt site`, against those of a full build.

use std::path::PathBuf;
use std::process::{Command, Output};

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let dir = std::env::temp_dir().join(format!("dkt-partial-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        TempDir(dir)
    }

    fn file(&self, name: &str, content: &str) {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("create dirs");
        std::fs::write(&path, content).expect("write file");
    }

    fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.0.join(name)).unwrap_or_else(|_| panic!("no file {name}"))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const CONFIG: &str = r#"
[site]
title = "S"
base_url = "https://example.com/s"

[[section]]
name = "a"

[[section]]
name = "b"

[[page]]
glob = "a/*.md"
section = "a"

[[page]]
glob = "b/*.md"
section = "b"
"#;

fn document(title: &str) -> String {
    format!(
        "# {title}\n\nText.\n\n## Part of {title}\n\n<!-- site: widget checklist id=c -->\n- [ ] one\n- [ ] two\n<!-- site: end -->\n"
    )
}

fn sources(dir: &TempDir) {
    dir.file("dkt.toml", CONFIG);
    for page in ["a/one.md", "a/two.md", "b/one.md", "b/two.md"] {
        dir.file(page, &document(page));
    }
}

fn build(dir: &TempDir, out: &str, extra: &[&str]) -> Output {
    let out = Command::new(env!("CARGO_BIN_EXE_dkt"))
        .current_dir(&dir.0)
        .args(["site", "dkt.toml", "-o", out])
        .args(extra)
        .output()
        .expect("run dkt site");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

#[test]
fn a_partial_build_writes_the_index_and_the_widgets_of_a_full_build() {
    let dir = TempDir::new("order");
    sources(&dir);
    build(&dir, "partial", &[]);
    dir.file("a/one.md", &document("a/one.md changed"));
    build(&dir, "partial", &["--only", "section:a"]);
    build(&dir, "full", &[]);
    for file in ["search.json", "widgets.json"] {
        assert_eq!(
            dir.read(&format!("partial/{file}")),
            dir.read(&format!("full/{file}")),
            "{file}"
        );
    }
    assert!(dir.read("partial/search.json").contains("a/one.md changed"));
}

#[test]
fn a_partial_build_drops_the_entries_of_a_page_the_site_no_longer_serves() {
    let dir = TempDir::new("drop");
    sources(&dir);
    build(&dir, "partial", &[]);
    std::fs::remove_file(dir.0.join("b/two.md")).expect("remove");
    build(&dir, "partial", &["--only", "section:a"]);
    build(&dir, "full", &[]);
    for file in ["search.json", "widgets.json"] {
        let partial = dir.read(&format!("partial/{file}"));
        assert!(!partial.contains("/s/b/two/"), "{file}: {partial}");
        assert_eq!(partial, dir.read(&format!("full/{file}")), "{file}");
    }
}
