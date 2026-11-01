//! End-to-end tests of `dkt site`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let dir = std::env::temp_dir().join(format!("dkt-site-{}-{name}", std::process::id()));
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

const CONFIG: &str = r#"
[site]
title = "T"
base_url = "https://example.com/t"

[[nav]]
name = "Tour"
url = "/tour/"

[home]
template = "index.html"

[[section]]
name = "guides"
title = "Guides"

[[page]]
src = "README.md"
url = "/tour/"

[[page]]
glob = "docs/guides/*.md"
section = "guides"
order = ["b", "a"]

[[asset]]
src = "docs/img"
url = "/img/"

[figures]
src = "/img/{name}-light.png"
dark = "/img/{name}-dark.png"

[data]
repo = "https://github.com/o/r"

[data.home]
lede = "Hello"
"#;

fn site(dir: &TempDir) {
    dir.file("dkt.toml", CONFIG);
    dir.file(
        "templates/page.html",
        "<title>{{ page.title }} | {{ site.title }}</title><nav>{% for n in site.nav %}<a href=\"{{ n.url }}\">{{ n.name }}</a>{% endfor %}</nav>{{ page.content }}",
    );
    dir.file(
        "templates/section.html",
        "<h1>{{ section.title }}</h1>{% for p in section.pages %}<a href=\"{{ p.url }}\">{{ p.nav_title }}</a>{% endfor %}",
    );
    dir.file("templates/index.html", "<h1>{{ data.home.lede }}</h1>");
    dir.file(
        "README.md",
        "# Tour\n\nSee [the guide](docs/guides/a.md#start), [the code](https://example.com/main.rs) and ![shot](docs/img/x.png).\n",
    );
    dir.file(
        "docs/guides/a.md",
        "# A\n\nlede\n\n## Start\n\n<!-- site: page nav-title=\"Alpha\" -->\n",
    );
    dir.file(
        "docs/guides/b.md",
        "# B\n\nlede of b\n\nBack to [a](a.md#start).\n\n<!-- site: figure name=shot caption=Shot -->\n",
    );
    dir.file(
        "docs/guides/secret.md",
        "# Secret\n\n<!-- site: page hide -->\n",
    );
    dir.file("docs/img/x.png", "png");
    dir.file("src/main.rs", "fn main() {}\n");
}

fn build(dir: &TempDir, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_dkt"))
        .arg("site")
        .arg(dir.path("dkt.toml"))
        .arg("-o")
        .arg(dir.path("public"))
        .args(extra)
        .output()
        .expect("run dkt site")
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

#[test]
fn builds_the_pages_with_their_links() {
    let dir = TempDir::new("pages");
    site(&dir);
    let out = build(&dir, &[]);
    assert!(out.status.success(), "{}", stderr(&out));

    let tour = dir.read("public/tour/index.html");
    assert!(tour.contains("<title>Tour | T</title>"), "{tour}");
    assert!(tour.contains(r#"<a href="/t/tour/">Tour</a>"#), "{tour}");
    assert!(tour.contains(r#"href="/t/guides/a/#start""#), "{tour}");
    assert!(
        tour.contains(r#"href="https://example.com/main.rs""#),
        "{tour}"
    );
    assert!(tour.contains(r#"src="/t/img/x.png""#), "{tour}");

    let b = dir.read("public/guides/b/index.html");
    assert!(b.contains(r#"href="/t/guides/a/#start""#), "{b}");
    assert!(b.contains(r#"srcset="/t/img/shot-dark.png""#), "{b}");
    assert!(b.contains(r#"src="/t/img/shot-light.png""#), "{b}");

    assert!(dir.read("public/index.html").contains("<h1>Hello</h1>"));
    assert!(dir.path("public/img/x.png").exists());
}

#[test]
fn a_section_lists_its_pages_in_order_with_their_menu_titles() {
    let dir = TempDir::new("section");
    site(&dir);
    assert!(build(&dir, &[]).status.success());
    let index = dir.read("public/guides/index.html");
    let b = index.find(">B<").expect("b is listed");
    let a = index
        .find(">Alpha<")
        .expect("a is listed by its menu title");
    assert!(b < a, "the order of the properties holds: {index}");
    assert!(!index.contains("Secret"), "{index}");
}

#[test]
fn a_hidden_page_is_not_written() {
    let dir = TempDir::new("hidden");
    site(&dir);
    assert!(build(&dir, &[]).status.success());
    assert!(!dir.path("public/guides/secret").exists());
}

#[test]
fn a_broken_link_stops_the_build_unless_the_check_is_off() {
    let dir = TempDir::new("lint");
    site(&dir);
    dir.file("docs/guides/b.md", "# B\n\n[nowhere](missing.md)\n");
    let out = build(&dir, &[]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("docs/guides/b.md:3:"),
        "{}",
        stderr(&out)
    );
    assert!(stderr(&out).contains("link-file"), "{}", stderr(&out));
    assert!(
        !dir.path("public").exists(),
        "nothing is written when the check fails"
    );
    assert!(build(&dir, &["--no-lint"]).status.success());
}

#[test]
fn a_directory_dkt_did_not_make_is_not_cleared() {
    let dir = TempDir::new("clear");
    site(&dir);
    dir.file("public/mine.txt", "keep");
    let out = build(&dir, &[]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("not cleared"), "{}", stderr(&out));
    assert!(Path::new(&dir.path("public/mine.txt")).exists());
}

#[test]
fn a_second_build_replaces_the_first() {
    let dir = TempDir::new("again");
    site(&dir);
    assert!(build(&dir, &[]).status.success());
    dir.file("public/stale.html", "old");
    assert!(build(&dir, &[]).status.success());
    assert!(!dir.path("public/stale.html").exists());
}

#[test]
fn the_base_address_can_be_replaced() {
    let dir = TempDir::new("base");
    site(&dir);
    assert!(
        build(&dir, &["--base-url", "http://localhost:8/preview"])
            .status
            .success()
    );
    let tour = dir.read("public/tour/index.html");
    assert!(
        tour.contains(r#"href="/preview/guides/a/#start""#),
        "{tour}"
    );
}

#[test]
fn a_theme_supplies_templates_and_files_and_the_site_wins() {
    let dir = TempDir::new("theme");
    dir.file(
        "dkt.toml",
        "[site]\ntitle = \"T\"\ntheme = \"shared\"\nstatic = \"own\"\n\n[home]\ntemplate = \"index.html\"\n\n[[page]]\nsrc = \"a.md\"\n",
    );
    dir.file("a.md", "# A\n\ntext\n");
    dir.file(
        "shared/templates/base.html",
        "<body>{% block main %}{% endblock %}</body>",
    );
    dir.file(
        "shared/templates/page.html",
        "{% extends \"base.html\" %}{% block main %}shared page{% endblock %}",
    );
    dir.file(
        "shared/templates/index.html",
        "{% extends \"base.html\" %}{% block main %}shared home{% endblock %}",
    );
    dir.file("shared/static/theme.css", "shared css");
    dir.file("shared/static/both.css", "from the theme");
    dir.file(
        "templates/page.html",
        "{% extends \"base.html\" %}{% block main %}own page: {{ page.content }}{% endblock %}",
    );
    dir.file("own/both.css", "from the site");
    let out = build(&dir, &[]);
    assert!(out.status.success(), "{}", stderr(&out));
    let widgets = "<link rel=\"stylesheet\" href=\"/content.css\">\n<link rel=\"stylesheet\" href=\"/widgets.css\">\n";
    let script = "<script src=\"/widgets.js\" defer></script>\n";
    assert_eq!(
        dir.read("public/a/index.html"),
        format!("{widgets}<body>own page: <h1 id=\"a\">A</h1>\n<p>text</p>\n{script}</body>")
    );
    assert_eq!(
        dir.read("public/index.html"),
        format!("{widgets}<body>shared home{script}</body>")
    );
    assert_eq!(dir.read("public/theme.css"), "shared css");
    assert_eq!(dir.read("public/both.css"), "from the site");
}

#[test]
fn a_built_in_theme_is_named_and_a_directory_of_that_name_wins() {
    let dir = TempDir::new("builtin");
    dir.file(
        "dkt.toml",
        "[site]\ntitle = \"T\"\ntheme = \"docs\"\n\n[home]\ntemplate = \"index.html\"\n\n[[page]]\nsrc = \"a.md\"\n",
    );
    dir.file("a.md", "# A\n\ntext\n");
    let out = build(&dir, &[]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(dir.read("public/a/index.html").contains("docs.css"));
    assert!(dir.read("public/index.html").contains("<h1>T</h1>"));
    assert!(dir.read("public/docs.css").contains("--bg"));
    assert!(dir.read("public/docs.js").contains("theme"));

    dir.file("docs/templates/page.html", "own docs page");
    dir.file("docs/templates/index.html", "own docs home");
    let out = build(&dir, &[]);
    assert!(out.status.success(), "{}", stderr(&out));
    let page = dir.read("public/a/index.html");
    assert!(page.contains("own docs page"), "{page}");
    assert!(!page.contains("docs.css"), "{page}");
}

#[test]
fn a_site_that_names_no_theme_gets_docs() {
    let dir = TempDir::new("default-theme");
    dir.file(
        "dkt.toml",
        "[site]\ntitle = \"T\"\n\n[[page]]\nsrc = \"a.md\"\n",
    );
    dir.file("a.md", "# A\n\ntext\n");
    let out = build(&dir, &[]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(dir.read("public/a/index.html").contains("docs.css"));
    assert!(dir.read("public/docs.css").contains("--bg"));
}

#[test]
fn every_built_in_theme_builds_a_site() {
    for theme in ["docs", "paper", "plain"] {
        let dir = TempDir::new(&format!("each-{theme}"));
        dir.file(
            "dkt.toml",
            &format!("[site]\ntitle = \"T\"\ntheme = \"{theme}\"\n\n[[nav]]\nname = \"Home\"\nurl = \"/\"\n\n[home]\ntemplate = \"index.html\"\n\n[[section]]\nname = \"g\"\ntitle = \"Guides\"\nsidebar = true\n\n[[page]]\nsrc = \"a.md\"\nsection = \"g\"\n"),
        );
        dir.file("a.md", "# A\n\ntext\n\n## Part\n\nmore\n");
        let out = build(&dir, &[]);
        assert!(out.status.success(), "{theme}: {}", stderr(&out));
        assert!(
            dir.read("public/g/a/index.html").contains("Part"),
            "{theme}"
        );
        assert!(dir.read("public/index.html").contains("Guides"), "{theme}");
        assert!(
            dir.read("public/g/index.html").contains("Guides"),
            "{theme}"
        );
        assert!(
            dir.read(&format!("public/{theme}.css")).len() > 100,
            "{theme}"
        );
        assert!(
            dir.read("public/g/a/index.html").contains("search.js"),
            "{theme}"
        );
        assert!(dir.exists("public/search.js"), "{theme}");
        assert!(dir.exists("public/search.json"), "{theme}");
    }
}

#[test]
fn a_directory_that_is_not_a_theme_does_not_hide_a_built_in_theme() {
    let dir = TempDir::new("docs-folder");
    dir.file(
        "dkt.toml",
        "[site]\ntitle = \"T\"\ntheme = \"docs\"\n\n[[page]]\nsrc = \"docs/a.md\"\n",
    );
    dir.file("docs/a.md", "# A\n\ntext\n");
    let out = build(&dir, &[]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(dir.read("public/a/index.html").contains("docs.css"));
}

#[test]
fn an_unknown_theme_stops_the_build_and_names_the_built_in_ones() {
    let dir = TempDir::new("unknown-theme");
    dir.file(
        "dkt.toml",
        "[site]\ntitle = \"T\"\ntheme = \"nothing\"\n\n[[page]]\nsrc = \"a.md\"\n",
    );
    dir.file("a.md", "# A\n\ntext\n");
    let out = build(&dir, &[]);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(
        err.contains("nothing")
            && err.contains("docs")
            && err.contains("paper")
            && err.contains("plain"),
        "{err}"
    );
}

#[test]
fn a_template_cannot_reach_outside_its_directories() {
    let dir = TempDir::new("outside");
    dir.file("secret.txt", "secret");
    dir.file(
        "dkt.toml",
        "[site]\ntitle = \"T\"\n\n[[page]]\nsrc = \"a.md\"\n",
    );
    dir.file("a.md", "# A\n");
    dir.file("templates/page.html", "{% include \"../secret.txt\" %}");
    let out = build(&dir, &[]);
    assert!(!out.status.success());
}

#[test]
fn a_site_gets_an_index_of_its_titles_unless_it_turns_search_off() {
    let dir = TempDir::new("search");
    dir.file(
        "dkt.toml",
        "[site]\ntitle = \"T\"\nbase_url = \"https://x.io/p\"\n\n[[page]]\nsrc = \"a.md\"\n\n[[page]]\nsrc = \"b.md\"\n",
    );
    dir.file(
        "a.md",
        "# A page\n\nAbout whales.\n\n## The sea\n\n<!-- site: alias words=\"ocean deep\" -->\ntext\n\n### Deep water\n\ntext\n",
    );
    dir.file(
        "b.md",
        "# Hidden from search\n\n<!-- site: page no-search -->\n\ntext\n",
    );
    let out = build(&dir, &[]);
    assert!(out.status.success(), "{}", stderr(&out));
    let index = dir.read("public/search.json");
    assert_eq!(
        index,
        concat!(
            "[{\"t\":\"A page\",\"p\":\"A page\",\"u\":\"/p/a/\",\"l\":1,\"d\":\"About whales.\"},",
            "{\"t\":\"The sea\",\"p\":\"A page\",\"u\":\"/p/a/#the-sea\",\"l\":2,\"a\":\"ocean deep\"},",
            "{\"t\":\"Deep water\",\"p\":\"A page\",\"u\":\"/p/a/#deep-water\",\"l\":3,\"b\":\"The sea\"}]"
        )
    );
    assert!(dir.read("public/a/index.html").contains("search.js"));
    assert!(dir.exists("public/search.js"));
    assert!(dir.exists("public/b/index.html"));
}

#[test]
fn a_site_that_turns_search_off_has_no_index() {
    let dir = TempDir::new("no-search");
    dir.file(
        "dkt.toml",
        "[site]\ntitle = \"T\"\nsearch = false\n\n[[page]]\nsrc = \"a.md\"\n",
    );
    dir.file("a.md", "# A\n\ntext\n");
    let out = build(&dir, &[]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(!dir.exists("public/search.json"));
    assert!(!dir.read("public/a/index.html").contains("search.js"));
}

#[test]
fn a_page_names_its_format_and_a_text_of_haproxy_is_converted() {
    let dir = TempDir::new("format");
    dir.file(
        "dkt.toml",
        r#"
[site]
title = "H"
base_url = "https://example.com/h"

[[nav]]
name = "Home"
url = "/"

[[section]]
name = "3.4"
title = "3.4"

[[page]]
src = "3.4/configuration.txt"
format = "haproxy"
section = "3.4"

[[page]]
src = "3.4/notes.md"
section = "3.4"
"#,
    );
    dir.file(
        "templates/page.html",
        "<title>{{ page.title }}</title>{{ page.content }}",
    );
    dir.file(
        "3.4/configuration.txt",
        "                          HAProxy\n                    Configuration Manual\n                       version 3.4\n\n\nSummary\n-------\n\n1.      First\n\n\n1. First\n--------\n\nSome text.\n",
    );
    dir.file("3.4/notes.md", "# Notes\n\nA note.\n");
    let out = build(&dir, &["--no-lint"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let page = dir.read("public/3.4/configuration/index.html");
    assert!(page.contains("Configuration Manual"), "{page}");
    assert!(page.contains("Some text."), "{page}");
    assert!(dir.read("public/3.4/notes/index.html").contains("A note."));
}

#[test]
fn a_file_of_an_unknown_format_is_refused_with_the_way_to_name_it() {
    let dir = TempDir::new("noformat");
    dir.file(
        "dkt.toml",
        "[site]\ntitle = \"H\"\nbase_url = \"https://example.com/h\"\n[[page]]\nsrc = \"a.xyz\"\n",
    );
    dir.file("a.xyz", "text\n");
    let out = build(&dir, &["--no-lint"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("format"), "{}", stderr(&out));
}

#[test]
fn the_home_page_shows_a_document_and_what_follows_its_first_paragraph() {
    let dir = TempDir::new("home");
    dir.file(
        "dkt.toml",
        "[site]\ntitle = \"H\"\nbase_url = \"https://example.com/h\"\n[home]\ntemplate = \"index.html\"\nsrc = \"home.md\"\n",
    );
    dir.file(
        "templates/index.html",
        "<title>{{ page.title }}</title><p class=\"d\">{{ page.description }}</p><div class=\"rest\">{{ page.rest }}</div>",
    );
    dir.file(
        "home.md",
        "# The home\n\nThe lede.\n\n## More\n\nSee [x](#more) and https://example.org/a.\n",
    );
    let out = build(&dir, &[]);
    assert!(out.status.success(), "{}", stderr(&out));
    let home = dir.read("public/index.html");
    assert!(home.contains("<title>The home</title>"), "{home}");
    assert!(home.contains("<p class=\"d\">The lede.</p>"), "{home}");
    assert!(home.contains("<h2 id=\"more\">"), "{home}");
    assert!(!home.contains("<h1"), "{home}");
}

#[test]
fn a_site_gives_its_colours_for_both_appearances_and_refuses_a_bad_one() {
    let dir = TempDir::new("colors");
    let config = |extra: &str| {
        format!(
            "[site]\ntitle = \"H\"\nbase_url = \"https://example.com/h\"\n[home]\ntemplate = \"index.html\"\n{extra}"
        )
    };
    dir.file(
        "templates/index.html",
        "{% extends \"base.html\" %}{% block main %}x{% endblock %}",
    );
    dir.file(
        "dkt.toml",
        &config("[colors.light]\naccent = \"#1d4ed8\"\n[colors.dark]\naccent = \"#7aa2ff\"\n"),
    );
    let out = build(&dir, &[]);
    assert!(out.status.success(), "{}", stderr(&out));
    let home = dir.read("public/index.html");
    // The links follow the accent when the site does not colour them.
    assert!(
        home.contains(":root { --accent: #1d4ed8; --link: #1d4ed8; }"),
        "{home}"
    );
    assert!(
        home.contains("[data-theme=\"dark\"] { --accent: #7aa2ff; --link: #7aa2ff; }"),
        "{home}"
    );

    dir.file(
        "dkt.toml",
        &config("[colors.light]\naccent = \"red; } body { display: none\"\n"),
    );
    let out = build(&dir, &["--no-lint"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("not a colour"), "{}", stderr(&out));
}

fn two_sections(dir: &TempDir) {
    dir.file(
        "dkt.toml",
        r#"
[site]
title = "S"
base_url = "https://example.com/s"

[home]
template = "index.html"
src = "home.md"

[[section]]
name = "a"
title = "A"

[[section]]
name = "b"
title = "B"

[[page]]
src = "a/doc.md"
section = "a"

[[page]]
src = "b/doc.md"
section = "b"
"#,
    );
    dir.file(
        "templates/page.html",
        "<title>{{ page.title }}</title>{{ page.content }}",
    );
    dir.file(
        "templates/section.html",
        "<h1>{{ section.title }}</h1>{% for p in section.pages %}<a href=\"{{ p.url }}\">{{ p.title }}</a>{% endfor %}",
    );
    dir.file(
        "templates/index.html",
        "<title>{{ page.title }}</title>{{ page.rest }}",
    );
    dir.file("home.md", "# Home\n\nlede\n\n## First text\n");
    dir.file("a/doc.md", "# Doc of a\n\nold text\n");
    dir.file("b/doc.md", "# Doc of b\n\ntext of b\n");
}

#[test]
fn a_partial_build_rebuilds_what_it_is_asked_and_leaves_the_rest() {
    let dir = TempDir::new("partial");
    two_sections(&dir);
    assert!(build(&dir, &[]).status.success());
    let b_before = dir.read("public/b/doc/index.html");

    // The documents of the section b are not there any more, and the build does not need them.
    std::fs::remove_file(dir.path("b/doc.md")).expect("remove");
    dir.file("a/doc.md", "# Doc of a\n\nnew text\n");
    dir.file("home.md", "# Home\n\nlede\n\n## Second text\n");
    let out = build(&dir, &["--only", "section:a", "--only", "home"]);
    assert!(out.status.success(), "{}", stderr(&out));

    assert!(dir.read("public/a/doc/index.html").contains("new text"));
    assert!(dir.read("public/index.html").contains("Second text"));
    assert_eq!(dir.read("public/b/doc/index.html"), b_before);
    let index = dir.read("public/search.json");
    assert!(index.contains("Doc of b"), "{index}");
    assert!(index.contains("Doc of a"), "{index}");
    assert_eq!(index.matches("\"u\":\"/s/a/doc/\"").count(), 1, "{index}");
}

#[test]
fn a_partial_build_names_what_it_cannot_find() {
    let dir = TempDir::new("partial-names");
    two_sections(&dir);
    let out = build(&dir, &["--only", "section:nope"]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("no section 'nope'"),
        "{}",
        stderr(&out)
    );
    let out = build(&dir, &["--only", "colour"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("expected home"), "{}", stderr(&out));
}

#[test]
fn a_page_takes_its_title_from_the_properties_and_tells_its_section_in_the_search() {
    let dir = TempDir::new("title");
    two_sections(&dir);
    let toml = dir.read("dkt.toml").replace(
        "src = \"a/doc.md\"\nsection = \"a\"",
        "src = \"a/doc.md\"\ntitle = \"Short\"\nsection = \"a\"",
    );
    dir.file("dkt.toml", &toml);
    let out = build(&dir, &[]);
    assert!(out.status.success(), "{}", stderr(&out));
    let page = dir.read("public/a/doc/index.html");
    assert!(page.contains("<h1 id=\"short\">"), "{page}");
    assert!(!page.contains("Doc of a"), "{page}");
    let index = dir.read("public/search.json");
    assert!(index.contains("\"t\":\"Short - A\""), "{index}");
    assert!(index.contains("\"t\":\"Doc of b - B\""), "{index}");
}

#[test]
fn a_title_does_not_go_with_a_glob() {
    let dir = TempDir::new("title-glob");
    two_sections(&dir);
    let toml = dir.read("dkt.toml").replace(
        "src = \"b/doc.md\"\nsection = \"b\"",
        "glob = \"b/*.md\"\ntitle = \"No\"\nsection = \"b\"",
    );
    dir.file("dkt.toml", &toml);
    let out = build(&dir, &["--no-lint"]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("a title goes with a src"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn every_theme_demo_of_the_examples_has_a_home_and_its_pages() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let tmp = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("theme-demos");
    let mut demos = 0;
    for entry in std::fs::read_dir(root.join("examples/themes")).expect("the demos") {
        let file = entry.expect("an entry").path();
        if file.extension().is_none_or(|e| e != "toml") {
            continue;
        }
        let out = tmp.join(file.file_stem().expect("a name"));
        let _ = std::fs::remove_dir_all(&out);
        let built = std::process::Command::new(env!("CARGO_BIN_EXE_dkt"))
            .arg("site")
            .arg(&file)
            .arg("-o")
            .arg(&out)
            .output()
            .expect("run dkt site");
        assert!(
            built.status.success(),
            "{}: {}",
            file.display(),
            String::from_utf8_lossy(&built.stderr)
        );
        for page in ["index.html", "guide/index.html", "guide/tour/index.html"] {
            assert!(out.join(page).exists(), "{} has no {page}", file.display());
        }
        demos += 1;
    }
    assert!(demos >= 4, "{demos} demos");
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn a_theme_of_its_own_gets_the_widgets_without_naming_them() {
    let dir = TempDir::new("widgets-theme");
    dir.file(
        "dkt.toml",
        "[site]\ntitle = \"T\"\ntheme = \"mine\"\nbase_url = \"https://example.com/sub\"\n\n[[page]]\nsrc = \"a.md\"\n",
    );
    dir.file(
        "a.md",
        "# A\n\n<!-- site: widget tabs -->\n## One\n\nx\n\n## Two\n\ny\n<!-- site: end -->\n",
    );
    dir.file(
        "mine/templates/base.html",
        "<html><head><title>T</title></head><body>{% block main %}{% endblock %}</body></html>",
    );
    dir.file(
        "mine/templates/page.html",
        "{% extends \"base.html\" %}{% block main %}{{ page.content }}{% endblock %}",
    );
    dir.file("mine/static/widgets.css", "from the theme");
    let out = build(&dir, &[]);
    assert!(out.status.success(), "{}", stderr(&out));
    let page = dir.read("public/a/index.html");
    assert!(
        page.contains("<script src=\"/sub/widgets.js\" defer></script>"),
        "{page}"
    );
    assert!(
        page.contains("<link rel=\"stylesheet\" href=\"/sub/content.css\">"),
        "{page}"
    );
    assert!(dir.exists("public/widgets.js"));
    assert!(dir.exists("public/widgets-extra.js"));
    assert!(dir.exists("public/content.css"));
    assert_eq!(dir.read("public/widgets.css"), "from the theme");
}

#[test]
fn a_built_in_theme_loads_the_widgets_once() {
    let dir = TempDir::new("widgets-builtin");
    dir.file(
        "dkt.toml",
        "[site]\ntitle = \"T\"\ntheme = \"docs\"\n\n[[page]]\nsrc = \"a.md\"\n",
    );
    dir.file("a.md", "# A\n\ntext\n");
    let out = build(&dir, &[]);
    assert!(out.status.success(), "{}", stderr(&out));
    let page = dir.read("public/a/index.html");
    assert_eq!(page.matches("widgets.js").count(), 1, "{page}");
    assert_eq!(page.matches("widgets.css").count(), 1, "{page}");
    assert_eq!(page.matches("content.css").count(), 1, "{page}");
}

#[test]
fn a_link_to_a_file_the_site_does_not_serve_stops_the_build() {
    let dir = TempDir::new("unserved");
    site(&dir);
    dir.file(
        "docs/guides/b.md",
        "# B\n\nlede of b\n\nSee [the code](../../src/main.rs#L1) and ![a](../img/x.png).\n",
    );
    let out = build(&dir, &[]);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(err.contains("docs/guides/b.md:5:"), "{err}");
    assert!(err.contains("link-unserved"), "{err}");
    assert!(err.contains("src/main.rs"), "{err}");
    assert!(!err.contains("img/x.png"), "an asset is served: {err}");
    assert!(build(&dir, &["--no-lint"]).status.success());

    dir.file(
        "dkt.toml",
        &format!("{}\n[lint]\ndisable = [\"link-unserved\"]\n", CONFIG),
    );
    let out = build(&dir, &[]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        dir.read("public/guides/b/index.html")
            .contains(r#"href="../../src/main.rs#L1""#)
    );
}

#[test]
fn the_repository_options_of_an_earlier_version_are_refused() {
    let dir = TempDir::new("repo-gone");
    site(&dir);
    dir.file(
        "dkt.toml",
        &CONFIG.replace("[site]\n", "[site]\nrepo = \"https://github.com/o/r\"\n"),
    );
    let out = build(&dir, &[]);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(
        err.contains("`repo` is no longer an option of [site]"),
        "{err}"
    );
    assert!(err.contains("[data]"), "{err}");
}

#[test]
fn an_asset_directory_leaves_out_what_its_patterns_name_at_any_depth() {
    let dir = TempDir::new("exclude");
    dir.file(
        "dkt.toml",
        "[site]\ntitle = \"T\"\n\n[[page]]\nsrc = \"a.md\"\n\n[[asset]]\nsrc = \"img\"\nurl = \"/img/\"\nexclude = [\"*.psd\", \"raw/\", \"**/old-*\"]\n",
    );
    dir.file("templates/page.html", "{{ page.content }}");
    dir.file("a.md", "# A\n\ntext ![a](img/a.png)\n");
    for file in [
        "img/a.png",
        "img/x.psd",
        "img/sub/y.psd",
        "img/raw/z.png",
        "img/deep/old-1.png",
        "img/deep/ok.png",
    ] {
        dir.file(file, "x");
    }
    let out = build(&dir, &[]);
    assert!(out.status.success(), "{}", stderr(&out));
    for kept in ["img/a.png", "img/deep/ok.png"] {
        assert!(dir.exists(&format!("public/{kept}")), "{kept}");
    }
    for left_out in [
        "img/x.psd",
        "img/sub/y.psd",
        "img/raw/z.png",
        "img/deep/old-1.png",
    ] {
        assert!(!dir.exists(&format!("public/{left_out}")), "{left_out}");
    }

    dir.file("a.md", "# A\n\n![a](img/sub/y.psd)\n");
    let out = build(&dir, &[]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("img/sub/y.psd"), "{}", stderr(&out));
}

#[test]
fn a_partial_build_keeps_the_widgets_of_the_other_pages_and_replaces_its_own() {
    let dir = TempDir::new("partial-widgets");
    two_sections(&dir);
    let both = |extra: &str| {
        let config = std::fs::read_to_string(dir.path("dkt.toml")).expect("config");
        config.replace(
            "[[page]]\nsrc = \"b/doc.md\"",
            &format!("{extra}[[page]]\nsrc = \"b/doc.md\""),
        )
    };
    let widget = |id: &str| {
        format!(
            "# Doc\n\nlede\n\n<!-- site: widget checklist id={id} -->\n- [ ] one\n- [ ] two\n<!-- site: end -->\n"
        )
    };
    dir.file("a/doc.md", &widget("a-old"));
    dir.file("a/gone.md", &widget("a-gone"));
    dir.file("b/doc.md", &widget("b-first"));
    dir.file(
        "dkt.toml",
        &both("[[page]]\nsrc = \"a/gone.md\"\nsection = \"a\"\n\n"),
    );
    assert!(build(&dir, &[]).status.success());
    let first = dir.read("public/widgets.json");
    for id in ["a-old", "a-gone", "b-first"] {
        assert!(first.contains(id), "{first}");
    }

    // The section a loses a page and changes a widget, the section b is not read again.
    dir.file("a/doc.md", &widget("a-new"));
    dir.file("b/doc.md", &widget("b-changed-but-not-built"));
    let config = both("").replace("[[page]]\nsrc = \"a/gone.md\"\nsection = \"a\"\n\n", "");
    dir.file("dkt.toml", &config);
    let out = build(&dir, &["--only", "section:a"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let second = dir.read("public/widgets.json");
    assert!(second.contains("a-new"), "{second}");
    assert!(!second.contains("a-old"), "{second}");
    assert!(!second.contains("a-gone"), "{second}");
    assert!(second.contains("b-first"), "{second}");
    assert!(!second.contains("b-changed-but-not-built"), "{second}");
}
