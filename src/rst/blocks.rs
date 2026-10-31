//! The blocks of a reStructuredText document.

/// A line of the text without its indent.
#[derive(Clone, Debug)]
pub struct Line {
    pub indent: usize,
    pub text: String,
}

impl Line {
    fn blank(&self) -> bool {
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
    let name = &rest[..at];
    let after = &rest[at + 2..];
    (!name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ":+._-".contains(c))
        && (after.is_empty() || after.starts_with(' ')))
    .then(|| (name, after.trim()))
}

/// The label and the address of a target, `.. _label: address`.
fn target(text: &str) -> Option<(String, String)> {
    let rest = text.strip_prefix(".. _")?;
    let at = rest.rfind(':')?;
    let label = rest[..at].trim_matches('`').to_string();
    (!label.is_empty()).then(|| (label, rest[at + 1..].trim().to_string()))
}

/// The marker of a list item and the width it takes: `* `, `1. ` and the like.
fn marker(text: &str) -> Option<(bool, usize, usize)> {
    let mut chars = text.chars();
    let first = chars.next()?;
    if "*+-".contains(first) && chars.next() == Some(' ') {
        return Some((false, 0, 2));
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
fn dedent(ls: &[Line]) -> Vec<Line> {
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

struct State {
    /// The adornments of the titles in the order they came, with whether there is an overline.
    styles: Vec<(char, bool)>,
    labels: Vec<String>,
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

fn blocks(ls: &[Line], st: &mut State) -> Vec<Block> {
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
        "toctree" | "contents" | "index" | "module" | "currentmodule" | "highlight" => {
            Block::Raw(Vec::new())
        }
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
