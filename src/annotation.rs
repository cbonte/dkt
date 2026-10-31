//! Annotations a document carries for the site, written as HTML comments GitHub does not show.
//!
//! An annotation is one comment, `<!-- site: KIND key=value key="a value" flag -->`.
//! Its kind sets what it applies to.
//! `page` applies to the whole page, wherever it stands.
//! `figure` applies to the place it is written.
//! `replace` applies to the blocks up to the next `end`.
//! `alias` applies to the section it is written in, and gives the words that find it in a search.
//! `widget` applies to the blocks up to the next `end`, and makes a component of them.
//! A comment that does not start with `site:` is not an annotation and is left alone.

use std::collections::{BTreeMap, BTreeSet};

/// What an annotation applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Page,
    Figure,
    Replace,
    End,
    Alias,
    Widget,
}

/// The components a `widget` annotation can name.
pub const WIDGETS: [&str; 5] = ["tabs", "checklist", "quiz", "steps", "carousel"];

/// The most slides a carousel shows at once.
const MAX_SHOWN: usize = 6;

/// The shortest delay of an automatic carousel, in milliseconds.
const MIN_AUTOPLAY: u32 = 500;

impl Kind {
    pub const ALL: [Kind; 6] = [
        Kind::Page,
        Kind::Figure,
        Kind::Replace,
        Kind::End,
        Kind::Alias,
        Kind::Widget,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Page => "page",
            Kind::Figure => "figure",
            Kind::Replace => "replace",
            Kind::End => "end",
            Kind::Alias => "alias",
            Kind::Widget => "widget",
        }
    }

    fn keys(self) -> &'static [&'static str] {
        match self {
            Kind::Page => &["title", "description", "nav-title", "weight"],
            Kind::Figure | Kind::Replace => &["name", "src", "dark", "alt", "caption"],
            Kind::End => &[],
            Kind::Alias => &["words"],
            Kind::Widget => &["id", "answer", "show", "autoplay"],
        }
    }

    fn flags(self) -> &'static [&'static str] {
        match self {
            Kind::Page => &["hide", "no-search"],
            Kind::Widget => &["loop", "zoom"],
            _ => &[],
        }
    }
}

/// One annotation, checked against what its kind accepts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Annotation {
    pub kind: Kind,
    pub args: BTreeMap<String, String>,
    pub flags: BTreeSet<String>,
}

impl Annotation {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.args.get(key).map(String::as_str)
    }

    pub fn has(&self, flag: &str) -> bool {
        self.flags.contains(flag)
    }

    /// The same annotation, with the images it names given by path.
    pub fn with_images(&self, src: String, dark: Option<String>) -> Annotation {
        let mut copy = self.clone();
        copy.args.insert("src".to_string(), src);
        if let Some(dark) = dark {
            copy.args.insert("dark".to_string(), dark);
        }
        copy
    }

    /// The figure this annotation stands for, when it names an image by its path.
    pub fn figure(&self) -> Option<String> {
        let src = self.get("src")?;
        let alt = self.get("alt").unwrap_or_default();
        let image = format!(r#"<img src="{}" alt="{}">"#, escape(src), escape(alt));
        let picture = match self.get("dark") {
            Some(dark) => format!(
                r#"<picture><source media="(prefers-color-scheme: dark)" srcset="{}">{image}</picture>"#,
                escape(dark)
            ),
            None => image,
        };
        let caption = self
            .get("caption")
            .map(|c| format!("<figcaption>{}</figcaption>", escape(c)))
            .unwrap_or_default();
        Some(format!("<figure>{picture}{caption}</figure>\n"))
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The text inside the comment, when `html` is exactly one comment.
pub fn comment(html: &str) -> Option<&str> {
    let html = html.trim();
    let body = html.strip_prefix("<!--")?.strip_suffix("-->")?;
    (!body.contains("-->")).then_some(body)
}

/// Splits on spaces, a double-quoted run keeping its spaces and losing its quotes.
fn tokens(text: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let (mut current, mut quoted, mut started) = (String::new(), false, false);
    for c in text.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    out.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            c => {
                current.push(c);
                started = true;
            }
        }
    }
    if quoted {
        return Err("a quote is never closed".to_string());
    }
    if started {
        out.push(current);
    }
    Ok(out)
}

/// Parses the text of a comment. `None` when it is not an annotation, an error when it is one
/// that does not hold.
pub fn parse(body: &str) -> Option<Result<Annotation, String>> {
    let rest = body.trim().strip_prefix("site:")?;
    Some(parse_annotation(rest))
}

/// Parses an HTML block as an annotation, when it is one.
pub fn from_html(html: &str) -> Option<Result<Annotation, String>> {
    parse(comment(html)?)
}

fn parse_annotation(rest: &str) -> Result<Annotation, String> {
    let mut words = tokens(rest)?.into_iter();
    let name = words.next().ok_or("it has no kind")?;
    let kind = Kind::ALL
        .into_iter()
        .find(|k| k.name() == name)
        .ok_or_else(|| {
            let kinds: Vec<&str> = Kind::ALL.iter().map(|k| k.name()).collect();
            format!(
                "unknown kind '{name}', expected one of {}",
                kinds.join(", ")
            )
        })?;
    let mut annotation = Annotation {
        kind,
        args: BTreeMap::new(),
        flags: BTreeSet::new(),
    };
    for word in words {
        if kind == Kind::Widget && !annotation.args.contains_key("widget") && !word.contains('=') {
            if !WIDGETS.contains(&word.as_str()) {
                return Err(format!(
                    "unknown widget '{word}', expected one of {}",
                    WIDGETS.join(", ")
                ));
            }
            annotation.args.insert("widget".to_string(), word);
            continue;
        }
        match word.split_once('=') {
            Some((key, value)) if kind.keys().contains(&key) => {
                annotation.args.insert(key.to_string(), value.to_string());
            }
            Some((key, _)) => {
                return Err(format!("'{}' does not take '{key}'", kind.name()));
            }
            None if kind.flags().contains(&word.as_str()) => {
                annotation.flags.insert(word);
            }
            None => return Err(format!("'{}' does not take '{word}'", kind.name())),
        }
    }
    if matches!(kind, Kind::Figure | Kind::Replace)
        && !annotation.args.contains_key("name")
        && !annotation.args.contains_key("src")
    {
        return Err(format!("'{}' needs a name or a src", kind.name()));
    }
    if kind == Kind::Widget {
        let Some(widget) = annotation.get("widget") else {
            return Err("'widget' needs a name".to_string());
        };
        match (widget == "quiz", annotation.get("answer")) {
            (true, None) => return Err("'quiz' needs an answer".to_string()),
            (true, Some(answer)) if answer.parse::<usize>().is_ok_and(|n| n > 0) => {}
            (true, Some(answer)) => {
                return Err(format!("answer '{answer}' is not a position from 1"));
            }
            (false, Some(_)) => return Err(format!("'{widget}' does not take 'answer'")),
            (false, None) => {}
        }
        // The options of a carousel.
        if widget != "carousel" {
            for key in ["show", "autoplay"] {
                if annotation.get(key).is_some() {
                    return Err(format!("'{widget}' does not take '{key}'"));
                }
            }
            for flag in ["loop", "zoom"] {
                if annotation.has(flag) {
                    return Err(format!("'{widget}' does not take '{flag}'"));
                }
            }
        }
        if let Some(show) = annotation.get("show") {
            let counts: Vec<&str> = show.split_whitespace().collect();
            let valid = (1..=3).contains(&counts.len())
                && counts.iter().all(|c| {
                    c.parse::<usize>()
                        .is_ok_and(|n| (1..=MAX_SHOWN).contains(&n))
                });
            if !valid {
                return Err(format!(
                    "show is 1 to 3 counts of slides from 1 to {MAX_SHOWN}, one for each width of page"
                ));
            }
        }
        if let Some(delay) = annotation.get("autoplay")
            && !delay.parse::<u32>().is_ok_and(|ms| ms >= MIN_AUTOPLAY)
        {
            return Err(format!(
                "autoplay is a delay in milliseconds, {MIN_AUTOPLAY} or more"
            ));
        }
        if annotation.get("id").is_some_and(|id| {
            id.is_empty()
                || !id
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        }) {
            return Err("an id is letters, digits, hyphens and underscores".to_string());
        }
    }
    if kind == Kind::Alias && annotation.get("words").is_none_or(|w| w.trim().is_empty()) {
        return Err("'alias' needs words".to_string());
    }
    if let Some(weight) = annotation.get("weight")
        && weight.parse::<i64>().is_err()
    {
        return Err(format!("weight '{weight}' is not a number"));
    }
    Ok(annotation)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(body: &str) -> Annotation {
        parse(body).expect("an annotation").expect("a valid one")
    }

    fn refused(body: &str) -> String {
        parse(body).expect("an annotation").expect_err("an error")
    }

    #[test]
    fn a_page_annotation_has_keys_and_flags() {
        let a = parsed(r#" site: page title="The tour" weight=3 hide "#);
        assert_eq!(a.kind, Kind::Page);
        assert_eq!(a.get("title"), Some("The tour"));
        assert_eq!(a.get("weight"), Some("3"));
        assert!(a.has("hide"));
    }

    #[test]
    fn other_comments_are_not_annotations() {
        assert!(parse(" toc ").is_none());
        assert!(parse(" picture:x caption ").is_none());
        assert!(from_html("<!-- a note -->\n").is_none());
    }

    #[test]
    fn a_comment_is_found_in_an_html_block() {
        let a = from_html("<!-- site: figure src=a.png -->\n")
            .expect("an annotation")
            .expect("valid");
        assert_eq!(a.kind, Kind::Figure);
        assert!(from_html("<div><!-- site: end --></div>").is_none());
    }

    #[test]
    fn a_widget_has_a_name_and_keys() {
        let a = parsed(" site: widget tabs id=install ");
        assert_eq!(a.kind, Kind::Widget);
        assert_eq!(a.get("widget"), Some("tabs"));
        assert_eq!(a.get("id"), Some("install"));
        let quiz = parsed(" site: widget quiz answer=2 ");
        assert_eq!(quiz.get("answer"), Some("2"));
    }

    #[test]
    fn a_carousel_takes_its_options() {
        let a = parsed(" site: widget carousel show=\"1 2 3\" autoplay=4500 loop zoom ");
        assert_eq!(a.get("show"), Some("1 2 3"));
        assert_eq!(a.get("autoplay"), Some("4500"));
        assert!(a.has("loop") && a.has("zoom"));
        assert!(refused(" site: widget carousel show=0 ").contains("show is"));
        assert!(refused(" site: widget carousel show=\"1 2 3 4\" ").contains("show is"));
        assert!(refused(" site: widget carousel autoplay=10 ").contains("autoplay is"));
        assert!(refused(" site: widget tabs loop ").contains("does not take 'loop'"));
        assert!(refused(" site: widget steps show=2 ").contains("does not take 'show'"));
    }

    #[test]
    fn widget_mistakes_are_named() {
        assert!(refused(" site: widget").contains("needs a name"));
        assert!(refused(" site: widget dance").contains("unknown widget 'dance'"));
        assert!(refused(" site: widget quiz").contains("needs an answer"));
        assert!(refused(" site: widget quiz answer=0").contains("not a position"));
        assert!(refused(" site: widget tabs answer=1").contains("does not take 'answer'"));
        assert!(refused(" site: widget tabs id=a/b").contains("an id is"));
        assert!(refused(" site: widget tabs color=red").contains("does not take 'color'"));
    }

    #[test]
    fn mistakes_are_named() {
        assert!(refused(" site: nope").contains("unknown kind 'nope'"));
        assert!(refused(" site: page color=red").contains("does not take 'color'"));
        assert!(refused(" site: page shout").contains("does not take 'shout'"));
        assert!(refused(" site: figure alt=x").contains("needs a name or a src"));
        assert!(refused(" site: page weight=heavy").contains("not a number"));
        assert!(refused(r#" site: page title="open"#).contains("never closed"));
        assert!(refused(" site:").contains("no kind"));
        assert!(refused(" site: end now").contains("does not take"));
        assert!(refused(" site: alias").contains("needs words"));
        assert!(refused(r#" site: alias words="" "#).contains("needs words"));
    }

    #[test]
    fn an_alias_gives_words_and_a_page_can_leave_the_search() {
        let a = parsed(r#" site: alias words="no-lint base-url" "#);
        assert_eq!(a.kind, Kind::Alias);
        assert_eq!(a.get("words"), Some("no-lint base-url"));
        assert!(parsed(" site: page no-search").has("no-search"));
    }

    #[test]
    fn a_figure_with_a_dark_variant() {
        let a =
            parsed(r#" site: figure src=a-light.png dark=a-dark.png alt="A & B" caption=Shot "#);
        assert_eq!(
            a.figure().expect("a figure"),
            "<figure><picture><source media=\"(prefers-color-scheme: dark)\" srcset=\"a-dark.png\"><img src=\"a-light.png\" alt=\"A &amp; B\"></picture><figcaption>Shot</figcaption></figure>\n"
        );
        assert!(parsed(" site: figure name=x").figure().is_none());
    }
}
