//! The Markdown of a document that `blocks` read.

use std::collections::HashMap;

use super::blocks::{Banner, Block, Item};
use crate::slug::github_anchor;

/// What the links of a document point at.
struct Links {
    /// The anchor of the heading of each numbered chapter.
    sections: HashMap<String, String>,
    /// The anchor of the heading of each keyword, the first one that defines it.
    keywords: HashMap<String, String>,
    /// The anchor of the first keyword that starts with a word that no keyword is named after, such
    /// as `stats`, for the lists that name a group of keywords by that word.
    groups: HashMap<String, String>,
    /// The names that have a placeholder, such as `map_<match_type>`, with the anchor of their
    /// keyword: any name that fits the pattern links there.
    patterns: Vec<(String, String)>,
}

impl Links {
    /// The anchor of the keyword that a name stands for.
    fn find(&self, name: &str) -> Option<&String> {
        self.keywords.get(name).or_else(|| {
            self.patterns
                .iter()
                .find(|(pattern, _)| fits(pattern, name))
                .map(|(_, anchor)| anchor)
        })
    }
}

/// Whether a name fits a pattern whose `<x>` stand for 1 or more letters or digits.
fn fits(pattern: &str, name: &str) -> bool {
    match pattern.split_once('<') {
        None => pattern == name,
        Some((literal, rest)) => {
            let Some(after) = rest.split_once('>').map(|(_, a)| a) else {
                return false;
            };
            let Some(tail) = name.strip_prefix(literal) else {
                return false;
            };
            let run = tail
                .bytes()
                .take_while(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
                .count();
            (1..=run).any(|n| fits(after, &tail[n..]))
        }
    }
}

/// The level of a heading in Markdown: the document title is 1, so a chapter of depth 1 is 2.
fn chapter_level(number: &str) -> usize {
    (number.split('.').count() + 1).min(6)
}

/// A heading of the document, in the order they are written.
struct Head {
    /// The text the anchor comes from.
    text: String,
    /// The number of the chapter, for a chapter.
    number: Option<String>,
    /// The name of the keyword and the chapter it is defined in, for a keyword.
    keyword: Option<(String, String)>,
    /// Whether the whole text is shown as code, for a keyword told apart by its syntax.
    code: bool,
}

/// The text of the heading of a keyword and whether it is shown as code. It is the name when no
/// other keyword has it, else the name and the title of the chapter. A keyword that the same
/// chapter holds again is told apart by its syntax line, and by a word when even that repeats.
fn keyword_text(
    name: &str,
    syntax: &str,
    chapter: &str,
    chapter_title: &str,
    names: &HashMap<&str, usize>,
    repeats: &mut HashMap<(String, String), usize>,
    used: &mut HashMap<String, usize>,
) -> (String, bool) {
    if names.get(name).copied().unwrap_or(0) <= 1 {
        return (name.to_string(), false);
    }
    let n = repeats
        .entry((name.to_string(), chapter.to_string()))
        .or_insert(0);
    *n += 1;
    let reference = chapter == PREFERRED && *n == 1;
    let (text, code) = if reference {
        (name.to_string(), false)
    } else if *n == 1 {
        (format!("{name} ({chapter_title})"), false)
    } else {
        (syntax.to_string(), true)
    };
    let seen = used.entry(text.clone()).or_insert(0);
    *seen += 1;
    if *seen > 1 {
        let word = ORDINALS.get(*seen).copied().unwrap_or("again");
        return (format!("{text}, {word}"), false);
    }
    (text, code)
}

const ORDINALS: [&str; 12] = [
    "", "", "again", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth",
    "eleventh",
];

fn headings(banner: &Banner, blocks: &[Block]) -> Vec<Head> {
    let mut names: HashMap<&str, usize> = HashMap::new();
    for block in blocks {
        if let Block::Keyword { name, .. } = block {
            *names.entry(name.as_str()).or_insert(0) += 1;
        }
    }
    let mut repeats = HashMap::new();
    let mut used = HashMap::new();
    let mut out = vec![Head {
        text: title(banner),
        number: None,
        keyword: None,
        code: false,
    }];
    let mut chapter = String::new();
    let mut chapter_title = String::new();
    for block in blocks {
        match block {
            Block::Heading { number, title } if number.is_empty() => {
                out.push(Head {
                    text: title.clone(),
                    number: None,
                    keyword: None,
                    code: false,
                });
            }
            Block::Heading { number, title } => {
                chapter = number.clone();
                chapter_title = title.clone();
                out.push(Head {
                    text: format!("{number}. {title}"),
                    number: Some(number.clone()),
                    keyword: None,
                    code: false,
                });
            }
            Block::Keyword { name, syntax, .. } => {
                let first = syntax.first().map(String::as_str).unwrap_or(name);
                let first = first.strip_prefix("no ").unwrap_or(first);
                let (text, code) = keyword_text(
                    name,
                    first,
                    &chapter,
                    &chapter_title,
                    &names,
                    &mut repeats,
                    &mut used,
                );
                out.push(Head {
                    text,
                    number: None,
                    keyword: Some((name.clone(), chapter.clone())),
                    code,
                });
            }
            _ => {}
        }
    }
    out
}

/// The title of the document, with the version after the first word: `HAProxy 3.4 Configuration
/// Manual`.
fn title(banner: &Banner) -> String {
    if banner.title.is_empty() {
        return "HAProxy documentation".to_string();
    }
    if banner.version.is_empty() {
        return banner.title.clone();
    }
    match banner.title.split_once(' ') {
        Some((first, rest)) => format!("{first} {} {rest}", banner.version),
        None => format!("{} {}", banner.title, banner.version),
    }
}

/// The chapter whose definition of a keyword the links go to: the reference of the keywords of the
/// proxies when it has one, else the first.
const PREFERRED: &str = "4.2";

fn links(banner: &Banner, blocks: &[Block]) -> Links {
    let mut seen = HashMap::new();
    let mut sections = HashMap::new();
    let mut keywords: HashMap<String, (String, bool)> = HashMap::new();
    let mut groups: HashMap<String, String> = HashMap::new();
    for head in headings(banner, blocks) {
        let anchor = github_anchor(&head.text, &mut seen);
        if let Some(n) = head.number {
            sections.entry(n).or_insert(anchor);
        } else if let Some((name, chapter)) = head.keyword {
            let preferred = chapter == PREFERRED;
            // Every start of the name, such as `option` and `option pgsql-check` for
            // `option pgsql-check user`, stands for the group of the keywords that start with it.
            for (at, _) in name.match_indices(' ') {
                groups
                    .entry(name[..at].to_string())
                    .or_insert_with(|| anchor.clone());
            }
            match keywords.get(&name) {
                Some((_, true)) => {}
                Some(_) if !preferred => {}
                _ => {
                    keywords.insert(name, (anchor, preferred));
                }
            }
        }
    }
    let mut keywords: HashMap<String, String> =
        keywords.into_iter().map(|(k, (a, _))| (k, a)).collect();
    // The other names that a keyword shows on stacked syntax lines go to the same definition.
    for block in blocks {
        if let Block::Keyword { name, syntax, .. } = block
            && let Some(anchor) = keywords.get(name).cloned()
        {
            for line in syntax {
                let line = line.strip_prefix("no ").unwrap_or(line);
                if let Some(other) = super::blocks::keyword_name(line) {
                    keywords.entry(other).or_insert_with(|| anchor.clone());
                }
            }
        }
    }
    groups.retain(|word, _| !keywords.contains_key(word));
    let mut patterns = Vec::new();
    for block in blocks {
        if let Block::Keyword { name, syntax, .. } = block
            && let Some(anchor) = keywords.get(name)
        {
            for line in syntax {
                let line = line.strip_prefix("no ").unwrap_or(line);
                let head = line.split(['(', ' ']).next().unwrap_or(line);
                if head.contains('<') && head.contains('>') {
                    patterns.push((head.to_string(), anchor.clone()));
                }
            }
        }
    }
    Links {
        sections,
        keywords,
        groups,
        patterns,
    }
}

fn fence(lines: &[String], out: &mut String) {
    fence_as("text", lines, out);
}

/// Preformatted lines in a fenced block that names what they are.
fn fence_as(kind: &str, lines: &[String], out: &mut String) {
    let longest = lines
        .iter()
        .map(|l| l.matches('`').count())
        .max()
        .unwrap_or(0);
    let ticks = "`".repeat(if longest >= 3 { longest + 1 } else { 3 });
    out.push_str(&ticks);
    out.push_str(kind);
    out.push('\n');
    for l in lines {
        out.push_str(l);
        out.push('\n');
    }
    out.push_str(&ticks);
    out.push_str("\n\n");
}

/// The name of a keyword that a call names, with its arguments or without: `name`, `name(arg1)` and
/// `name(arg1,arg2)` are all `name`.
fn call_name(text: &str) -> &str {
    text.split('(').next().unwrap_or(text)
}

/// The end of a call that starts at the beginning of the text, up to the parenthesis that closes
/// its arguments, or the end of its name when it has none.
fn call_end(text: &str) -> usize {
    let name = call_name(text).len();
    if !text[name..].starts_with('(') {
        return name;
    }
    let mut depth = 0;
    for (i, c) in text[name..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return name + i + 1;
                }
            }
            _ => {}
        }
    }
    name
}

/// The text as Markdown inline: placeholders in code, special characters escaped, references to
/// sections and to quoted keywords as links.
fn inline(text: &str, links: &Links) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 8);
    let mut i = 0;
    let alnum = |c: Option<&char>| c.is_some_and(|c| c.is_alphanumeric());
    while i < chars.len() {
        let c = chars[i];
        match c {
            '<' => {
                if let Some(len) = chars[i + 1..]
                    .iter()
                    .position(|&x| x == '>')
                    .filter(|&n| n > 0 && n < 60)
                {
                    let inner: String = chars[i + 1..i + 1 + len].iter().collect();
                    if !inner.contains([' ', '`', '<']) {
                        if inner.contains("://") {
                            out.push_str(&format!("<{inner}>"));
                        } else {
                            out.push_str(&format!("`<{inner}>`"));
                        }
                        i += len + 2;
                        continue;
                    }
                }
                out.push_str("\\<");
            }
            '"' => {
                if let Some(len) = chars[i + 1..].iter().take(60).position(|&x| x == '"') {
                    let inner: String = chars[i + 1..i + 1 + len].iter().collect();
                    if call_end(&inner) == inner.len()
                        && let Some(anchor) = links.find(call_name(&inner))
                    {
                        out.push_str(&format!("[`{inner}`](#{anchor})"));
                        i += len + 2;
                        continue;
                    }
                }
                out.push('"');
            }
            // An address written in the text is a link to itself.
            'h' if (text_at(&chars, i, "http://") || text_at(&chars, i, "https://"))
                && !alnum(i.checked_sub(1).and_then(|p| chars.get(p))) =>
            {
                let mut end = i;
                while end < chars.len()
                    && !chars[end].is_whitespace()
                    && !"<>\"`".contains(chars[end])
                {
                    end += 1;
                }
                // What ends a sentence or closes a parenthesis does not belong to the address.
                while end > i
                    && (".,;:'!?".contains(chars[end - 1])
                        || (chars[end - 1] == ')' && !chars[i..end].contains(&'(')))
                {
                    end -= 1;
                }
                let url: String = chars[i..end].iter().collect();
                if url.len() > "https://".len() {
                    out.push_str(&format!("<{url}>"));
                    i = end;
                    continue;
                }
                out.push('h');
            }
            's' if text_at(&chars, i, "section ")
                && !alnum(i.checked_sub(1).and_then(|p| chars.get(p))) =>
            {
                let start = i + "section ".len();
                let mut end = start;
                while end < chars.len() && (chars[end].is_ascii_digit() || chars[end] == '.') {
                    end += 1;
                }
                let number: String = chars[start..end].iter().collect();
                let number = number.trim_end_matches('.');
                if let Some(anchor) = links.sections.get(number).filter(|_| !number.is_empty()) {
                    out.push_str(&format!("[section {number}](#{anchor})"));
                    i = start + number.chars().count();
                    continue;
                }
                out.push('s');
            }
            '\\' => out.push_str("\\\\"),
            '`' => out.push_str("\\`"),
            '*' => out.push_str("\\*"),
            '~' => out.push_str("\\~"),
            '[' => out.push_str("\\["),
            ']' => out.push_str("\\]"),
            '>' => out.push_str("\\>"),
            '_' if !(alnum(i.checked_sub(1).and_then(|p| chars.get(p)))
                && alnum(chars.get(i + 1))) =>
            {
                out.push_str("\\_")
            }
            c => out.push(c),
        }
        i += 1;
    }
    start_of_line(&out)
}

fn text_at(chars: &[char], at: usize, word: &str) -> bool {
    word.chars()
        .enumerate()
        .all(|(k, w)| chars.get(at + k) == Some(&w))
}

/// Escapes what would make the start of a paragraph a heading, a list or a quote.
fn start_of_line(text: &str) -> String {
    let b = text.as_bytes();
    let digits = b.iter().take_while(|c| c.is_ascii_digit()).count();
    if text.starts_with('#')
        || text.starts_with("+ ")
        || text.starts_with("- ")
        || text.starts_with("= ")
    {
        return format!("\\{text}");
    }
    if digits > 0
        && digits + 1 < b.len()
        && (b[digits] == b'.' || b[digits] == b')')
        && b[digits + 1] == b' '
    {
        return format!("{}\\{}", &text[..digits], &text[digits..]);
    }
    text.to_string()
}

/// The text of a list item, with the keyword it starts with as a link: the longest one that is
/// followed by a space, a colon or the end of the text.
/// An ordered list, numbered from `start`, with what is under each item indented to its text.
fn list_numbered(items: &[Item], start: u32, depth: usize, links: &Links, out: &mut String) {
    let pad = "  ".repeat(depth);
    for (k, item) in items.iter().enumerate() {
        let marker = format!("{}. ", start as usize + k);
        let inner_pad = " ".repeat(marker.len());
        out.push_str(&format!(
            "{pad}{marker}{}\n",
            breaks(&item_text(&item.text, links), &format!("{pad}{inner_pad}"))
        ));
        for block in &item.rest {
            let mut inner = String::new();
            content(block, links, &mut inner);
            out.push('\n');
            for line in inner.trim_end().lines() {
                if line.is_empty() {
                    out.push('\n');
                } else {
                    out.push_str(&format!("{pad}{inner_pad}{line}\n"));
                }
            }
        }
    }
}

/// A line break of the text as a hard break of Markdown, then what starts the next line.
fn breaks(text: &str, indent: &str) -> String {
    text.replace('\n', &format!("  \n{indent}"))
}

fn item_text(text: &str, links: &Links) -> String {
    // A call such as `streq(<str1>,<str2>)` links to the keyword it names.
    let end = call_end(text);
    if end > call_name(text).len()
        && let Some(anchor) = links.find(call_name(text))
        && (text.len() == end || text[end..].starts_with([' ', ':']))
    {
        return format!(
            "[`{}`](#{anchor}){}",
            &text[..end],
            inline(&text[end..], links)
        );
    }
    let words: Vec<&str> = text.split(' ').collect();
    for n in (1..=words.len().min(4)).rev() {
        let name = words[..n].join(" ");
        let rest = &text[name.len()..];
        if let Some(anchor) = links.keywords.get(&name)
            && (rest.is_empty() || rest.starts_with([' ', ':']))
        {
            return format!("[`{name}`](#{anchor}){}", inline(rest, links));
        }
    }
    let word = words[0];
    if let Some(anchor) = links.groups.get(word)
        && (text.len() == word.len() || text[word.len()..].starts_with(':'))
    {
        return format!(
            "[`{word}`](#{anchor}){}",
            inline(&text[word.len()..], links)
        );
    }
    inline(text, links)
}

fn list(items: &[Item], depth: usize, links: &Links, out: &mut String) {
    let pad = "  ".repeat(depth);
    for item in items {
        out.push_str(&pad);
        out.push_str("- ");
        out.push_str(&breaks(&item_text(&item.text, links), &format!("{pad}  ")));
        out.push('\n');
        for block in &item.rest {
            let mut inner = String::new();
            content(block, links, &mut inner);
            out.push('\n');
            for line in inner.trim_end().lines() {
                if line.is_empty() {
                    out.push('\n');
                } else {
                    out.push_str(&format!("{pad}  {line}\n"));
                }
            }
        }
    }
}

/// The places where a keyword is usable: those with a mark of yes.
/// The text of a table cell. A cell that starts with the marker the matrix gives to a keyword with
/// an anchor is a link to it.
fn cell(text: &str, links: &Links) -> String {
    // A cell that starts with the verbatim marker is code, whatever it holds.
    if let Some(code) = text.strip_prefix('\u{2}') {
        return format!("`{code}`");
    }
    let mut parts = text.splitn(4, '\u{1}');
    if parts.next() == Some("")
        && let (Some(name), Some(anchor), Some(rest)) = (parts.next(), parts.next(), parts.next())
    {
        return format!("[`{name}`](#{anchor}){}", inline(rest, links));
    }
    inline(text, links)
}

fn usable_line(rows: &[(String, String)]) -> String {
    let named: Vec<&str> = rows
        .iter()
        .filter(|(_, mark)| mark.starts_with(['X', 'x']) || mark.starts_with("yes"))
        .map(|(name, _)| name.as_str())
        .collect();
    if named.is_empty() {
        "none".to_string()
    } else {
        named.join(", ")
    }
}

fn sections_line(rows: &[(String, String)]) -> String {
    let named: Vec<String> = rows
        .iter()
        .filter_map(|(name, mark)| {
            let yes = mark.starts_with("yes") || mark.starts_with("X");
            match (yes, mark.contains("(!)")) {
                (true, true) => Some(format!("{name} (named only)")),
                (true, false) => Some(name.clone()),
                (false, _) if mark == "no" || mark == "-" || mark.is_empty() => None,
                _ => Some(format!("{name} ({mark})")),
            }
        })
        .collect();
    if named.is_empty() {
        "none".to_string()
    } else {
        named.join(", ")
    }
}

/// The Markdown of a document.
pub fn render(banner: &Banner, blocks: &[Block]) -> String {
    let links = links(banner, blocks);
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", title(banner)));
    let mut meta = Vec::new();
    if !banner.version.is_empty() {
        meta.push(format!("Version {}", banner.version));
    }
    if !banner.date.is_empty() {
        meta.push(banner.date.clone());
    }
    if !banner.author.is_empty() {
        meta.push(format!("by {}", banner.author));
    }
    let meta = meta.join(", ");
    let mut meta_done = meta.is_empty();
    let mut level = 2;
    let mut seen_heading = false;
    let heads = headings(banner, blocks);
    // The anchors are given in the order of the headings, as the site gives them.
    let anchors: Vec<String> = {
        let mut seen = HashMap::new();
        heads
            .iter()
            .map(|h| github_anchor(&h.text, &mut seen))
            .collect()
    };
    let mut keyword_heads = heads
        .iter()
        .zip(&anchors)
        .filter(|(h, _)| h.keyword.is_some());
    for (i, block) in blocks.iter().enumerate() {
        if !meta_done && (seen_heading || i == 1 || !matches!(blocks.first(), Some(Block::Para(_))))
        {
            out.push_str(&format!("{meta}.\n\n"));
            meta_done = true;
        }
        match block {
            Block::Heading { number, title } => {
                seen_heading = true;
                if number.is_empty() {
                    level = 2;
                    out.push_str(&format!("## {title}\n\n"));
                } else {
                    level = chapter_level(number);
                    out.push_str(&format!("{} {number}. {title}\n\n", "#".repeat(level)));
                }
            }
            Block::Summary(entries) => {
                for (number, title) in entries {
                    let depth = number.split('.').count() - 1;
                    out.push_str(&"  ".repeat(depth));
                    match links.sections.get(number) {
                        Some(anchor) => out.push_str(&format!(
                            "- [{number}. {}](#{anchor})\n",
                            inline_title(title)
                        )),
                        None => out.push_str(&format!("- {number}. {}\n", inline_title(title))),
                    }
                }
                out.push('\n');
            }
            Block::Keyword {
                name,
                syntax,
                deprecated,
            } => {
                seen_heading = true;
                let (head, anchor) = keyword_heads
                    .next()
                    .map(|(h, a)| ((h.text.clone(), h.code), Some(a.clone())))
                    .unwrap_or_else(|| ((name.clone(), false), None));
                let shown = if head.1 {
                    format!("`{}`", head.0)
                } else {
                    format!("`{name}`{}", &head.0[name.len()..])
                };
                // The whole heading is a link to itself.
                let shown = match anchor {
                    Some(a) => format!("[{shown}](#{a})"),
                    None => shown,
                };
                out.push_str(&format!("{} {shown}\n\n", "#".repeat((level + 1).min(6))));
                fence(syntax, &mut out);
                if *deprecated {
                    out.push_str("> [!WARNING]\n> This keyword is deprecated.\n\n");
                }
            }
            other => content(other, &links, &mut out),
        }
    }
    if !meta_done {
        out.push_str(&format!("{meta}.\n\n"));
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}

/// The Markdown of a block that is not a heading, a summary or a keyword.
fn content(block: &Block, links: &Links, out: &mut String) {
    match block {
        Block::Para(text) => out.push_str(&format!("{}\n\n", breaks(&inline(text, links), ""))),
        Block::Code(lines) => fence(lines, out),
        Block::Figure(lines) => fence_as("diagram", lines, out),
        Block::Definitions(items) => {
            for (name, description) in items {
                out.push_str(&format!(
                    "- `{name}` {}\n",
                    breaks(&inline(description, links), "  ")
                ));
            }
            out.push('\n');
        }
        Block::Ordered { start, items } => {
            list_numbered(items, *start, 0, links, out);
            out.push('\n');
        }
        Block::List(items) => {
            list(items, 0, links, out);
            out.push('\n');
        }
        Block::Contexts(c) => out.push_str(&format!("**Contexts:** {}\n\n", c.join(", "))),
        Block::Sections(rows) => {
            out.push_str(&format!("**Sections:** {}\n\n", sections_line(rows)))
        }
        Block::Arguments { text, items } => {
            if items.is_empty() && !text.is_empty() {
                out.push_str(&format!("**Arguments:** {}\n\n", inline(text, links)));
            } else if items.is_empty() {
                out.push_str("**Arguments:**\n\n");
            } else {
                out.push_str("**Arguments:**\n\n");
                for (name, description) in items {
                    out.push_str(&format!("- `{name}` {}\n", inline(description, links)));
                }
                out.push('\n');
            }
        }
        Block::Example { text, code } => {
            if code.is_empty() {
                out.push_str(&format!("**Example:** {}\n\n", inline(text, links)));
            } else {
                if text.is_empty() {
                    out.push_str("**Example:**\n\n");
                } else {
                    out.push_str(&format!("**Example:** {}\n\n", inline(text, links)));
                }
                // An example may draw a schema as well as show a configuration.
                if super::blocks::is_figure(code) {
                    fence_as("diagram", code, out);
                } else {
                    fence(code, out);
                }
            }
        }
        Block::Note { kind, text } => out.push_str(&format!(
            "> [!{kind}]\n> {}\n\n",
            breaks(&inline(text, links), "> ")
        )),
        Block::Subtitle(text) => out.push_str(&format!("**{}**\n\n", inline(text.trim(), links))),
        Block::Usable(rows) => out.push_str(&format!("**Usable in:** {}\n\n", usable_line(rows))),
        Block::Label(text) => out.push_str(&format!("**{}:**\n\n", inline(text.trim(), links))),
        Block::Table { header, rows } => {
            let line = |cells: &[String]| {
                format!(
                    "| {} |\n",
                    cells
                        .iter()
                        .map(|c| cell(c, links).replace('|', "\\|"))
                        .collect::<Vec<_>>()
                        .join(" | ")
                )
            };
            out.push_str(&line(header));
            out.push_str(&format!("|{}\n", " --- |".repeat(header.len())));
            let matrix = header.first().is_some_and(|h| h == "keyword");
            for row in rows {
                let mut row = row.clone();
                if matrix && let Some(first) = row.first_mut() {
                    // A row may carry the argument of the keyword, as `set-dst <expr>` does.
                    let name = first
                        .split(" (")
                        .next()
                        .unwrap_or(first)
                        .split(" <")
                        .next()
                        .unwrap_or(first)
                        .trim()
                        .to_string();
                    if let Some(anchor) = links
                        .find(call_name(&name))
                        .or_else(|| links.groups.get(name.as_str()))
                    {
                        let rest = first[name.len()..].to_string();
                        *first = format!("\u{1}{name}\u{1}{anchor}\u{1}{rest}");
                    }
                }
                if matrix
                    && row.len() > 1
                    && let Some(rest) = row[1].strip_prefix("(*)")
                {
                    row[1] = rest.trim().to_string();
                    row[0].push_str(" (*)");
                }
                out.push_str(&line(&row));
            }
            out.push('\n');
        }
        Block::SeeAlso(text) => out.push_str(&format!("**See also:** {}\n\n", inline(text, links))),
        Block::Heading { .. } | Block::Summary(_) | Block::Keyword { .. } => {}
    }
}

fn inline_title(title: &str) -> String {
    title
        .replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('<', "\\<")
        .replace('>', "\\>")
        .replace('`', "\\`")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_names_on_stacked_syntax_lines_link_to_the_same_definition() {
        let block = Block::Keyword {
            name: "harden.reject-privileged-ports.tcp".into(),
            syntax: vec![
                "harden.reject-privileged-ports.tcp { on | off }".into(),
                "harden.reject-privileged-ports.quic { on | off }".into(),
            ],
            deprecated: false,
        };
        let links = links(&Banner::default(), &[block]);
        let tcp = links.keywords.get("harden.reject-privileged-ports.tcp");
        assert!(tcp.is_some());
        assert_eq!(
            links.keywords.get("harden.reject-privileged-ports.quic"),
            tcp
        );
    }

    fn markdown_of(source: &str) -> String {
        crate::haproxy::convert(source).markdown
    }

    #[test]
    fn a_line_that_ends_a_sentence_breaks_and_a_line_that_only_wraps_does_not() {
        let text = "Title\n-----\n\nThis sentence ends here and the next line starts a new one in the text.\nThe next sentence is long enough to wrap over the width of the line and\nthen it stops there. What follows is a list of the things that matter:\nthe last line of the paragraph ends the text.\n";
        let md = markdown_of(text);
        assert!(
            md.contains(
                "in the text.  \nThe next sentence is long enough to wrap over the width of the line and then it stops there. What follows is a list of the things that matter:  \nthe last line"
            ),
            "{md}"
        );
    }

    #[test]
    fn a_break_inside_a_note_keeps_the_quote_marker() {
        let text = "Title\n-----\n\nNote: a first sentence.\n      A second one that wraps\n      onto this line.\n";
        let md = markdown_of(text);
        assert!(
            md.contains(
                "> [!NOTE]\n> a first sentence.  \n> A second one that wraps onto this line."
            ),
            "{md}"
        );
    }

    #[test]
    fn a_break_inside_an_item_stays_in_the_item() {
        let text = "Title\n-----\n\n  - first item, a sentence.\n    It goes on over\n    two lines.\n  - second item\n";
        let md = markdown_of(text);
        assert!(
            md.contains("- first item, a sentence.  \n  It goes on over two lines.\n"),
            "{md}"
        );
    }

    #[test]
    fn a_line_breaks_after_its_punctuation_unless_it_is_full() {
        use super::super::text::{Line, separator};
        let line = |text| Line { text, indent: 0 };
        let short = line("A short line that ends here.");
        assert_eq!(separator(&short, &line("Next")), '\n');
        assert_eq!(separator(&line("and so on..."), &line("Next")), '\n');
        assert_eq!(separator(&line("a list:"), &line("Next")), '\n');
        assert_eq!(separator(&line("wraps over"), &line("Next")), ' ');
        assert_eq!(separator(&line("quoted \"end.\""), &line("Next")), '\n');
        let near = "x".repeat(76) + "end.";
        assert_eq!(
            separator(&line(&near), &line("a")),
            ' ',
            "ends at the width"
        );
        let seventy = "y".repeat(66) + "end.";
        assert_eq!(
            separator(&line(&seventy), &line("It")),
            '\n',
            "the next word fits"
        );
        assert_eq!(
            separator(&line(&seventy), &line("overwhelmingly")),
            ' ',
            "the next word could not come in"
        );
        let indented = Line {
            text: &seventy,
            indent: 8,
        };
        assert_eq!(separator(&indented, &line("It")), ' ', "the indent counts");
    }
}
