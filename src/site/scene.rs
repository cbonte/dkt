//! The `scene` widget: the steps of a list light, one after the other, the parts of an SVG figure.
//!
//! The figure is the first picture of the widget, an SVG file next to the document, or an `svg`
//! element written in the widget.
//! The site writes the figure in the page, so that the style of the page reaches its parts.
//! A part is an element of the figure with an `id`.
//!
//! A scene is the first ordered list under a heading of the shallowest level of the widget.
//! A widget with no heading holds 1 scene, its first ordered list.
//! Each item of that list is a step.
//! A `light` annotation in a step names the parts that the step lights and turns off.
//!
//! The ids of the figure take a prefix, so that 2 figures of a page never share one.
//! The style of the figure applies to the figure only, and its animations take the same prefix.
//! A script or an event attribute of the figure is left out.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::ops::Range;
use std::path::Path;

use anyhow::{Context, Result, bail};
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};

use crate::annotation::{self, Kind};
use crate::parse::parser_options;

/// A mistake in a scene, at a byte of the blocks of the widget, or at the widget itself.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Problem {
    pub offset: Option<usize>,
    pub message: String,
}

/// Where the figure of a scene comes from.
enum Source {
    /// A picture: its path as written, its alternative text and the paragraph that holds it.
    Picture {
        path: String,
        alt: String,
        block: Range<usize>,
    },
    /// An `svg` element written in the widget.
    Inline { block: Range<usize> },
}

/// A `light` annotation of a step: the comment and the words it gives.
struct Cue {
    range: Range<usize>,
    words: Vec<String>,
}

/// A scene: its name, and the cues of each of its steps.
struct Scene {
    name: Option<String>,
    steps: Vec<Vec<Cue>>,
    listed: bool,
}

/// What the blocks of a scene widget hold.
struct Analysis {
    source: Option<Source>,
    scenes: Vec<Scene>,
    problems: Vec<Problem>,
}

impl Analysis {
    fn problem(&mut self, offset: Option<usize>, message: String) {
        self.problems.push(Problem { offset, message });
    }

    /// The scenes that have steps.
    fn played(&self) -> impl Iterator<Item = &Scene> {
        self.scenes.iter().filter(|s| !s.steps.is_empty())
    }
}

/// The containers that take a block out of the top level of the widget.
fn nests(tag: &Tag<'_>) -> bool {
    matches!(
        tag,
        Tag::List(_) | Tag::BlockQuote(_) | Tag::FootnoteDefinition(_) | Tag::Table(_)
    )
}

fn unnests(tag: &TagEnd) -> bool {
    matches!(
        tag,
        TagEnd::List(_) | TagEnd::BlockQuote(_) | TagEnd::FootnoteDefinition | TagEnd::Table
    )
}

/// The range of the comment that a piece of HTML holds, in the blocks.
fn comment_in(held: &str, range: &Range<usize>) -> Option<Range<usize>> {
    let text = &held[range.clone()];
    let start = text.find("<!--")?;
    let end = text[start..].find("-->")? + start + 3;
    Some(range.start + start..range.start + end)
}

/// Reads the blocks of a scene widget.
fn analyse(held: &str) -> Analysis {
    let events: Vec<(Event<'_>, Range<usize>)> = Parser::new_ext(held, parser_options())
        .into_offset_iter()
        .collect();
    let mut nest = 0usize;
    let mut shallowest: Option<HeadingLevel> = None;
    for (event, _) in &events {
        match event {
            Event::Start(Tag::Heading { level, .. }) if nest == 0 => {
                shallowest = Some(shallowest.map_or(*level, |s| s.min(*level)));
            }
            Event::Start(tag) if nests(tag) => nest += 1,
            Event::End(tag) if unnests(tag) => nest = nest.saturating_sub(1),
            _ => {}
        }
    }
    let mut found = Analysis {
        source: None,
        scenes: Vec::new(),
        problems: Vec::new(),
    };
    if shallowest.is_none() {
        found.scenes.push(Scene {
            name: None,
            steps: Vec::new(),
            listed: false,
        });
    }
    // The list whose items are the steps of the last scene, while it is open.
    let mut in_steps = false;
    let mut nest = 0usize;
    let mut paragraph: Option<Range<usize>> = None;
    let mut skip_to = 0;
    let mut i = 0;
    while i < events.len() {
        let (event, range) = &events[i];
        i += 1;
        if range.start < skip_to {
            continue;
        }
        match event {
            // An `svg` element starts a block of HTML, or a paragraph when more follows its tag.
            Event::Start(Tag::Paragraph | Tag::HtmlBlock)
                if nest == 0
                    && found.source.is_none()
                    && held[range.start..]
                        .trim_start()
                        .get(..4)
                        .is_some_and(|t| t.eq_ignore_ascii_case("<svg")) =>
            {
                let start = range.start + held[range.start..].find('<').unwrap_or(0);
                match tokens(&held[start..]) {
                    Ok((_, end)) => {
                        skip_to = start + end;
                        found.source = Some(Source::Inline {
                            block: start..start + end,
                        });
                    }
                    Err(why) => found.problem(Some(start), format!("{why:#}")),
                }
            }
            Event::Start(Tag::Heading { level, .. }) if nest == 0 && Some(*level) == shallowest => {
                let mut name = String::new();
                while let Some((inner, _)) = events.get(i) {
                    i += 1;
                    match inner {
                        Event::End(TagEnd::Heading(_)) => break,
                        Event::Text(t) | Event::Code(t) => name.push_str(t),
                        Event::SoftBreak | Event::HardBreak => name.push(' '),
                        _ => {}
                    }
                }
                found.scenes.push(Scene {
                    name: Some(name.trim().to_string()),
                    steps: Vec::new(),
                    listed: false,
                });
            }
            Event::Start(Tag::Paragraph) if nest == 0 => paragraph = Some(range.clone()),
            Event::End(TagEnd::Paragraph) if nest == 0 => paragraph = None,
            Event::Start(Tag::List(Some(_))) if nest == 0 => {
                nest += 1;
                if let Some(scene) = found.scenes.last_mut()
                    && !scene.listed
                {
                    scene.listed = true;
                    in_steps = true;
                }
            }
            Event::Start(tag) if nests(tag) => nest += 1,
            Event::End(tag) if unnests(tag) => {
                nest = nest.saturating_sub(1);
                if nest == 0 {
                    in_steps = false;
                }
            }
            Event::Start(Tag::Item) if in_steps && nest == 1 => {
                if let Some(scene) = found.scenes.last_mut() {
                    scene.steps.push(Vec::new());
                }
            }
            Event::Start(Tag::Image { dest_url: path, .. })
                if nest == 0 && found.source.is_none() =>
            {
                let mut alt = String::new();
                while let Some((inner, _)) = events.get(i) {
                    i += 1;
                    match inner {
                        Event::End(TagEnd::Image) => break,
                        Event::Text(t) | Event::Code(t) => alt.push_str(t),
                        _ => {}
                    }
                }
                let block = paragraph.clone().unwrap_or_else(|| range.clone());
                let around = format!(
                    "{}{}",
                    &held[block.start..range.start],
                    &held[range.end..block.end]
                );
                if !around.trim().is_empty() {
                    found.problem(
                        Some(range.start),
                        "the picture of a scene stands alone in its paragraph".to_string(),
                    );
                }
                found.source = Some(Source::Picture {
                    path: path.to_string(),
                    alt: alt.trim().to_string(),
                    block,
                });
            }
            Event::Html(_) | Event::InlineHtml(_) => {
                let Some(at) = comment_in(held, range) else {
                    continue;
                };
                match annotation::from_html(&held[at.clone()]) {
                    Some(Ok(note)) if note.kind == Kind::Light => {
                        let step = found
                            .scenes
                            .last_mut()
                            .filter(|_| in_steps && nest >= 1)
                            .and_then(|scene| scene.steps.last_mut());
                        let words = note
                            .get("parts")
                            .unwrap_or_default()
                            .split_whitespace()
                            .map(str::to_string)
                            .collect();
                        match step {
                            Some(cues) => cues.push(Cue { range: at, words }),
                            None => found.problem(
                                Some(at.start),
                                "a light outside a step: write it in an item of the list"
                                    .to_string(),
                            ),
                        }
                    }
                    Some(Err(why))
                        if annotation::comment(&held[at.clone()])
                            .is_some_and(|b| b.trim().starts_with("site: light")) =>
                    {
                        found.problem(Some(at.start), format!("site annotation: {why}"));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    if found.source.is_none() {
        found.problem(
            None,
            "a scene needs a picture or an svg element, the figure that its steps light"
                .to_string(),
        );
    }
    if found.played().next().is_none() {
        found.problem(
            None,
            "a scene needs an ordered list, whose items are its steps".to_string(),
        );
    }
    found
}

/// The parts lit after each step: a step turns off before it lights.
/// `-*` turns off every part, `-id` turns off 1 part, and `id` lights it.
fn states(steps: &[Vec<String>]) -> Vec<BTreeSet<String>> {
    let mut lit: BTreeSet<String> = BTreeSet::new();
    steps
        .iter()
        .map(|words| {
            if words.iter().any(|w| w == "-*") {
                lit.clear();
            }
            for word in words {
                if let Some(off) = word.strip_prefix('-') {
                    lit.remove(off);
                }
            }
            for word in words.iter().filter(|w| !w.starts_with('-')) {
                lit.insert(word.clone());
            }
            lit.clone()
        })
        .collect()
}

/// The words of each step of a scene.
fn words(scene: &Scene) -> Vec<Vec<String>> {
    scene
        .steps
        .iter()
        .map(|cues| cues.iter().flat_map(|c| c.words.iter().cloned()).collect())
        .collect()
}

/// Whether the blocks are those of a scene that the site has already written.
fn written(held: &str) -> bool {
    held.contains("<figure class=\"dkt-stage\"")
}

/// The text of the figure of a scene, or why it cannot be read.
fn figure_text(held: &str, source: &Source, dir: Option<&Path>) -> Result<String> {
    match source {
        Source::Inline { block } => Ok(held[block.clone()].to_string()),
        Source::Picture { path, .. } => {
            if !path.to_ascii_lowercase().ends_with(".svg") {
                bail!("the picture of a scene is an SVG file, not {path}");
            }
            let file = dir.map_or_else(|| path.into(), |d| d.join(path));
            std::fs::read_to_string(&file)
                .with_context(|| format!("the scene names {path}, which cannot be read"))
        }
    }
}

/// The ids that the steps name and the figure lacks.
fn missing(found: &Analysis, ids: &BTreeSet<String>) -> Vec<Problem> {
    let mut out = Vec::new();
    for scene in found.played() {
        for (n, cues) in scene.steps.iter().enumerate() {
            for cue in cues {
                for word in &cue.words {
                    let (verb, id) = match word.strip_prefix('-') {
                        Some("*") => continue,
                        Some(id) => ("turns off", id),
                        None => ("lights", word.as_str()),
                    };
                    if ids.contains(id) {
                        continue;
                    }
                    let of = scene
                        .name
                        .as_ref()
                        .map(|name| format!(" of the scene '{name}'"))
                        .unwrap_or_default();
                    out.push(Problem {
                        offset: Some(cue.range.start),
                        message: format!(
                            "step {}{of} {verb} '{id}', but no part of the figure has that id",
                            n + 1
                        ),
                    });
                }
            }
        }
    }
    out
}

/// What `dkt lint` reports of a scene widget. A picture that does not exist is left to the
/// check of the links.
pub(crate) fn problems(held: &str, dir: &Path) -> Vec<Problem> {
    if written(held) {
        return Vec::new();
    }
    let mut found = analyse(held);
    let Some(source) = &found.source else {
        return found.problems;
    };
    if let Source::Picture { path, .. } = source
        && path.to_ascii_lowercase().ends_with(".svg")
        && !dir.join(path).exists()
    {
        return found.problems;
    }
    let svg = match figure_text(held, source, Some(dir)) {
        Ok(svg) => svg,
        Err(why) => {
            found.problem(None, format!("{why:#}"));
            return found.problems;
        }
    };
    match tokens(&svg) {
        Ok((list, _)) => {
            let more = missing(&found, &ids(&list));
            found.problems.extend(more);
            if matches!(source, Source::Inline { .. }) && name_of(&list).is_none() {
                found.problem(
                    None,
                    "the svg element of a scene needs a title or an aria-label, which names the figure"
                        .to_string(),
                );
            }
        }
        Err(why) => found.problem(None, format!("the figure does not read: {why:#}")),
    }
    found.problems
}

/// The blocks of a scene widget, with its figure written in place and its cues made into marks
/// that the script reads. `prefix` makes the ids of the figure unique in the page.
pub(crate) fn expand(held: &str, dir: Option<&Path>, prefix: &str) -> Result<String> {
    let found = analyse(held);
    if let Some(first) = found.problems.first() {
        bail!("{}", first.message);
    }
    let Some(source) = &found.source else {
        bail!("a scene needs a figure");
    };
    let svg = figure_text(held, source, dir)?;
    let (list, _) = tokens(&svg).context("the figure does not read")?;
    if let Some(first) = missing(&found, &ids(&list)).first() {
        bail!("{}", first.message);
    }
    let mut cast = BTreeSet::new();
    for scene in found.played() {
        for step in words(scene) {
            cast.extend(step.into_iter().filter(|w| !w.starts_with('-')));
        }
    }
    // Without the script, the figure shows what the first scene leaves lit.
    let end = found
        .played()
        .next()
        .and_then(|scene| states(&words(scene)).pop())
        .unwrap_or_default();
    let (alt, block) = match source {
        Source::Picture { alt, block, .. } => (alt.clone(), block.clone()),
        Source::Inline { block } => (String::new(), block.clone()),
    };
    let figure = figure(&list, prefix, &alt, &cast, &end);
    let mut edits: Vec<(Range<usize>, String)> = vec![(block, format!("\n{figure}\n\n"))];
    for scene in found.played() {
        for cue in scene.steps.iter().flatten() {
            edits.push((
                cue.range.clone(),
                format!(
                    "<span class=\"dkt-cue\" data-cue=\"{}\" hidden></span>",
                    cue.words.join(" ")
                ),
            ));
        }
    }
    edits.sort_by_key(|(range, _)| range.start);
    let mut out = String::with_capacity(held.len() + svg.len());
    let mut at = 0;
    for (range, text) in edits {
        out.push_str(&held[at..range.start]);
        out.push_str(&text);
        at = range.end;
    }
    out.push_str(&held[at..]);
    Ok(out)
}

/// A piece of the text of an SVG figure.
#[derive(Debug)]
enum Token<'a> {
    Text(&'a str),
    /// The text of a `style` or a `script` element.
    Raw(&'a str),
    Cdata(&'a str),
    Tag(Element<'a>),
}

/// A tag, with its attributes as written.
#[derive(Debug)]
struct Element<'a> {
    name: &'a str,
    attrs: Vec<Attr<'a>>,
    closing: bool,
    empty: bool,
}

/// An attribute: its name, its value and the quote around it, `None` for an attribute with no value.
#[derive(Debug)]
struct Attr<'a> {
    name: &'a str,
    value: Option<(&'a str, char)>,
}

impl Element<'_> {
    fn is(&self, name: &str) -> bool {
        self.name.eq_ignore_ascii_case(name)
    }

    fn get(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case(name))
            .and_then(|a| a.value.map(|(v, _)| v))
    }
}

fn is_tag_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '-' | '_' | ':' | '.')
}

/// Reads a tag at the start of `s`, and returns it with its length.
fn tag(s: &str) -> Result<(Element<'_>, usize)> {
    let mut i = 1;
    let closing = s[i..].starts_with('/');
    if closing {
        i += 1;
    }
    let start = i;
    i += s[i..]
        .find(|c: char| !is_tag_char(c))
        .unwrap_or(s.len() - i);
    if i == start {
        bail!("a tag has no name");
    }
    let name = &s[start..i];
    let mut attrs = Vec::new();
    loop {
        i += s[i..].len() - s[i..].trim_start().len();
        let rest = &s[i..];
        if rest.starts_with("/>") {
            return Ok((
                Element {
                    name,
                    attrs,
                    closing,
                    empty: true,
                },
                i + 2,
            ));
        }
        if rest.starts_with('>') {
            return Ok((
                Element {
                    name,
                    attrs,
                    closing,
                    empty: false,
                },
                i + 1,
            ));
        }
        if rest.is_empty() {
            bail!("the tag {name} is never closed");
        }
        let len = rest
            .find(|c: char| c.is_whitespace() || matches!(c, '=' | '>' | '/'))
            .unwrap_or(rest.len());
        if len == 0 {
            bail!("the tag {name} holds a stray character");
        }
        let attr = &rest[..len];
        i += len;
        let after = s[i..].trim_start();
        if !after.starts_with('=') {
            attrs.push(Attr {
                name: attr,
                value: None,
            });
            continue;
        }
        i += s[i..].len() - after.len() + 1;
        i += s[i..].len() - s[i..].trim_start().len();
        let rest = &s[i..];
        match rest.chars().next() {
            Some(q @ ('"' | '\'')) => {
                let end = rest[1..]
                    .find(q)
                    .with_context(|| format!("the value of {attr} is never closed"))?;
                attrs.push(Attr {
                    name: attr,
                    value: Some((&rest[1..1 + end], q)),
                });
                i += end + 2;
            }
            _ => {
                let end = rest
                    .find(|c: char| c.is_whitespace() || c == '>')
                    .unwrap_or(rest.len());
                attrs.push(Attr {
                    name: attr,
                    value: Some((&rest[..end], '"')),
                });
                i += end;
            }
        }
    }
}

/// The pieces of the first `svg` element of a text, and the end of that element.
/// What comes before it, a declaration, a doctype or a comment, is left out, and so are the
/// comments inside it.
fn tokens(s: &str) -> Result<(Vec<Token<'_>>, usize)> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut i = 0;
    while i < s.len() {
        let rest = &s[i..];
        let Some(lt) = rest.find('<') else {
            break;
        };
        if lt > 0 {
            if depth > 0 {
                out.push(Token::Text(&rest[..lt]));
            }
            i += lt;
            continue;
        }
        if let Some(body) = rest.strip_prefix("<![CDATA[") {
            let end = body
                .find("]]>")
                .context("a CDATA section is never closed")?;
            if depth > 0 {
                out.push(Token::Cdata(&body[..end]));
            }
            i += 9 + end + 3;
            continue;
        }
        if rest.starts_with("<!--") {
            i += rest.find("-->").context("a comment is never closed")? + 3;
            continue;
        }
        if rest.starts_with("<!") || rest.starts_with("<?") {
            i += rest.find('>').context("a declaration is never closed")? + 1;
            continue;
        }
        let (element, len) = tag(rest)?;
        i += len;
        if depth == 0 && !element.is("svg") {
            bail!(
                "the figure starts with {}, not with an svg element",
                element.name
            );
        }
        let raw =
            !element.closing && !element.empty && (element.is("style") || element.is("script"));
        let name = element.name;
        if element.closing {
            depth = depth.saturating_sub(1);
        } else if !element.empty {
            depth += 1;
        }
        out.push(Token::Tag(element));
        if raw {
            let lower = s[i..].to_ascii_lowercase();
            let close = lower
                .find(&format!("</{}", name.to_ascii_lowercase()))
                .with_context(|| format!("the element {name} is never closed"))?;
            out.push(Token::Raw(&s[i..i + close]));
            i += close;
        }
        if depth == 0 {
            return Ok((out, i));
        }
    }
    if out.is_empty() {
        bail!("the figure holds no svg element");
    }
    bail!("the svg element is never closed")
}

/// The ids of the elements of a figure.
fn ids(tokens: &[Token<'_>]) -> BTreeSet<String> {
    tokens
        .iter()
        .filter_map(|t| match t {
            Token::Tag(e) if !e.closing => e.get("id").map(str::to_string),
            _ => None,
        })
        .collect()
}

/// What names a figure written in the widget: the text of its first `title`, or its `aria-label`.
fn name_of(tokens: &[Token<'_>]) -> Option<String> {
    if let Some(Token::Tag(root)) = tokens.first()
        && let Some(label) = root.get("aria-label").filter(|l| !l.trim().is_empty())
    {
        return Some(label.to_string());
    }
    let at = tokens
        .iter()
        .position(|t| matches!(t, Token::Tag(e) if e.is("title") && !e.closing))?;
    match tokens.get(at + 1) {
        Some(Token::Text(text)) if !text.trim().is_empty() => Some(text.trim().to_string()),
        _ => None,
    }
}

/// The names of a figure, with the prefix they take in the page.
struct Rename<'a> {
    prefix: &'a str,
    ids: BTreeSet<String>,
    frames: BTreeSet<String>,
}

impl Rename<'_> {
    fn id(&self, id: &str) -> String {
        if self.ids.contains(id) {
            format!("{}-{id}", self.prefix)
        } else {
            id.to_string()
        }
    }

    fn frame(&self, name: &str) -> String {
        if self.frames.contains(name) {
            format!("{}-{name}", self.prefix)
        } else {
            name.to_string()
        }
    }

    /// A value with each `url(#id)` pointing at the renamed id.
    fn urls(&self, value: &str) -> String {
        let mut out = String::with_capacity(value.len());
        let mut rest = value;
        while let Some(at) = rest.find("url(") {
            out.push_str(&rest[..at + 4]);
            rest = &rest[at + 4..];
            let Some(close) = rest.find(')') else {
                break;
            };
            let inner = rest[..close].trim().trim_matches(['"', '\'']);
            match inner.strip_prefix('#') {
                Some(id) if self.ids.contains(id) => {
                    let _ = write!(out, "#{}", self.id(id));
                }
                _ => out.push_str(&rest[..close]),
            }
            rest = &rest[close..];
        }
        out.push_str(rest);
        out
    }

    /// The words of an animation with the names of the keyframes renamed.
    fn animation(&self, value: &str) -> String {
        let mut out = String::with_capacity(value.len());
        let mut word = String::new();
        for c in value.chars().chain(std::iter::once('\0')) {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                word.push(c);
                continue;
            }
            out.push_str(&self.frame(&word));
            word.clear();
            if c != '\0' {
                out.push(c);
            }
        }
        out
    }

    /// Declarations of style, with their `url(#id)` and their animations renamed.
    fn declarations(&self, body: &str) -> String {
        split_top(body, ';')
            .into_iter()
            .map(|declaration| match declaration.split_once(':') {
                Some((property, value))
                    if property.trim().to_ascii_lowercase().ends_with("animation")
                        || property
                            .trim()
                            .to_ascii_lowercase()
                            .ends_with("animation-name") =>
                {
                    format!("{property}:{}", self.urls(&self.animation(value)))
                }
                _ => self.urls(declaration),
            })
            .collect::<Vec<_>>()
            .join(";")
    }

    /// A selector with its ids renamed, under the figure.
    fn selector(&self, selector: &str) -> String {
        let scope = format!(":where(#{})", self.prefix);
        let mut out = String::with_capacity(selector.len());
        let mut chars = selector.char_indices().peekable();
        let mut quote = None;
        while let Some((_, c)) = chars.next() {
            match quote {
                Some(q) => {
                    out.push(c);
                    if c == q {
                        quote = None;
                    }
                }
                None if c == '"' || c == '\'' => {
                    quote = Some(c);
                    out.push(c);
                }
                None if c == '#' => {
                    let mut name = String::new();
                    while let Some(&(_, d)) = chars.peek() {
                        if !(d.is_alphanumeric() || d == '-' || d == '_') {
                            break;
                        }
                        name.push(d);
                        chars.next();
                    }
                    out.push('#');
                    out.push_str(&self.id(&name));
                }
                None => out.push(c),
            }
        }
        match out.strip_prefix(":root") {
            Some(rest) => format!("{scope}{rest}"),
            None => format!("{scope} {out}"),
        }
    }

    /// A style sheet whose rules apply under the figure only.
    fn sheet(&self, css: &str) -> String {
        let mut out = String::new();
        self.rules(&strip_comments(css), &mut out);
        out
    }

    fn rules(&self, css: &str, out: &mut String) {
        let mut rest = css.trim_start();
        while !rest.is_empty() {
            let Some((at, c)) = find_top(rest, &['{', ';']) else {
                break;
            };
            if c == ';' {
                // A statement such as `@import` would reach beyond the figure, and is left out.
                rest = rest[at + 1..].trim_start();
                continue;
            }
            let prelude = rest[..at].trim();
            let Some(close) = closing_brace(rest, at) else {
                break;
            };
            let body = &rest[at + 1..close];
            rest = rest[close + 1..].trim_start();
            if let Some(at_rule) = prelude.strip_prefix('@') {
                let word: String = at_rule
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '-')
                    .collect();
                if word.ends_with("keyframes") {
                    let name = at_rule[word.len()..].trim().trim_matches(['"', '\'']);
                    let _ = write!(out, "@{word} {}{{{body}}}", self.frame(name));
                } else if matches!(word.as_str(), "media" | "supports" | "container" | "layer") {
                    let _ = write!(out, "{prelude}{{");
                    self.rules(body, out);
                    out.push('}');
                } else {
                    let _ = write!(out, "{prelude}{{{}}}", self.declarations(body));
                }
                continue;
            }
            let selectors: Vec<String> = split_top(prelude, ',')
                .into_iter()
                .filter(|s| !s.trim().is_empty())
                .map(|s| self.selector(s.trim()))
                .collect();
            let _ = write!(
                out,
                "{}{{{}}}",
                selectors.join(","),
                self.declarations(body)
            );
        }
    }
}

fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(at) = rest.find("/*") {
        out.push_str(&rest[..at]);
        match rest[at + 2..].find("*/") {
            Some(end) => rest = &rest[at + 2 + end + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// The first of `stops` outside quotes and parentheses, with its position.
fn find_top(text: &str, stops: &[char]) -> Option<(usize, char)> {
    let mut quote = None;
    let mut parens = 0usize;
    for (i, c) in text.char_indices() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None => match c {
                '"' | '\'' => quote = Some(c),
                '(' => parens += 1,
                ')' => parens = parens.saturating_sub(1),
                c if parens == 0 && stops.contains(&c) => return Some((i, c)),
                _ => {}
            },
        }
    }
    None
}

/// The brace that closes the one at `open`.
fn closing_brace(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote = None;
    for (i, c) in text[open..].char_indices() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None => match c {
                '"' | '\'' => quote = Some(c),
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(open + i);
                    }
                }
                _ => {}
            },
        }
    }
    None
}

/// The parts of a text between the `sep` outside quotes and parentheses, the last one included
/// even when it is empty, so that joining them gives the text back.
fn split_top(text: &str, sep: char) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some((at, _)) = find_top(rest, &[sep]) {
        out.push(&rest[..at]);
        rest = &rest[at + 1..];
    }
    out.push(rest);
    out
}

/// The names of the keyframes that the style of a figure defines.
fn frames(tokens: &[Token<'_>]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for token in tokens {
        let Token::Raw(css) = token else { continue };
        let mut rest = *css;
        while let Some(at) = rest.find("keyframes") {
            rest = &rest[at + 9..];
            let name: String = rest
                .trim_start()
                .trim_start_matches(['"', '\''])
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect();
            if !name.is_empty() {
                out.insert(name);
            }
        }
    }
    out
}

/// The text of a style element of the figure, as CSS: its CDATA markers and its entities undone.
fn css_of(raw: &str) -> String {
    if raw.contains("<![CDATA[") {
        raw.replace("<![CDATA[", "").replace("]]>", "")
    } else {
        crate::html::decode_entities(raw)
    }
}

/// An attribute as written in the page, or `None` when it is left out.
fn attribute(attr: &Attr<'_>, names: &Rename<'_>) -> Option<String> {
    let name = attr.name;
    let lower = name.to_ascii_lowercase();
    if lower.starts_with("on") {
        return None;
    }
    let Some((value, quote)) = attr.value else {
        return Some(name.to_string());
    };
    let value = match lower.as_str() {
        "id" => names.id(value),
        "href" | "xlink:href" => {
            if value.trim().to_ascii_lowercase().starts_with("javascript:") {
                return None;
            }
            match value.strip_prefix('#') {
                Some(id) => format!("#{}", names.id(id)),
                None => value.to_string(),
            }
        }
        "aria-labelledby"
        | "aria-describedby"
        | "aria-controls"
        | "aria-owns"
        | "aria-flowto"
        | "aria-details"
        | "aria-activedescendant" => value
            .split_whitespace()
            .map(|id| names.id(id))
            .collect::<Vec<_>>()
            .join(" "),
        // The time of an animation may start from another one, as `fade.end`.
        "begin" | "end" => value
            .split(';')
            .map(|part| match part.trim().split_once('.') {
                Some((id, event)) if names.ids.contains(id) => {
                    format!("{}.{event}", names.id(id))
                }
                _ => part.to_string(),
            })
            .collect::<Vec<_>>()
            .join(";"),
        "style" => names.declarations(value),
        _ => names.urls(value),
    };
    Some(format!("{name}={quote}{value}{quote}"))
}

/// The figure of a scene as HTML: the `svg` element with its ids prefixed and its style scoped,
/// in a `figure` element.
/// The parts that the steps name carry `data-dkt-part`, and those lit at the end of the first
/// scene carry `data-dkt-end`.
fn figure(
    tokens: &[Token<'_>],
    prefix: &str,
    alt: &str,
    cast: &BTreeSet<String>,
    end: &BTreeSet<String>,
) -> String {
    let names = Rename {
        prefix,
        ids: ids(tokens),
        frames: frames(tokens),
    };
    let mut style = String::new();
    let mut svg = String::new();
    // The element whose text is being read: a style that is kept apart, or a script left out.
    let mut raw: Option<bool> = None;
    let mut root = true;
    for token in tokens {
        match token {
            Token::Text(text) => svg.push_str(text),
            Token::Cdata(text) => {
                let _ = write!(svg, "<![CDATA[{text}]]>");
            }
            Token::Raw(text) => {
                if raw == Some(true) {
                    style.push_str(&names.sheet(&css_of(text)));
                }
            }
            Token::Tag(element) if element.is("style") || element.is("script") => {
                raw = (!element.closing && !element.empty).then(|| element.is("style"));
            }
            Token::Tag(element) => {
                let _ = write!(
                    svg,
                    "<{}{}",
                    if element.closing { "/" } else { "" },
                    element.name
                );
                let labelled = root && !alt.is_empty();
                for attr in &element.attrs {
                    let lower = attr.name.to_ascii_lowercase();
                    if labelled && (lower == "aria-label" || lower == "aria-labelledby") {
                        continue;
                    }
                    if let Some(text) = attribute(attr, &names) {
                        let _ = write!(svg, " {text}");
                    }
                }
                if let Some(id) = element.get("id").filter(|_| !element.closing) {
                    if cast.contains(id) {
                        let _ = write!(svg, " data-dkt-part=\"{id}\"");
                    }
                    if end.contains(id) {
                        svg.push_str(" data-dkt-end");
                    }
                }
                if root && !element.closing {
                    root = false;
                    if element.get("role").is_none() {
                        svg.push_str(" role=\"img\"");
                    }
                    if !alt.is_empty() {
                        let _ = write!(svg, " aria-label=\"{}\"", crate::escape::html(alt));
                    }
                    if element.get("viewbox").is_none() {
                        let size = |key| {
                            element
                                .get(key)
                                .map(|v| v.trim().trim_end_matches("px").to_string())
                                .filter(|v| v.parse::<f64>().is_ok())
                        };
                        if let (Some(w), Some(h)) = (size("width"), size("height")) {
                            let _ = write!(svg, " viewBox=\"0 0 {w} {h}\"");
                        }
                    }
                }
                svg.push_str(if element.empty { "/>" } else { ">" });
            }
        }
    }
    let mut out = format!("<figure class=\"dkt-stage\" id=\"{prefix}\">\n");
    if !style.is_empty() {
        let _ = writeln!(out, "<style>{}</style>", style.replace("</", "<\\/"));
    }
    out.push_str(&svg);
    out.push_str("\n</figure>");
    // A blank line would end the block of HTML in the Markdown.
    out.lines()
        .filter(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SVG: &str = r##"<?xml version="1.0"?>
<!-- drawn by hand -->
<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100" onload="alert(1)">
  <title>A machine</title>
  <style><![CDATA[
    .box { fill: #eee; }
    #gate { animation: pulse 2s infinite; }
    @keyframes pulse { 50% { opacity: .6 } }
    @import url(other.css);
  ]]></style>
  <defs><linearGradient id="shade"><stop offset="0"/></linearGradient></defs>
  <g id="gate" class="box"><rect width="40" height="40" fill="url(#shade)"/></g>
  <g id="core"><circle r="10"/></g>

  <path id="wire" d="M0 0L10 10" stroke="black" fill="none"/>
  <use href="#core" x="50"/>
  <script>alert(2)</script>
</svg>
"##;

    fn scene(steps: &str) -> String {
        format!("![The machine](m.svg)\n\n{steps}")
    }

    #[test]
    fn a_step_turns_off_before_it_lights() {
        let steps: Vec<Vec<String>> = [
            vec!["gate"],
            vec!["core", "wire"],
            vec!["-gate"],
            vec!["-*", "gate"],
            vec!["gate", "-gate"],
            vec![],
        ]
        .iter()
        .map(|s| s.iter().map(|w| w.to_string()).collect())
        .collect();
        let lit: Vec<Vec<String>> = states(&steps)
            .into_iter()
            .map(|s| s.into_iter().collect())
            .collect();
        assert_eq!(lit[0], ["gate"]);
        assert_eq!(lit[1], ["core", "gate", "wire"]);
        assert_eq!(lit[2], ["core", "wire"]);
        assert_eq!(lit[3], ["gate"]);
        assert_eq!(lit[4], ["gate"]);
        assert_eq!(lit[5], ["gate"]);
    }

    #[test]
    fn the_scenes_are_the_lists_under_the_headings() {
        let held = "![The machine](m.svg)\n\n### First\n\n1. One <!-- site: light gate -->\n2. Two\n   <!-- site: light core -gate -->\n\nA note.\n\n1. Not a step <!-- site: light wire -->\n\n### Second\n\n- not steps\n\n1. Alone\n";
        let found = analyse(held);
        let names: Vec<_> = found.scenes.iter().map(|s| s.name.clone()).collect();
        assert_eq!(names, [Some("First".into()), Some("Second".into())]);
        assert_eq!(
            words(&found.scenes[0]),
            [vec!["gate"], vec!["core", "-gate"]]
        );
        assert_eq!(found.scenes[1].steps.len(), 1);
        assert_eq!(found.problems.len(), 1, "{:?}", found.problems);
        assert!(found.problems[0].message.contains("a light outside a step"));
        assert_eq!(&held[found.problems[0].offset.unwrap()..][..4], "<!--");
    }

    #[test]
    fn a_widget_with_no_heading_is_1_scene() {
        let found = analyse(&scene("1. A <!-- site: light gate -->\n2. B\n"));
        assert!(found.problems.is_empty(), "{:?}", found.problems);
        assert_eq!(found.scenes.len(), 1);
        assert_eq!(words(&found.scenes[0]), [vec!["gate"], vec![]]);
        assert!(
            matches!(found.source, Some(Source::Picture { ref alt, .. }) if alt == "The machine")
        );
    }

    #[test]
    fn a_scene_without_a_figure_or_steps_is_refused() {
        let found = analyse("Some text.\n");
        let messages: Vec<_> = found.problems.iter().map(|p| p.message.as_str()).collect();
        assert!(messages[0].contains("needs a picture or an svg element"));
        assert!(messages[1].contains("needs an ordered list"));
        let found = analyse("Look: ![x](m.svg)\n\n1. A\n");
        assert!(found.problems[0].message.contains("stands alone"));
        let found = analyse(&scene("1. A <!-- site: light a/b -->\n"));
        assert!(found.problems[0].message.contains("'a/b' is not a part"));
    }

    #[test]
    fn the_ids_a_step_names_are_in_the_figure() {
        let dir = std::env::temp_dir().join(format!("dkt-scene-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("m.svg"), SVG).unwrap();
        let held = "![The machine](m.svg)\n\n### Boot\n\n1. A <!-- site: light gate -->\n2. B <!-- site: light coer -gaet -->\n";
        let found = problems(held, &dir);
        let messages: Vec<_> = found.iter().map(|p| p.message.as_str()).collect();
        assert_eq!(
            messages,
            [
                "step 2 of the scene 'Boot' lights 'coer', but no part of the figure has that id",
                "step 2 of the scene 'Boot' turns off 'gaet', but no part of the figure has that id",
            ]
        );
        let err = expand(held, Some(&dir), "p").unwrap_err().to_string();
        assert!(err.contains("lights 'coer'"), "{err}");
        // A picture that does not exist is left to the check of the links.
        assert!(problems("![x](gone.svg)\n\n1. A <!-- site: light x -->\n", &dir).is_empty());
        let found = problems("![x](m.png)\n\n1. A\n", &dir);
        assert!(found[0].message.contains("is an SVG file"), "{found:?}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_inline_figure_needs_a_name() {
        let dir = Path::new(".");
        let held = "<svg viewBox=\"0 0 10 10\">\n<rect id=\"a\"/>\n\n<rect id=\"b\"/>\n</svg>\n\n1. A <!-- site: light a b -->\n";
        let found = problems(held, dir);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].message.contains("needs a title"));
        let named = held.replace("<svg ", "<svg aria-label=\"Two boxes\" ");
        assert!(problems(&named, dir).is_empty());
        let out = expand(&named, None, "dkt-f1").unwrap();
        assert!(
            out.contains("<rect id=\"dkt-f1-b\" data-dkt-part=\"b\" data-dkt-end/>"),
            "{out}"
        );
        assert!(
            !out.contains("\n\n<rect"),
            "the blank line inside is gone: {out}"
        );
    }

    #[test]
    fn the_figure_is_written_with_its_names_prefixed() {
        let dir = std::env::temp_dir().join(format!("dkt-scene-w-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("m.svg"), SVG).unwrap();
        let held = scene("1. A <!-- site: light gate -->\n2. B <!-- site: light wire -gate -->\n");
        let out = expand(&held, Some(&dir), "dkt-f9").unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(
            out.starts_with("\n<figure class=\"dkt-stage\" id=\"dkt-f9\">\n<style>"),
            "{out}"
        );
        // The style applies under the figure, with its ids and its keyframes renamed.
        assert!(out.contains(":where(#dkt-f9) .box{ fill: #eee; }"), "{out}");
        assert!(
            out.contains(":where(#dkt-f9) #dkt-f9-gate{ animation: dkt-f9-pulse 2s infinite; }"),
            "{out}"
        );
        assert!(
            out.contains("@keyframes dkt-f9-pulse{ 50% { opacity: .6 } }"),
            "{out}"
        );
        assert!(!out.contains("@import"), "{out}");
        // The root names the figure and gets a box to scale in.
        assert!(
            out.contains("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"200\" height=\"100\" role=\"img\" aria-label=\"The machine\" viewBox=\"0 0 200 100\">"),
            "{out}"
        );
        assert!(
            out.contains("<g id=\"dkt-f9-gate\" class=\"box\" data-dkt-part=\"gate\">"),
            "{out}"
        );
        assert!(out.contains("fill=\"url(#dkt-f9-shade)\""), "{out}");
        assert!(out.contains("<use href=\"#dkt-f9-core\""), "{out}");
        assert!(out.contains("<path id=\"dkt-f9-wire\" d=\"M0 0L10 10\" stroke=\"black\" fill=\"none\" data-dkt-part=\"wire\" data-dkt-end/>"), "{out}");
        assert!(
            out.contains("<g id=\"dkt-f9-core\">"),
            "a part no step names: {out}"
        );
        assert!(
            !out.contains("alert") && !out.contains("<?xml") && !out.contains("drawn by hand"),
            "{out}"
        );
        // The cues are marks that the script reads.
        assert!(
            out.contains("1. A <span class=\"dkt-cue\" data-cue=\"gate\" hidden></span>"),
            "{out}"
        );
        assert!(out.contains("data-cue=\"wire -gate\""), "{out}");
        // The figure is 1 block of HTML, followed by the list.
        let figure = &out[..out.find("1. A").unwrap()];
        assert!(!figure.trim().contains("\n\n"), "{figure}");
        assert!(
            figure.trim_end().ends_with("</figure>") && figure.ends_with("\n\n"),
            "{figure}"
        );
    }

    #[test]
    fn a_figure_that_does_not_read_is_named() {
        assert!(
            tokens("<g/>")
                .unwrap_err()
                .to_string()
                .contains("not with an svg")
        );
        assert!(
            tokens("<svg><g>")
                .unwrap_err()
                .to_string()
                .contains("never closed")
        );
        assert!(tokens("<svg><g a=\"x></svg>").is_err());
        let (list, end) = tokens("<svg><svg/></svg>tail").unwrap();
        assert_eq!(end, 17);
        assert_eq!(list.len(), 3);
    }
}
