//! The blocks of a HAProxy documentation: headings, keywords with their notes, paragraphs, lists,
//! arguments, examples and preformatted text.
//!
//! Whatever is not recognised is kept as preformatted text, so that nothing is lost.

use super::text::Line;

/// The banner at the top of a document.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Banner {
    pub title: String,
    pub version: String,
    pub author: String,
    pub date: String,
}

/// An item of a list: its text, then the blocks under it, such as paragraphs, examples and lists.
#[derive(Debug, PartialEq, Eq)]
pub struct Item {
    pub text: String,
    pub rest: Vec<Block>,
}

/// A part of a document.
#[derive(Debug, PartialEq, Eq)]
pub enum Block {
    /// A numbered title, such as `4.2. Alphabetically sorted keywords reference`.
    Heading {
        number: String,
        title: String,
    },
    /// The list of the chapters, as numbers and titles.
    Summary(Vec<(String, String)>),
    /// A keyword with the lines that give its syntax, and the name taken from the first one.
    Keyword {
        name: String,
        syntax: Vec<String>,
        /// Whether a syntax line is marked `(deprecated)`.
        deprecated: bool,
    },
    Para(String),
    /// Preformatted lines, with their common indent removed.
    Code(Vec<String>),
    List(Vec<Item>),
    /// The proxy types that a keyword is used in, from `May be used in the following contexts`.
    Contexts(Vec<String>),
    /// The sections that a keyword is used in, each with its mark such as `yes`, `no` or `yes(!)`.
    Sections(Vec<(String, String)>),
    /// What follows `Arguments :`, as `<name>` and its description, or the text after the colon.
    Arguments {
        text: String,
        items: Vec<(String, String)>,
    },
    /// What follows `Example :`, with the text that came after the colon.
    Example {
        text: String,
        code: Vec<String>,
    },
    SeeAlso(String),
    /// A note, a warning or the like, from `Note:` and its hanging text.
    Note {
        kind: &'static str,
        text: String,
    },
    /// A line that ends with a colon and introduces the indented lines under it.
    Label(String),
    /// A title underlined with dashes inside a chapter.
    Subtitle(String),
    /// Names with a description each, such as the values a parameter takes.
    Definitions(Vec<(String, String)>),
    /// A list whose items are numbered, from the number of its first item.
    Ordered {
        start: u32,
        items: Vec<Item>,
    },
    /// Where a keyword is usable, from `Usable in:` and its marks.
    Usable(Vec<(String, String)>),
    /// A drawing made of characters, kept as it is.
    Figure(Vec<String>),
    /// A table drawn with dashes and pluses, with its header row when it has one.
    Table {
        header: Vec<String>,
        rows: Vec<Vec<String>>,
    },
}

/// A heading, when the line at `i` starts one: a number and a title, underlined with dashes.
pub fn heading_at(ls: &[Line<'_>], i: usize) -> Option<(String, String)> {
    let line = ls.get(i)?;
    let under = ls.get(i + 1)?;
    if line.indent != 0 || line.blank() || under.indent != 0 || under.text.len() < 3 {
        return None;
    }
    if !under.text.bytes().all(|b| b == b'-') {
        return None;
    }
    if line.text == "Summary" {
        return Some((String::new(), "Summary".to_string()));
    }
    let (number, title) = split_number(line.text)?;
    Some((number, title))
}

/// The number and the title of a line such as `7.3.6.   Fetching HTTP samples`.
pub fn split_number(text: &str) -> Option<(String, String)> {
    let end = text.find(|c: char| !(c.is_ascii_digit() || c == '.'))?;
    let number = &text[..end];
    if number.is_empty() || !number.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    // A number may be closed with a parenthesis, as in `9.2) Typed output format`.
    let after = text[end..].strip_prefix(')').unwrap_or(&text[end..]);
    let title = after.trim();
    if title.is_empty() || !after.starts_with(' ') {
        return None;
    }
    Some((number.trim_end_matches('.').to_string(), title.to_string()))
}

/// The banner at the top of a document and the number of lines it takes.
pub fn banner(ls: &[Line<'_>]) -> (Banner, usize) {
    let dashes = |l: &Line<'_>| l.text.len() >= 3 && l.text.bytes().all(|b| b == b'-');
    let mut at = 0;
    while at < ls.len() && ls[at].blank() {
        at += 1;
    }
    if at >= ls.len() || !dashes(&ls[at]) {
        return (Banner::default(), 0);
    }
    let mut banner = Banner::default();
    let mut title = Vec::new();
    let mut i = at + 1;
    while i < ls.len() && !dashes(&ls[i]) {
        if !ls[i].blank() {
            title.push(ls[i].text);
        }
        i += 1;
    }
    banner.title = title.join(" ");
    i += 1;
    while i < ls.len() && !ls[i].blank() {
        let t = ls[i].text;
        if let Some(v) = t.strip_prefix("version ") {
            banner.version = v.trim().to_string();
        } else if is_date(t) {
            banner.date = t.to_string();
        } else if banner.author.is_empty() {
            banner.author = t.to_string();
        }
        i += 1;
    }
    (banner, i)
}

fn is_date(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() == 10
        && b[4] == b'/'
        && b[7] == b'/'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

/// The blocks of the lines that follow the banner.
pub fn parse(ls: &[Line<'_>]) -> Vec<Block> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < ls.len() {
        if let Some((number, title)) = heading_at(ls, i) {
            i += 2;
            let end = next_heading(ls, i);
            let summary = number.is_empty();
            out.push(Block::Heading { number, title });
            if summary {
                summary_block(&ls[i..end], &mut out);
            } else {
                region(&ls[i..end], &mut out);
            }
            i = end;
        } else {
            let end = next_heading(ls, i + 1);
            region(&ls[i..end], &mut out);
            i = end;
        }
    }
    out
}

fn next_heading(ls: &[Line<'_>], from: usize) -> usize {
    (from..ls.len())
        .find(|&j| heading_at(ls, j).is_some())
        .unwrap_or(ls.len())
}

fn summary_block(ls: &[Line<'_>], out: &mut Vec<Block>) {
    let mut entries = Vec::new();
    let mut rest = Vec::new();
    for l in ls {
        if l.blank() {
            continue;
        }
        match split_number(l.text) {
            Some(e) if rest.is_empty() => entries.push(e),
            _ => rest.push(*l),
        }
    }
    if !entries.is_empty() {
        out.push(Block::Summary(entries));
    }
    if !rest.is_empty() {
        out.push(Block::Code(common_indent(&rest)));
    }
}

/// The text of the lines with their common indent removed.
pub fn common_indent(ls: &[Line<'_>]) -> Vec<String> {
    let min = ls
        .iter()
        .filter(|l| !l.blank())
        .map(|l| l.indent)
        .min()
        .unwrap_or(0);
    ls.iter()
        .map(|l| {
            if l.blank() {
                String::new()
            } else {
                format!("{}{}", " ".repeat(l.indent - min), l.text)
            }
        })
        .collect()
}

fn trim_blank<'a, 'b>(ls: &'b [Line<'a>]) -> &'b [Line<'a>] {
    let start = ls.iter().position(|l| !l.blank()).unwrap_or(ls.len());
    let end = ls.iter().rposition(|l| !l.blank()).map_or(start, |p| p + 1);
    &ls[start..end]
}

/// A line that names a keyword and its parameters, such as `option httpclose` or `bind <address>`:
/// the words that name it, and none of the shapes of prose.
pub fn keyword_name(text: &str) -> Option<String> {
    let first = text.chars().next()?;
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return None;
    }
    let mut words: Vec<&str> = Vec::new();
    let mut at = 0;
    let mut stopped = false;
    while at < text.len() && !stopped {
        let rest = &text[at..];
        let skip = rest.len() - rest.trim_start_matches(' ').len();
        let start = at + skip;
        let tok_len = text[start..].find(' ').unwrap_or(text.len() - start);
        let tok = &text[start..start + tok_len];
        if tok.is_empty() {
            at = text.len();
            break;
        }
        let (word, tail) = match tok.find('(') {
            Some(p) if p > 0 => (&tok[..p], &tok[p..]),
            _ => (tok, ""),
        };
        let plain = !word.is_empty()
            && word
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._+-".contains(&b));
        if !plain || words.len() >= 4 {
            break;
        }
        words.push(word);
        at = start + tok_len;
        if !tail.is_empty() {
            stopped = true;
            at = start + word.len();
        }
    }
    if words.is_empty() {
        return None;
    }
    let rest = text[at..].trim_start();
    let parameters_ok =
        rest.is_empty() || rest.starts_with(['<', '[', '{', '(']) || rest.starts_with(": ");
    parameters_ok.then(|| words.join(" "))
}

/// The lines of a paragraph as one text, with a line break where a line ends a sentence.
fn flow(span: &[Line<'_>]) -> String {
    let mut text = String::new();
    for (k, l) in span.iter().enumerate() {
        if k > 0 {
            text.push(super::text::separator(&span[k - 1], l));
        }
        text.push_str(l.text);
    }
    text
}

fn is_marker(text: &str) -> bool {
    super::defs::marker(text).is_some()
}

fn label(text: &str) -> Option<(&'static str, &str)> {
    for (name, key) in [
        ("Arguments", "Arguments"),
        ("Argument", "Arguments"),
        ("Examples", "Example"),
        ("Example", "Example"),
        ("See also", "See also"),
    ] {
        if let Some(rest) = text.strip_prefix(name) {
            let rest = rest.trim_start();
            if let Some(after) = rest.strip_prefix(':') {
                return Some((key, after.trim()));
            }
        }
    }
    None
}

/// The lines of a region as blocks.
fn region(ls: &[Line<'_>], out: &mut Vec<Block>) {
    let ls = trim_blank(ls);
    let base = ls
        .iter()
        .filter(|l| !l.blank())
        .map(|l| l.indent)
        .min()
        .unwrap_or(0);
    let mut i = 0;
    let mut after_blank = true;
    while i < ls.len() {
        let l = ls[i];
        if l.blank() {
            after_blank = true;
            i += 1;
            continue;
        }
        if l.indent == base
            && after_blank
            && let Some(next) = keyword(ls, i, out)
        {
            i = next;
            after_blank = true;
            continue;
        }
        after_blank = false;
        if let Some((block, next)) = table(ls, i) {
            out.push(block);
            i = next;
            continue;
        }
        if l.indent == base
            && let Some((block, next)) = quoted_entries(ls, i)
        {
            out.push(block);
            i = next;
            continue;
        }
        if let Some(under) = ls.get(i + 1)
            && under.indent == l.indent
            && under.text.len() >= 3
            && under.text.len() == l.text.len()
            && under.text.bytes().all(|b| b == b'-')
        {
            out.push(Block::Subtitle(l.text.to_string()));
            i += 2;
            continue;
        }
        if let Some((key, text)) = label(l.text) {
            i = labelled(ls, i, key, text, out);
            continue;
        }
        if let Some(rest) = l.text.strip_prefix("Usable in")
            && let Some(header) = rest.trim_start().strip_prefix(':')
            && let Some(next) = ls.get(i + 1).filter(|n| !n.blank())
            && let Some(rows) = usable(header, next.text)
        {
            out.push(Block::Usable(rows));
            i += 2;
            continue;
        }
        if l.indent == base
            && ends_with_colon(l.text)
            && ls
                .get(i + 1)
                .is_some_and(|n| !n.blank() && n.indent > l.indent)
            && label(l.text).is_none()
        {
            out.push(Block::Label(
                l.text.trim_end_matches([':', ' ']).to_string(),
            ));
            i += 1;
            continue;
        }
        if l.indent == base {
            if let Some(c) = l
                .text
                .strip_prefix("May be used in the following contexts:")
            {
                out.push(Block::Contexts(
                    c.split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect(),
                ));
                i += 1;
                continue;
            }
            if let Some(header) = l
                .text
                .strip_prefix("May be used in sections")
                .map(|h| h.trim_start().trim_start_matches(':'))
                && let Some(next) = ls.get(i + 1).filter(|n| !n.blank())
            {
                let names: Vec<&str> = header.split('|').map(str::trim).collect();
                let marks: Vec<&str> = next.text.split('|').map(str::trim).collect();
                if names.len() == marks.len() && names.iter().all(|n| !n.is_empty()) {
                    out.push(Block::Sections(
                        names
                            .iter()
                            .zip(&marks)
                            .map(|(n, m)| (n.to_string(), m.to_string()))
                            .collect(),
                    ));
                    i += 2;
                    continue;
                }
            }
        }
        if is_marker(l.text) {
            i = list(ls, i, out);
            continue;
        }
        i = prose(ls, i, base, out);
    }
}

/// A keyword whose lines start at `i`, and the index after its body.
fn keyword(ls: &[Line<'_>], i: usize, out: &mut Vec<Block>) -> Option<usize> {
    let first = keyword_name(ls[i].text)?;
    let word = first.split(' ').next().unwrap_or_default().to_string();
    let mut j = i;
    while j < ls.len()
        && ls[j].indent == 0
        && !ls[j].blank()
        && (keyword_name(ls[j].text).is_some()
            || (j > i && ls[j].text.starts_with(&format!("{word} ")))
            // A variant of the name with a placeholder, such as `map_<match_type>(...)`.
            || (j > i
                && ls[j].text.starts_with(&format!("{word}_"))
                && ls[j].text.contains('<')))
    {
        j += 1;
    }
    if j == i || j >= ls.len() {
        return None;
    }
    // The body may start after one blank line.
    let body = if ls[j].blank() { j + 1 } else { j };
    if body >= ls.len() || ls[body].blank() || ls[body].indent < 2 {
        return None;
    }
    let mut name = first;
    if let Some(rest) = name.strip_prefix("no ") {
        name = rest.to_string();
    }
    let syntax: Vec<String> = ls[i..j].iter().map(|l| l.text.to_string()).collect();
    let mut end = j;
    while end < ls.len() && (ls[end].blank() || ls[end].indent >= 1) {
        end += 1;
    }
    let deprecated = syntax.iter().any(|l| l.contains("(deprecated)"));
    out.push(Block::Keyword {
        name,
        syntax,
        deprecated,
    });
    region(&ls[j..end], out);
    Some(end)
}

/// The lines under a label, those that are indented more than it.
fn under<'a, 'b>(ls: &'b [Line<'a>], from: usize, indent: usize) -> (&'b [Line<'a>], usize) {
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
    (&ls[from..last], last)
}

fn labelled(ls: &[Line<'_>], i: usize, key: &str, text: &str, out: &mut Vec<Block>) -> usize {
    let (body, end) = under(ls, i + 1, ls[i].indent);
    let body = trim_blank(body);
    match key {
        "Arguments" => {
            let items = super::defs::definitions(body, 1);
            out.push(Block::Arguments {
                text: text.to_string(),
                items: items.clone().unwrap_or_default(),
            });
            if items.is_none() && !body.is_empty() {
                region(body, out);
            }
        }
        "Example" => {
            // What follows the label may go on with the sentence of the label, in lines that are
            // indented more than the example that comes after them.
            let mut sentence = text.to_string();
            let mut from = 0;
            let min = body
                .iter()
                .filter(|l| !l.blank())
                .map(|l| l.indent)
                .min()
                .unwrap_or(0);
            let first_end = body.iter().position(|l| l.blank()).unwrap_or(body.len());
            if first_end < body.len() && first_end > 0 {
                let first = &body[..first_end];
                let rest_min = body[first_end..]
                    .iter()
                    .filter(|l| !l.blank())
                    .map(|l| l.indent)
                    .min()
                    .unwrap_or(min);
                if first[0].indent > rest_min
                    && first.iter().all(|l| l.indent == first[0].indent)
                    && super::defs::wrapped(first)
                {
                    for l in first {
                        if !sentence.is_empty() {
                            sentence.push(' ');
                        }
                        sentence.push_str(l.text);
                    }
                    from = first_end;
                }
            }
            out.push(Block::Example {
                text: sentence,
                code: common_indent(trim_blank(&body[from..])),
            });
        }
        _ => {
            let mut all = vec![text.to_string()];
            all.extend(body.iter().map(|l| l.text.to_string()));
            out.push(Block::SeeAlso(all.join(" ").trim().to_string()));
        }
    }
    end
}

fn list(ls: &[Line<'_>], i: usize, out: &mut Vec<Block>) -> usize {
    let indent = ls[i].indent;
    let mut end = i;
    while end < ls.len() {
        let l = ls[end];
        if l.blank() {
            // A blank line belongs to the list when what follows is indented under an item or is
            // another item.
            let next = ls[end..].iter().find(|n| !n.blank());
            match next {
                Some(n) if n.indent > indent || (n.indent == indent && is_marker(n.text)) => {
                    end += 1
                }
                _ => break,
            }
        } else if l.indent > indent || (l.indent == indent && is_marker(l.text)) {
            end += 1;
        } else {
            break;
        }
    }
    let span = trim_blank(&ls[i..end]);
    let ordered = super::defs::marker(ls[i].text).is_some_and(|(_, o)| o);
    match items(span, indent, ordered) {
        Some(items) if ordered => out.push(Block::Ordered {
            start: super::defs::number(ls[i].text).unwrap_or(1),
            items,
        }),
        Some(items) => out.push(Block::List(items)),
        None => out.push(code_or_figure(common_indent(span))),
    }
    end
}

/// The items of a list whose markers are at `indent`, or none when a line does not belong.
fn items(ls: &[Line<'_>], indent: usize, ordered: bool) -> Option<Vec<Item>> {
    let mut out: Vec<Item> = Vec::new();
    let mut i = 0;
    while i < ls.len() {
        let l = ls[i];
        if l.blank() {
            i += 1;
            continue;
        }
        if l.indent <= indent && !(l.indent == indent && is_marker(l.text)) {
            return None;
        }
        if l.indent == indent && super::defs::marker(l.text).is_some_and(|(_, o)| o != ordered) {
            return None;
        }
        // The item takes the lines up to the next marker at its own indent.
        let mut end = i + 1;
        while end < ls.len() && (ls[end].blank() || ls[end].indent > indent) {
            end += 1;
        }
        out.push(item(&ls[i..end], indent)?);
        i = end;
    }
    Some(out)
}

/// One item: the lines from its marker to the end of what is under it.
fn item(ls: &[Line<'_>], _indent: usize) -> Option<Item> {
    let width = super::defs::marker(ls[0].text).map_or(2, |(w, _)| w);
    let mut text = ls[0].text[width..].trim().to_string();
    let mut rest = Vec::new();
    let mut i = 1;
    // The lines that follow without a blank line continue the text, at the indent of the first of
    // them. Lines indented deeper are an inner block, and the lines at the indent again go on in a
    // paragraph of their own.
    let hang = ls.get(1).map_or(0, |l| l.indent);
    let mut paragraph: Option<String> = None;
    while i < ls.len() && !ls[i].blank() && !is_marker(ls[i].text) {
        if ls[i].indent > hang {
            let start = i;
            while i < ls.len() && !ls[i].blank() && !is_marker(ls[i].text) && ls[i].indent > hang {
                i += 1;
            }
            if let Some(p) = paragraph.take() {
                rest.push(Block::Para(p));
            }
            inner_block(&ls[start..i], &mut rest);
            continue;
        }
        match paragraph.as_mut() {
            Some(p) => {
                p.push(super::text::separator(&ls[i - 1], &ls[i]));
                p.push_str(ls[i].text);
            }
            None if rest.is_empty() => {
                text.push(super::text::separator(&ls[i - 1], &ls[i]));
                text.push_str(ls[i].text);
            }
            None => paragraph = Some(ls[i].text.to_string()),
        }
        i += 1;
    }
    if let Some(p) = paragraph.take() {
        rest.push(Block::Para(p));
    }
    region(&ls[i..], &mut rest);
    Some(Item { text, rest })
}

/// Lines indented under the text of an item: numbered entries such as `17: Name` are a list, and
/// anything else is read as a region.
fn inner_block(span: &[Line<'_>], out: &mut Vec<Block>) {
    let entry = |t: &str| {
        let digits = t.chars().take_while(char::is_ascii_digit).count();
        digits > 0 && t[digits..].starts_with([':', ';']) && t[digits + 1..].starts_with(' ')
    };
    if span.iter().all(|l| entry(l.text)) {
        out.push(Block::List(
            span.iter()
                .map(|l| Item {
                    text: l.text.to_string(),
                    rest: Vec::new(),
                })
                .collect(),
        ));
    } else {
        region(span, out);
    }
}

fn ends_with_colon(text: &str) -> bool {
    text.len() < 70 && text.ends_with(':') && !text.contains("  ") && !text.starts_with("- ")
}

/// A note label at the start of a paragraph: the kind of alert and the text after the label.
fn note(text: &str) -> Option<(&'static str, &str)> {
    const LABELS: [(&str, &str); 7] = [
        ("important note", "IMPORTANT"),
        ("important", "IMPORTANT"),
        ("warning", "WARNING"),
        ("attention", "WARNING"),
        ("caution", "CAUTION"),
        ("notes", "NOTE"),
        ("note", "NOTE"),
    ];
    for (word, kind) in LABELS {
        let Some(head) = text.get(..word.len()) else {
            continue;
        };
        if head.eq_ignore_ascii_case(word) {
            let rest = text[word.len()..].trim_start();
            if let Some(after) = rest.strip_prefix(':') {
                return Some((kind, after.trim()));
            }
        }
    }
    None
}

/// Whether a line is a rule between the rows of a table, such as `-----+-----` or `-----|-----`.
fn is_rule(text: &str) -> bool {
    text.len() >= 5
        && text.starts_with('-')
        && text.ends_with('-')
        && (text.contains('+') || text.contains('|'))
        && text.bytes().all(|b| b == b'-' || b == b'+' || b == b'|')
}

/// The cells of a line cut at the columns of the rule. A cut that falls inside a word moves to the
/// end of the word.
fn cells(line: &Line<'_>, cuts: &[usize]) -> Vec<String> {
    let full: Vec<char> = format!("{}{}", " ".repeat(line.indent), line.text)
        .chars()
        .collect();
    let mut out = Vec::new();
    let mut start = 0;
    for &cut in cuts {
        let mut end = cut.min(full.len());
        while end < full.len()
            && end > 0
            && full[end] != ' '
            && full[end] != '|'
            && full[end - 1] != ' '
        {
            end += 1;
        }
        let end = end.max(start);
        out.push(
            full[start.min(full.len())..end.min(full.len())]
                .iter()
                .collect::<String>()
                .trim()
                .to_string(),
        );
        start = if full.get(end) == Some(&'|') {
            end + 1
        } else {
            end
        };
    }
    out.push(
        full[start.min(full.len())..]
            .iter()
            .collect::<String>()
            .trim()
            .to_string(),
    );
    out
}

/// A table that starts at `i`: lines up to the next blank line, with at least one rule between
/// rows, and the index after it.
fn table(ls: &[Line<'_>], i: usize) -> Option<(Block, usize)> {
    let mut end = i;
    while end < ls.len() && !ls[end].blank() {
        end += 1;
    }
    let run = &ls[i..end];
    if let Some((header, rows, used)) = super::grid::table(run) {
        return Some((Block::Table { header, rows }, i + used));
    }
    // Arrows drawn over several lines are a timeline, not the columns of a table.
    if has_arrows(run) {
        return None;
    }
    let rules: Vec<usize> = (0..run.len()).filter(|&k| is_rule(run[k].text)).collect();
    let first = *rules.first()?;
    let cuts: Vec<usize> = run[first]
        .text
        .char_indices()
        .filter(|(_, c)| *c == '+' || *c == '|')
        .map(|(p, _)| run[first].indent + p)
        .collect();
    if cuts.is_empty() {
        return None;
    }
    // Rows are what lies between rules. With rules only above and below the rows, the line above
    // the first rule is the header and each line is a row.
    let between = rules.len() >= 3 || (rules.len() == 2 && first == 0);
    let mut header: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<String>> = Vec::new();
    if between {
        let mut group: Vec<&Line<'_>> = Vec::new();
        let flush = |group: &mut Vec<&Line<'_>>, rows: &mut Vec<Vec<String>>| {
            if group.is_empty() {
                return;
            }
            let mut row = vec![String::new(); cuts.len() + 1];
            for l in group.iter() {
                for (c, cell) in cells(l, &cuts).into_iter().enumerate() {
                    if !cell.is_empty() {
                        if !row[c].is_empty() {
                            row[c].push(' ');
                        }
                        row[c].push_str(&cell);
                    }
                }
            }
            rows.push(row);
            group.clear();
        };
        for (k, l) in run.iter().enumerate() {
            if rules.contains(&k) {
                flush(&mut group, &mut rows);
            } else {
                group.push(l);
            }
        }
        flush(&mut group, &mut rows);
    } else {
        for (k, l) in run.iter().enumerate() {
            if rules.contains(&k) {
                continue;
            }
            let row = cells(l, &cuts);
            if k < first {
                header = row;
            } else if !repeats_names(l.text, &header) {
                rows.push(row);
            }
        }
        if rows.last().is_some_and(|r| *r == header) {
            rows.pop();
        }
    }
    if rows.is_empty() {
        return None;
    }
    if header.is_empty() {
        header = vec![String::new(); cuts.len() + 1];
    }
    Some((Block::Table { header, rows }, end))
}

/// The places a keyword is usable in, from the names and the marks of two lines such as
/// `QUIC Ini| TCP RqCon| RqSes` over `X | - | X`. A name with one word takes the first word of the
/// last name that has several.
fn usable(header: &str, marks: &str) -> Option<Vec<(String, String)>> {
    let names: Vec<&str> = header.split('|').map(str::trim).collect();
    let marks: Vec<&str> = marks.split('|').map(str::trim).collect();
    if names.len() != marks.len() || names.iter().any(|n| n.is_empty()) {
        return None;
    }
    let mut group: Option<&str> = None;
    let mut out = Vec::new();
    for (name, mark) in names.iter().zip(&marks) {
        let shown = if name.contains(' ') {
            group = name.split(' ').next();
            name.split_whitespace().collect::<Vec<_>>().join(" ")
        } else if let Some(g) = group {
            format!("{g} {name}")
        } else {
            name.to_string()
        };
        out.push((shown, mark.to_string()));
    }
    Some(out)
}

/// The words of a line with the column each one starts at.
fn words_at<'a>(line: &Line<'a>) -> Vec<(usize, &'a str)> {
    let mut out = Vec::new();
    let mut start = None;
    for (p, c) in line
        .text
        .char_indices()
        .chain(std::iter::once((line.text.len(), ' ')))
    {
        match (c == ' ', start) {
            (false, None) => start = Some(p),
            (true, Some(s)) => {
                out.push((
                    line.indent + line.text[..s].chars().count(),
                    &line.text[s..p],
                ));
                start = None;
            }
            _ => {}
        }
    }
    out
}

/// A table whose columns are lined up with spaces: a header line, then rows that start under the
/// columns, and lines under the other columns that go on with the row above.
fn aligned_table(span: &[Line<'_>]) -> Option<Block> {
    if span.len() < 3 || span.iter().any(|l| l.blank()) {
        return None;
    }
    let header = &span[0];
    let (starts, names) = header_cells(header);
    if starts.len() < 2
        || names
            .iter()
            .any(|n| n.len() > 40 || n.contains(['=', '<', '(', ':']))
    {
        return None;
    }
    let width = starts.len();
    // A line that repeats the names of the columns between dashes, to help reading a long table,
    // is not a row.
    let body: Vec<Line<'_>> = span[1..]
        .iter()
        .filter(|l| !repeats_names(l.text, &names))
        .copied()
        .collect();
    let rows = rows_by_gaps(&body, &starts).or_else(|| rows_by_columns(&body, &starts))?;
    let full = rows
        .iter()
        .filter(|r| r.iter().all(|c| !c.is_empty()))
        .count();
    if rows.len() < 2 || full * 10 < rows.len() * 7 {
        return None;
    }
    let _ = width;
    Some(Block::Table {
        header: names,
        rows,
    })
}

/// The cells of a line, cut at the runs of two spaces or more.
fn gap_cells(line: &Line<'_>) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = Vec::new();
    let mut previous_end: Option<usize> = None;
    for (col, word) in words_at(line) {
        let joined = previous_end.is_some_and(|e| col <= e + 1);
        if joined && let Some(last) = out.last_mut() {
            last.1.push(' ');
            last.1.push_str(word);
        } else {
            out.push((col, word.to_string()));
        }
        previous_end = Some(col + word.chars().count());
    }
    out
}

/// The rows when every line cut at its gaps has as many cells as the header, a line with fewer that
/// starts under the second column going on with the row above.
fn rows_by_gaps(lines: &[Line<'_>], starts: &[usize]) -> Option<Vec<Vec<String>>> {
    let width = starts.len();
    let mut rows: Vec<Vec<String>> = Vec::new();
    for l in lines {
        let cells = gap_cells(l);
        if cells.len() == width {
            rows.push(cells.into_iter().map(|(_, c)| c).collect());
        } else if cells.len() < width && cells.first().is_some_and(|(col, _)| *col + 1 >= starts[1])
        {
            let last = rows.last_mut()?;
            let offset = width - cells.len();
            for (k, (_, c)) in cells.into_iter().enumerate() {
                let k = k + offset;
                if !last[k].is_empty() {
                    last[k].push(' ');
                }
                last[k].push_str(&c);
            }
        } else {
            return None;
        }
    }
    Some(rows)
}

/// The rows when each word is given to the column that starts at or before it.
fn rows_by_columns(lines: &[Line<'_>], starts: &[usize]) -> Option<Vec<Vec<String>>> {
    let width = starts.len();
    let column = |col: usize| {
        (0..width)
            .rev()
            .find(|&k| starts[k] <= col + 1)
            .unwrap_or(0)
    };
    let mut rows: Vec<Vec<String>> = Vec::new();
    for l in lines {
        let mut cells = vec![String::new(); width];
        for (col, word) in words_at(l) {
            let k = column(col);
            if !cells[k].is_empty() {
                cells[k].push(' ');
            }
            cells[k].push_str(word);
        }
        if cells[0].is_empty() {
            let last = rows.last_mut()?;
            for (k, c) in cells.into_iter().enumerate().skip(1) {
                if !c.is_empty() {
                    if !last[k].is_empty() {
                        last[k].push(' ');
                    }
                    last[k].push_str(&c);
                }
            }
        } else {
            if cells[0].len() > 40 {
                return None;
            }
            rows.push(cells);
        }
    }
    Some(rows)
}

/// Whether a line is the names of the columns drawn between dashes, such as
/// `-- keyword -------+- input type + output type -`.
fn repeats_names(text: &str, names: &[String]) -> bool {
    // The letters and digits alone: what separates the names, dashes, spaces or colons, does not
    // matter.
    let letters = |t: &str| -> String { t.chars().filter(|c| c.is_alphanumeric()).collect() };
    let wanted: String = names.iter().map(|n| letters(n)).collect();
    text.starts_with('-') && wanted.len() >= 3 && letters(text) == wanted
}

/// The start columns and the texts of the cells of a header line, a cell being a run of words with
/// at most one space between them.
fn header_cells(line: &Line<'_>) -> (Vec<usize>, Vec<String>) {
    let mut starts = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let mut previous_end: Option<usize> = None;
    for (col, word) in words_at(line) {
        let joined = previous_end.is_some_and(|e| col <= e + 1);
        if joined && let Some(last) = names.last_mut() {
            last.push(' ');
            last.push_str(word);
        } else {
            starts.push(col);
            names.push(word.to_string());
        }
        previous_end = Some(col + word.chars().count());
    }
    (starts, names)
}

/// A quoted name followed by its description, and the entries that go on as long as the next one
/// starts with a quote at the same column, as a table of the names and their descriptions.
fn quoted_entries(ls: &[Line<'_>], from: usize) -> Option<(Block, usize)> {
    let base = ls[from].indent;
    let entry = |l: &Line<'_>| -> Option<(String, String)> {
        if l.indent != base || !l.text.starts_with('\'') {
            return None;
        }
        let close = l.text[1..].find('\'')? + 1;
        let rest = l.text[close + 1..].strip_prefix(' ')?.trim();
        (!rest.is_empty() && !l.text[1..close].contains(' '))
            .then(|| (l.text[1..close].to_string(), rest.to_string()))
    };
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut i = from;
    while i < ls.len() {
        let Some((name, mut text)) = entry(&ls[i]) else {
            break;
        };
        let mut last = i;
        i += 1;
        loop {
            // The description goes on in the lines indented under the name, after blank lines too.
            let mut j = i;
            while j < ls.len() && ls[j].blank() {
                j += 1;
            }
            if j < ls.len() && ls[j].indent > base {
                text.push(super::text::separator(&ls[last], &ls[j]));
                text.push_str(ls[j].text);
                last = j;
                i = j + 1;
            } else {
                break;
            }
        }
        rows.push(vec![format!("\u{2}{name}"), text]);
        let mut j = i;
        while j < ls.len() && ls[j].blank() {
            j += 1;
        }
        if j < ls.len() && entry(&ls[j]).is_some() {
            i = j;
        } else {
            break;
        }
    }
    (rows.len() >= 2).then(|| {
        (
            Block::Table {
                header: vec!["Prefix".to_string(), "Meaning".to_string()],
                rows,
            },
            i,
        )
    })
}

/// Adds definitions, to the ones just before when there are some.
fn push_definitions(defs: Vec<(String, String)>, out: &mut Vec<Block>) {
    if let Some(Block::Definitions(before)) = out.last_mut() {
        before.extend(defs);
    } else {
        out.push(Block::Definitions(defs));
    }
}

/// Preformatted lines, as a figure when they draw one.
fn code_or_figure(lines: Vec<String>) -> Block {
    if is_figure(&lines) {
        Block::Figure(lines)
    } else {
        Block::Code(lines)
    }
}

/// Whether the lines draw something: arrows or the borders of boxes.
pub fn is_figure(lines: &[String]) -> bool {
    if lines.len() < 2 {
        return false;
    }
    let arrows = ["|<", ">|", ":<", ">:"];
    let drawn = lines
        .iter()
        .filter(|l| {
            is_span(l)
                || arrows.iter().any(|a| l.contains(a))
                || l.contains("+--")
                || l.contains("--+")
        })
        .count();
    let verticals = lines
        .iter()
        .filter(|l| l.trim_start().starts_with(['|', ':', '^']) && l.trim().len() <= 3)
        .count();
    // A command with marks drawn under it to point at its parts, such as `|-----|--|` and `arg1 _/`.
    let marks = lines
        .iter()
        .skip(1)
        .filter(|l| l.contains("|--") || l.contains("--|") || l.contains("_/") || l.contains("\\_"))
        .count();
    // Or a line of dashes in runs, one under each part of the command above it.
    let runs = lines.iter().skip(1).filter(|l| {
        let t = l.trim();
        t.bytes().all(|b| matches!(b, b'-' | b'^' | b'~' | b' '))
            && t.split(' ').filter(|r| !r.is_empty()).count() >= 2
    });
    // A row of spans such as `<-><------>`, with lines that hang from them.
    let spans = lines.iter().any(|l| {
        l.contains("<-")
            && l.matches("->").count() >= 2
            && l.bytes().all(|b| matches!(b, b'<' | b'>' | b'-' | b' '))
    });
    let spans = spans && verticals >= 2 || spans && lines.iter().any(|l| l.contains("`-"));
    drawn >= 2 || (drawn >= 1 && verticals >= 2) || marks >= 1 || runs.count() >= 1 || spans
}

/// Whether the lines are an option of a command that goes on over a line indented under it:
/// `-V : enable verbose mode` and a line that continues it.
fn option_item(span: &[Line<'_>]) -> bool {
    let first = span[0].text;
    let mut chars = first.chars();
    chars.next() == Some('-')
        && chars
            .next()
            .is_some_and(|c| c.is_alphanumeric() || c == '-')
        && first.find(" : ").is_some_and(|at| at < 40)
        && span.len() > 1
        && span[1..]
            .iter()
            .all(|l| l.indent > span[0].indent && !l.text.contains("   "))
}

/// Whether the lines draw arrows that span a width, such as `:<---- Tq ---->:`, on at least 2 lines.
fn has_arrows(ls: &[Line<'_>]) -> bool {
    ls.iter().filter(|l| is_span(l.text)).count() >= 2
}

/// Whether a line has an arrow head at each end, as in `:<---- Tq ---->:`. A line with one head
/// only, such as a comment that points back at a command, is not a span.
fn is_span(t: &str) -> bool {
    ["<-", "<="].iter().any(|open| {
        t.find(open)
            .is_some_and(|at| ["->", "=>"].iter().any(|close| t[at + 2..].contains(close)))
    })
}

/// Whether a line is a row of marks that point at the parts of the command above it, such as
/// `|---|----|`.
fn is_marks(text: &str) -> bool {
    text.starts_with("|-")
        && text.contains("--|")
        && text.bytes().all(|b| matches!(b, b'|' | b'-' | b' '))
}

/// Whether a line is made of what draws a table or a figure.
fn drawn(text: &str) -> bool {
    let rules = text.len() >= 3
        && text.bytes().all(|b| b"-+|=_ ".contains(&b))
        && text.contains(['-', '=']);
    rules || text.contains("+--") || text.contains("--+")
}

/// A paragraph or an indented block that starts at `i`, and the index after it.
fn prose(ls: &[Line<'_>], i: usize, base: usize, out: &mut Vec<Block>) -> usize {
    let indent = ls[i].indent;
    let mut end = i;
    if indent > base {
        // An indented block runs while the lines stay indented more than the base.
        // The marks drawn under a command belong to it, even when they start left of the command.
        let marks = |k: usize| k > i && !ls[k - 1].blank() && is_marks(ls[k].text);
        while end < ls.len() && (ls[end].blank() || ls[end].indent > base || marks(end)) {
            if !ls[end].blank() && end > i && ls[end].indent == base && !marks(end) {
                break;
            }
            end += 1;
        }
        let span = trim_blank(&ls[i..end]);
        if span.iter().any(|l| is_marks(l.text)) {
            out.push(code_or_figure(common_indent(span)));
        } else if let Some(first) = span.iter().position(|l| l.blank())
            && has_arrows(&span[..first])
        {
            // A timeline, and then what follows it under the same indent.
            out.push(Block::Figure(common_indent(&span[..first])));
            region(&span[first..], out);
        } else if has_arrows(span) {
            out.push(Block::Figure(common_indent(span)));
        } else if let Some(table) = aligned_table(span) {
            out.push(table);
        } else if let Some(defs) = super::defs::definitions(span, 2) {
            push_definitions(defs, out);
        } else {
            // An indented block that is only code is one example, whatever single blank lines it
            // has. Two blank lines in a row end an example.
            let mut start = 0;
            let mut k = 0;
            while k <= span.len() {
                let double = k + 1 < span.len() && span[k].blank() && span[k + 1].blank();
                if k == span.len() || double {
                    let chunk = trim_blank(&span[start..k]);
                    if !chunk.is_empty() {
                        let mut pieces = Vec::new();
                        region(chunk, &mut pieces);
                        if pieces.len() > 1 && pieces.iter().all(|b| matches!(b, Block::Code(_))) {
                            out.push(Block::Code(common_indent(chunk)));
                        } else {
                            out.append(&mut pieces);
                        }
                    }
                    while k < span.len() && span[k].blank() {
                        k += 1;
                    }
                    start = k;
                    if k >= span.len() {
                        break;
                    }
                } else {
                    k += 1;
                }
            }
        }
        return end;
    }
    while end < ls.len() && !ls[end].blank() {
        // A line that ends with a colon and is followed by more indented lines introduces them.
        if end > i
            && ls[end - 1].text.ends_with(':')
            && ls[end].indent > indent
            && ls[i..end].iter().all(|l| l.indent == indent)
        {
            break;
        }
        if end > i
            && (is_marker(ls[end].text)
                || (super::grid::is_border(ls[end].text)
                    && !ls[i..end].iter().any(|l| super::grid::is_border(l.text)))
                || ls[end].text.starts_with("May be used in")
                || label(ls[end].text).is_some())
        {
            break;
        }
        end += 1;
    }
    let span = &ls[i..end];
    if let Some((kind, first)) = note(span[0].text) {
        let hang = span.get(1).map_or(indent, |l| l.indent);
        if span[1..].iter().all(|l| l.indent == hang)
            && !span.iter().any(|l| l.text.contains("   "))
        {
            let mut text = first.to_string();
            for (k, l) in span.iter().enumerate().skip(1) {
                text.push(super::text::separator(&span[k - 1], l));
                text.push_str(l.text);
            }
            out.push(Block::Note {
                kind,
                text: text.trim().to_string(),
            });
            return end;
        }
    }
    // The lines after the first share one indent, which may be deeper than the first.
    let hang = span.get(1).map_or(indent, |l| l.indent);
    let same = hang >= indent && span[1..].iter().all(|l| l.indent == hang);
    let gaps = span.iter().any(|l| l.text.contains("   ") || drawn(l.text));
    let spaced = |t: &str| {
        ["  ", " : ", " - "]
            .iter()
            .any(|m| t.find(m).is_some_and(|p| p < 48))
    };
    if same
        && spaced(span[0].text)
        && let Some(defs) = super::defs::definitions(span, 1)
    {
        push_definitions(defs, out);
    } else if span.len() == 1
        && (span[0].text.starts_with("http://") || span[0].text.starts_with("https://"))
        && !span[0].text.contains(' ')
    {
        out.push(Block::Para(format!("<{}>", span[0].text)));
    } else if option_item(span) || (same && !gaps && super::defs::wrapped(span)) {
        out.push(Block::Para(flow(span)));
    } else if has_arrows(span) {
        out.push(Block::Figure(common_indent(span)));
    } else if let Some(table) = aligned_table(span) {
        out.push(table);
    } else if let Some(defs) = super::defs::definitions(span, 2) {
        push_definitions(defs, out);
    } else {
        out.push(code_or_figure(common_indent(span)));
    }
    end
}

#[cfg(test)]
mod tests {
    use super::super::text::{lines, normalise};
    use super::*;

    fn blocks(text: &str) -> Vec<Block> {
        let t = normalise(text);
        parse(&lines(&t))
    }

    #[test]
    fn a_banner_holds_a_title_a_version_and_a_date_whatever_the_author() {
        let t = normalise(
            "   -----\n   HAProxy\n   Configuration Manual\n   -----\n   version 3.4\n   2026/06/03\n\nbody\n",
        );
        let (b, used) = banner(&lines(&t));
        assert_eq!(b.title, "HAProxy Configuration Manual");
        assert_eq!(
            (b.version.as_str(), b.date.as_str(), b.author.as_str()),
            ("3.4", "2026/06/03", "")
        );
        assert_eq!(used, 6);
        let t = normalise("-----\nHAProxy\n-----\nversion 1.3.23\nwilly tarreau\n2010/01/28\n\n");
        let (b, _) = banner(&lines(&t));
        assert_eq!(
            (b.author.as_str(), b.date.as_str()),
            ("willy tarreau", "2010/01/28")
        );
    }

    #[test]
    fn a_numbered_title_underlined_with_dashes_is_a_heading() {
        let b = blocks("4.2. Sorted keywords\n--------------------\n\nText here.\n");
        assert_eq!(
            b[0],
            Block::Heading {
                number: "4.2".into(),
                title: "Sorted keywords".into()
            }
        );
        assert_eq!(b[1], Block::Para("Text here.".into()));
    }

    #[test]
    fn a_keyword_has_its_syntax_its_contexts_its_arguments_and_its_example() {
        let doc = "backlog <conns>\n  Give hints.\n\n  May be used in the following contexts: tcp, http\n\n  May be used in sections :   defaults | frontend | listen | backend\n                                 yes   |    yes   |   yes  |   no\n\n  Arguments :\n    <conns>   is the number\n              of connections.\n\n  Some text that runs long enough to be a wrapped line of prose\n  and its end.\n\n  Example:\n        backlog 10\n\nnext <x>\n  body\n";
        let b = blocks(doc);
        assert_eq!(
            b[0],
            Block::Keyword {
                name: "backlog".into(),
                syntax: vec!["backlog <conns>".into()],
                deprecated: false
            }
        );
        assert_eq!(b[1], Block::Para("Give hints.".into()));
        assert_eq!(b[2], Block::Contexts(vec!["tcp".into(), "http".into()]));
        assert_eq!(
            b[3],
            Block::Sections(vec![
                ("defaults".into(), "yes".into()),
                ("frontend".into(), "yes".into()),
                ("listen".into(), "yes".into()),
                ("backend".into(), "no".into())
            ])
        );
        assert_eq!(
            b[4],
            Block::Arguments {
                text: String::new(),
                items: vec![("<conns>".into(), "is the number of connections.".into())]
            }
        );
        assert_eq!(
            b[5],
            Block::Para(
                "Some text that runs long enough to be a wrapped line of prose and its end.".into()
            )
        );
        assert_eq!(
            b[6],
            Block::Example {
                text: String::new(),
                code: vec!["backlog 10".into()]
            }
        );
        assert_eq!(
            b[7],
            Block::Keyword {
                name: "next".into(),
                syntax: vec!["next <x>".into()],
                deprecated: false
            }
        );
    }

    #[test]
    fn stacked_syntax_lines_name_one_keyword() {
        let b = blocks("option httpclose\nno option httpclose\n  Enable or disable.\n");
        assert_eq!(
            b[0],
            Block::Keyword {
                name: "option httpclose".into(),
                syntax: vec!["option httpclose".into(), "no option httpclose".into()],
                deprecated: false
            }
        );
    }

    #[test]
    fn an_item_with_numbered_lines_under_its_text_has_an_inner_list() {
        let b = blocks(
            "  - CONNECT (or 1): flags,\n                    username\n                    only:\n                       17: Session\n                       33: Receive\n                    Not yet:\n                       38: User\n                       39: Other\n",
        );
        let Block::List(items) = &b[0] else {
            panic!("{b:?}")
        };
        assert_eq!(items[0].text, "CONNECT (or 1): flags, username only:");
        assert!(
            matches!(&items[0].rest[0], Block::List(l) if l.len() == 2 && l[0].text == "17: Session"),
            "{items:?}"
        );
        assert!(
            matches!(&items[0].rest[1], Block::Para(p) if p == "Not yet:"),
            "{items:?}"
        );
        assert!(
            matches!(&items[0].rest[2], Block::List(l) if l.len() == 2),
            "{items:?}"
        );
    }

    #[test]
    fn a_list_keeps_its_wrapped_items_and_its_nested_items() {
        let b = blocks("  - one\n    wraps\n  - two\n      - inner\n  - three\n");
        let Block::List(items) = &b[0] else {
            panic!("{b:?}")
        };
        assert_eq!(items[0].text, "one wraps");
        assert!(matches!(&items[1].rest[0], Block::List(inner) if inner[0].text == "inner"));
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn aligned_text_and_tables_stay_preformatted() {
        let b = blocks("Names:\n\n  a    b\n  ---+---\n  1  | 2\n");
        assert!(
            matches!(&b[1], Block::Table { header, rows } if header == &["a", "b"] && rows == &[vec!["1".to_string(), "2".to_string()]])
        );
        let b = blocks("Figure:\n\n  +---+\n  | a |\n  +---+\n");
        assert!(matches!(&b[1], Block::Figure(c) if c.len() == 3));
        let b = blocks("A line   with a gap\nand more.\n");
        assert!(matches!(&b[0], Block::Code(_)));
    }

    #[test]
    fn prose_that_starts_in_lower_case_is_not_a_keyword() {
        assert!(keyword_name("the following list of words").is_none());
        assert_eq!(
            keyword_name("timeout client <timeout>").as_deref(),
            Some("timeout client")
        );
        assert_eq!(
            keyword_name("req.hdr([<name>[,<occ>]]) : string").as_deref(),
            Some("req.hdr")
        );
        assert_eq!(keyword_name("base : string").as_deref(), Some("base"));
        assert!(keyword_name("Option").is_none());
    }
}

#[cfg(test)]
mod item_tests {
    use super::super::text::{lines, normalise};
    use super::*;

    #[test]
    fn an_item_keeps_its_paragraphs_and_its_examples() {
        let t = normalise(
            "  - A form :\n\n      /serv/login.php\n\n    It is complete.\n\n  - Another one :\n\n    Text.\n",
        );
        let ls = lines(&t);
        let mut out = Vec::new();
        region(&ls, &mut out);
        let [Block::List(items)] = &out[..] else {
            panic!("{out:?}")
        };
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].text, "A form :");
        assert_eq!(
            items[0].rest,
            vec![
                Block::Code(vec!["/serv/login.php".into()]),
                Block::Para("It is complete.".into())
            ]
        );
        assert_eq!(items[1].rest, vec![Block::Para("Text.".into())]);
    }
}

#[cfg(test)]
mod note_tests {
    use super::*;

    #[test]
    fn a_note_label_gives_its_alert() {
        assert_eq!(note("Important note: text"), Some(("IMPORTANT", "text")));
        assert_eq!(note("Important note:"), Some(("IMPORTANT", "")));
        assert_eq!(note("NOTE : text"), Some(("NOTE", "text")));
        assert_eq!(note("Warning: x"), Some(("WARNING", "x")));
        assert_eq!(note("Attention: x"), Some(("WARNING", "x")));
        assert_eq!(note("Notes: x"), Some(("NOTE", "x")));
        assert_eq!(note("Noted: x"), None);
        assert_eq!(note("Importantly: x"), None);
        assert_eq!(note("A note: x"), None);
    }
}

#[cfg(test)]
mod layout_tests {
    use super::super::text::{lines, normalise};
    use super::*;

    fn blocks(text: &str) -> Vec<Block> {
        let t = normalise(text);
        parse(&lines(&t))
    }

    #[test]
    fn numbered_items_make_an_ordered_list() {
        let b = blocks(
            "1. a configuration file is an ordered sequence of statements that
   goes on.
2. next one that is long enough to be a sentence of its own right here
",
        );
        let [Block::Ordered { start, items }] = &b[..] else {
            panic!("{b:?}")
        };
        assert_eq!(*start, 1);
        assert_eq!(items.len(), 2);
        assert!(items[0].text.ends_with("goes on."));
    }

    #[test]
    fn one_argument_is_a_definition() {
        let b = blocks(
            "kw <x>
  Text.

  Arguments :
    <username> This is the username which will be used when connecting.
",
        );
        assert!(b.iter().any(|x| matches!(x, Block::Arguments { items, .. } if items.len() == 1 && items[0].0 == "<username>")));
    }

    #[test]
    fn usable_in_names_the_places_with_a_mark() {
        let b = blocks(
            "accept
  Usable in:  QUIC Ini|    TCP RqCon| RqSes|    HTTP Req| Res
                    X |          X  |   -  |         X |  -

  Text.
",
        );
        let Some(Block::Usable(rows)) = b.iter().find(|x| matches!(x, Block::Usable(_))) else {
            panic!("{b:?}")
        };
        let names: Vec<&str> = rows.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            ["QUIC Ini", "TCP RqCon", "TCP RqSes", "HTTP Req", "HTTP Res"]
        );
    }

    #[test]
    fn columns_lined_up_with_spaces_make_a_table_whose_cells_go_on_below() {
        let b = blocks(
            "Codes :\n\n  Code  When / reason\n   200  access to the stats page\n   401  when an authentication is required (when\n        accessing the page)\n   404  when the resource is not found\n",
        );
        let Some(Block::Table { header, rows }) =
            b.iter().find(|x| matches!(x, Block::Table { .. }))
        else {
            panic!("{b:?}")
        };
        assert_eq!(header, &["Code", "When / reason"]);
        assert_eq!(
            rows[1],
            [
                "401",
                "when an authentication is required (when accessing the page)"
            ]
        );
    }

    #[test]
    fn a_sentence_that_carries_code_words_stays_prose_and_marks_stay_with_their_command() {
        let text = "Example:\n\n   .if defined( HAPROXY_MWORKER )\n\nwill test for the existence of variable \" HAPROXY_MWORKER \" (with spaces),\nand this one:\n\n   .if streq(\"$X\",     1)\n  |---|--------------------|   |--|\n    1           2               3\n\nthen.\n";
        let mut b = Vec::new();
        region(&lines(&normalise(text)), &mut b);
        assert!(
            b.iter()
                .any(|x| matches!(x, Block::Para(p) if p.starts_with("will test"))),
            "{b:?}"
        );
        assert!(
            b.iter()
                .any(|x| matches!(x, Block::Figure(f) if f.len() == 3)),
            "{b:?}"
        );
    }

    #[test]
    fn quoted_names_with_their_descriptions_are_a_table() {
        let text = "'stream+<family>@<address>' forces socket type and transport method\n                            to \"stream\"\n\n'dgram+<family>@<address>'  forces socket type to \"datagram\".\n\n'quic+<family>@<address>'   forces it.\n\nThen a sentence.\n";
        let mut b = Vec::new();
        region(&lines(&normalise(text)), &mut b);
        let Some(Block::Table { rows, .. }) = b.first() else {
            panic!("{b:?}");
        };
        assert_eq!(rows.len(), 3, "{b:?}");
        assert_eq!(rows[0][0], "\u{2}stream+<family>@<address>");
        assert!(rows[0][1].ends_with("to \"stream\""), "{rows:?}");
        assert!(matches!(&b[1], Block::Para(_)), "{b:?}");
    }

    #[test]
    fn an_indented_example_with_blank_lines_is_one_block() {
        let text = "Examples\n\n    # a comment\n    global\n        daemon\n\n    defaults\n        mode http\n\n    backend b\n        server s 127.0.0.1:80\n\nThen a sentence.\n";
        let mut b = Vec::new();
        region(&lines(&normalise(text)), &mut b);
        let codes: Vec<_> = b.iter().filter(|x| matches!(x, Block::Code(_))).collect();
        assert_eq!(codes.len(), 1, "{b:?}");
        let Block::Code(c) = codes[0] else {
            unreachable!()
        };
        assert_eq!(c.len(), 9, "{c:?}");
    }

    #[test]
    fn a_line_that_repeats_the_column_names_is_not_a_row() {
        let text = "keyword      input type   output type\nadd(value)   integer      integer\nand(value)   integer      integer\n-- keyword ------+- input type + output type -\ndiv(value)   integer      integer\nmul(value)   integer      integer\n";
        let mut b = Vec::new();
        region(&lines(&normalise(text)), &mut b);
        let Some(Block::Table { rows, .. }) = b.first() else {
            panic!("{b:?}");
        };
        assert_eq!(rows.len(), 4, "{rows:?}");
        assert!(rows.iter().all(|r| !r[0].contains("keyword")), "{rows:?}");
    }

    #[test]
    fn a_matrix_with_a_rule_under_its_header_drops_the_repeated_names_too() {
        let text = " keyword        defaults   frontend\n--------------+----------+---------\nacl                 X          X\nbind                -          X\n-- keyword ------ defaults - frontend -\nmode                X          X\n";
        let mut b = Vec::new();
        region(&lines(&normalise(text)), &mut b);
        let Some(Block::Table { rows, .. }) = b.first() else {
            panic!("{b:?}");
        };
        assert_eq!(rows.len(), 3, "{rows:?}");
        assert!(rows.iter().all(|r| !r[0].contains("keyword")), "{rows:?}");
    }

    #[test]
    fn a_timeline_with_arrows_is_a_figure_and_not_a_table() {
        let text = "Timings:\n\n      |<--------->|<----- ...\n      t     tr      t\n   ---|----|----|----|--\n      : Th   Ti   TR : Ti\n      :<-- Tq -->:\n      :<--- Tt --->:\n\nAfter.\n";
        let mut b = Vec::new();
        region(&lines(&normalise(text)), &mut b);
        assert!(
            b.iter()
                .any(|x| matches!(x, Block::Figure(f) if f.len() == 6)),
            "{b:?}"
        );
        assert!(!b.iter().any(|x| matches!(x, Block::Table { .. })), "{b:?}");
    }

    #[test]
    fn a_session_with_arrows_that_point_back_is_not_a_figure() {
        let text = "Example:\n\n  [0:00:58] master> @!11955     <-- master, switch\n  [0:01:03] 11955> @!11942      <-- current worker\n  [0:02:17] 11942> @            <-- older worker\n";
        let mut b = Vec::new();
        region(&lines(&normalise(text)), &mut b);
        assert!(!b.iter().any(|x| matches!(x, Block::Figure(_))), "{b:?}");
    }

    #[test]
    fn an_option_that_goes_on_under_its_first_line_is_a_paragraph() {
        let text = "-V : enable verbose mode (disables quiet mode). Reverts the effect of \"-q\" or\n  \"quiet\".\n\n-W : master-worker mode.\n";
        let mut b = Vec::new();
        region(&lines(&normalise(text)), &mut b);
        assert!(
            matches!(&b[0], Block::Para(p) if p.ends_with("or \"quiet\".")),
            "{b:?}"
        );
        assert!(!b.iter().any(|x| matches!(x, Block::Code(_))), "{b:?}");
    }

    #[test]
    fn an_example_goes_on_with_its_sentence_before_the_code() {
        let b = blocks(
            "  Example: track the source address by default, or the one advertised\n\n       in the header for the connections that come from the local proxies\n       and the second rule tracks whatever address we decide to keep.\n\n    tcp-request session track-sc0 src\n",
        );
        let Some(Block::Example { text, code }) =
            b.iter().find(|x| matches!(x, Block::Example { .. }))
        else {
            panic!("{b:?}")
        };
        assert!(text.contains("in the header for the connections"), "{text}");
        assert_eq!(code, &["tcp-request session track-sc0 src"]);
    }

    #[test]
    fn a_title_underlined_inside_a_chapter_is_a_subtitle() {
        let b = blocks(
            "1. Chapter\n----------\n\nSome words\n----------\n\nText follows here and it is a full sentence.\n",
        );
        assert!(
            b.iter()
                .any(|x| matches!(x, Block::Subtitle(t) if t == "Some words"))
        );
    }

    #[test]
    fn a_deprecated_keyword_is_flagged() {
        let b = blocks("monitor-net <source> (deprecated)\n  Text of the keyword.\n");
        assert!(matches!(
            &b[0],
            Block::Keyword {
                deprecated: true,
                ..
            }
        ));
    }

    #[test]
    fn indented_prose_after_a_label_is_read_as_prose_and_code_stays_code() {
        let b = blocks(
            "Parameters:\n\n  They are optional and can be used to describe how a thing should open and\n  use the connection.\n\n  log global\n  option httplog\n",
        );
        assert!(
            b.iter()
                .any(|x| matches!(x, Block::Para(t) if t.starts_with("They are optional")))
        );
        assert!(
            b.iter()
                .any(|x| matches!(x, Block::Code(c) if c == &["log global", "option httplog"]))
        );
    }
}

#[cfg(test)]
mod figure_tests {
    use super::*;

    fn lines(text: &[&str]) -> Vec<String> {
        text.iter().map(|l| l.to_string()).collect()
    }

    #[test]
    fn a_command_with_marks_under_it_is_a_figure() {
        assert!(is_figure(&lines(&[
            "http-request set-path %[path,regsub((foo|bar),blah,g)]",
            "                                   |---------|----|-|",
            "                             arg1 _/         /    /",
        ])));
        assert!(!is_figure(&lines(&[
            "http-request set-path %[path,regsub((foo|bar),blah,g)]"
        ])));
        assert!(is_figure(&lines(&[
            "http-request set-path %[path,regsub(\"(foo|bar)\",blah,g)]",
            "------------ -------- ----------------------------------",
            "   word1       word2    word3=%[path]",
        ])));
        assert!(!is_figure(&lines(&["log global", "option httplog"])));
    }
}
