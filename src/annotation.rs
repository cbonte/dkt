//! Annotations a document carries for the site, written as HTML comments GitHub does not show.
//!
//! An annotation is one comment, `<!-- site: KIND key=value key="a value" flag -->`.
//! Its kind sets what it applies to.
//! `page` applies to the whole page, wherever it stands.
//! `figure` applies to the place it is written.
//! `replace` applies to the blocks up to the next `end`.
//! `alias` applies to the section it is written in, and gives the words that find it in a search.
//! `widget` applies to the blocks up to the next `end`, and makes a component of them.
//! `light` applies to the step of a `scene` widget it is written in, and names the parts of the
//! figure that the step lights or turns off.
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
    Light,
}

/// The components a `widget` annotation can name, with the keys and the flags each one takes
/// besides `id`.
pub const WIDGETS: [(&str, &[&str], &[&str]); 16] = [
    ("tabs", &["group"], &[]),
    ("checklist", &[], &[]),
    ("quiz", &["answer"], &[]),
    ("steps", &[], &[]),
    ("carousel", &["show", "autoplay"], &["loop", "zoom"]),
    ("table", &["src", "columns"], &[]),
    ("compare", &[], &[]),
    ("diff", &[], &[]),
    ("chart", &["type"], &[]),
    ("flashcards", &[], &[]),
    ("annotated", &["pins"], &[]),
    ("builder", &[], &[]),
    ("replay", &["speed", "src"], &[]),
    ("since", &[], &[]),
    ("progress", &[], &[]),
    ("scene", &["autoplay"], &[]),
];

/// The names of the widgets.
pub fn widget_names() -> Vec<&'static str> {
    WIDGETS.iter().map(|(name, _, _)| *name).collect()
}

/// The widget that an annotation names, with what it takes.
fn widget_of(
    annotation: &Annotation,
) -> Option<&'static (
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
)> {
    let name = annotation.get("widget")?;
    WIDGETS.iter().find(|(n, _, _)| *n == name)
}

/// How an annotation is called in a message: the widget it names, else its kind.
fn owner(kind: Kind, annotation: &Annotation) -> String {
    match annotation.get("widget") {
        Some(name) if kind == Kind::Widget => name.to_string(),
        _ => kind.name().to_string(),
    }
}

/// True for a word of letters, digits, hyphens and underscores.
fn is_name(word: &str) -> bool {
    !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

/// True for a word of a `light` annotation: an id, `-` and an id, or `-*`.
fn is_cue(word: &str) -> bool {
    word == "-*" || (is_name(word.strip_prefix('-').unwrap_or(word)) && !word.starts_with("--"))
}

/// The kinds of chart.
const CHART_TYPES: [&str; 2] = ["bar", "line"];

/// The most pins on a picture.
const MAX_PINS: usize = 50;

/// The slowest and the fastest replay of a terminal session.
const MIN_SPEED: f64 = 0.25;
const MAX_SPEED: f64 = 8.0;

/// The most slides a carousel shows at once.
const MAX_SHOWN: usize = 6;

/// The shortest delay of an automatic carousel, in milliseconds.
const MIN_AUTOPLAY: u32 = 500;

impl Kind {
    pub const ALL: [Kind; 7] = [
        Kind::Page,
        Kind::Figure,
        Kind::Replace,
        Kind::End,
        Kind::Alias,
        Kind::Widget,
        Kind::Light,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Page => "page",
            Kind::Figure => "figure",
            Kind::Replace => "replace",
            Kind::End => "end",
            Kind::Alias => "alias",
            Kind::Widget => "widget",
            Kind::Light => "light",
        }
    }

    fn keys(self) -> &'static [&'static str] {
        match self {
            Kind::Page => &["title", "description", "nav-title", "weight"],
            Kind::Figure | Kind::Replace => &["name", "src", "dark", "alt", "caption"],
            Kind::End | Kind::Light => &[],
            Kind::Alias => &["words"],
            Kind::Widget => &["id"],
        }
    }

    fn flags(self) -> &'static [&'static str] {
        match self {
            Kind::Page => &["hide", "no-search"],
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
        let image = format!(
            r#"<img src="{}" alt="{}">"#,
            crate::escape::html(src),
            crate::escape::html(alt)
        );
        let picture = match self.get("dark") {
            Some(dark) => format!(
                r#"<picture><source media="(prefers-color-scheme: dark)" srcset="{}">{image}</picture>"#,
                crate::escape::html(dark)
            ),
            None => image,
        };
        let caption = self
            .get("caption")
            .map(|c| format!("<figcaption>{}</figcaption>", crate::escape::html(c)))
            .unwrap_or_default();
        Some(format!("<figure>{picture}{caption}</figure>\n"))
    }
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
    if kind == Kind::Light {
        let parts: Vec<String> = words.collect();
        if parts.is_empty() {
            return Err("'light' needs the ids of parts of the figure".to_string());
        }
        if let Some(bad) = parts.iter().find(|p| !is_cue(p)) {
            return Err(format!(
                "'{bad}' is not a part: write an id, - and an id to turn it off, or -* to turn off every part"
            ));
        }
        annotation.args.insert("parts".to_string(), parts.join(" "));
        return Ok(annotation);
    }
    for word in words {
        if kind == Kind::Widget && !annotation.args.contains_key("widget") && !word.contains('=') {
            if !WIDGETS.iter().any(|(name, _, _)| *name == word) {
                return Err(format!(
                    "unknown widget '{word}', expected one of {}",
                    widget_names().join(", ")
                ));
            }
            annotation.args.insert("widget".to_string(), word);
            continue;
        }
        let widget = widget_of(&annotation);
        let takes_key = |key: &str| {
            kind.keys().contains(&key)
                || widget.is_some_and(|(_, keys, _)| kind == Kind::Widget && keys.contains(&key))
        };
        let takes_flag = |flag: &str| {
            kind.flags().contains(&flag)
                || widget.is_some_and(|(_, _, flags)| kind == Kind::Widget && flags.contains(&flag))
        };
        match word.split_once('=') {
            Some((key, value)) if takes_key(key) => {
                annotation.args.insert(key.to_string(), value.to_string());
            }
            Some((key, _)) => {
                return Err(format!(
                    "'{}' does not take '{key}'",
                    owner(kind, &annotation)
                ));
            }
            None if takes_flag(&word) => {
                annotation.flags.insert(word);
            }
            None => {
                return Err(format!(
                    "'{}' does not take '{word}'",
                    owner(kind, &annotation)
                ));
            }
        }
    }
    if matches!(kind, Kind::Figure | Kind::Replace)
        && !annotation.args.contains_key("name")
        && !annotation.args.contains_key("src")
    {
        return Err(format!("'{}' needs a name or a src", kind.name()));
    }
    if kind == Kind::Widget {
        let Some(widget) = annotation.get("widget").map(str::to_string) else {
            return Err("'widget' needs a name".to_string());
        };
        if widget == "quiz" && annotation.get("answer").is_none() {
            return Err("'quiz' needs an answer".to_string());
        }
        if let Some(answer) = annotation.get("answer")
            && !answer.parse::<usize>().is_ok_and(|n| n > 0)
        {
            return Err(format!("answer '{answer}' is not a position from 1"));
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
        if let Some(group) = annotation.get("group")
            && !is_name(group)
        {
            return Err("a group is letters, digits, hyphens and underscores".to_string());
        }
        if let Some(kind) = annotation.get("type")
            && !CHART_TYPES.contains(&kind)
        {
            return Err(format!(
                "a chart is one of {}, not '{kind}'",
                CHART_TYPES.join(", ")
            ));
        }
        if let Some(pins) = annotation.get("pins") {
            let valid = !pins.split_whitespace().next().is_none()
                && pins.split_whitespace().count() <= MAX_PINS
                && pins.split_whitespace().all(|pin| {
                    pin.split_once(',').is_some_and(|(x, y)| {
                        [x, y]
                            .iter()
                            .all(|n| n.parse::<f64>().is_ok_and(|v| (0.0..=100.0).contains(&v)))
                    })
                });
            if !valid {
                return Err(format!(
                    "pins are up to {MAX_PINS} positions as x,y in percent of the picture, from 0 to 100"
                ));
            }
        }
        if let Some(speed) = annotation.get("speed")
            && !speed
                .parse::<f64>()
                .is_ok_and(|s| (MIN_SPEED..=MAX_SPEED).contains(&s))
        {
            return Err(format!("speed is a number from {MIN_SPEED} to {MAX_SPEED}"));
        }
        if annotation.get("id").is_some_and(|id| !is_name(id)) {
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
    fn a_light_names_parts_to_light_and_to_turn_off() {
        let a = parsed(" site: light gateway -vision -* ");
        assert_eq!(a.kind, Kind::Light);
        assert_eq!(a.get("parts"), Some("gateway -vision -*"));
        assert!(refused(" site: light").contains("needs the ids"));
        assert!(refused(" site: light a/b").contains("'a/b' is not a part"));
        assert!(refused(" site: light --x").contains("is not a part"));
        assert!(refused(" site: light -").contains("is not a part"));
        let scene = parsed(" site: widget scene id=flow autoplay=3000 ");
        assert_eq!(scene.get("autoplay"), Some("3000"));
        assert!(refused(" site: widget scene autoplay=10 ").contains("autoplay is"));
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
