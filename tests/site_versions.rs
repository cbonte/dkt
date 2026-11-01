//! End-to-end tests of the versions of a documentation built by `dkt site`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let dir = std::env::temp_dir().join(format!("dkt-versions-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        TempDir(dir)
    }

    fn file(&self, name: &str, content: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().expect("a parent")).expect("create dirs");
        std::fs::write(&path, content).expect("write file");
        path
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn exists(&self, name: &str) -> bool {
        self.0.join(name).exists()
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

/// Every file under `dir` with its bytes.
fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(dir: &Path, rel: &str, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).expect("read dir") {
            let entry = entry.expect("entry");
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = format!("{rel}{name}");
            if entry.file_type().expect("type").is_dir() {
                walk(&entry.path(), &format!("{path}/"), out);
            } else {
                out.insert(path, std::fs::read(entry.path()).expect("read"));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, "", &mut out);
    out
}

const SOURCE: &str = r#"
[site]
title = "Product"
base_url = "https://example.com/docs"

[home]
template = "index.html"

[[nav]]
name = "Guide"
url = "/guide/"
section = "guide"

[[section]]
name = "guide"
title = "Guide"
sidebar = true

[[page]]
glob = "guide/*.md"
section = "guide"
"#;

/// Writes the sources of a version with the given pages of the guide, each with a heading.
fn sources(dir: &TempDir, at: &str, pages: &[&str]) {
    dir.file(&format!("{at}/dkt.toml"), SOURCE);
    for page in pages {
        dir.file(
            &format!("{at}/guide/{page}.md"),
            &format!("# Page {page}\n\nText of {page} in {at}.\n\n## Install\n\n### Deep {page}\n"),
        );
    }
}

fn dkt(args: &[&str], dir: &TempDir) -> Output {
    Command::new(env!("CARGO_BIN_EXE_dkt"))
        .current_dir(&dir.0)
        .args(args)
        .output()
        .expect("run dkt")
}

fn ok(out: Output) -> Output {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn build_version(dir: &TempDir, at: &str, name: &str) -> Output {
    dkt(
        &[
            "site",
            &format!("{at}/dkt.toml"),
            "-o",
            "out",
            "--docs-version",
            name,
        ],
        dir,
    )
}

fn declare(dir: &TempDir, versions: &str) {
    dir.file("main/dkt.toml", &format!("{SOURCE}\n{versions}"));
}

fn refresh(dir: &TempDir) -> Output {
    dkt(
        &["site", "main/dkt.toml", "-o", "out", "--only", "versions"],
        dir,
    )
}

const THREE: &str = r#"
[[version]]
name = "2.0"
title = "2.0-dev"
status = "dev"

[[version]]
name = "1.1"

[[version]]
name = "1.0"
status = "deprecated"
"#;

/// 3 versions built and declared, with the root refreshed.
fn three(dir: &TempDir) {
    sources(dir, "v1.0", &["a", "old"]);
    sources(dir, "v1.1", &["a", "b"]);
    sources(dir, "v2.0", &["a", "b", "new"]);
    for (at, name) in [("v1.0", "1.0"), ("v1.1", "1.1"), ("v2.0", "2.0")] {
        ok(build_version(dir, at, name));
    }
    declare(dir, THREE);
    ok(refresh(dir));
}

#[test]
fn a_version_is_built_under_its_name_with_its_base_and_leaves_its_siblings() {
    let dir = TempDir::new("build");
    sources(&dir, "v1.0", &["a"]);
    sources(&dir, "v1.1", &["a", "b"]);
    let out = ok(build_version(&dir, "v1.0", "1.0"));
    assert!(
        stderr(&out).contains("pages written to out/1.0"),
        "{}",
        stderr(&out)
    );
    let first = tree(&dir.path("out/1.0"));
    ok(build_version(&dir, "v1.1", "1.1"));
    assert_eq!(tree(&dir.path("out/1.0")), first);

    let page = dir.read("out/1.1/guide/b/index.html");
    assert!(page.contains("href=\"/docs/1.1/guide/a/\""), "{page}");
    assert!(page.contains("href=\"/docs/1.1/docs.css\""), "{page}");
    assert!(page.contains("<meta name=\"dkt-versions\" content=\"/docs/versions.json\" data-root=\"/docs\" data-current=\"1.1\">"));
    assert!(page.contains("<script src=\"/docs/1.1/versions.js\" defer></script>"));
    assert!(dir.exists("out/1.1/versions.js"));
    assert!(dir.exists("out/1.1/versions.css"));
    assert!(
        dir.read("out/1.1/search.json")
            .contains("\"u\":\"/docs/1.1/guide/b/\"")
    );
    // A version build never writes the root.
    for root in ["versions.json", "index.html", "latest", ".dkt-site"] {
        assert!(!dir.exists(&format!("out/{root}")), "{root}");
    }
    let leftovers: Vec<_> = std::fs::read_dir(dir.path("out"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with('.'))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn a_bad_version_name_is_refused() {
    let dir = TempDir::new("names");
    sources(&dir, "v", &["a"]);
    for bad in ["", "..", ".x", "a/b", "latest", "versions.json", "a b"] {
        let out = build_version(&dir, "v", bad);
        assert!(!out.status.success(), "{bad}");
        assert!(stderr(&out).contains(bad), "{bad}: {}", stderr(&out));
    }
    assert!(!dir.exists("out"));
}

#[test]
fn a_version_and_a_part_of_a_site_do_not_go_together() {
    let dir = TempDir::new("together");
    sources(&dir, "v", &["a"]);
    let out = dkt(
        &[
            "site",
            "v/dkt.toml",
            "-o",
            "out",
            "--docs-version",
            "1.0",
            "--only",
            "home",
        ],
        &dir,
    );
    assert!(!out.status.success());
    assert!(stderr(&out).contains("built whole"), "{}", stderr(&out));
    let out = dkt(
        &[
            "site",
            "v/dkt.toml",
            "-o",
            "out",
            "--only",
            "versions",
            "--only",
            "home",
        ],
        &dir,
    );
    assert!(!out.status.success());
    assert!(stderr(&out).contains("no other part"), "{}", stderr(&out));
}

#[test]
fn the_root_lists_the_declared_versions_and_leads_to_the_latest_release() {
    let dir = TempDir::new("root");
    three(&dir);
    let list = dir.read("out/versions.json");
    assert_eq!(
        list,
        r#"{
  "format": 1,
  "latest": "1.1",
  "versions": [
    {
      "name": "2.0",
      "title": "2.0-dev",
      "status": "dev",
      "url": "/docs/2.0/"
    },
    {
      "name": "1.1",
      "title": "1.1",
      "status": "release",
      "url": "/docs/1.1/"
    },
    {
      "name": "1.0",
      "title": "1.0",
      "status": "deprecated",
      "url": "/docs/1.0/"
    }
  ]
}
"#
    );
    let index = dir.read("out/index.html");
    assert!(index.contains("<meta http-equiv=\"refresh\" content=\"0; url=/docs/1.1/\">"));
    assert!(index.contains("<li><a href=\"/docs/2.0/\">2.0-dev (development)</a></li>"));
    assert!(index.contains("<li><a href=\"/docs/1.1/\">1.1 (latest)</a></li>"));
    assert!(index.contains("<li><a href=\"/docs/1.0/\">1.0 (deprecated)</a></li>"));
    assert!(!dir.exists("out/.dkt-site"));

    // The alias holds 1 redirect page per page of the latest release.
    let stub = dir.read("out/latest/guide/b/index.html");
    assert!(stub.contains("<meta http-equiv=\"refresh\" content=\"0; url=/docs/1.1/guide/b/\">"));
    assert!(
        stub.contains("<link rel=\"canonical\" href=\"https://example.com/docs/1.1/guide/b/\">")
    );
    assert!(stub.contains("location.replace(\"/docs/1.1/guide/b/\"+location.hash)"));
    assert!(dir.exists("out/latest/index.html"));
    assert!(dir.exists("out/latest/guide/index.html"));
    assert!(!dir.exists("out/latest/guide/new"));

    // A second refresh gives the same bytes.
    let before = tree(&dir.path("out"));
    ok(refresh(&dir));
    assert_eq!(tree(&dir.path("out")), before);
}

#[test]
fn the_refresh_refuses_a_missing_or_an_undeclared_version_and_writes_nothing() {
    let dir = TempDir::new("refuse");
    three(&dir);
    let before = tree(&dir.path("out"));

    declare(&dir, &format!("{THREE}\n[[version]]\nname = \"0.9\"\n"));
    let out = refresh(&dir);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("--docs-version 0.9"),
        "{}",
        stderr(&out)
    );
    assert_eq!(tree(&dir.path("out")), before);

    declare(&dir, "[[version]]\nname = \"1.1\"\n");
    let out = refresh(&dir);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("do not declare: 1.0, 2.0"),
        "{}",
        stderr(&out)
    );
    assert_eq!(tree(&dir.path("out")), before);

    declare(&dir, "");
    let out = refresh(&dir);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("declare no [[version]]"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn a_site_without_versions_cannot_be_built_over_a_root_of_versions() {
    let dir = TempDir::new("protect");
    three(&dir);
    for extra in [&[][..], &["--only", "home"][..]] {
        let mut args = vec!["site", "v1.1/dkt.toml", "-o", "out"];
        args.extend_from_slice(extra);
        let out = dkt(&args, &dir);
        assert!(!out.status.success());
        assert!(
            stderr(&out).contains("holds the versions"),
            "{}",
            stderr(&out)
        );
    }
    assert!(dir.exists("out/1.1/guide/a/index.html"));
    assert!(dir.exists("out/versions.json"));

    // A version is not built into a site without versions.
    ok(dkt(&["site", "v1.1/dkt.toml", "-o", "plain"], &dir));
    let out = dkt(
        &[
            "site",
            "v1.1/dkt.toml",
            "-o",
            "plain",
            "--docs-version",
            "1.1",
        ],
        &dir,
    );
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("without versions"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn each_page_carries_a_selector_that_leads_to_the_same_page_when_the_version_has_it() {
    let dir = TempDir::new("selector");
    three(&dir);
    // 1.1 is built again now that the list is published and its siblings exist.
    ok(build_version(&dir, "v1.1", "1.1"));
    let page = dir.read("out/1.1/guide/b/index.html");
    assert!(
        page.contains("<details class=\"dkt-versions\"><summary>Version <b>1.1</b></summary><ul aria-label=\"Versions\">"),
        "{page}"
    );
    assert!(page.contains(
        "<li><a href=\"/docs/2.0/guide/b/\" data-version=\"2.0\">2.0-dev (development)</a></li>"
    ));
    assert!(page.contains("<li><a href=\"/docs/1.1/guide/b/\" data-version=\"1.1\" aria-current=\"true\">1.1 (latest)</a></li>"));
    // 1.0 has no page b, so its link leads to its home.
    assert!(
        page.contains("<li><a href=\"/docs/1.0/\" data-version=\"1.0\">1.0 (deprecated)</a></li>")
    );
    assert!(page.contains("<div class=\"dkt-version-note\" role=\"note\" hidden></div>"));

    // A deprecated version carries its warning with a link to the same page in the latest release.
    ok(build_version(&dir, "v1.0", "1.0"));
    let old = dir.read("out/1.0/guide/a/index.html");
    assert!(
        old.contains("<div class=\"dkt-version-note\" role=\"note\">This is the documentation of 1.0, a deprecated version. <a href=\"/docs/1.1/guide/a/\" data-version=\"1.1\">Read it in 1.1</a>.</div>"),
        "{old}"
    );
    ok(build_version(&dir, "v2.0", "2.0"));
    let dev = dir.read("out/2.0/guide/new/index.html");
    assert!(dev.contains("a development version that is not released. <a href=\"/docs/1.1/\" data-version=\"1.1\">"), "{dev}");
}

#[test]
fn every_built_in_theme_shows_the_selector_and_the_warning() {
    let dir = TempDir::new("themes");
    for theme in ["docs", "paper", "plain"] {
        let at = format!("src-{theme}");
        sources(&dir, &at, &["a"]);
        dir.file(
            &format!("{at}/dkt.toml"),
            &SOURCE.replace("[site]\n", &format!("[site]\ntheme = \"{theme}\"\n")),
        );
        let out = format!("out-{theme}");
        ok(dkt(
            &[
                "site",
                &format!("{at}/dkt.toml"),
                "-o",
                &out,
                "--docs-version",
                "1.0",
            ],
            &dir,
        ));
        for page in ["index.html", "guide/index.html", "guide/a/index.html"] {
            let html = dir.read(&format!("{out}/1.0/{page}"));
            assert_eq!(
                html.matches("<details class=\"dkt-versions\">").count(),
                1,
                "{theme} {page}"
            );
            assert_eq!(
                html.matches("class=\"dkt-version-note\"").count(),
                1,
                "{theme} {page}"
            );
            assert_eq!(html.matches("versions.css").count(), 1, "{theme} {page}");
        }
    }
}

#[test]
fn a_theme_of_its_own_gets_the_meta_and_the_files_and_reads_the_versions() {
    let dir = TempDir::new("own-theme");
    sources(&dir, "v", &["a"]);
    dir.file(
        "v/dkt.toml",
        &SOURCE.replace("[site]\n", "[site]\ntheme = \"mine\"\n"),
    );
    dir.file(
        "v/mine/templates/page.html",
        "<html><head><title>{{ page.title }}</title></head><body>{% if site.version %}<p class=\"v\">{{ site.version.name }} {{ site.version.status }} {{ site.version.root }}{% for v in versions %} {{ v.url }}{% endfor %}</p>{% endif %}{{ page.content }}</body></html>",
    );
    dir.file(
        "v/mine/templates/section.html",
        "<html><head></head><body></body></html>",
    );
    dir.file(
        "v/mine/templates/index.html",
        "<html><head></head><body></body></html>",
    );
    dir.file("v/mine/static/versions.css", "/* mine */\n");
    ok(build_version(&dir, "v", "3"));
    // The style of the theme replaces the one of the selector.
    assert_eq!(dir.read("out/3/versions.css"), "/* mine */\n");
    let page = dir.read("out/3/guide/a/index.html");
    assert!(
        page.contains("<p class=\"v\">3 release /docs /docs/3/guide/a/</p>"),
        "{page}"
    );
    assert!(page.contains("<meta name=\"dkt-versions\""));
    assert!(page.contains("/docs/3/versions.css"));
    assert!(page.contains("/docs/3/versions.js"));
    assert!(page.contains("/docs/3/widgets.js"));
}

#[test]
fn a_site_without_versions_carries_nothing_of_them() {
    let dir = TempDir::new("none");
    for theme in ["docs", "paper", "plain"] {
        let at = format!("src-{theme}");
        sources(&dir, &at, &["a"]);
        dir.file(
            &format!("{at}/dkt.toml"),
            &SOURCE.replace("[site]\n", &format!("[site]\ntheme = \"{theme}\"\n")),
        );
        let out = format!("out-{theme}");
        ok(dkt(&["site", &format!("{at}/dkt.toml"), "-o", &out], &dir));
        for (name, bytes) in tree(&dir.path(&out)) {
            let text = String::from_utf8_lossy(&bytes);
            assert!(!name.starts_with("version"), "{theme} {name}");
            assert!(!text.contains("dkt-version"), "{theme} {name}");
        }
        assert_eq!(dir.read(&format!("{out}/.dkt-site")), "");
    }
}

#[test]
fn the_pages_of_a_version_list_the_anchors_of_their_headings() {
    let dir = TempDir::new("anchors");
    sources(&dir, "v", &["a"]);
    ok(build_version(&dir, "v", "1.0"));
    assert_eq!(
        dir.read("out/1.0/version.json"),
        r#"{"format":1,"name":"1.0","pages":{"":[],"guide/":[],"guide/a/":["page-a","install","deep-a"]}}"#
    );
}

#[test]
fn a_new_build_removes_what_the_last_one_wrote_and_keeps_the_files_dkt_did_not_write() {
    let dir = TempDir::new("manifest");
    sources(&dir, "v0.3", &["a", "b"]);
    sources(&dir, "v0.2", &["a"]);
    ok(build_version(&dir, "v0.3", "0.3"));
    let manifest = dir.read("out/0.3/.dkt-site");
    assert!(manifest.starts_with(&format!("dkt {}\nversion 0.3\n", env!("CARGO_PKG_VERSION"))));
    assert!(
        manifest.contains("\nfile guide/b/index.html\n"),
        "{manifest}"
    );
    assert!(!manifest.contains(".dkt-site"));
    dir.file("out/0.3/CNAME", "docs.example.com\n");
    dir.file("out/0.3/extra/notes.txt", "kept\n");
    ok(build_version(&dir, "v0.2", "0.2"));
    let sibling = tree(&dir.path("out/0.2"));

    std::fs::remove_file(dir.path("v0.3/guide/b.md")).unwrap();
    ok(build_version(&dir, "v0.3", "0.3"));
    assert!(!dir.exists("out/0.3/guide/b"));
    assert!(dir.exists("out/0.3/guide/a/index.html"));
    assert_eq!(dir.read("out/0.3/CNAME"), "docs.example.com\n");
    assert_eq!(dir.read("out/0.3/extra/notes.txt"), "kept\n");
    assert_eq!(tree(&dir.path("out/0.2")), sibling);
    let manifest = dir.read("out/0.3/.dkt-site");
    assert!(!manifest.contains("guide/b/"), "{manifest}");
    assert!(!manifest.contains("CNAME"), "{manifest}");

    // A file that dkt did not write and that the new build writes is replaced, with a warning.
    dir.file("out/0.3/guide/b/index.html", "mine\n");
    std::fs::write(dir.path("v0.3/guide/b.md"), "# Page b\n").unwrap();
    let out = ok(build_version(&dir, "v0.3", "0.3"));
    assert!(
        stderr(&out).contains("was not written by dkt"),
        "{}",
        stderr(&out)
    );
    assert_ne!(dir.read("out/0.3/guide/b/index.html"), "mine\n");
}

#[test]
fn a_build_that_fails_leaves_the_version_as_it_was() {
    let dir = TempDir::new("fail");
    sources(&dir, "v", &["a"]);
    ok(build_version(&dir, "v", "1.0"));
    let before = tree(&dir.path("out"));
    dir.file("v/templates/page.html", "{% if %}");
    let out = build_version(&dir, "v", "1.0");
    assert!(!out.status.success());
    assert_eq!(tree(&dir.path("out")), before);

    // A sibling left by a run that was killed is cleared by the next build.
    std::fs::remove_file(dir.path("v/templates/page.html")).unwrap();
    dir.file("out/.1.0.dkt-new/stale.html", "x");
    dir.file("out/.1.0.dkt-old/stale.html", "x");
    ok(build_version(&dir, "v", "1.0"));
    assert!(!dir.exists("out/.1.0.dkt-new"));
    assert!(!dir.exists("out/.1.0.dkt-old"));
    assert!(!dir.exists("out/1.0/stale.html"));
}

#[test]
fn a_version_built_by_another_dkt_is_built_again_with_a_warning() {
    let dir = TempDir::new("other-dkt");
    sources(&dir, "v", &["a"]);
    ok(build_version(&dir, "v", "1.0"));
    dir.file(
        "out/1.0/.dkt-site",
        "dkt 0.0.1\nversion 1.0\nfile index.html\n",
    );
    let out = ok(build_version(&dir, "v", "1.0"));
    assert!(
        stderr(&out).contains(&format!(
            "was built by dkt 0.0.1, and dkt {} builds it now",
            env!("CARGO_PKG_VERSION")
        )),
        "{}",
        stderr(&out)
    );
    // An empty marker of an earlier dkt owns every file of the directory.
    dir.file("out/1.0/.dkt-site", "");
    dir.file("out/1.0/stale/index.html", "old page");
    let out = ok(build_version(&dir, "v", "1.0"));
    assert!(
        stderr(&out).contains("does not record its version"),
        "{}",
        stderr(&out)
    );
    assert!(!dir.exists("out/1.0/stale"));
}

#[test]
fn a_version_that_the_list_does_not_name_yet_is_built_with_a_warning() {
    let dir = TempDir::new("unlisted");
    three(&dir);
    sources(&dir, "v3", &["a"]);
    let out = ok(build_version(&dir, "v3", "3.0"));
    assert!(
        stderr(&out).contains("does not list the version 3.0"),
        "{}",
        stderr(&out)
    );
    let page = dir.read("out/3.0/guide/a/index.html");
    assert!(
        page.contains("data-version=\"3.0\" aria-current=\"true\">3.0</a>"),
        "{page}"
    );
    // It is undeclared until the default branch declares it.
    let out = refresh(&dir);
    assert!(!out.status.success());
    declare(&dir, &format!("[[version]]\nname = \"3.0\"\n{THREE}"));
    ok(refresh(&dir));
    assert!(
        dir.read("out/versions.json")
            .contains("\"latest\": \"3.0\"")
    );
    assert!(
        dir.read("out/latest/guide/a/index.html")
            .contains("/docs/3.0/guide/a/")
    );
    // The stubs of a page that the new latest release lacks are gone.
    assert!(!dir.exists("out/latest/guide/b"));
}

#[test]
fn a_refresh_in_any_order_gives_the_same_root() {
    let dir = TempDir::new("order");
    three(&dir);
    let first = tree(&dir.path("out"));
    // Rebuilding 1.0 then 2.0, or 2.0 then 1.0, ends with the same root after a refresh.
    ok(build_version(&dir, "v2.0", "2.0"));
    ok(build_version(&dir, "v1.0", "1.0"));
    ok(refresh(&dir));
    let a = tree(&dir.path("out"));
    ok(build_version(&dir, "v1.0", "1.0"));
    ok(build_version(&dir, "v2.0", "2.0"));
    ok(refresh(&dir));
    let b = tree(&dir.path("out"));
    for root in ["versions.json", "index.html"] {
        assert_eq!(a[root], first[root], "{root}");
        assert_eq!(a[root], b[root], "{root}");
    }
    let stubs = |t: &BTreeMap<String, Vec<u8>>| -> Vec<(String, Vec<u8>)> {
        t.iter()
            .filter(|(k, _)| k.starts_with("latest/"))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    };
    assert_eq!(stubs(&a), stubs(&b));
}

#[test]
fn the_choice_of_a_version_keeps_the_page_and_the_anchor_when_it_can() {
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("node is not installed: the script of the versions is not checked");
        return;
    }
    let out = Command::new("node")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/js/versions_core.js"
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
fn the_example_of_the_versions_builds_and_its_root_leads_to_the_release() {
    let dir = TempDir::new("example");
    let examples = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/versions");
    let out = dir.path("out");
    let out = out.to_str().expect("a path");
    for name in ["1.0", "2.0"] {
        ok(dkt(
            &[
                "site",
                &format!("{examples}/{name}/dkt.toml"),
                "-o",
                out,
                "--docs-version",
                name,
            ],
            &dir,
        ));
    }
    ok(dkt(
        &[
            "site",
            &format!("{examples}/dkt.toml"),
            "-o",
            out,
            "--only",
            "versions",
        ],
        &dir,
    ));
    assert!(dir.read("out/index.html").contains("url=/sample/1.0/"));
    assert!(dir.exists("out/latest/guide/configure/index.html"));
    assert!(
        dir.read("out/2.0/version.json")
            .contains("\"guide/upgrade/\":[\"upgrade\",\"legacy-options\"]")
    );
}
