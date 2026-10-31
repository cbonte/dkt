//! End-to-end tests of the binary.

use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run(args: &[&str], stdin: &str, env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_dkt"));
    cmd.arg("cat")
        .args(args)
        .env_remove("NO_COLOR")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().expect("spawn dkt cat");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

/// Runs `dkt` without a subcommand.
fn top(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_dkt"))
        .args(args)
        .output()
        .expect("spawn dkt")
}

fn stdout(o: &Output) -> String {
    String::from_utf8(o.stdout.clone()).unwrap()
}

#[test]
fn reads_stdin_and_honours_width() {
    let o = run(
        &["--width", "20", "--borders", "ascii"],
        "aaaa bbbb cccc dddd eeee ffff\n",
        &[],
    );
    assert!(o.status.success());
    assert_eq!(stdout(&o), "aaaa bbbb cccc dddd\neeee ffff\n");
}

#[test]
fn soft_breaks_flag() {
    let md = "one\ntwo\n";
    assert_eq!(stdout(&run(&["-w", "40"], md, &[])), "one two\n");
    assert_eq!(
        stdout(&run(&["-w", "40", "--soft-breaks", "preserve"], md, &[])),
        "one\ntwo\n"
    );
}

#[test]
fn piped_output_has_no_escapes() {
    let o = run(&["-w", "40"], "# Title\n\n**bold** [l](http://x)\n", &[]);
    assert!(!stdout(&o).contains('\x1b'));
    let o = run(
        &["-w", "40", "--color", "never", "--links", "osc8"],
        "[l](http://x)\n",
        &[],
    );
    assert!(!stdout(&o).contains('\x1b'));
}

#[test]
fn color_always_emits_escapes_unless_no_color_with_auto() {
    let o = run(&["-w", "40", "--color", "always"], "**bold**\n", &[]);
    assert!(stdout(&o).contains("\x1b[0;1m"));
    let o = run(&["-w", "40"], "**bold**\n", &[("NO_COLOR", "1")]);
    assert!(!stdout(&o).contains('\x1b'));
}

#[test]
fn missing_file_fails_with_message() {
    let o = run(&["/nonexistent/file.md"], "", &[]);
    assert!(!o.status.success());
    let err = String::from_utf8(o.stderr).unwrap();
    assert!(err.contains("cannot read"), "{err}");
}

#[test]
fn list_themes_includes_default() {
    let o = run(&["--list-themes"], "", &[]);
    assert!(o.status.success());
    assert!(
        stdout(&o)
            .lines()
            .any(|l| l == dkt::options::defaults::CODE_THEME)
    );
}

#[test]
fn several_files_get_headers_and_failures_do_not_stop_the_run() {
    let dir = std::env::temp_dir().join(format!("dkt-cat-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let a = dir.join("a.md");
    let b = dir.join("b.md");
    std::fs::write(&a, "# A\n").unwrap();
    std::fs::write(&b, "# B\n").unwrap();
    let missing = dir.join("missing.md");
    let args = [
        a.to_str().unwrap(),
        missing.to_str().unwrap(),
        b.to_str().unwrap(),
    ];
    let o = run(
        &["-w", "20", "--borders", "ascii"]
            .iter()
            .chain(args.iter())
            .copied()
            .collect::<Vec<_>>(),
        "",
        &[],
    );
    assert!(!o.status.success());
    let out = stdout(&o);
    let expected = format!(
        "{}\n{}\n# A\n\n\n{}\n{}\n# B\n",
        a.display(),
        "-".repeat(20),
        b.display(),
        "-".repeat(20)
    );
    assert_eq!(out, expected);
    assert!(String::from_utf8(o.stderr).unwrap().contains("missing.md"));
    let o = run(&["-w", "20", "--no-header", args[0], args[2]], "", &[]);
    assert_eq!(stdout(&o), "# A\n\n# B\n");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn completions_and_man_page() {
    let o = top(&["--completions", "bash"]);
    assert!(o.status.success());
    assert!(stdout(&o).contains("dkt"));
    let o = top(&["--man"]);
    assert!(o.status.success());
    let man = stdout(&o);
    assert!(man.contains("\n.TH dkt 1"), "{:?}", &man[..60]);
    assert!(man.contains("\n.TH dkt-cat 1"));
    assert!(man.contains("soft\\-breaks"));
}

#[test]
fn pager_flag_is_ignored_without_a_terminal() {
    let md = "# Title\n";
    let plain = stdout(&run(&["-w", "20"], md, &[]));
    let paged = stdout(&run(&["-w", "20", "--pager"], md, &[("PAGER", "false")]));
    assert_eq!(paged, plain);
}

#[test]
fn outline_and_section() {
    let md = "# Title\n\nintro\n\n## Usage\n\nuse it\n\n### Flags\n\nflags\n\n## Usage\n\nagain\n";
    let o = run(&["--toc", "--borders", "ascii", "-w", "40"], md, &[]);
    assert_eq!(stdout(&o), "- Title\n  - Usage\n    - Flags\n  - Usage\n");
    let o = run(
        &["--toc", "--toc-depth", "2", "--borders", "ascii"],
        md,
        &[],
    );
    assert_eq!(stdout(&o), "- Title\n  - Usage\n  - Usage\n");

    let o = run(&["--section", "Flags", "-w", "40"], md, &[]);
    assert_eq!(stdout(&o), "### Flags\n\nflags\n");
    // The second Usage is reachable by its disambiguated anchor.
    let o = run(&["--section", "usage-1", "-w", "40"], md, &[]);
    assert_eq!(stdout(&o), "## Usage\n\nagain\n");
    let o = run(&["--section", "nope", "-w", "40"], md, &[]);
    assert!(!o.status.success());
    assert!(
        String::from_utf8(o.stderr)
            .unwrap()
            .contains("no heading matching")
    );
}

#[test]
fn a_heading_that_is_a_link_is_found_by_its_text_and_shows_no_address() {
    let dir = std::env::temp_dir().join(format!("dkt-cat-heading-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let file = dir.join("a.md");
    std::fs::write(
        &file,
        "# T\n\n## [`node`](#node)\n\nBody of node.\n\n## Other\n\nx\n",
    )
    .expect("write");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_dkt"))
        .args(["cat", "--section", "node"])
        .arg(&file)
        .output()
        .expect("run dkt cat");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(text.contains("node") && !text.contains("(#node)"), "{text}");
    assert!(
        text.contains("Body of node.") && !text.contains("Other"),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_link_to_the_same_document_shows_its_text_and_not_its_address() {
    let dir = std::env::temp_dir().join(format!("dkt-cat-anchor-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let file = dir.join("a.md");
    std::fs::write(
        &file,
        "# T\n\nSee [the part](#part) and [the site](https://example.org/x).\n\n## Part\n",
    )
    .expect("write");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_dkt"))
        .args(["cat", "--links", "inline"])
        .arg(&file)
        .output()
        .expect("run dkt cat");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("the part") && !text.contains("(#part)"),
        "{text}"
    );
    assert!(text.contains("the site (https://example.org/x)"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_that_no_format_claims_is_written_as_it_is_with_a_warning() {
    let dir = std::env::temp_dir().join(format!("dkt-cat-raw-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let file = dir.join("notes.txt");
    std::fs::write(&file, "# Not rendered\n\n* as written\n").expect("write");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_dkt"))
        .arg("cat")
        .arg(&file)
        .output()
        .expect("run dkt cat");
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "# Not rendered\n\n* as written\n"
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("no known format") && err.contains("--format"),
        "{err}"
    );

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_dkt"))
        .args(["cat", "--format", "markdown", "--borders", "unicode"])
        .arg(&file)
        .output()
        .expect("run dkt cat");
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("• as written"));
    let _ = std::fs::remove_dir_all(&dir);
}
