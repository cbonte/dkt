//! The themes that come with the binary.
//!
//! A theme is a set of templates and static files. A site names one with `theme` in its
//! properties file, and a directory of that name wins over a built-in one.

/// A theme held in the binary: its templates and its static files, each with its path inside.
pub struct Builtin {
    pub name: &'static str,
    pub templates: &'static [(&'static str, &'static str)],
    pub statics: &'static [(&'static str, &'static str)],
}

macro_rules! file {
    ($dir:literal, $path:literal) => {
        ($path, include_str!(concat!("../themes/", $dir, "/", $path)))
    };
}

static DOCS: Builtin = Builtin {
    name: "docs",
    templates: &[
        file!("docs", "templates/base.html"),
        file!("docs", "templates/index.html"),
        file!("docs", "templates/page.html"),
        file!("docs", "templates/section.html"),
        file!("docs", "templates/partials/brand.html"),
        file!("docs", "templates/partials/pager.html"),
        file!("docs", "templates/partials/side-top.html"),
        file!("common", "templates/partials/versions.html"),
        file!("common", "templates/partials/version-note.html"),
    ],
    statics: &[
        file!("docs", "static/docs.css"),
        file!("docs", "static/docs.js"),
        file!("docs", "static/search.js"),
        file!("common", "static/content.css"),
        file!("common", "static/widgets.css"),
        file!("common", "static/widgets.js"),
        file!("common", "static/widgets-extra.js"),
    ],
};

static PAPER: Builtin = Builtin {
    name: "paper",
    templates: &[
        file!("paper", "templates/base.html"),
        file!("paper", "templates/index.html"),
        file!("paper", "templates/page.html"),
        file!("paper", "templates/section.html"),
        file!("paper", "templates/partials/brand.html"),
        file!("paper", "templates/partials/pager.html"),
        file!("common", "templates/partials/versions.html"),
        file!("common", "templates/partials/version-note.html"),
    ],
    statics: &[
        file!("paper", "static/paper.css"),
        file!("paper", "static/paper.js"),
        file!("paper", "static/search.js"),
        file!("common", "static/content.css"),
        file!("common", "static/widgets.css"),
        file!("common", "static/widgets.js"),
        file!("common", "static/widgets-extra.js"),
    ],
};

static PLAIN: Builtin = Builtin {
    name: "plain",
    templates: &[
        file!("plain", "templates/base.html"),
        file!("plain", "templates/index.html"),
        file!("plain", "templates/page.html"),
        file!("plain", "templates/section.html"),
        file!("plain", "templates/partials/brand.html"),
        file!("plain", "templates/partials/pager.html"),
        file!("common", "templates/partials/versions.html"),
        file!("common", "templates/partials/version-note.html"),
    ],
    statics: &[
        file!("plain", "static/plain.css"),
        file!("plain", "static/search.js"),
        file!("common", "static/content.css"),
        file!("common", "static/widgets.css"),
        file!("common", "static/widgets.js"),
        file!("common", "static/widgets-extra.js"),
    ],
};

/// The files of the widgets and of the content style, written to every site whatever its theme.
/// A file of the theme or of the site with the same name replaces them.
pub const WIDGET_FILES: [(&str, &str); 4] = [
    (
        "content.css",
        include_str!("../themes/common/static/content.css"),
    ),
    (
        "widgets.css",
        include_str!("../themes/common/static/widgets.css"),
    ),
    (
        "widgets.js",
        include_str!("../themes/common/static/widgets.js"),
    ),
    (
        "widgets-extra.js",
        include_str!("../themes/common/static/widgets-extra.js"),
    ),
];

/// The style and the script of the selector of versions, written only into a version of a
/// documentation.
pub const VERSION_FILES: [(&str, &str); 2] = [
    (
        "versions.css",
        include_str!("../themes/common/static/versions.css"),
    ),
    (
        "versions.js",
        include_str!("../themes/common/static/versions.js"),
    ),
];

/// The page with the style and the script of the widgets, for a theme whose templates do not
/// load them.
///
/// A file that the page already loads is left alone, so a built-in theme and a theme that loads
/// them itself get nothing twice.
/// The style goes at the end of the head and the script at the end of the body.
pub fn with_widgets(html: &str, base: &str) -> String {
    let loads =
        |file: &str| html.contains(&format!("/{file}\"")) || html.contains(&format!("/{file}'"));
    let mut head = String::new();
    for css in ["content.css", "widgets.css"] {
        if !loads(css) {
            head.push_str(&format!(
                "<link rel=\"stylesheet\" href=\"{base}/{css}\">\n"
            ));
        }
    }
    let script = (!loads("widgets.js"))
        .then(|| format!("<script src=\"{base}/widgets.js\" defer></script>\n"));
    let mut out = html.to_string();
    if !head.is_empty() {
        let at = out.find("</head>").unwrap_or(0);
        out.insert_str(at, &head);
    }
    if let Some(script) = script {
        match out.rfind("</body>") {
            Some(at) => out.insert_str(at, &script),
            None => out.push_str(&script),
        }
    }
    out
}

/// The theme a site gets when it names none.
pub const DEFAULT: &str = "docs";

static ALL: &[&Builtin] = &[&DOCS, &PAPER, &PLAIN];

/// The built-in theme called `name`.
pub fn builtin(name: &str) -> Option<&'static Builtin> {
    ALL.iter().copied().find(|t| t.name == name)
}

/// The names of the built-in themes.
pub fn names() -> Vec<&'static str> {
    ALL.iter().map(|t| t.name).collect()
}

impl Builtin {
    /// The text of the template `name`, such as `base.html` or `partials/hero.html`.
    pub fn template(&self, name: &str) -> Option<&'static str> {
        let wanted = format!("templates/{name}");
        self.templates
            .iter()
            .find(|(p, _)| *p == wanted)
            .map(|(_, t)| *t)
    }

    /// The static files with their path under the output directory.
    pub fn static_files(&self) -> impl Iterator<Item = (&'static str, &'static str)> + '_ {
        self.statics
            .iter()
            .map(|(p, t)| (p.strip_prefix("static/").unwrap_or(p), *t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_theme_holds_the_templates_a_site_needs() {
        for name in names() {
            let theme = builtin(name).unwrap();
            for template in [
                "base.html",
                "index.html",
                "page.html",
                "section.html",
                "partials/brand.html",
                "partials/pager.html",
                "partials/versions.html",
                "partials/version-note.html",
            ] {
                assert!(
                    theme.template(template).is_some(),
                    "{name} lacks {template}"
                );
            }
            assert!(
                theme
                    .static_files()
                    .any(|(p, _)| p == format!("{name}.css"))
            );
            for shared in [
                "content.css",
                "widgets.css",
                "widgets.js",
                "widgets-extra.js",
            ] {
                assert!(
                    theme.static_files().any(|(p, _)| p == shared),
                    "{name} lacks {shared}"
                );
            }
        }
        assert!(builtin("nothing").is_none());
        assert!(builtin(DEFAULT).is_some());
    }

    #[test]
    fn a_page_that_loads_nothing_gets_the_style_and_the_script() {
        let page = with_widgets(
            "<html><head><title>T</title></head><body>x</body></html>",
            "/s",
        );
        assert_eq!(
            page,
            "<html><head><title>T</title><link rel=\"stylesheet\" href=\"/s/content.css\">\n<link rel=\"stylesheet\" href=\"/s/widgets.css\">\n</head><body>x<script src=\"/s/widgets.js\" defer></script>\n</body></html>"
        );
    }

    #[test]
    fn a_page_that_loads_the_files_gets_nothing_twice() {
        let page = "<head><link rel=\"stylesheet\" href=\"/content.css\"><link rel=\"stylesheet\" href=\"/widgets.css\"></head><body><script src=\"/widgets.js\" defer></script></body>";
        assert_eq!(with_widgets(page, ""), page);
    }

    #[test]
    fn a_page_that_only_mentions_the_files_in_its_text_still_gets_them() {
        let page =
            "<body><code>widgets.js</code> and <pre>href=&quot;/widgets.css&quot;</pre></body>";
        let out = with_widgets(page, "");
        assert_eq!(out.matches("widgets.js").count(), 2);
        assert!(out.contains("<script src=\"/widgets.js\" defer></script>"));
    }

    #[test]
    fn a_page_without_head_or_body_gets_them_at_the_start_and_the_end() {
        let out = with_widgets("<p>x</p>", "");
        assert!(out.starts_with("<link rel=\"stylesheet\" href=\"/content.css\">"));
        assert!(out.ends_with("<script src=\"/widgets.js\" defer></script>\n"));
    }
}
