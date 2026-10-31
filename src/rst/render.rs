//! Markdown from the blocks of a reStructuredText document.

use std::collections::HashMap;

use super::blocks::Block;
use super::inline::{Refs, inline, plain};
use crate::slug::github_anchor;

/// The anchors of the titles and of the objects, in the order of the document.
fn collect(blocks: &[Block], refs: &mut Refs, seen: &mut HashMap<String, usize>) {
    for block in blocks {
        match block {
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
            Block::Target { label, address } => {
                refs.urls.insert(label.to_lowercase(), address.clone());
            }
            Block::Quote(inner) => collect(inner, refs, seen),
            _ => {}
        }
    }
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
        Block::Target { .. } => return None,
    })
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
