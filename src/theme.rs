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
        file!("docs", "templates/partials/side-top.html"),
    ],
    statics: &[
        file!("docs", "static/docs.css"),
        file!("docs", "static/docs.js"),
        file!("docs", "static/search.js"),
        file!("common", "static/widgets.css"),
        file!("common", "static/widgets.js"),
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
    ],
    statics: &[
        file!("paper", "static/paper.css"),
        file!("paper", "static/paper.js"),
        file!("paper", "static/search.js"),
        file!("common", "static/widgets.css"),
        file!("common", "static/widgets.js"),
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
    ],
    statics: &[
        file!("plain", "static/plain.css"),
        file!("plain", "static/search.js"),
        file!("common", "static/widgets.css"),
        file!("common", "static/widgets.js"),
    ],
};

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
            for shared in ["widgets.css", "widgets.js"] {
                assert!(
                    theme.static_files().any(|(p, _)| p == shared),
                    "{name} lacks {shared}"
                );
            }
        }
        assert!(builtin("nothing").is_none());
        assert!(builtin(DEFAULT).is_some());
    }
}
