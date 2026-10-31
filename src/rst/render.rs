//! Markdown from the blocks of a reStructuredText document.

use std::collections::{HashMap, HashSet};

use super::blocks::Block;
use super::inline::{Refs, inline, plain};
use crate::slug::github_anchor;

/// Calls `visit` on every block, the ones inside others included, in the order of the document.
fn walk<'a>(blocks: &'a [Block], visit: &mut dyn FnMut(&'a Block)) {
    for block in blocks {
        visit(block);
        match block {
            Block::Quote(inner) | Block::Group(inner) => walk(inner, visit),
            Block::Admonition { body, .. }
            | Block::Object { body, .. }
            | Block::Footnote { body, .. } => walk(body, visit),
            Block::List { items, .. } => items.iter().for_each(|item| walk(item, visit)),
            Block::Definitions(items) => items.iter().for_each(|(_, body)| walk(body, visit)),
            _ => {}
        }
    }
}

/// The id of a citation in Markdown: its label as lower case words with hyphens.
fn citation_id(label: &str) -> String {
    let id: String = label
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    format!("cite-{id}")
}

/// The anchors of the titles and of the objects, the footnotes, the substitutions and the
/// anonymous targets, in the order of the document.
fn collect(blocks: &[Block], refs: &mut Refs, seen: &mut HashMap<String, usize>) {
    let mut explicit = HashSet::new();
    walk(blocks, &mut |block| {
        if let Block::Footnote { label, .. } = block
            && let Ok(n) = label.parse::<usize>()
        {
            explicit.insert(n);
        }
    });
    let mut next_number = 0;
    let mut symbols = 0;
    walk(blocks, &mut |block| match block {
        Block::Heading { text, labels, .. } => {
            let anchor = github_anchor(&plain(text), seen);
            refs.titles
                .insert(plain(text).to_lowercase(), anchor.clone());
            for label in labels {
                refs.labels.insert(label.to_lowercase(), anchor.clone());
                refs.texts.insert(label.to_lowercase(), text.clone());
            }
        }
        Block::Object {
            signature, labels, ..
        } => {
            let anchor = github_anchor(signature, seen);
            let name = signature.split('(').next().unwrap_or(signature).trim();
            refs.objects
                .entry(name.to_string())
                .or_insert_with(|| anchor.clone());
            for label in labels {
                refs.labels.insert(label.to_lowercase(), anchor.clone());
                refs.texts.insert(label.to_lowercase(), signature.clone());
            }
        }
        Block::Target { label, address } if label == "_" => refs.anonymous.push(address.clone()),
        Block::Target { label, address } => {
            refs.urls.insert(label.to_lowercase(), address.clone());
        }
        Block::Substitution { name, value } => {
            refs.substitutions.insert(name.clone(), value.clone());
        }
        Block::Footnote { label, .. } => {
            let mut number = || {
                loop {
                    next_number += 1;
                    if !explicit.contains(&next_number) {
                        return next_number.to_string();
                    }
                }
            };
            let id = match label.as_str() {
                "#" => {
                    let id = number();
                    refs.auto_notes.push(id.clone());
                    id
                }
                "*" => {
                    symbols += 1;
                    let id = format!("symbol-{symbols}");
                    refs.symbol_notes.push(id.clone());
                    id
                }
                l if l.starts_with('#') => {
                    let id = number();
                    refs.notes.insert(l[1..].to_lowercase(), id.clone());
                    id
                }
                l if l.chars().all(|c| c.is_ascii_digit()) => l.to_string(),
                l => {
                    let id = citation_id(l);
                    refs.notes.insert(l.to_lowercase(), id.clone());
                    id
                }
            };
            refs.definitions.push(id);
        }
        _ => {}
    });
}

/// The text of a paragraph without what would make Markdown read a heading, a list or a quote.
fn start_of_line(text: &str) -> String {
    let digits = text.bytes().take_while(u8::is_ascii_digit).count();
    if text.starts_with('#')
        || text.starts_with("+ ")
        || text.starts_with("- ")
        || text.starts_with("> ")
    {
        return format!("\\{text}");
    }
    if digits > 0 && text[digits..].starts_with(['.', ')']) && text[digits + 1..].starts_with(' ') {
        return format!("{}\\{}", &text[..digits], &text[digits..]);
    }
    text.to_string()
}

fn fence(lang: &str, lines: &[String]) -> String {
    let longest = lines
        .iter()
        .map(|l| l.chars().take_while(|c| *c == '`').count())
        .max()
        .unwrap_or(0);
    let ticks = "`".repeat(longest.max(2) + 1);
    format!("{ticks}{lang}\n{}\n{ticks}", lines.join("\n"))
}

struct Context<'a> {
    refs: &'a Refs,
    /// The Markdown level of the last title.
    level: usize,
}

fn indent_lines(text: &str, first: &str, rest: &str) -> String {
    let mut out = String::new();
    for (k, line) in text.lines().enumerate() {
        let pad = if k == 0 { first } else { rest };
        if line.is_empty() {
            out.push_str(pad.trim_end());
        } else {
            out.push_str(pad);
            out.push_str(line);
        }
        out.push('\n');
    }
    out.trim_end().to_string()
}

fn field_item(name: &str, text: &str, refs: &Refs) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    let body = inline(text, refs);
    let label = |l: &str| format!("**{l}**");
    match words.as_slice() {
        ["param" | "parameter" | "arg" | "argument", rest @ ..] if !rest.is_empty() => {
            let n = rest[rest.len() - 1];
            let kind = rest[..rest.len() - 1].join(" ");
            let kind = if kind.is_empty() {
                String::new()
            } else {
                format!(" (*{}*)", kind)
            };
            format!("- **{n}**{kind}: {body}")
        }
        ["returns" | "return"] => format!("- {}: {body}", label("Returns")),
        ["rtype"] => format!("- {}: {body}", label("Return type")),
        ["see"] => format!("- {}: {body}", label("See")),
        _ => format!("- {}: {body}", label(&inline(name, refs))),
    }
}

fn block(b: &Block, ctx: &mut Context<'_>) -> Option<String> {
    Some(match b {
        Block::Heading { level, text, .. } => {
            ctx.level = (level + 1).min(6);
            format!("{} {}", "#".repeat(ctx.level), inline(text, ctx.refs))
        }
        Block::Para(text) => start_of_line(&inline(text, ctx.refs)),
        Block::Literal { lang, lines } => fence(lang, lines),
        Block::List {
            ordered,
            start,
            items,
        } => {
            let mut out = Vec::new();
            for (k, item) in items.iter().enumerate() {
                let marker = if *ordered {
                    format!("{}. ", start + k)
                } else {
                    "- ".to_string()
                };
                let inner = blocks_md(item, ctx);
                out.push(indent_lines(&inner, &marker, &" ".repeat(marker.len())));
            }
            out.join("\n")
        }
        Block::Admonition { kind, body } => {
            let inner = blocks_md(body, ctx);
            format!("> [!{kind}]\n{}", indent_lines(&inner, "> ", "> "))
        }
        Block::Object {
            signature, body, ..
        } => {
            let level = (ctx.level + 1).clamp(2, 6);
            let mut out = format!("{} `{}`", "#".repeat(level), signature);
            let inner = blocks_md(body, ctx);
            if !inner.is_empty() {
                out.push_str("\n\n");
                out.push_str(&inner);
            }
            out
        }
        Block::Fields(fields) => fields
            .iter()
            .map(|(n, t)| field_item(n, t, ctx.refs))
            .collect::<Vec<_>>()
            .join("\n"),
        Block::Quote(inner) => {
            let md = blocks_md(inner, ctx);
            indent_lines(&md, "> ", "> ")
        }
        Block::Image { src, alt } => format!("![{alt}]({src})"),
        Block::Raw(lines) if lines.is_empty() => return None,
        Block::Raw(lines) => fence("text", lines),
        Block::Target { .. } | Block::Substitution { .. } => return None,
        Block::Table { header, rows } => table(header, rows, ctx),
        Block::Definitions(items) => items
            .iter()
            .map(|(term, body)| {
                let term = match term.split_once(" : ") {
                    Some((term, kind)) => format!(
                        "**{}** (*{}*)",
                        inline(term, ctx.refs),
                        inline(kind, ctx.refs)
                    ),
                    None => format!("**{}**", inline(term, ctx.refs)),
                };
                let inner = blocks_md(body, ctx);
                let text = if inner.is_empty() {
                    term
                } else {
                    format!("{term}\n\n{inner}")
                };
                indent_lines(&text, "- ", "  ")
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Block::Lines(lines) => lines
            .iter()
            .map(|line| {
                let spaces = line.len() - line.trim_start().len();
                format!(
                    "{}{}",
                    "&nbsp;".repeat(spaces),
                    inline(line.trim_start(), ctx.refs)
                )
            })
            .collect::<Vec<_>>()
            .join("\\\n"),
        Block::Footnote { body, .. } => {
            let at = ctx.refs.definitions_written.get();
            ctx.refs.definitions_written.set(at + 1);
            let id = ctx.refs.definitions.get(at).cloned().unwrap_or_default();
            let inner = blocks_md(body, ctx);
            indent_lines(&inner, &format!("[^{id}]: "), "    ")
        }
        Block::Options(items) => items
            .iter()
            .map(|(options, text)| {
                let shown: Vec<String> = options.split(", ").map(|o| format!("`{o}`")).collect();
                format!("- {}: {}", shown.join(", "), inline(text, ctx.refs))
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Block::Group(inner) => {
            let md = blocks_md(inner, ctx);
            if md.is_empty() {
                return None;
            }
            md
        }
        Block::Rubric(text) => format!("**{}**", inline(text, ctx.refs)),
        Block::Rule => "---".to_string(),
        Block::Html(lines) => lines.join("\n"),
    })
}

/// The cell of a table on one line: its blocks one after the other with breaks between them.
fn cell(blocks: &[Block], ctx: &mut Context<'_>) -> String {
    let parts: Vec<String> = blocks
        .iter()
        .filter_map(|b| match b {
            Block::Para(text) => Some(inline(text, ctx.refs)),
            Block::List {
                ordered,
                start,
                items,
            } => Some(
                items
                    .iter()
                    .enumerate()
                    .map(|(k, item)| {
                        let marker = if *ordered {
                            format!("{}. ", start + k)
                        } else {
                            "- ".to_string()
                        };
                        format!("{marker}{}", cell(item, ctx))
                    })
                    .collect::<Vec<_>>()
                    .join("<br>"),
            ),
            Block::Literal { lines, .. } | Block::Raw(lines) => Some(format!(
                "<code>{}</code>",
                lines
                    .iter()
                    .map(|l| l
                        .replace('&', "&amp;")
                        .replace('<', "&lt;")
                        .replace('>', "&gt;"))
                    .collect::<Vec<_>>()
                    .join("<br>")
            )),
            other => block(other, ctx).map(|md| md.replace('\n', "<br>")),
        })
        .collect();
    escape_pipes(&parts.join("<br><br>"))
}

/// The pipes of a cell, which would end it.
fn escape_pipes(text: &str) -> String {
    let mut out = String::new();
    let mut escaped = false;
    for c in text.chars() {
        if c == '|' && !escaped {
            out.push('\\');
        }
        escaped = c == '\\' && !escaped;
        out.push(c);
    }
    out
}

fn table(header: &[Vec<Block>], rows: &[Vec<Vec<Block>>], ctx: &mut Context<'_>) -> String {
    let width = rows
        .iter()
        .map(Vec::len)
        .chain([header.len()])
        .max()
        .unwrap_or(0);
    let mut line = |cells: &[Vec<Block>]| {
        let mut texts: Vec<String> = cells.iter().map(|c| cell(c, ctx)).collect();
        texts.resize(width, String::new());
        format!("| {} |", texts.join(" | "))
    };
    let mut out = vec![line(header)];
    out.push(format!("|{}", " --- |".repeat(width)));
    for row in rows {
        out.push(line(row));
    }
    out.join("\n")
}

fn blocks_md(blocks: &[Block], ctx: &mut Context<'_>) -> String {
    let parts: Vec<String> = blocks.iter().filter_map(|b| block(b, ctx)).collect();
    parts.join("\n\n")
}

/// The Markdown of the blocks of a document, with its title on top when it has one.
pub fn render(blocks: &[Block], title: Option<&str>) -> String {
    let mut refs = Refs::default();
    collect(blocks, &mut refs, &mut HashMap::new());
    let mut ctx = Context {
        refs: &refs,
        level: 1,
    };
    let mut out = String::new();
    if let Some(title) = title {
        out.push_str(&format!("# {}\n\n", inline(title, &refs)));
    }
    out.push_str(&blocks_md(blocks, &mut ctx));
    out.push('\n');
    out
}
