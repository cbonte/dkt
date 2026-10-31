//! The blocks of a reStructuredText document.

use super::tables;

/// A line of the text without its indent.
#[derive(Clone, Debug)]
pub struct Line {
    pub indent: usize,
    pub text: String,
}

impl Line {
    pub(super) fn blank(&self) -> bool {
        self.text.is_empty()
    }
}

/// A block of the document.
#[derive(Debug, PartialEq, Eq)]
pub enum Block {
    /// A title, whose level is the order in which its adornment first came.
    Heading {
        level: usize,
        text: String,
        labels: Vec<String>,
    },
    Para(String),
    /// Preformatted lines, with the language a code block names.
    Literal {
        lang: String,
        lines: Vec<String>,
    },
    List {
        ordered: bool,
        start: usize,
        items: Vec<Vec<Block>>,
    },
    /// A note, a warning and the like.
    Admonition {
        kind: String,
        body: Vec<Block>,
    },
    /// What a domain documents: a function, a class or an attribute.
    Object {
        kind: String,
        signature: String,
        body: Vec<Block>,
        labels: Vec<String>,
    },
    /// The fields of a list such as `:param str name: text`, each with its text.
    Fields(Vec<(String, String)>),
    Quote(Vec<Block>),
    Image {
        src: String,
        alt: String,
    },
    /// What is not understood, kept as it is.
    Raw(Vec<String>),
    /// A name for an address, `.. _name: address`.
    Target {
        label: String,
        address: String,
    },
    /// A table, with its header row and its rows, each cell holding blocks.
    Table {
        header: Vec<Vec<Block>>,
        rows: Vec<Vec<Vec<Block>>>,
    },
    /// Terms, each with what defines it.
    Definitions(Vec<(String, Vec<Block>)>),
    /// Lines that keep their breaks, from a line block.
    Lines(Vec<String>),
    /// A footnote or a citation, `.. [label] text`.
    Footnote {
        label: String,
        body: Vec<Block>,
    },
    /// What a name stands for in the text, `.. |name| replace:: text`.
    Substitution {
        name: String,
        value: Substitute,
    },
    /// The options of a command, each with its description.
    Options(Vec<(String, String)>),
    /// Blocks that stand together, from a directive that only holds them.
    Group(Vec<Block>),
    /// A title that belongs to no section.
    Rubric(String),
    /// A transition between 2 parts of a section.
    Rule,
    /// HTML that a `raw` directive carries.
    Html(Vec<String>),
}

/// What a substitution stands for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Substitute {
    Text(String),
    Image { src: String, alt: String },
    Chars(String),
}

/// The lines of a text, with the tabs as spaces to the next multiple of 8.
pub fn lines(source: &str) -> Vec<Line> {
    source
        .lines()
        .map(|raw| {
            let mut expanded = String::new();
            let mut col = 0;
            for c in raw.trim_end().chars() {
                if c == '\t' {
                    let to = (col / 8 + 1) * 8;
                    expanded.extend(std::iter::repeat_n(' ', to - col));
                    col = to;
                } else {
                    expanded.push(c);
                    col += 1;
                }
            }
            let text = expanded.trim_start_matches(' ');
            Line {
                indent: expanded.len() - text.len(),
                text: text.to_string(),
            }
        })
        .collect()
}

/// The characters that can underline a title.
const ADORNMENTS: &str = "=-`:.'\"~^_*+#<>";

fn adornment(text: &str) -> Option<char> {
    let first = text.chars().next()?;
    (ADORNMENTS.contains(first) && text.len() >= 3 && text.chars().all(|c| c == first))
        .then_some(first)
}

/// The name and the arguments of a directive line, `.. name:: arguments`.
fn directive(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix(".. ")?;
    let at = rest.find("::")?;
    // One space may come before the colons, as in `.. note ::`.
    let name = rest[..at].strip_suffix(' ').unwrap_or(&rest[..at]);
    let after = &rest[at + 2..];
    (!name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ":+._-".contains(c))
        && (after.is_empty() || after.starts_with(' ')))
    .then(|| (name, after.trim()))
}

/// The label and the address of a target, `.. _label: address`.
/// The label ends at the first colon that is followed by a space or by the end of the line, and
/// it may be written between backticks when it holds a colon.
fn target(text: &str) -> Option<(String, String)> {
    let rest = text.strip_prefix(".. _")?;
    let (label, address) = if let Some(quoted) = rest.strip_prefix('`') {
        let end = quoted.find("`:")?;
        (&quoted[..end], &quoted[end + 2..])
    } else {
        let bytes = rest.as_bytes();
        let at = (0..bytes.len()).find(|&k| {
            bytes[k] == b':'
                && (k == 0 || bytes[k - 1] != b'\\')
                && (k + 1 == bytes.len() || bytes[k + 1] == b' ')
        })?;
        (&rest[..at], &rest[at + 1..])
    };
    let label = label.trim().to_string();
    (!label.is_empty()).then(|| (label, address.trim().to_string()))
}

/// The marker of a list item and the width it takes: `* `, `1. ` and the like.
fn marker(text: &str) -> Option<(bool, usize, usize)> {
    let mut chars = text.chars();
    let first = chars.next()?;
    if "*+-".contains(first) && chars.next() == Some(' ') {
        return Some((false, 0, 2));
    }
    // `(1) ` is a number between parentheses.
    if let Some(rest) = text.strip_prefix('(') {
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        if digits > 0 && rest[digits..].starts_with(") ") {
            return Some((true, rest[..digits].parse().unwrap_or(1), digits + 3));
        }
    }
    let digits = text.chars().take_while(char::is_ascii_digit).count();
    let hash = text.starts_with('#');
    let len = if hash { 1 } else { digits };
    if len > 0
        && matches!(text[len..].chars().next(), Some('.' | ')'))
        && text[len + 1..].starts_with(' ')
    {
        let start = if hash {
            1
        } else {
            text[..len].parse().unwrap_or(1)
        };
        return Some((true, start, len + 2));
    }
    None
}

/// The name and the text of a field, `:name: text`.
fn field(text: &str) -> Option<(String, String)> {
    let rest = text.strip_prefix(':')?;
    let mut at = None;
    let bytes = rest.as_bytes();
    for k in 0..bytes.len() {
        if bytes[k] == b':'
            && (k + 1 == bytes.len() || bytes[k + 1] == b' ')
            && (k == 0 || bytes[k - 1] != b'\\')
        {
            at = Some(k);
            break;
        }
    }
    let k = at?;
    let name = rest[..k].trim().to_string();
    (!name.is_empty() && !name.starts_with(' ')).then(|| (name, rest[k + 1..].trim().to_string()))
}

/// The lines that belong to the construct that starts at `from`: the ones after it that are blank
/// or indented deeper than `indent`, without the blank lines at the end.
fn indented(ls: &[Line], from: usize, indent: usize) -> usize {
    let mut end = from;
    let mut last = from;
    while end < ls.len() {
        if ls[end].blank() {
            end += 1;
        } else if ls[end].indent > indent {
            end += 1;
            last = end;
        } else {
            break;
        }
    }
    last
}

/// The lines moved left by the smallest indent they have.
pub(super) fn dedent(ls: &[Line]) -> Vec<Line> {
    let min = ls
        .iter()
        .filter(|l| !l.blank())
        .map(|l| l.indent)
        .min()
        .unwrap_or(0);
    ls.iter()
        .map(|l| Line {
            indent: l.indent.saturating_sub(min),
            text: l.text.clone(),
        })
        .collect()
}

/// The label and the text of a footnote or a citation, `.. [label] text`.
fn footnote(text: &str) -> Option<(String, String)> {
    let rest = text.strip_prefix(".. [")?;
    let at = rest.find(']')?;
    let after = &rest[at + 1..];
    let label = rest[..at].to_string();
    (!label.is_empty() && (after.is_empty() || after.starts_with(' ')))
        .then(|| (label, after.trim().to_string()))
}

/// The name, the directive and its arguments of a substitution, `.. |name| replace:: text`.
fn substitution(text: &str) -> Option<(String, String, String)> {
    let rest = text.strip_prefix(".. |")?;
    let at = rest.find('|')?;
    let name = rest[..at].trim().to_string();
    let line = format!(".. {}", rest[at + 1..].trim());
    let (kind, args) = directive(&line)?;
    (!name.is_empty()).then(|| (name, kind.to_string(), args.to_string()))
}

/// The characters that a `unicode` substitution names, as `U+00A9`, `0xA9`, `&#169;` or text.
fn characters(args: &str) -> String {
    let mut out = String::new();
    for word in args.split_whitespace() {
        if word == ".." {
            break;
        }
        let code = word
            .strip_prefix("U+")
            .or_else(|| word.strip_prefix("u+"))
            .or_else(|| word.strip_prefix("0x"))
            .or_else(|| word.strip_prefix("\\u"))
            .or_else(|| word.strip_prefix("&#x").and_then(|w| w.strip_suffix(';')))
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
            .or_else(|| {
                word.strip_prefix("&#")
                    .and_then(|w| w.strip_suffix(';'))
                    .and_then(|dec| dec.parse().ok())
            })
            .and_then(char::from_u32);
        match code {
            Some(c) => out.push(c),
            None => out.push_str(word),
        }
    }
    out
}

/// An option of a command followed by its description, `-a, --all   text`.
fn option_line(text: &str) -> Option<(String, String)> {
    let at = text.find("  ")?;
    let (options, description) = (&text[..at], text[at..].trim());
    let valid = |part: &str| {
        let name = part.trim_start_matches(['-', '/']);
        let dashes = part.len() - name.len();
        let word_end = name
            .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-'))
            .unwrap_or(name.len());
        let argument = &name[word_end..];
        (1..=2).contains(&dashes)
            && word_end > 0
            && (argument.is_empty()
                || (argument.len() > 1
                    && (argument.starts_with('=') || argument.starts_with(' '))
                    && !argument[1..].contains(' ')))
    };
    (!description.is_empty() && options.split(", ").all(valid))
        .then(|| (options.to_string(), description.to_string()))
}

/// The options that start at `from`, each with the lines that go on under it.
fn options(ls: &[Line], from: usize) -> (Vec<(String, String)>, usize) {
    let indent = ls[from].indent;
    let mut out = Vec::new();
    let mut i = from;
    while i < ls.len() {
        if ls[i].blank() {
            let next = ls[i..].iter().position(|l| !l.blank()).map(|k| i + k);
            match next {
                Some(n) if ls[n].indent == indent && option_line(&ls[n].text).is_some() => {
                    i = n;
                    continue;
                }
                _ => break,
            }
        }
        if ls[i].indent != indent {
            break;
        }
        let Some((option, mut text)) = option_line(&ls[i].text) else {
            break;
        };
        i += 1;
        while i < ls.len() && !ls[i].blank() && ls[i].indent > indent {
            text.push(' ');
            text.push_str(&ls[i].text);
            i += 1;
        }
        out.push((option, text));
    }
    (out, i)
}

/// Terms that start at `from`: a line, and the lines under it that are indented deeper.
fn definitions(ls: &[Line], from: usize, st: &mut State) -> Option<(Block, usize)> {
    let indent = ls[from].indent;
    let term_at = |k: usize| {
        k + 1 < ls.len()
            && !ls[k].blank()
            && ls[k].indent == indent
            && !ls[k].text.ends_with("::")
            && !ls[k + 1].blank()
            && ls[k + 1].indent > indent
    };
    if !term_at(from) {
        return None;
    }
    let mut items = Vec::new();
    let mut i = from;
    while term_at(i) {
        let end = indented(ls, i + 1, indent);
        items.push((ls[i].text.clone(), blocks(&dedent(&ls[i + 1..end]), st)));
        i = end;
        let next = ls[i..].iter().position(|l| !l.blank()).map(|k| i + k);
        match next {
            Some(n) if term_at(n) => i = n,
            _ => break,
        }
    }
    Some((Block::Definitions(items), i))
}

pub(super) struct State {
    /// The adornments of the titles in the order they came, with whether there is an overline.
    styles: Vec<(char, bool)>,
    pub(super) labels: Vec<String>,
}

/// The blocks of a document.
pub fn parse(ls: &[Line]) -> Vec<Block> {
    let mut state = State {
        styles: Vec::new(),
        labels: Vec::new(),
    };
    blocks(ls, &mut state)
}

fn admonition(name: &str) -> Option<&'static str> {
    Some(match name.to_ascii_lowercase().as_str() {
        "note" | "seealso" => "NOTE",
        "warning" | "attention" | "danger" | "error" => "WARNING",
        "important" => "IMPORTANT",
        "tip" | "hint" => "TIP",
        "caution" => "CAUTION",
        _ => return None,
    })
}

pub(super) fn blocks(ls: &[Line], st: &mut State) -> Vec<Block> {
    let mut out = Vec::new();
    let mut i = 0;
    let base = ls.iter().find(|l| !l.blank()).map_or(0, |l| l.indent);
    while i < ls.len() {
        if ls[i].blank() {
            i += 1;
            continue;
        }
        let l = &ls[i];
        if l.indent > base {
            let end = indented(ls, i, base);
            out.push(Block::Quote(blocks(&dedent(&ls[i..end]), st)));
            i = end;
            continue;
        }
        // A comment, which an empty one only ends the construct before it.
        if l.text == ".." {
            i += 1;
            continue;
        }
        if let Some((label, first)) = footnote(&l.text) {
            let end = indented(ls, i + 1, l.indent).max(i + 1);
            let mut lines = vec![Line {
                indent: 0,
                text: first,
            }];
            lines.extend(dedent(&ls[i + 1..end]));
            out.push(Block::Footnote {
                label,
                body: blocks(&lines, st),
            });
            i = end;
            continue;
        }
        if let Some((name, kind, args)) = substitution(&l.text) {
            let end = indented(ls, i + 1, l.indent).max(i + 1);
            let body = dedent(&ls[i + 1..end]);
            let value = match kind.as_str() {
                "replace" => {
                    let more: Vec<&str> = body.iter().map(|b| b.text.as_str()).collect();
                    Some(Substitute::Text(
                        std::iter::once(args.as_str())
                            .chain(more)
                            .filter(|s| !s.is_empty())
                            .collect::<Vec<_>>()
                            .join(" "),
                    ))
                }
                "image" => Some(Substitute::Image {
                    alt: body
                        .iter()
                        .find_map(|b| b.text.strip_prefix(":alt:").map(|a| a.trim().to_string()))
                        .unwrap_or_default(),
                    src: args,
                }),
                "unicode" => Some(Substitute::Chars(characters(&args))),
                _ => None,
            };
            if let Some(value) = value {
                out.push(Block::Substitution { name, value });
            }
            i = end;
            continue;
        }
        if let Some(address) = l.text.strip_prefix("__ ") {
            out.push(Block::Target {
                label: "_".to_string(),
                address: address.trim().to_string(),
            });
            i += 1;
            continue;
        }
        if l.text.starts_with(".. ") && directive(&l.text).is_none() && target(&l.text).is_none() {
            i = indented(ls, i + 1, l.indent).max(i + 1);
            continue;
        }
        if let Some((label, address)) = target(&l.text) {
            if address.is_empty() {
                st.labels.push(label);
            } else {
                out.push(Block::Target { label, address });
            }
            i += 1;
            continue;
        }
        if let Some((name, args)) = directive(&l.text) {
            let end = indented(ls, i + 1, l.indent).max(i + 1);
            let body = dedent(&ls[i + 1..end]);
            out.push(directive_block(name, args, &body, &ls[i..end], st));
            i = end;
            continue;
        }
        // A title, with an underline and maybe an overline.
        if let Some(next) = ls.get(i + 1)
            && next.indent == l.indent
            && let Some(ch) = adornment(&next.text)
            && adornment(&l.text).is_none()
            && !l.text.starts_with(".. ")
        {
            let style = (ch, false);
            let level = style_level(st, style);
            out.push(Block::Heading {
                level,
                text: l.text.clone(),
                labels: std::mem::take(&mut st.labels),
            });
            i += 2;
            continue;
        }
        if let Some(ch) = adornment(&l.text)
            && let Some(title) = ls.get(i + 1)
            && let Some(under) = ls.get(i + 2)
            && adornment(&under.text) == Some(ch)
            && !title.blank()
        {
            let level = style_level(st, (ch, true));
            out.push(Block::Heading {
                level,
                text: title.text.clone(),
                labels: std::mem::take(&mut st.labels),
            });
            i += 3;
            continue;
        }
        if let Some((table, end)) = tables::grid(ls, i, st).or_else(|| tables::simple(ls, i, st)) {
            out.push(table);
            i = end;
            continue;
        }
        if adornment(&l.text).is_some() && ls.get(i + 1).is_none_or(Line::blank) {
            out.push(Block::Rule);
            i += 1;
            continue;
        }
        if l.text == "|" || l.text.starts_with("| ") {
            let mut lines: Vec<String> = Vec::new();
            while i < ls.len() && !ls[i].blank() && ls[i].indent >= l.indent {
                let text = &ls[i].text;
                if ls[i].indent == l.indent && (text == "|" || text.starts_with("| ")) {
                    lines.push(text[1..].strip_prefix(' ').unwrap_or("").to_string());
                } else if let Some(last) = lines.last_mut() {
                    last.push(' ');
                    last.push_str(text);
                }
                i += 1;
            }
            out.push(Block::Lines(lines));
            continue;
        }
        if l.text.starts_with(">>> ") {
            let mut lines = Vec::new();
            while i < ls.len() && !ls[i].blank() {
                lines.push(format!(
                    "{}{}",
                    " ".repeat(ls[i].indent - l.indent),
                    ls[i].text
                ));
                i += 1;
            }
            out.push(Block::Literal {
                lang: "pycon".to_string(),
                lines,
            });
            continue;
        }
        if let Some((ordered, start, width)) = marker(&l.text) {
            let (items, end) = list(ls, i, ordered, width, st);
            out.push(Block::List {
                ordered,
                start,
                items,
            });
            i = end;
            continue;
        }
        if field(&l.text).is_some() {
            let (fields, end) = fields(ls, i);
            out.push(Block::Fields(fields));
            i = end;
            continue;
        }
        if option_line(&l.text).is_some() {
            let (items, end) = options(ls, i);
            out.push(Block::Options(items));
            i = end;
            continue;
        }
        if let Some((defs, end)) = definitions(ls, i, st) {
            out.push(defs);
            i = end;
            continue;
        }
        i = paragraph(ls, i, &mut out, st);
    }
    out
}

fn style_level(st: &mut State, style: (char, bool)) -> usize {
    match st.styles.iter().position(|s| *s == style) {
        Some(k) => k + 1,
        None => {
            st.styles.push(style);
            st.styles.len()
        }
    }
}

/// A list that starts at `from` with items that have the same kind of marker.
fn list(
    ls: &[Line],
    from: usize,
    ordered: bool,
    width: usize,
    st: &mut State,
) -> (Vec<Vec<Block>>, usize) {
    let indent = ls[from].indent;
    let mut items = Vec::new();
    let mut i = from;
    let _ = width;
    while i < ls.len() {
        if ls[i].blank() {
            // A blank line goes on with the list when another item follows at the same indent.
            let next = ls[i..].iter().position(|l| !l.blank()).map(|k| i + k);
            match next {
                Some(n)
                    if ls[n].indent == indent
                        && marker(&ls[n].text).is_some_and(|m| m.0 == ordered) =>
                {
                    i = n;
                    continue;
                }
                _ => break,
            }
        }
        if ls[i].indent != indent {
            break;
        }
        let Some((is_ordered, _, w)) = marker(&ls[i].text) else {
            break;
        };
        if is_ordered != ordered {
            break;
        }
        let end = indented(ls, i + 1, indent).max(i + 1);
        let mut item: Vec<Line> = vec![Line {
            indent: indent + w,
            text: ls[i].text[w..].trim_start().to_string(),
        }];
        item.extend(ls[i + 1..end].iter().cloned());
        // A continuation that is less indented than the text of the first line sets the margin.
        let margin = item[1..]
            .iter()
            .filter(|l| !l.blank())
            .map(|l| l.indent)
            .min()
            .unwrap_or(indent + w)
            .min(indent + w);
        item[0].indent = margin;
        items.push(blocks(&item, st));
        i = end;
    }
    (items, i)
}

/// The fields that start at `from`, each with its continuation lines.
fn fields(ls: &[Line], from: usize) -> (Vec<(String, String)>, usize) {
    let indent = ls[from].indent;
    let mut out = Vec::new();
    let mut i = from;
    while i < ls.len() && !ls[i].blank() && ls[i].indent == indent {
        let Some((name, mut text)) = field(&ls[i].text) else {
            break;
        };
        i += 1;
        while i < ls.len() && !ls[i].blank() && ls[i].indent > indent {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(&ls[i].text);
            i += 1;
        }
        out.push((name, text));
    }
    (out, i)
}

/// A paragraph that starts at `from`, and the literal block that it introduces with `::`.
fn paragraph(ls: &[Line], from: usize, out: &mut Vec<Block>, st: &mut State) -> usize {
    let indent = ls[from].indent;
    let mut i = from;
    let mut words: Vec<&str> = Vec::new();
    while i < ls.len() && !ls[i].blank() && ls[i].indent >= indent {
        words.push(&ls[i].text);
        i += 1;
    }
    let mut text = words.join(" ");
    let literal = text.ends_with("::");
    if literal {
        text.truncate(text.len() - 1);
        if text.ends_with(' ') || text == ":" {
            text = text.trim_end_matches([' ', ':']).to_string();
        }
    }
    if !text.is_empty() {
        out.push(Block::Para(text));
    }
    st.labels.clear();
    if literal {
        let mut k = i;
        while k < ls.len() && ls[k].blank() {
            k += 1;
        }
        if k < ls.len() && ls[k].indent > indent {
            let end = indented(ls, k, indent);
            let body = dedent(&ls[k..end]);
            out.push(Block::Literal {
                lang: "text".to_string(),
                lines: body
                    .iter()
                    .map(|l| format!("{}{}", " ".repeat(l.indent), l.text))
                    .collect(),
            });
            return end;
        }
    }
    i
}

fn directive_block(name: &str, args: &str, body: &[Line], raw: &[Line], st: &mut State) -> Block {
    let lower = name.to_ascii_lowercase();
    // The options come first and are fields, then a blank line and the content.
    let mut at = 0;
    while at < body.len()
        && !body[at].blank()
        && body[at].text.starts_with(':')
        && field(&body[at].text).is_some()
        && body[at].indent == 0
    {
        at += 1;
    }
    let after_options = if at > 0 { &body[at..] } else { body };
    match lower.as_str() {
        "code-block" | "code" | "sourcecode" => {
            let lines: Vec<Line> = trim_blank(after_options);
            Block::Literal {
                lang: args.to_string(),
                lines: lines
                    .iter()
                    .map(|l| format!("{}{}", " ".repeat(l.indent), l.text))
                    .collect(),
            }
        }
        "toctree" => {
            let hidden = body.iter().any(|l| l.text == ":hidden:");
            let entries: Vec<Vec<Block>> = after_options
                .iter()
                .filter(|l| !l.blank() && !l.text.starts_with(':'))
                .map(|l| {
                    let name = match l.text.rsplit_once(" <") {
                        Some((title, _)) => title.trim().to_string(),
                        None => l.text.rsplit('/').next().unwrap_or(&l.text).to_string(),
                    };
                    vec![Block::Para(name)]
                })
                .collect();
            if hidden || entries.is_empty() {
                Block::Raw(Vec::new())
            } else {
                Block::List {
                    ordered: false,
                    start: 0,
                    items: entries,
                }
            }
        }
        "contents" | "index" | "module" | "currentmodule" | "highlight" | "sectnum"
        | "default-role" | "role" | "title" | "meta" | "tabularcolumns" | "cssclass"
        | "sectionauthor" | "codeauthor" | "moduleauthor" | "header" | "footer"
        | "target-notes" | "include-only" => Block::Raw(Vec::new()),
        "table" => {
            let mut inner = blocks(after_options, st);
            if !args.is_empty() {
                inner.insert(0, Block::Para(format!("**{args}**")));
            }
            Block::Group(inner)
        }
        "list-table" => {
            let header_rows = option(body, "header-rows")
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
            let table = match blocks(after_options, st).into_iter().next() {
                Some(Block::List { items, .. }) => tables::from_lists(items, header_rows),
                _ => None,
            };
            titled(args, table, raw)
        }
        "csv-table" => {
            let delimiter = match option(body, "delim").as_deref() {
                Some("tab") => '\t',
                Some("space") => ' ',
                Some(c) if c.chars().count() == 1 => c.chars().next().unwrap_or(','),
                _ => ',',
            };
            let quote = option(body, "quote")
                .and_then(|q| q.chars().next())
                .unwrap_or('"');
            let mut rows = Vec::new();
            if let Some(header) = option(body, "header") {
                rows.extend(tables::csv(&[header], delimiter, quote));
            }
            let mut header_rows = rows.len();
            let content: Vec<String> = trim_blank(after_options)
                .iter()
                .map(|l| l.text.clone())
                .collect();
            rows.extend(tables::csv(&content, delimiter, quote));
            if header_rows == 0 {
                header_rows = option(body, "header-rows")
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(0);
            }
            let reads_a_file = option(body, "file").is_some() || option(body, "url").is_some();
            titled(
                args,
                (!reads_a_file)
                    .then(|| tables::from_text(rows, header_rows))
                    .flatten(),
                raw,
            )
        }
        "versionadded" | "versionchanged" | "deprecated" | "versionremoved" => {
            let (version, rest) = args.split_once(' ').unwrap_or((args, ""));
            let label = match lower.as_str() {
                "versionadded" => format!("**New in version {version}.**"),
                "versionchanged" => format!("**Changed in version {version}.**"),
                "versionremoved" => format!("**Removed in version {version}.**"),
                _ => format!("**Deprecated since version {version}.**"),
            };
            let mut lines = vec![Line {
                indent: 0,
                text: if rest.is_empty() {
                    label
                } else {
                    format!("{label} {rest}")
                },
            }];
            if !after_options.iter().all(Line::blank) {
                lines.push(Line {
                    indent: 0,
                    text: String::new(),
                });
                lines.extend(after_options.iter().cloned());
            }
            Block::Admonition {
                kind: if lower == "deprecated" {
                    "WARNING"
                } else {
                    "NOTE"
                }
                .to_string(),
                body: blocks(&lines, st),
            }
        }
        "rubric" => Block::Rubric(args.to_string()),
        "centered" => Block::Para(format!("**{args}**")),
        "topic" | "sidebar" | "admonition" => {
            let mut inner = Vec::new();
            if !args.is_empty() {
                inner.push(Block::Para(format!("**{args}**")));
            }
            inner.extend(blocks(after_options, st));
            Block::Admonition {
                kind: "NOTE".to_string(),
                body: inner,
            }
        }
        "epigraph" | "pull-quote" | "highlights" | "blockquote" => {
            Block::Quote(blocks(after_options, st))
        }
        "only" | "container" | "class" | "compound" | "hlist" | "glossary" | "tab" | "tabs"
        | "tab-set" | "tab-item" | "grid" | "grid-item-card" | "dropdown" | "ifconfig" => {
            Block::Group(blocks(after_options, st))
        }
        "parsed-literal" | "productionlist" => Block::Literal {
            lang: "text".to_string(),
            lines: trim_blank(after_options)
                .iter()
                .map(|l| format!("{}{}", " ".repeat(l.indent), l.text))
                .collect(),
        },
        "math" => Block::Literal {
            lang: "math".to_string(),
            lines: std::iter::once(args.to_string())
                .filter(|a| !a.is_empty())
                .chain(
                    trim_blank(after_options)
                        .iter()
                        .map(|l| format!("{}{}", " ".repeat(l.indent), l.text)),
                )
                .collect(),
        },
        "raw"
            if args
                .split_whitespace()
                .any(|f| f.eq_ignore_ascii_case("html")) =>
        {
            Block::Html(
                trim_blank(after_options)
                    .iter()
                    .map(|l| format!("{}{}", " ".repeat(l.indent), l.text))
                    .collect(),
            )
        }
        "raw" => Block::Raw(Vec::new()),
        "image" | "figure" => Block::Image {
            src: args.to_string(),
            // The alternative text is the one given, or else the name of the file.
            alt: body
                .iter()
                .find_map(|l| l.text.strip_prefix(":alt:").map(|a| a.trim().to_string()))
                .unwrap_or_else(|| {
                    let name = args.rsplit('/').next().unwrap_or(args);
                    name.rsplit_once('.')
                        .map_or(name, |(stem, _)| stem)
                        .to_string()
                }),
        },
        _ if admonition(&lower).is_some() => {
            let mut lines: Vec<Line> = Vec::new();
            if !args.is_empty() {
                lines.push(Line {
                    indent: 0,
                    text: args.to_string(),
                });
            }
            lines.extend(after_options.iter().cloned());
            Block::Admonition {
                kind: admonition(&lower).unwrap_or("NOTE").to_string(),
                body: blocks(&lines, st),
            }
        }
        _ if object_kind(&lower).is_some() => Block::Object {
            kind: object_kind(&lower).unwrap_or_default().to_string(),
            signature: args.to_string(),
            body: blocks(after_options, st),
            labels: std::mem::take(&mut st.labels),
        },
        _ => Block::Raw(
            raw.iter()
                .map(|l| format!("{}{}", " ".repeat(l.indent), l.text))
                .collect(),
        ),
    }
}

/// The value of an option of a directive, `:name: value`.
fn option(body: &[Line], name: &str) -> Option<String> {
    body.iter()
        .take_while(|l| l.blank() || l.text.starts_with(':'))
        .find_map(|l| {
            let (key, value) = field(&l.text)?;
            (key == name).then_some(value)
        })
}

/// A table with its title above it, or the directive as it was written when it holds no table.
fn titled(title: &str, table: Option<Block>, raw: &[Line]) -> Block {
    match table {
        Some(table) if title.is_empty() => table,
        Some(table) => Block::Group(vec![Block::Para(format!("**{title}**")), table]),
        None => Block::Raw(
            raw.iter()
                .map(|l| format!("{}{}", " ".repeat(l.indent), l.text))
                .collect(),
        ),
    }
}

/// The kinds of object that a domain documents: `function`, `py:class`, `js:attribute` and the like,
/// whatever the domain is.
const OBJECT_KINDS: [&str; 24] = [
    "function",
    "func",
    "class",
    "method",
    "meth",
    "attribute",
    "attr",
    "data",
    "exception",
    "exc",
    "option",
    "cmdoption",
    "envvar",
    "describe",
    "member",
    "type",
    "macro",
    "var",
    "enum",
    "struct",
    "union",
    "object",
    "property",
    "staticmethod",
];

fn object_kind(directive: &str) -> Option<&str> {
    let kind = directive.rsplit(':').next()?;
    OBJECT_KINDS.contains(&kind).then_some(kind)
}

fn trim_blank(ls: &[Line]) -> Vec<Line> {
    let start = ls.iter().position(|l| !l.blank()).unwrap_or(ls.len());
    let end = ls.iter().rposition(|l| !l.blank()).map_or(start, |e| e + 1);
    ls[start..end].to_vec()
}

/// When the first title is the only one of its level, it is the title of the document, and the
/// others move up. The title is taken out of the blocks and returned.
pub fn promote_title(blocks: &mut Vec<Block>) -> Option<String> {
    fn headings(blocks: &[Block], out: &mut Vec<usize>) {
        for b in blocks {
            if let Block::Heading { level, .. } = b {
                out.push(*level);
            }
        }
    }
    let mut levels = Vec::new();
    headings(blocks, &mut levels);
    let first = *levels.first()?;
    if levels.iter().filter(|l| **l == first).count() != 1 || levels[0] != first {
        return None;
    }
    let at = blocks
        .iter()
        .position(|b| matches!(b, Block::Heading { .. }))?;
    let Block::Heading { text, .. } = blocks.remove(at) else {
        return None;
    };
    for b in blocks.iter_mut() {
        if let Block::Heading { level, .. } = b {
            *level = level.saturating_sub(1).max(1);
        }
    }
    Some(text)
}
