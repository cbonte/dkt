//! The checks themselves. Each one walks the parsed document and reports findings.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::slug::github_anchor;
use crate::source::LineIndex;
use crate::tree::parser_options;
use pulldown_cmark::{BrokenLink, CodeBlockKind, Event, LinkType, Parser, Tag, TagEnd};
use std::cell::RefCell;

use crate::cmd::lint::finding::{Finding, Rule};

/// Everything a check needs about one file.
pub struct Source<'a> {
    pub path: &'a Path,
    pub text: &'a str,
    pub index: LineIndex,
}

impl<'a> Source<'a> {
    pub fn new(path: &'a Path, text: &'a str) -> Source<'a> {
        Source {
            path,
            text,
            index: LineIndex::new(text),
        }
    }

    fn at(&self, offset: usize) -> (usize, usize) {
        self.index.line_col(self.text, offset)
    }

    fn finding(&self, offset: usize, rule: Rule, message: String) -> Finding {
        let (line, column) = self.at(offset);
        Finding {
            line,
            column,
            rule,
            message,
        }
    }
}

/// Anchors a Markdown file offers: its heading anchors plus explicit HTML names.
///
/// A heading inside a block quote, a list or a footnote has no anchor, as on GitHub.
/// A link in a heading gives its text to the anchor, not its address.
pub fn anchors(text: &str) -> HashSet<String> {
    let mut set = heading_anchors(text);
    for tag in crate::html::tokenize(text) {
        if let crate::html::Token::Tag(tag) = tag {
            for name in ["id", "name"] {
                if let Some(value) = tag.attr(name) {
                    set.insert(value);
                }
            }
        }
    }
    set
}

/// The anchors of the headings of a Markdown file.
fn heading_anchors(text: &str) -> HashSet<String> {
    let mut set = HashSet::new();
    let mut seen = HashMap::new();
    let mut containers = 0usize;
    let mut heading: Option<String> = None;
    for event in Parser::new_ext(text, parser_options()) {
        match event {
            Event::Start(Tag::BlockQuote(_) | Tag::List(_) | Tag::FootnoteDefinition(_)) => {
                containers += 1;
            }
            Event::End(TagEnd::BlockQuote(_) | TagEnd::List(_) | TagEnd::FootnoteDefinition) => {
                containers -= 1;
            }
            Event::Start(Tag::Heading { .. }) if containers == 0 => heading = Some(String::new()),
            Event::End(TagEnd::Heading(_)) => {
                if let Some(words) = heading.take() {
                    set.insert(github_anchor(&words, &mut seen));
                }
            }
            Event::Text(words) | Event::Code(words) => {
                if let Some(heading) = heading.as_mut() {
                    heading.push_str(&words);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(heading) = heading.as_mut() {
                    heading.push(' ');
                }
            }
            _ => {}
        }
    }
    set
}

/// Splits a link target into its path part and its anchor.
fn split_target(dest: &str) -> (&str, Option<&str>) {
    match dest.split_once('#') {
        Some((path, anchor)) => (path, Some(anchor)),
        None => (dest, None),
    }
}

/// True for a target that points outside the file tree.
fn is_external(dest: &str) -> bool {
    dest.contains("://") || dest.starts_with("mailto:") || dest.starts_with("tel:")
}

/// Percent-decodes the few sequences a local path can carry.
fn decode_path(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

/// The paths that the links and the images of a document point at inside the file tree, each with
/// the line and the column where it is written. The query and the anchor are left out.
pub fn local_targets(path: &Path, text: &str) -> Vec<(usize, usize, String)> {
    let src = Source::new(path, text);
    let mut out = Vec::new();
    for (event, range) in Parser::new_ext(text, parser_options()).into_offset_iter() {
        let (dest, link_type) = match &event {
            Event::Start(Tag::Link {
                dest_url,
                link_type,
                ..
            })
            | Event::Start(Tag::Image {
                dest_url,
                link_type,
                ..
            }) => (dest_url.to_string(), *link_type),
            _ => continue,
        };
        if matches!(link_type, LinkType::Email) || is_external(&dest) || dest.is_empty() {
            continue;
        }
        let (path_part, _) = split_target(&dest);
        let path_part = path_part.split('?').next().unwrap_or(path_part);
        if path_part.is_empty() {
            continue;
        }
        let (line, col) = src.at(range.start);
        out.push((line, col, decode_path(path_part)));
    }
    out
}

/// The addresses that the links and the images of a document point at, each with the line and the
/// column where it is written.
pub fn external_targets(path: &Path, text: &str) -> Vec<(usize, usize, String)> {
    let src = Source::new(path, text);
    let mut out = Vec::new();
    for (event, range) in Parser::new_ext(text, parser_options()).into_offset_iter() {
        let (Event::Start(Tag::Link {
            dest_url,
            link_type,
            ..
        })
        | Event::Start(Tag::Image {
            dest_url,
            link_type,
            ..
        })) = &event
        else {
            continue;
        };
        if matches!(link_type, LinkType::Email) || !dest_url.contains("://") {
            continue;
        }
        let (line, col) = src.at(range.start);
        out.push((line, col, dest_url.to_string()));
    }
    out
}

/// Checks every link and image target that stays inside the file tree.
fn check_targets(src: &Source<'_>, enabled: &HashSet<Rule>, out: &mut Vec<Finding>) {
    let dir = src.path.parent().unwrap_or(Path::new("."));
    let mut cache: HashMap<PathBuf, HashSet<String>> = HashMap::new();
    let own_anchors = anchors(src.text);
    for (event, range) in Parser::new_ext(src.text, parser_options()).into_offset_iter() {
        let (dest, link_type, is_image) = match &event {
            Event::Start(Tag::Link {
                dest_url,
                link_type,
                ..
            }) => (dest_url.to_string(), *link_type, false),
            Event::Start(Tag::Image {
                dest_url,
                link_type,
                ..
            }) => (dest_url.to_string(), *link_type, true),
            _ => continue,
        };
        // An email autolink has no scheme in its target, so it would look like a path.
        if matches!(link_type, LinkType::Email) || is_external(&dest) || dest.is_empty() {
            continue;
        }
        let (path_part, anchor) = split_target(&dest);
        let path_part = path_part.split('?').next().unwrap_or(path_part);
        let target = if path_part.is_empty() {
            src.path.to_path_buf()
        } else {
            dir.join(decode_path(path_part))
        };
        if !path_part.is_empty() && enabled.contains(&Rule::LinkFile) && !target.exists() {
            out.push(src.finding(
                range.start,
                Rule::LinkFile,
                format!("{path_part} does not exist"),
            ));
            continue;
        }
        let Some(anchor) = anchor.filter(|a| !a.is_empty()) else {
            continue;
        };
        if is_image || !enabled.contains(&Rule::LinkAnchor) {
            continue;
        }
        let known = if path_part.is_empty() {
            Some(&own_anchors)
        } else if target.extension().is_some_and(|e| e == "md") {
            let entry = cache.entry(target.clone()).or_insert_with(|| {
                std::fs::read_to_string(&target)
                    .map(|t| anchors(&t))
                    .unwrap_or_default()
            });
            Some(&*entry)
        } else {
            None
        };
        if let Some(known) = known
            && !known.contains(anchor)
        {
            let where_ = if path_part.is_empty() {
                "this file".to_string()
            } else {
                path_part.to_string()
            };
            out.push(src.finding(
                range.start,
                Rule::LinkAnchor,
                format!("no heading '{anchor}' in {where_}"),
            ));
        }
    }
}

/// Checks reference links without a definition, and definitions nobody uses.
///
/// An undefined reference produces no link event, so it is collected through the broken
/// link callback. Returning `None` there keeps the text rendered as written.
fn check_references(src: &Source<'_>, out: &mut Vec<Finding>) {
    let defined: HashMap<String, usize> = Parser::new_ext(src.text, parser_options())
        .reference_definitions()
        .iter()
        .map(|(label, def)| (label.to_lowercase(), def.span.start))
        .collect();
    let broken: RefCell<Vec<(usize, String)>> = RefCell::new(Vec::new());
    let callback =
        |link: BrokenLink<'_>| -> Option<(pulldown_cmark::CowStr<'_>, pulldown_cmark::CowStr<'_>)> {
            // The callback reports the form that was attempted. A shortcut is often just
            // bracketed prose, so only the explicit forms count as a broken reference.
            if matches!(link.link_type, LinkType::Reference | LinkType::Collapsed) {
                broken
                    .borrow_mut()
                    .push((link.span.start, link.reference.to_string()));
            }
            None
        };
    let parser = Parser::new_with_broken_link_callback(src.text, parser_options(), Some(callback));
    let mut used: HashSet<String> = HashSet::new();
    for (event, _) in parser.into_offset_iter() {
        let (link_type, id) = match &event {
            Event::Start(Tag::Link { link_type, id, .. })
            | Event::Start(Tag::Image { link_type, id, .. }) => (*link_type, id.to_string()),
            _ => continue,
        };
        if matches!(
            link_type,
            LinkType::Reference | LinkType::Collapsed | LinkType::Shortcut
        ) {
            used.insert(id.to_lowercase());
        }
    }
    for (offset, label) in broken.into_inner() {
        out.push(src.finding(
            offset,
            Rule::LinkRef,
            format!("reference '{label}' has no definition"),
        ));
    }
    let mut unused: Vec<(&String, &usize)> = defined
        .iter()
        .filter(|(label, _)| !used.contains(*label))
        .collect();
    unused.sort_by_key(|(_, offset)| **offset);
    for (label, offset) in unused {
        out.push(src.finding(
            *offset,
            Rule::LinkRef,
            format!("definition '{label}' is never used"),
        ));
    }
}

/// Checks heading levels, duplicate anchors and the number of level one headings.
fn check_headings(src: &Source<'_>, enabled: &HashSet<Rule>, out: &mut Vec<Finding>) {
    let mut previous: Option<u8> = None;
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut anchor_count: HashMap<String, usize> = HashMap::new();
    let mut h1 = 0;
    let mut current: Option<(u8, usize, String)> = None;
    // The headings of the tabs of a widget name its tabs, and the tabs of a group repeat them.
    let mut in_tabs = false;
    for (event, range) in Parser::new_ext(src.text, parser_options()).into_offset_iter() {
        match event {
            Event::Html(html) => {
                if let Some(Ok(note)) = crate::annotation::from_html(&html) {
                    match note.kind {
                        crate::annotation::Kind::Widget => {
                            in_tabs = note.get("widget") == Some("tabs");
                        }
                        crate::annotation::Kind::End => in_tabs = false,
                        _ => {}
                    }
                }
            }
            Event::Start(Tag::Heading { level, .. }) => {
                current = Some((level as u8, range.start, String::new()));
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, _, text)) = &mut current {
                    text.push_str(&t);
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                let Some((level, offset, text)) = current.take() else {
                    continue;
                };
                if level == 1 {
                    h1 += 1;
                    if h1 > 1 && enabled.contains(&Rule::HeadingSingleH1) {
                        out.push(src.finding(
                            offset,
                            Rule::HeadingSingleH1,
                            format!("a second level one heading: '{text}'"),
                        ));
                    }
                }
                if let Some(prev) = previous
                    && level > prev + 1
                    && enabled.contains(&Rule::HeadingSkip)
                {
                    out.push(src.finding(
                        offset,
                        Rule::HeadingSkip,
                        format!("level {level} follows level {prev}"),
                    ));
                }
                previous = Some(level);
                let anchor = github_anchor(&text, &mut seen);
                let base = anchor.rsplit_once('-').map_or(anchor.clone(), |(b, n)| {
                    if n.chars().all(|c| c.is_ascii_digit()) {
                        b.to_string()
                    } else {
                        anchor.clone()
                    }
                });
                let count = anchor_count.entry(base.clone()).or_insert(0);
                *count += 1;
                if *count > 1 && !in_tabs && enabled.contains(&Rule::HeadingDup) {
                    out.push(src.finding(
                        offset,
                        Rule::HeadingDup,
                        format!("'{text}' repeats the anchor '{base}'"),
                    ));
                }
            }
            _ => {}
        }
    }
}

/// Checks images without alt text and fences without a language.
fn check_blocks(src: &Source<'_>, enabled: &HashSet<Rule>, out: &mut Vec<Finding>) {
    let mut image: Option<(usize, String)> = None;
    for (event, range) in Parser::new_ext(src.text, parser_options()).into_offset_iter() {
        match event {
            Event::Start(Tag::Image { .. }) => image = Some((range.start, String::new())),
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, alt)) = &mut image {
                    alt.push_str(&t);
                }
            }
            Event::End(TagEnd::Image) => {
                if let Some((offset, alt)) = image.take()
                    && alt.trim().is_empty()
                    && enabled.contains(&Rule::ImageAlt)
                {
                    out.push(src.finding(
                        offset,
                        Rule::ImageAlt,
                        "image without alt text".to_string(),
                    ));
                }
            }
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info)))
                if enabled.contains(&Rule::FenceLang) && info.trim().is_empty() =>
            {
                out.push(src.finding(
                    range.start,
                    Rule::FenceLang,
                    "fenced code block without a language".to_string(),
                ));
            }
            _ => {}
        }
    }
}

/// Checks the annotations for the site: each one holds, and a `replace` is closed by an `end`.
fn check_annotations(src: &Source<'_>, out: &mut Vec<Finding>) {
    use crate::annotation::{self, Kind};
    // What is open: the kind, the name of the widget and where it starts.
    let mut open: Vec<(Kind, String, usize)> = Vec::new();
    for (event, range) in Parser::new_ext(src.text, parser_options()).into_offset_iter() {
        let Event::Html(html) = event else { continue };
        let Some(parsed) = annotation::from_html(&html) else {
            continue;
        };
        let mut report = |message: String| {
            out.push(src.finding(range.start, Rule::SiteAnnotation, message));
        };
        match parsed {
            Err(why) => report(format!("site annotation: {why}")),
            Ok(note) => match note.kind {
                Kind::Replace | Kind::Widget if !open.is_empty() => {
                    let (kind, name, _) = open.last().expect("an open region").clone();
                    let inner = note.get("widget").unwrap_or_default().to_string();
                    if note.kind == Kind::Widget
                        && open.len() == 1
                        && kind == Kind::Widget
                        && name == "tabs"
                        && NESTED_IN_TABS.contains(&inner.as_str())
                    {
                        open.push((note.kind, inner, range.start));
                    } else {
                        report(if kind == note.kind {
                            format!(
                                "a {} inside another one: close the first with an end",
                                kind.name()
                            )
                        } else {
                            format!(
                                "a {} inside a {}: close the first with an end",
                                note.kind.name(),
                                kind.name()
                            )
                        });
                    }
                }
                Kind::Replace | Kind::Widget => open.push((
                    note.kind,
                    note.get("widget").unwrap_or_default().to_string(),
                    range.start,
                )),
                Kind::End if open.pop().is_none() => {
                    report("an end with no replace or widget before it".to_string());
                }
                _ => {}
            },
        }
    }
    for (kind, _, offset) in open {
        out.push(src.finding(
            offset,
            Rule::SiteAnnotation,
            format!("a {} is never closed by an end", kind.name()),
        ));
    }
}

/// Checks the files that the `table` and the `replay` widgets name: each one exists, and reads
/// as the site would read it.
fn check_widget_files(src: &Source<'_>, enabled: &HashSet<Rule>, out: &mut Vec<Finding>) {
    let dir = src.path.parent().unwrap_or(Path::new("."));
    for (event, range) in Parser::new_ext(src.text, parser_options()).into_offset_iter() {
        let Event::Html(html) = event else { continue };
        let Some((widget, file, columns)) = crate::site::fromfile::file_widget(&html) else {
            continue;
        };
        let path = dir.join(&file);
        if !path.exists() {
            if enabled.contains(&Rule::LinkFile) {
                out.push(src.finding(
                    range.start,
                    Rule::LinkFile,
                    format!("{file} does not exist"),
                ));
            }
        } else if enabled.contains(&Rule::SiteAnnotation)
            && let Err(why) = crate::site::fromfile::check(widget, &path, columns.as_deref())
        {
            out.push(src.finding(
                range.start,
                Rule::SiteAnnotation,
                format!("the {widget} cannot read {file}: {why:#}"),
            ));
        }
    }
}

/// The widgets that a `tabs` widget may hold, 1 level deep.
const NESTED_IN_TABS: [&str; 2] = ["steps", "checklist"];

/// Lines that hold verbatim content, where whitespace and length rules do not apply.
fn verbatim_lines(src: &Source<'_>) -> HashSet<usize> {
    let mut lines = HashSet::new();
    for (event, range) in Parser::new_ext(src.text, parser_options()).into_offset_iter() {
        let covered = matches!(
            event,
            Event::Start(
                Tag::CodeBlock(_) | Tag::HtmlBlock | Tag::MetadataBlock(_) | Tag::Table(_)
            )
        );
        if covered {
            let first = src.index.line_of(range.start);
            let last = src.index.line_of(range.end.saturating_sub(1));
            lines.extend(first..=last);
        }
    }
    lines
}

/// Checks trailing whitespace and line length.
fn check_lines(
    src: &Source<'_>,
    enabled: &HashSet<Rule>,
    line_length: usize,
    out: &mut Vec<Finding>,
) {
    if !enabled.contains(&Rule::TrailingSpace) && !enabled.contains(&Rule::LineLength) {
        return;
    }
    let verbatim = verbatim_lines(src);
    for line_no in 1..=src.index.line_count() {
        let range = src.index.line_range(src.text, line_no);
        let line = &src.text[range.clone()];
        if verbatim.contains(&line_no) {
            continue;
        }
        if enabled.contains(&Rule::TrailingSpace) {
            let trimmed = line.trim_end_matches([' ', '\t']);
            let trailing = &line[trimmed.len()..];
            // Exactly two spaces are a hard break, and a blank line has nothing to trim.
            let is_break = trailing == "  " && !trimmed.is_empty();
            if !trailing.is_empty() && !is_break {
                out.push(Finding {
                    line: line_no,
                    column: trimmed.chars().count() + 1,
                    rule: Rule::TrailingSpace,
                    message: format!("{} trailing whitespace characters", trailing.len()),
                });
            }
        }
        if enabled.contains(&Rule::LineLength) {
            let width = line.chars().count();
            if width > line_length && !line.contains("://") {
                out.push(Finding {
                    line: line_no,
                    column: line_length + 1,
                    rule: Rule::LineLength,
                    message: format!("line is {width} characters, over {line_length}"),
                });
            }
        }
    }
}

/// Runs the enabled rules over one file and returns the findings in source order.
pub fn check(path: &Path, text: &str, enabled: &HashSet<Rule>, line_length: usize) -> Vec<Finding> {
    let src = Source::new(path, text);
    let mut out = Vec::new();
    check_targets(&src, enabled, &mut out);
    if enabled.contains(&Rule::LinkRef) {
        check_references(&src, &mut out);
    }
    check_headings(&src, enabled, &mut out);
    check_blocks(&src, enabled, &mut out);
    if enabled.contains(&Rule::SiteAnnotation) {
        check_annotations(&src, &mut out);
    }
    check_widget_files(&src, enabled, &mut out);
    check_lines(&src, enabled, line_length, &mut out);
    out.sort_by_key(|f| (f.line, f.column, f.rule));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The anchors the document model gives, which the streaming version has to match.
    fn anchors_of_the_model(text: &str) -> HashSet<String> {
        let opts = crate::RenderOptions {
            links: crate::options::LinkMode::None,
            ..crate::RenderOptions::default()
        };
        let doc = crate::document(text, &opts);
        crate::slug::outline(&doc)
            .into_iter()
            .map(|e| e.anchor)
            .collect()
    }

    #[test]
    fn heading_anchors_match_the_model_on_a_corpus() {
        let Ok(dir) = std::env::var("DKT_CORPUS") else {
            return;
        };
        let mut todo = vec![std::path::PathBuf::from(dir)];
        let mut checked = 0;
        while let Some(path) = todo.pop() {
            if path.is_dir() {
                todo.extend(std::fs::read_dir(&path).unwrap().map(|e| e.unwrap().path()));
            } else if path.extension().is_some_and(|e| e == "md") {
                let text = std::fs::read_to_string(&path).unwrap();
                assert_eq!(
                    heading_anchors(&text),
                    anchors_of_the_model(&text),
                    "{}",
                    path.display()
                );
                checked += 1;
            }
        }
        eprintln!("{checked} documents compared");
    }

    #[test]
    fn anchors_are_those_of_the_document_model() {
        for text in [
            "# A\n\n## B `c` d\n\n## B `c` d\n\n> ## Quoted\n\n- ## Listed\n\n### [Link](http://x) *em* **b**\n\nSetext\n===\n\nline\nbreak\n---\n\n#### 中文 & more!\n",
            "## Dup\n## Dup\n## Dup\n\n[^1]: note\n\n    ## inside\n",
            "| a |\n|---|\n| b |\n\n# ![img](x.png) alt\n",
        ] {
            assert_eq!(heading_anchors(text), anchors_of_the_model(text), "{text}");
        }
    }

    fn findings(text: &str, rules: &[Rule]) -> Vec<String> {
        let enabled: HashSet<Rule> = rules.iter().copied().collect();
        check(Path::new("doc.md"), text, &enabled, 80)
            .into_iter()
            .map(|f| format!("{}:{}: {}: {}", f.line, f.column, f.rule, f.message))
            .collect()
    }

    #[test]
    fn heading_levels_and_duplicates() {
        let out = findings(
            "# A\n\n### C\n\n## B\n\n## B\n",
            &[Rule::HeadingSkip, Rule::HeadingDup],
        );
        assert_eq!(
            out,
            [
                "3:1: heading-skip: level 3 follows level 1",
                "7:1: heading-dup: 'B' repeats the anchor 'b'"
            ]
        );
    }

    #[test]
    fn the_tabs_of_a_widget_may_repeat_their_names() {
        let tabs = "<!-- site: widget tabs group=os -->\n### Linux\n\na\n\n### macOS\n\nb\n<!-- site: end -->\n\n<!-- site: widget tabs group=os -->\n### Linux\n\nc\n\n### macOS\n\nd\n<!-- site: end -->\n";
        assert!(findings(tabs, &[Rule::HeadingDup]).is_empty());
        let plain = "### Linux\n\na\n\n### Linux\n\nb\n";
        assert_eq!(findings(plain, &[Rule::HeadingDup]).len(), 1);
    }

    #[test]
    fn missing_and_unused_references() {
        let out = findings(
            "see [a][known] and [b][gone]\n\n[known]: http://x\n[spare]: http://y\n",
            &[Rule::LinkRef],
        );
        assert_eq!(
            out,
            [
                "1:20: link-ref: reference 'gone' has no definition",
                "4:1: link-ref: definition 'spare' is never used"
            ]
        );
    }

    #[test]
    fn anchors_within_the_same_file() {
        let out = findings(
            "# Title\n\n[ok](#title) [bad](#nope)\n",
            &[Rule::LinkAnchor],
        );
        assert_eq!(out, ["3:14: link-anchor: no heading 'nope' in this file"]);
    }

    #[test]
    fn a_heading_that_is_a_link_has_the_anchor_of_its_text() {
        let out = findings(
            "# T\n\n## [Foo bar](#foo-bar)\n\n[go](#foo-bar)\n",
            &[Rule::LinkAnchor],
        );
        assert!(out.is_empty(), "{out:?}");
    }

    #[test]
    fn explicit_html_names_count_as_anchors() {
        let out = findings(
            "<a name=\"here\"></a>\n\n[go](#here)\n",
            &[Rule::LinkAnchor],
        );
        assert!(out.is_empty(), "{out:?}");
    }

    #[test]
    fn email_autolinks_are_not_paths() {
        let out = findings("write to <someone@example.com> now\n", &[Rule::LinkFile]);
        assert!(out.is_empty(), "{out:?}");
    }

    #[test]
    fn images_need_alt_text() {
        let out = findings("![](a.png) ![ok](b.png)\n", &[Rule::ImageAlt]);
        assert_eq!(out, ["1:1: image-alt: image without alt text"]);
    }

    #[test]
    fn annotations_for_the_site_are_checked() {
        let ok = "<!-- site: page title=\"A\" -->\n\n<!-- site: replace src=a.png -->\n\ntext\n\n<!-- site: end -->\n";
        assert!(findings(ok, &[Rule::SiteAnnotation]).is_empty());
        let out = findings(
            "<!-- site: nope -->\n\n<!-- site: end -->\n\n<!-- site: replace src=a.png -->\n\ntext\n",
            &[Rule::SiteAnnotation],
        );
        assert_eq!(
            out,
            [
                "1:1: site-annotation: site annotation: unknown kind 'nope', expected one of page, figure, replace, end, alias, widget",
                "3:1: site-annotation: an end with no replace or widget before it",
                "5:1: site-annotation: a replace is never closed by an end",
            ]
        );
        let nested = "<!-- site: replace src=a.png -->\n\n<!-- site: replace src=b.png -->\n\n<!-- site: end -->\n";
        assert!(findings(nested, &[Rule::SiteAnnotation])[0].contains("inside another one"));
        assert!(findings("<!-- toc -->\n", &[Rule::SiteAnnotation]).is_empty());
    }

    #[test]
    fn widgets_are_closed_and_not_nested() {
        let ok = "<!-- site: widget tabs -->\n\n### A\n\ntext\n\n<!-- site: end -->\n";
        assert!(findings(ok, &[Rule::SiteAnnotation]).is_empty());
        let open = "<!-- site: widget steps -->\n\n1. a\n";
        assert_eq!(
            findings(open, &[Rule::SiteAnnotation]),
            ["1:1: site-annotation: a widget is never closed by an end"]
        );
        let nested =
            "<!-- site: widget steps -->\n\n<!-- site: widget steps -->\n\n<!-- site: end -->\n";
        assert!(findings(nested, &[Rule::SiteAnnotation])[0].contains("inside another one"));
        let in_tabs = "<!-- site: widget tabs -->\n\n### A\n\n<!-- site: widget steps -->\n\n1. a\n\n<!-- site: end -->\n\n<!-- site: end -->\n";
        assert!(findings(in_tabs, &[Rule::SiteAnnotation]).is_empty());
        let too_deep = "<!-- site: widget tabs -->\n\n<!-- site: widget steps -->\n\n<!-- site: widget checklist -->\n";
        assert!(
            findings(too_deep, &[Rule::SiteAnnotation])
                .iter()
                .any(|f| f.contains("inside another one"))
        );
        let not_allowed = "<!-- site: widget tabs -->\n\n<!-- site: widget table -->\n";
        assert!(
            findings(not_allowed, &[Rule::SiteAnnotation])
                .iter()
                .any(|f| f.contains("inside another one"))
        );
        let mixed = "<!-- site: replace src=a.png -->\n\n<!-- site: widget tabs -->\n";
        assert!(
            findings(mixed, &[Rule::SiteAnnotation])
                .iter()
                .any(|f| f.contains("3:1: site-annotation: a widget inside a replace"))
        );
        let unknown = "<!-- site: widget dance -->\n";
        assert!(findings(unknown, &[Rule::SiteAnnotation])[0].contains("unknown widget"));
    }

    #[test]
    fn hard_breaks_are_not_trailing_whitespace() {
        let out = findings("a  \nb \nc\t\n\nd\n", &[Rule::TrailingSpace]);
        assert_eq!(
            out,
            [
                "2:2: trailing-space: 1 trailing whitespace characters",
                "3:2: trailing-space: 1 trailing whitespace characters"
            ]
        );
    }

    #[test]
    fn verbatim_blocks_escape_the_line_rules() {
        let text = "```\ncode   \n```\n\ntext   \n";
        let out = findings(text, &[Rule::TrailingSpace]);
        assert_eq!(
            out,
            ["5:5: trailing-space: 3 trailing whitespace characters"]
        );
    }

    #[test]
    fn long_lines_holding_a_url_are_left_alone() {
        let long = "x".repeat(90);
        let url = format!("see https://example.com/{}", "y".repeat(90));
        let text = format!("{long}\n\n{url}\n");
        let out = findings(&text, &[Rule::LineLength]);
        assert_eq!(out.len(), 1);
        assert!(out[0].starts_with("1:81: line-length:"), "{out:?}");
    }

    #[test]
    fn fences_without_a_language() {
        let out = findings("```\nx\n```\n\n```sh\ny\n```\n", &[Rule::FenceLang]);
        assert_eq!(
            out,
            ["1:1: fence-lang: fenced code block without a language"]
        );
    }
}
