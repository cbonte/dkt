//! Shapes of text inside a chapter that are told apart by how they are laid out: list markers,
//! lists of definitions whose descriptions line up, and paragraphs wrapped like prose.

use super::text::Line;

/// A list marker at the start of a text: its length with the space after it, and whether it is a
/// number.
pub fn marker(text: &str) -> Option<(usize, bool)> {
    if text.starts_with("- ") || text.starts_with("* ") {
        return Some((2, false));
    }
    let b = text.as_bytes();
    let digits = b.iter().take_while(|c| c.is_ascii_digit()).count();
    if (1..=3).contains(&digits)
        && matches!(b.get(digits), Some(b'.' | b')'))
        && b.get(digits + 1) == Some(&b' ')
    {
        return Some((digits + 2, true));
    }
    None
}

/// The number that starts an ordered item.
pub fn number(text: &str) -> Option<u32> {
    let digits = text.bytes().take_while(|c| c.is_ascii_digit()).count();
    text[..digits].parse().ok()
}

/// A line of a definition list: the name, the text after it and the column that text starts at.
/// The name is a word and the placeholders that follow it, or a word before a colon or a dash.
fn split(line: &Line<'_>) -> Option<(String, String, Option<usize>)> {
    let t = line.text;
    for sep in [" : ", " - "] {
        if let Some(p) = t.find(sep)
            && !t[..p].trim().is_empty()
            && t[..p].trim().chars().count() <= 40
            && (sep == " : " || !t[..p].trim().contains(' '))
        {
            let rest = t[p + 3..].trim_start();
            let col = line.indent + t[..t.len() - rest.len()].chars().count();
            return Some((t[..p].trim_end().to_string(), rest.to_string(), Some(col)));
        }
    }
    let mut end = t.find(' ').unwrap_or(t.len());
    let first = &t[..end];
    if first.is_empty()
        || first.chars().count() > 40
        || first.ends_with([',', '.', ';'])
        || first.contains('=')
    {
        return None;
    }
    // Placeholders that follow the word belong to the name.
    loop {
        let rest = t[end..].trim_start();
        if !rest.starts_with('<') {
            break;
        }
        let token_end = rest.find(' ').unwrap_or(rest.len());
        if !rest[..token_end].ends_with(['>', '*']) {
            break;
        }
        end = t.len() - rest.len() + token_end;
    }
    let name = &t[..end];
    let rest = t[end..].trim_start();
    if rest.is_empty() {
        return Some((name.to_string(), String::new(), None));
    }
    let col = line.indent + t[..t.len() - rest.len()].chars().count();
    Some((name.to_string(), rest.to_string(), Some(col)))
}

/// The entries of a list of definitions, such as the arguments of a keyword: a name, then a
/// description that starts at the same column for every entry, and goes on under itself.
pub fn definitions(body: &[Line<'_>], least: usize) -> Option<Vec<(String, String)>> {
    let lines: Vec<&Line<'_>> = body.iter().filter(|l| !l.blank()).collect();
    let min = lines.iter().map(|l| l.indent).min()?;
    struct Entry {
        name: String,
        text: String,
        col: Option<usize>,
        continued: Vec<usize>,
    }
    let mut entries: Vec<Entry> = Vec::new();
    for l in &lines {
        if l.indent == min {
            let (name, text, col) = split(l)?;
            entries.push(Entry {
                name,
                text,
                col,
                continued: Vec::new(),
            });
        } else {
            let e = entries.last_mut()?;
            e.continued.push(l.indent);
            if !e.text.is_empty() {
                e.text.push(' ');
            }
            e.text.push_str(l.text);
        }
    }
    if entries.len() < least {
        return None;
    }
    if entries.len() == 1
        && !lone(
            &entries[0].name,
            &entries[0].text,
            entries[0].col,
            min,
            lines[0].text,
        )
    {
        return None;
    }
    let mut names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    if names.len() * 10 < entries.len() * 8 {
        return None;
    }
    let cols: Vec<usize> = entries.iter().filter_map(|e| e.col).collect();
    let common = cols
        .iter()
        .copied()
        .max_by_key(|c| cols.iter().filter(|d| *d == c).count())?;
    let aligned = cols.iter().filter(|c| **c == common).count();
    if aligned < least.min(2) || aligned * 10 < cols.len() * 7 {
        return None;
    }
    for e in &entries {
        if e.col.is_some_and(|c| c != common) && !e.name.starts_with('<') {
            return None;
        }
        if e.continued.iter().any(|c| c.abs_diff(common) > 1) {
            return None;
        }
        if e.col.is_none() && e.continued.is_empty() {
            return None;
        }
    }
    Some(entries.into_iter().map(|e| (e.name, e.text)).collect())
}

/// Whether a line has a mark of a sentence: a period, a comma, a semicolon or a colon that ends a
/// word.
fn sentence_mark(text: &str) -> bool {
    text.ends_with(['.', ',', ';', ':', '!', '?'])
        || [". ", ", ", "; ", ": "].iter().any(|m| text.contains(m))
}

/// Whether a single entry is clearly a definition: a description that starts with a letter, after
/// a gap of two spaces or more, a colon or a dash.
fn lone(name: &str, text: &str, col: Option<usize>, indent: usize, line: &str) -> bool {
    let placeholder = name.starts_with(['<', '\'', '"']);
    let gap = line.find("  ").is_some_and(|p| p < name.len() + 4)
        || line.contains(" : ")
        || line.contains(" - ");
    let reach = if placeholder { 1 } else { 2 };
    let far = col.is_some_and(|c| c >= indent + name.chars().count() + reach);
    (placeholder || gap) && far && text.chars().next().is_some_and(|c| c.is_alphabetic())
}

/// Whether lines that start at the same column read as a wrapped paragraph rather than commands:
/// a sentence on one line, or lines that are mostly long and carry the punctuation of sentences.
pub fn wrapped(span: &[Line<'_>]) -> bool {
    let words: usize = span
        .iter()
        .map(|l| l.text.split(' ').filter(|w| !w.is_empty()).count())
        .sum();
    if span.len() == 1 {
        let t = span[0].text;
        if t.chars().next().is_some_and(char::is_uppercase) {
            return true;
        }
        let code = t.contains(['_', '(', '=', '<', '[', '{', '%', '$']) || t.ends_with("...");
        let sentence = t.ends_with(['.', '!', '?']) && !t.ends_with("...");
        return (words >= 4 && sentence)
            || (!code && ((words >= 3 && t.ends_with(':')) || words >= 8));
    }
    let body = &span[..span.len() - 1];
    let short = body
        .iter()
        .filter(|l| l.text.chars().count() < 25 && !l.text.ends_with(['.', '!', '?', ':']))
        .count();
    let marked = span.iter().filter(|l| sentence_mark(l.text)).count();
    let upper = span[0].text.chars().next().is_some_and(char::is_uppercase);
    let long = body.iter().filter(|l| l.text.chars().count() >= 55).count();
    let dense = span
        .iter()
        .filter(|l| l.text.contains(['_', '(', ')', '=', '%', '$']))
        .count();
    // A command line does not end on a comma, a sentence that goes on does.
    let goes_on = body.iter().all(|l| l.text.ends_with(','));
    let score = usize::from(goes_on)
        + usize::from(upper)
        + usize::from(marked * 2 >= span.len())
        + usize::from(words >= 8 * span.len())
        + usize::from(long * 2 >= body.len());
    let needed = if dense * 2 >= span.len() { 3 } else { 2 };
    score >= needed && short * 4 <= body.len()
}

#[cfg(test)]
mod tests {
    use super::super::text::{lines, normalise};
    use super::*;

    fn read(text: &str) -> Vec<(String, String)> {
        let t = normalise(text);
        definitions(&lines(&t), 2).unwrap_or_default()
    }

    #[test]
    fn markers_are_bullets_and_numbers() {
        assert_eq!(marker("- a"), Some((2, false)));
        assert_eq!(marker("12. a"), Some((4, true)));
        assert_eq!(marker("1) a"), Some((3, true)));
        assert_eq!(marker("2.4 a"), None);
        assert_eq!(marker("-a"), None);
        assert_eq!(number("12. a"), Some(12));
    }

    #[test]
    fn definitions_have_names_and_descriptions_that_line_up() {
        let d = read("<conns>   is the number\n          of connections.\n<other>   is another.\n");
        assert_eq!(
            d,
            [
                (
                    "<conns>".to_string(),
                    "is the number of connections.".to_string()
                ),
                ("<other>".to_string(), "is another.".to_string())
            ]
        );
        let d = read("base     : exact string match\nbase_beg : prefix match\n");
        assert_eq!(d[1], ("base_beg".to_string(), "prefix match".to_string()));
    }

    #[test]
    fn a_configuration_is_not_a_list_of_definitions() {
        assert!(read("server a 1.1.1.1:80\nserver b 1.1.1.2:80\n").is_empty());
        assert!(read("global\n    log x\nfrontend www\n    bind :80\n").is_empty());
        assert!(read("acl a  src 1\nacl b  src_port 2\n").is_empty());
    }

    #[test]
    fn prose_runs_long_and_commands_do_not() {
        let t = normalise(
            "This is a line long enough to look like a wrapped paragraph of prose\nand its end.\n",
        );
        assert!(wrapped(&lines(&t)));
        let t = normalise("log global\noption httplog\nmode http\n");
        assert!(!wrapped(&lines(&t)));
        let t = normalise("stick-table type ip size 1m expire 5m store gpc0\nstick on src\n");
        assert!(!wrapped(&lines(&t)));
        let t = normalise(
            "This action performs a resolution of the output and stores\nthe result in the variable. It uses the section\npointed by <resolvers>.\n",
        );
        assert!(wrapped(&lines(&t)));
        let t = normalise("acl <aclname> <criterion> [flags] [operator] <value> ...\n");
        assert!(!wrapped(&lines(&t)));
        let t = normalise("bind :80 ssl\n");
        assert!(!wrapped(&lines(&t)));
    }
}
