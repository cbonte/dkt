//! The inline markup of reStructuredText, as Markdown.

use std::cell::Cell;
use std::collections::HashMap;

use super::blocks::Substitute;

/// What the references of a document lead to.
#[derive(Default)]
pub struct Refs {
    /// The anchor of the title that a label stands above.
    pub labels: HashMap<String, String>,
    /// The text of that title.
    pub texts: HashMap<String, String>,
    /// The anchor of each title, by its text in lower case.
    pub titles: HashMap<String, String>,
    /// The anchor of each documented object, by its name.
    pub objects: HashMap<String, String>,
    /// The addresses that names were given, in lower case.
    pub urls: HashMap<String, String>,
    /// The addresses of the anonymous targets, in order, and how many were used.
    pub anonymous: Vec<String>,
    pub anonymous_used: Cell<usize>,
    /// The footnotes that a label names, and the citations, by label in lower case.
    pub notes: HashMap<String, String>,
    /// The numbers given to the footnotes with `#`, and to the ones with `*`, in order.
    pub auto_notes: Vec<String>,
    pub auto_used: Cell<usize>,
    pub symbol_notes: Vec<String>,
    pub symbol_used: Cell<usize>,
    /// The id of each footnote definition, in order, and how many were written.
    pub definitions: Vec<String>,
    pub definitions_written: Cell<usize>,
    /// What the substitutions stand for.
    pub substitutions: HashMap<String, Substitute>,
}

/// How deep a substitution may hold another one.
const SUBSTITUTION_DEPTH: usize = 4;

/// The text of some inline markup without the markup: what a title is named by.
pub fn plain(text: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\\' if i + 1 < chars.len() => {
                out.push(chars[i + 1]);
                i += 2;
            }
            '`' | '*' => i += 1,
            ':' => {
                // A role: `:ref:` before a backtick.
                let rest: String = chars[i + 1..].iter().collect();
                match rest.find(":`") {
                    Some(at)
                        if rest[..at]
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || ":+.-".contains(c)) =>
                    {
                        i += at + 2;
                    }
                    _ => {
                        out.push(':');
                        i += 1;
                    }
                }
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// Code in Markdown: with enough backticks around it for the ones it holds.
fn code(text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let ticks = "`".repeat(longest + 1);
    if text.starts_with('`') || text.ends_with('`') {
        format!("{ticks} {text} {ticks}")
    } else {
        format!("{ticks}{text}{ticks}")
    }
}

fn escaped(c: char, before: Option<char>, after: Option<char>, out: &mut String) {
    let word = |x: Option<char>| x.is_some_and(char::is_alphanumeric);
    match c {
        '\\' | '*' | '[' | ']' | '<' | '`' | '|' => {
            out.push('\\');
            out.push(c);
        }
        '_' if !(word(before) && word(after)) => out.push_str("\\_"),
        c => out.push(c),
    }
}

fn find(chars: &[char], from: usize, needle: &str) -> Option<usize> {
    let n: Vec<char> = needle.chars().collect();
    (from..chars.len().saturating_sub(n.len() - 1)).find(|&k| chars[k..k + n.len()] == n[..])
}

fn link(text: &str, anchor: Option<&String>) -> String {
    match anchor {
        Some(a) => format!("[{text}](#{a})"),
        None => text.to_string(),
    }
}

fn role(name: &str, content: &str, refs: &Refs) -> String {
    match name {
        "ref" | "doc" | "numref" => {
            let (text, label) = match content
                .rsplit_once(" <")
                .and_then(|(t, l)| l.strip_suffix('>').map(|l| (t, l)))
            {
                Some((t, l)) => (Some(t.trim().to_string()), l.trim().to_string()),
                None => (None, content.trim().to_string()),
            };
            let anchor = refs.labels.get(&label.to_lowercase());
            let text = text
                .or_else(|| refs.texts.get(&label.to_lowercase()).cloned())
                .unwrap_or(label);
            link(&inline_plain(&text), anchor)
        }
        // A role of a domain, such as `js:func` and `py:class`, names an object.
        _ if matches!(
            name.rsplit(':').next(),
            Some(
                "func"
                    | "function"
                    | "attr"
                    | "attribute"
                    | "class"
                    | "meth"
                    | "method"
                    | "data"
                    | "obj"
                    | "exc"
                    | "type"
                    | "macro"
                    | "var"
                    | "member"
            )
        ) =>
        {
            let shown = content.trim_start_matches('~');
            let key = shown.trim_end_matches("()");
            match refs.objects.get(key) {
                Some(a) => format!("[{}](#{a})", code(shown)),
                None => code(shown),
            }
        }
        "sup" => format!("<sup>{}</sup>", inline_plain(content)),
        "sub" => format!("<sub>{}</sub>", inline_plain(content)),
        "kbd" => format!("<kbd>{}</kbd>", inline_plain(content)),
        "math" => format!("$`{content}`$"),
        "abbr" => inline_plain(content.split(" (").next().unwrap_or(content).trim()),
        "guilabel" | "menuselection" | "strong-ref" => format!("**{}**", inline_plain(content)),
        "emphasis" => format!("*{}*", inline_plain(content)),
        "strong" => format!("**{}**", inline_plain(content)),
        _ => code(content),
    }
}

/// Text with the characters that Markdown reads escaped, and no other markup.
fn inline_plain(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    for (k, &c) in chars.iter().enumerate() {
        escaped(
            c,
            k.checked_sub(1).map(|p| chars[p]),
            chars.get(k + 1).copied(),
            &mut out,
        );
    }
    out
}

/// The inline markup of a text as Markdown.
pub fn inline(text: &str, refs: &Refs) -> String {
    inline_at(text, refs, 0)
}

/// The id that a footnote reference leads to, `[1]_`, `[#]_`, `[#name]_`, `[*]_` or `[label]_`.
fn note_id(label: &str, refs: &Refs) -> Option<String> {
    let next = |ids: &Vec<String>, used: &Cell<usize>| {
        let id = ids.get(used.get()).cloned();
        used.set(used.get() + 1);
        id
    };
    match label {
        "#" => next(&refs.auto_notes, &refs.auto_used),
        "*" => next(&refs.symbol_notes, &refs.symbol_used),
        l if l.chars().all(|c| c.is_ascii_digit()) => Some(l.to_string()),
        l => refs
            .notes
            .get(&l.trim_start_matches('#').to_lowercase())
            .cloned(),
    }
}

/// A reference of one word followed by an underscore, such as `Python_`, when its name is known.
fn simple_reference(chars: &[char], i: usize, refs: &Refs) -> Option<(String, usize)> {
    let mut end = i;
    while end < chars.len()
        && (chars[end].is_alphanumeric()
            || ("-.+:_".contains(chars[end])
                && chars.get(end + 1).is_some_and(|c| c.is_alphanumeric())))
    {
        end += 1;
    }
    if end == i || chars.get(end) != Some(&'_') {
        return None;
    }
    let mut next = end + 1;
    let anonymous = chars.get(next) == Some(&'_');
    if anonymous {
        next += 1;
    }
    if chars.get(next).is_some_and(|c| c.is_alphanumeric()) {
        return None;
    }
    let name: String = chars[i..end].iter().collect();
    let key = name.to_lowercase();
    let text = inline_plain(&name);
    if anonymous {
        let url = refs.anonymous.get(refs.anonymous_used.get())?;
        refs.anonymous_used.set(refs.anonymous_used.get() + 1);
        return Some((format!("[{text}]({url})"), next));
    }
    if let Some(url) = refs.urls.get(&key) {
        return Some((format!("[{text}]({url})"), next));
    }
    let anchor = refs.titles.get(&key).or_else(|| refs.labels.get(&key))?;
    Some((format!("[{text}](#{anchor})"), next))
}

fn inline_at(text: &str, refs: &Refs, depth: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let word = |x: Option<char>| x.is_some_and(char::is_alphanumeric);
    let mut out = String::with_capacity(text.len() + 8);
    let mut i = 0;
    while i < n {
        let c = chars[i];
        let before = i.checked_sub(1).map(|p| chars[p]);
        let after = chars.get(i + 1).copied();
        if c == '\\' && i + 1 < n {
            escaped(chars[i + 1], None, None, &mut out);
            i += 2;
            continue;
        }
        if c == '['
            && !word(before)
            && let Some(close) = find(&chars, i + 1, "]")
            && close > i + 1
            && chars.get(close + 1) == Some(&'_')
            && !word(chars.get(close + 2).copied())
            && let Some(id) = note_id(&chars[i + 1..close].iter().collect::<String>(), refs)
        {
            out.push_str(&format!("[^{id}]"));
            i = close + 2;
            continue;
        }
        if c == '|'
            && !word(before)
            && after.is_some_and(|a| !a.is_whitespace())
            && let Some(close) = find(&chars, i + 1, "|")
            && close > i + 1
            && !chars[close - 1].is_whitespace()
            && depth < SUBSTITUTION_DEPTH
            && let Some(value) = refs
                .substitutions
                .get(&chars[i + 1..close].iter().collect::<String>())
        {
            match value {
                Substitute::Text(text) => out.push_str(&inline_at(text, refs, depth + 1)),
                Substitute::Image { src, alt } => out.push_str(&format!("![{alt}]({src})")),
                Substitute::Chars(text) => out.push_str(&inline_plain(text)),
            }
            i = close + 1;
            continue;
        }
        if c.is_alphanumeric()
            && !word(before)
            && let Some((link, next)) = simple_reference(&chars, i, refs)
        {
            out.push_str(&link);
            i = next;
            continue;
        }
        if c == '`'
            && after == Some('`')
            && let Some(end) = find(&chars, i + 2, "``")
        {
            out.push_str(&code(&chars[i + 2..end].iter().collect::<String>()));
            i = end + 2;
            continue;
        }
        if c == ':'
            && !word(before)
            && let Some(tick) = (i + 1..n)
                .find(|&k| !(chars[k].is_ascii_alphanumeric() || "+.-:".contains(chars[k])))
            && chars[tick] == '`'
            && tick > i + 2
            && chars[tick - 1] == ':'
            && let Some(end) = find(&chars, tick + 1, "`")
        {
            let name: String = chars[i + 1..tick - 1].iter().collect();
            let content: String = chars[tick + 1..end].iter().collect();
            out.push_str(&role(&name, &content, refs));
            i = end + 1;
            continue;
        }
        if c == '`'
            && let Some(end) = find(&chars, i + 1, "`")
            && end > i + 1
        {
            let content: String = chars[i + 1..end].iter().collect();
            let mut next = end + 1;
            if chars.get(next) == Some(&'_') {
                next += 1;
                let anonymous = chars.get(next) == Some(&'_');
                if anonymous {
                    next += 1;
                }
                out.push_str(&if anonymous {
                    anonymous_reference(&content, refs)
                } else {
                    reference(&content, refs)
                });
            } else {
                out.push_str(&code(&content));
            }
            i = next;
            continue;
        }
        if c == '*'
            && after == Some('*')
            && let Some(end) = find(&chars, i + 2, "**")
            && end > i + 2
        {
            out.push_str(&format!(
                "**{}**",
                inline(&chars[i + 2..end].iter().collect::<String>(), refs)
            ));
            i = end + 2;
            continue;
        }
        if c == '*'
            && !word(before)
            && after.is_some_and(|a| !a.is_whitespace() && a != '*')
            && let Some(end) = find(&chars, i + 1, "*")
            && end > i + 1
            && !chars[end - 1].is_whitespace()
            && !word(chars.get(end + 1).copied())
        {
            out.push_str(&format!(
                "*{}*",
                inline(&chars[i + 1..end].iter().collect::<String>(), refs)
            ));
            i = end + 1;
            continue;
        }
        if (c == 'h')
            && !word(before)
            && (chars[i..].starts_with(&['h', 't', 't', 'p', ':', '/', '/'])
                || chars[i..].starts_with(&['h', 't', 't', 'p', 's', ':', '/', '/']))
        {
            let mut end = i;
            while end < n && !chars[end].is_whitespace() && !"<>\"`".contains(chars[end]) {
                end += 1;
            }
            while end > i && ".,;:)'!?".contains(chars[end - 1]) {
                end -= 1;
            }
            let url: String = chars[i..end].iter().collect();
            if url.len() > 8 {
                out.push_str(&format!("<{url}>"));
                i = end;
                continue;
            }
        }
        escaped(c, before, after, &mut out);
        i += 1;
    }
    out
}

/// A reference in backticks and 2 underscores, which leads to its own address or to the next
/// anonymous target.
fn anonymous_reference(content: &str, refs: &Refs) -> String {
    if let Some((text, url)) = content
        .rsplit_once(" <")
        .and_then(|(t, l)| l.strip_suffix('>').map(|l| (t, l)))
    {
        return format!("[{}]({url})", inline_plain(text.trim()));
    }
    let text = inline_plain(content.trim());
    match refs.anonymous.get(refs.anonymous_used.get()) {
        Some(url) => {
            refs.anonymous_used.set(refs.anonymous_used.get() + 1);
            format!("[{text}]({url})")
        }
        None => text,
    }
}

/// A reference in backticks and an underscore: `` `text <address>`_ `` or `` `name`_ ``.
fn reference(content: &str, refs: &Refs) -> String {
    if let Some((text, target)) = content
        .rsplit_once(" <")
        .and_then(|(t, l)| l.strip_suffix('>').map(|l| (t, l)))
    {
        let text = inline_plain(text.trim());
        if let Some(label) = target.strip_suffix('_') {
            return link(
                &text,
                refs.titles
                    .get(&label.to_lowercase())
                    .or_else(|| refs.labels.get(&label.to_lowercase())),
            );
        }
        return format!("[{text}]({target})");
    }
    let key = content.trim().to_lowercase();
    if let Some(url) = refs.urls.get(&key) {
        return format!("[{}]({url})", inline_plain(content.trim()));
    }
    link(
        &inline_plain(content.trim()),
        refs.titles.get(&key).or_else(|| refs.labels.get(&key)),
    )
}
