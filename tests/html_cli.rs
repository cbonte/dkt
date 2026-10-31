//! End-to-end tests of `dkt html`.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

fn run(args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_dkt"))
        .arg("html")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn dkt html");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(stdin.as_bytes())
        .expect("write stdin");
    child.wait_with_output().expect("wait")
}

fn stdout(o: &Output) -> String {
    String::from_utf8(o.stdout.clone()).expect("utf-8 output")
}

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let dir = std::env::temp_dir().join(format!("dkt-html-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        TempDir(dir)
    }

    fn file(&self, name: &str, content: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, content).expect("write file");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn writes_the_body_without_a_template() {
    let out = run(&["-"], "# Title\n\ntext\n");
    assert!(out.status.success());
    assert_eq!(stdout(&out), "<h1 id=\"title\">Title</h1>\n<p>text</p>\n");
}

#[test]
fn sets_the_page_in_a_template() {
    let dir = TempDir::new("template");
    let template = dir.file(
        "page.html",
        "<title>{{ page.title }} - {{ vars.site }}</title>\n{{ page.content }}",
    );
    let doc = dir.file("doc.md", "# A & B\n\nbody\n");
    let out = run(
        &[
            doc.to_str().unwrap(),
            "--template",
            template.to_str().unwrap(),
            "--set",
            "site=lokode",
        ],
        "",
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let html = stdout(&out);
    assert!(html.contains("<title>A &amp; B - lokode</title>"), "{html}");
    assert!(
        html.contains("<p>body</p>"),
        "the content is not escaped: {html}"
    );
}

#[test]
fn a_template_can_extend_another_next_to_it() {
    let dir = TempDir::new("extends");
    dir.file("base.html", "<main>{% block body %}{% endblock %}</main>");
    let template = dir.file(
        "page.html",
        "{% extends \"base.html\" %}{% block body %}{{ page.content }}{% endblock %}",
    );
    let out = run(&["--template", template.to_str().unwrap()], "# T\n");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(stdout(&out), "<main><h1 id=\"t\">T</h1>\n</main>");
}

#[test]
fn set_without_a_template_is_refused() {
    let out = run(&["--set", "a=b"], "# T\n");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--set needs --template"));
}

#[test]
fn a_pair_without_equals_is_refused() {
    let out = run(&["--set", "ab"], "# T\n");
    assert!(!out.status.success());
}

#[test]
fn a_diagram_block_is_drawn_and_other_code_is_left_alone() {
    let md = "```diagram\ncmd arg\n|---|\n  a _/\n```\n\n```text\n|---|\n```\n";
    let out = run(&[], md);
    assert!(out.status.success());
    let html = stdout(&out);
    assert_eq!(
        html.matches("<figure class=\"diagram\">").count(),
        1,
        "{html}"
    );
    assert!(html.contains("<svg"));
    assert!(html.contains("<desc>cmd arg\n|---|\n  a _/</desc>"));
    assert!(
        html.contains("<pre><code class=\"language-text\">|---|\n</code></pre>"),
        "{html}"
    );
}

#[test]
fn a_file_is_read_in_the_format_its_extension_gives_and_the_input_in_the_one_named() {
    let dir = TempDir::new("formats");
    let rst = dir.file(
        "doc.rst",
        "Doc\n===\n\nA *word* and ``code``.\n\nPart\n----\n\ntext\n",
    );
    let out = run(&[rst.to_str().expect("a path")], "");
    assert!(out.status.success());
    let html = stdout(&out);
    assert!(html.contains("<h1 id=\"doc\">"), "{html}");
    assert!(
        html.contains("<em>word</em>") && html.contains("<code>code</code>"),
        "{html}"
    );
    assert!(html.contains("<h2 id=\"part\">"), "{html}");

    let out = run(&["--format", "rst"], "Title\n=====\n\nbody\n");
    assert!(
        stdout(&out).contains("<h1 id=\"title\">"),
        "{}",
        stdout(&out)
    );

    let other = dir.file("doc.html", "# x\n");
    let out = run(&[other.to_str().expect("a path")], "");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("markdown, haproxy or rst"));
}

#[test]
fn a_text_file_needs_its_format_and_a_named_format_reads_it() {
    let dir = TempDir::new("txt");
    let file = dir.file("notes.txt", "# Notes\n\nSome text.\n");
    let path = file.to_str().expect("a path");
    let out = run(&[path], "");
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("from its extension"), "{err}");
    let out = run(&["--format", "markdown", path], "");
    assert!(out.status.success());
    assert!(
        stdout(&out).contains("<h1 id=\"notes\">"),
        "{}",
        stdout(&out)
    );
}
