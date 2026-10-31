//! The page a Markdown document makes: its HTML body, title, description and headings.
//!
//! The body is what pulldown-cmark writes, with two changes.
//! Every heading carries the anchor GitHub gives it, so a link written for GitHub holds.
//! A metadata block at the top of the document is left out.
//! An annotation takes effect: a `page` one sets the title, the description and the rest, and wins
//! over what the text gives. A `figure` one writes its figure, and a `replace` one writes it in
//! place of the blocks up to the next `end`.

use std::collections::HashMap;

use pulldown_cmark::{CodeBlockKind, CowStr, Event, Parser, Tag, TagEnd, html};

use crate::annotation::{self, Annotation, Kind};
use crate::parse::parser_options;
use crate::slug::github_anchor;

/// What a document makes as a page.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Page {
    /// Text of the first level one heading.
    pub title: Option<String>,
    /// Text of the paragraph that directly follows the title.
    pub description: Option<String>,
    /// The body, as HTML.
    pub content: String,
    /// Every heading of the body, in order.
    pub headings: Vec<Heading>,
    /// Short title for a menu, from a `page` annotation.
    pub nav_title: Option<String>,
    /// Position among its siblings, from a `page` annotation.
    pub weight: Option<i64>,
    /// True when a `page` annotation asks to leave the page out of the site.
    pub hidden: bool,
    /// True when a `page` annotation asks to leave the page out of the search.
    pub no_search: bool,
    /// The words that `alias` annotations give to sections, in order.
    pub aliases: Vec<Alias>,
}

/// Words that find a section in a search, from an `alias` annotation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alias {
    /// The id of the heading the annotation was written under, or none before the first heading.
    pub anchor: Option<String>,
    pub words: String,
}

/// A heading of a page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heading {
    pub level: u8,
    pub id: String,
    pub text: String,
}

/// Rewrites the target of a link or an image, or leaves it as written when it returns `None`.
pub type Rewrite<'a> = &'a dyn Fn(&str) -> Option<String>;

/// Writes the figure of an annotation that names its image instead of giving its path.
pub type Figure<'a> = &'a dyn Fn(&Annotation) -> Option<String>;

/// What a caller can change in the way a page is rendered.
#[derive(Clone, Copy, Default)]
pub struct Hooks<'a> {
    pub rewrite: Option<Rewrite<'a>>,
    pub figure: Option<Figure<'a>>,
}

/// Plain text of the events of an inline run, with breaks as spaces.
fn plain_text(events: &[Event<'_>]) -> String {
    let mut out = String::new();
    for event in events {
        match event {
            Event::Text(t) | Event::Code(t) => out.push_str(t),
            Event::SoftBreak | Event::HardBreak => out.push(' '),
            _ => {}
        }
    }
    out.trim().to_string()
}

/// True for a character that draws a line, a corner, a block or an arrow head in a text.
fn is_box(c: char) -> bool {
    ('\u{2500}'..='\u{25ff}').contains(&c)
}

/// True when the code block that starts at `from` holds characters that draw boxes.
fn draws_boxes(events: &[Event<'_>], from: usize) -> bool {
    events[from + 1..]
        .iter()
        .take_while(|e| !matches!(e, Event::End(TagEnd::CodeBlock)))
        .any(|e| matches!(e, Event::Text(t) if t.chars().any(is_box)))
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Index of the first event from `from` on that satisfies `is_end`.
fn find_end(events: &[Event<'_>], from: usize, is_end: impl Fn(&Event<'_>) -> bool) -> usize {
    (from..events.len())
        .find(|&i| is_end(&events[i]))
        .unwrap_or(events.len() - 1)
}

fn target<'a>(dest: &CowStr<'a>, rewrite: Option<Rewrite<'_>>) -> CowStr<'a> {
    rewrite
        .and_then(|f| f(dest))
        .map(CowStr::from)
        .unwrap_or_else(|| dest.clone())
}

/// The tag that opens a widget. The script of the theme builds the component from what it holds.
/// A widget with no `id` is named by its place on the page.
/// A carousel is cut into its slides here, so that the page scrolls them without the script.
fn widget_open(note: &Annotation, position: usize) -> String {
    let name = note.get("widget").unwrap_or_default();
    let id = note
        .get("id")
        .map(str::to_string)
        .unwrap_or_else(|| format!("w{position}"));
    let mut tag = format!("<div class=\"dkt-widget\" data-widget=\"{name}\" data-id=\"{id}\"");
    if let Some(answer) = note.get("answer") {
        tag.push_str(&format!(" data-answer=\"{answer}\""));
    }
    if name != "carousel" {
        tag.push_str(">\n");
        return tag;
    }
    let shown = shown(note);
    tag.push_str(&format!(
        " data-show=\"{} {} {}\"",
        shown[0], shown[1], shown[2]
    ));
    if let Some(delay) = note.get("autoplay") {
        tag.push_str(&format!(" data-autoplay=\"{delay}\""));
    }
    for flag in ["loop", "zoom"] {
        if note.has(flag) {
            tag.push_str(&format!(" data-{flag}"));
        }
    }
    tag.push_str(&format!(
        " style=\"--dkt-show:{};--dkt-show-m:{};--dkt-show-l:{}\">\n<div class=\"dkt-track\" tabindex=\"0\">\n<div class=\"dkt-slide\">\n",
        shown[0], shown[1], shown[2]
    ));
    tag
}

/// The figure, its image loaded on request when it comes after the slides that a carousel shows at once.
fn lazy_after(figure: String, carousel: Option<(usize, usize)>) -> String {
    match carousel {
        Some((slide, most)) if slide > most => figure.replace("<img ", "<img loading=\"lazy\" "),
        _ => figure,
    }
}

/// How many slides a carousel shows at the 3 widths of page: the count that each width gets, the
/// last one given standing for the widths after it.
fn shown(note: &Annotation) -> [usize; 3] {
    let given: Vec<usize> = note
        .get("show")
        .unwrap_or("1")
        .split_whitespace()
        .filter_map(|n| n.parse().ok())
        .collect();
    let at = |k: usize| given.get(k).or(given.last()).copied().unwrap_or(1);
    [at(0), at(1), at(2)]
}

/// Renders `md` as a page. `rewrite` sees the target of every link and image.
pub fn render(md: &str, rewrite: Option<Rewrite<'_>>) -> Page {
    render_with(
        md,
        &Hooks {
            rewrite,
            figure: None,
        },
    )
}

/// Renders `md` as a page, with the changes the hooks ask for.
pub fn render_with(md: &str, hooks: &Hooks<'_>) -> Page {
    let rewrite = hooks.rewrite;
    let events: Vec<Event<'_>> = Parser::new_ext(md, parser_options()).collect();
    let mut page = Page::default();
    let mut body: Vec<Event<'_>> = Vec::with_capacity(events.len());
    let mut seen = HashMap::new();
    let mut page_notes = Vec::new();
    let mut under: Option<String> = None;
    let mut widgets = 0;
    // The carousel that is open, with the number of its slides and the most it shows at once.
    let mut carousel: Option<(usize, usize)> = None;
    let mut open_widgets = 0;
    let mut i = 0;
    while i < events.len() {
        match &events[i] {
            Event::Start(Tag::MetadataBlock(_)) => {
                i = find_end(&events, i, |e| {
                    matches!(e, Event::End(TagEnd::MetadataBlock(_)))
                }) + 1;
            }
            Event::Start(Tag::Heading {
                level,
                classes,
                attrs,
                ..
            }) => {
                let end = find_end(&events, i + 1, |e| {
                    matches!(e, Event::End(TagEnd::Heading(_)))
                });
                let text = plain_text(&events[i + 1..end]);
                let id = github_anchor(&text, &mut seen);
                let level_number = *level as u8;
                body.push(Event::Start(Tag::Heading {
                    level: *level,
                    id: Some(CowStr::from(id.clone())),
                    classes: classes.clone(),
                    attrs: attrs.clone(),
                }));
                // A heading below the title is a link to itself, unless it holds a link already.
                let linked = events[i + 1..end]
                    .iter()
                    .any(|e| matches!(e, Event::Start(Tag::Link { .. })));
                if level_number >= 2 && !linked {
                    body.push(Event::Start(Tag::Link {
                        link_type: pulldown_cmark::LinkType::Inline,
                        dest_url: CowStr::from(format!("#{id}")),
                        title: CowStr::from(""),
                        id: CowStr::from(""),
                    }));
                    body.extend(events[i + 1..end].iter().cloned());
                    body.push(Event::End(TagEnd::Link));
                    body.push(events[end].clone());
                } else {
                    body.extend(events[i + 1..=end].iter().cloned());
                }
                if level_number == 1 && page.title.is_none() {
                    page.title = Some(text.clone());
                    if matches!(events.get(end + 1), Some(Event::Start(Tag::Paragraph))) {
                        let stop = find_end(&events, end + 2, |e| {
                            matches!(e, Event::End(TagEnd::Paragraph))
                        });
                        page.description = Some(plain_text(&events[end + 2..stop]));
                    }
                }
                under = Some(id.clone());
                page.headings.push(Heading {
                    level: level_number,
                    id,
                    text,
                });
                i = end + 1;
            }
            Event::Html(html) if annotation::from_html(html).is_some() => {
                i += 1;
                let Some(Ok(note)) = annotation::from_html(html) else {
                    body.push(Event::Html(html.clone()));
                    continue;
                };
                match note.kind {
                    Kind::Page => {
                        page_notes.push(note);
                    }
                    Kind::Figure => {
                        if let Some(figure) = note
                            .figure()
                            .or_else(|| hooks.figure.and_then(|f| f(&note)))
                        {
                            body.push(Event::Html(CowStr::from(lazy_after(figure, carousel))));
                        }
                    }
                    Kind::Replace => {
                        let end = (i..events.len()).find(|&j| {
                            matches!(&events[j], Event::Html(h)
                                if matches!(annotation::from_html(h), Some(Ok(n)) if n.kind == Kind::End))
                        });
                        if let Some(figure) = note
                            .figure()
                            .or_else(|| hooks.figure.and_then(|f| f(&note)))
                        {
                            body.push(Event::Html(CowStr::from(figure)));
                        }
                        if let Some(end) = end {
                            i = end + 1;
                        }
                    }
                    Kind::Widget => {
                        widgets += 1;
                        open_widgets += 1;
                        if note.get("widget") == Some("carousel") {
                            let most = shown(&note).into_iter().max().unwrap_or(1);
                            carousel = Some((1, most));
                        }
                        body.push(Event::Html(CowStr::from(widget_open(&note, widgets))));
                    }
                    Kind::End => {
                        if open_widgets > 0 {
                            open_widgets -= 1;
                            let close = if carousel.take().is_some() {
                                "</div>\n</div>\n</div>\n"
                            } else {
                                "</div>\n"
                            };
                            body.push(Event::Html(CowStr::from(close)));
                        }
                    }
                    Kind::Alias => {
                        if let Some(words) = note.get("words") {
                            page.aliases.push(Alias {
                                anchor: under.clone(),
                                words: words.trim().to_string(),
                            });
                        }
                    }
                }
            }
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info)))
                if info.split_whitespace().next() == Some("diagram") =>
            {
                let end = find_end(&events, i + 1, |e| {
                    matches!(e, Event::End(TagEnd::CodeBlock))
                });
                let text: String = events[i + 1..end]
                    .iter()
                    .filter_map(|e| match e {
                        Event::Text(t) => Some(t.as_ref()),
                        _ => None,
                    })
                    .collect();
                body.push(Event::Html(CowStr::from(crate::diagram::to_svg(&text))));
                i = end + 1;
            }
            // A thematic break cuts the slides of a carousel.
            Event::Rule if carousel.is_some() => {
                if let Some((slide, _)) = carousel.as_mut() {
                    *slide += 1;
                }
                body.push(Event::Html(CowStr::from(
                    "</div>\n<div class=\"dkt-slide\">\n",
                )));
                i += 1;
            }
            // A block that draws with box characters is marked, so that a theme can close the gaps
            // between its lines.
            Event::Start(Tag::CodeBlock(kind)) if draws_boxes(&events, i) => {
                let end = find_end(&events, i + 1, |e| {
                    matches!(e, Event::End(TagEnd::CodeBlock))
                });
                let text: String = events[i + 1..end]
                    .iter()
                    .filter_map(|e| match e {
                        Event::Text(t) => Some(t.as_ref()),
                        _ => None,
                    })
                    .collect();
                let class = match kind {
                    CodeBlockKind::Fenced(info) => info
                        .split_whitespace()
                        .next()
                        .map(|lang| format!(" class=\"language-{}\"", html_escape(lang)))
                        .unwrap_or_default(),
                    CodeBlockKind::Indented => String::new(),
                };
                body.push(Event::Html(CowStr::from(format!(
                    "<pre class=\"box\"><code{class}>{}</code></pre>\n",
                    html_escape(&text)
                ))));
                i = end + 1;
            }
            // A table that is wider than the page scrolls inside its own box.
            Event::Start(Tag::Table(_)) => {
                body.push(Event::Html(CowStr::from("<div class=\"table-wrap\">")));
                body.push(events[i].clone());
                i += 1;
            }
            Event::End(TagEnd::Table) => {
                body.push(events[i].clone());
                body.push(Event::Html(CowStr::from("</div>\n")));
                i += 1;
            }
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                body.push(Event::Start(Tag::Link {
                    link_type: *link_type,
                    dest_url: target(dest_url, rewrite),
                    title: title.clone(),
                    id: id.clone(),
                }));
                i += 1;
            }
            Event::Start(Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                body.push(Event::Start(Tag::Image {
                    link_type: *link_type,
                    dest_url: target(dest_url, rewrite),
                    title: title.clone(),
                    id: id.clone(),
                }));
                i += 1;
            }
            other => {
                body.push(other.clone());
                i += 1;
            }
        }
    }
    for note in page_notes {
        if let Some(title) = note.get("title") {
            page.title = Some(title.to_string());
        }
        if let Some(description) = note.get("description") {
            page.description = Some(description.to_string());
        }
        if let Some(nav_title) = note.get("nav-title") {
            page.nav_title = Some(nav_title.to_string());
        }
        if let Some(weight) = note.get("weight") {
            page.weight = weight.parse().ok();
        }
        page.hidden |= note.has("hide");
        page.no_search |= note.has("no-search");
    }
    drop(events);
    html::push_html(&mut page.content, body.into_iter());
    page
}
