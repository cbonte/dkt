//! End-to-end tests of `dkt convert`.

use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run(args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_dkt"))
        .arg("convert")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn dkt convert");
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

#[test]
fn a_rst_input_comes_out_as_markdown_and_can_be_wrapped() {
    let rst = "Doc\n===\n\nA *word* and ``code``, and a long enough sentence to be wrapped at a narrow width.\n\nPart\n----\n\n* one\n* two\n";
    let out = run(&["--format", "rst"], rst);
    assert!(out.status.success());
    let md = stdout(&out);
    assert!(md.starts_with("# Doc\n"), "{md}");
    assert!(
        md.contains(
            "A *word* and `code`, and a long enough sentence to be wrapped at a narrow width.\n"
        ),
        "{md}"
    );
    assert!(md.contains("## Part"), "{md}");

    let wrapped = stdout(&run(&["--format", "rst", "--width", "30"], rst));
    assert!(
        wrapped.lines().all(|l| l.chars().count() <= 30),
        "{wrapped}"
    );
    assert!(wrapped.contains("- one\n- two"), "{wrapped}");
}

#[test]
fn the_extension_gives_the_format_and_an_unknown_one_is_an_error() {
    let dir = std::env::temp_dir().join(format!("dkt-convert-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let rst = dir.join("a.rst");
    std::fs::write(&rst, "Title\n=====\n\nbody\n").expect("write");
    let out = run(&[rst.to_str().expect("a path")], "");
    assert!(stdout(&out).contains("# Title"), "{}", stdout(&out));
    let txt = dir.join("a.txt");
    std::fs::write(&txt, "x\n").expect("write");
    let out = run(&[txt.to_str().expect("a path")], "");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("from its extension"));
    let _ = std::fs::remove_dir_all(&dir);
}
