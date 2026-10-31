//! The `include` directive, which puts the text of another file in its place before anything is read.

use std::path::Path;

/// How many files an include may reach through other includes.
const DEPTH: usize = 8;

/// The text with each `.. include::` of a file that can be read replaced by what the file holds.
/// A file that cannot be read leaves its directive as it was.
pub fn expand(source: &str, dir: &Path) -> String {
    expand_at(source, dir, 0)
}

fn expand_at(source: &str, dir: &Path, depth: usize) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let text = line.trim_start();
        let indent = line.len() - text.len();
        let Some(name) = text
            .strip_prefix(".. include::")
            .map(str::trim)
            .filter(|n| !n.is_empty())
        else {
            out.push(line.to_string());
            i += 1;
            continue;
        };
        // The options come on the lines under the directive, indented deeper.
        let mut end = i + 1;
        let mut options: Vec<(String, String)> = Vec::new();
        while end < lines.len() {
            let option = lines[end].trim_start();
            let deeper = lines[end].len() - option.len() > indent;
            match option.strip_prefix(':').and_then(|o| o.split_once(':')) {
                Some((key, value)) if deeper => {
                    options.push((key.to_string(), value.trim().to_string()));
                    end += 1;
                }
                _ => break,
            }
        }
        let read = (depth < DEPTH)
            .then(|| {
                let path = dir.join(name.trim_start_matches('/'));
                std::fs::read_to_string(&path).ok().map(|t| (t, path))
            })
            .flatten();
        let Some((text, path)) = read else {
            out.extend(lines[i..end].iter().map(|l| l.to_string()));
            i = end;
            continue;
        };
        let included = cut(&text, &options);
        let literal = options.iter().find(|(k, _)| k == "literal");
        let code = options.iter().find(|(k, _)| k == "code");
        let pad = " ".repeat(indent);
        out.push(String::new());
        if literal.is_some() || code.is_some() {
            let lang = code
                .map(|(_, l)| l.as_str())
                .filter(|l| !l.is_empty())
                .unwrap_or("text");
            out.push(format!("{pad}.. code-block:: {lang}"));
            out.push(String::new());
            out.extend(included.lines().map(|l| format!("{pad}   {l}")));
        } else {
            let parent = path.parent().unwrap_or(dir);
            let inner = expand_at(&included, parent, depth + 1);
            out.extend(inner.lines().map(|l| {
                if l.is_empty() {
                    String::new()
                } else {
                    format!("{pad}{l}")
                }
            }));
        }
        out.push(String::new());
        i = end;
    }
    out.join("\n")
}

/// The part of a text that the options of an include select.
fn cut(text: &str, options: &[(String, String)]) -> String {
    let get = |name: &str| {
        options
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };
    let mut lines: Vec<&str> = text.lines().collect();
    if let Some(marker) = get("start-after")
        && let Some(at) = lines.iter().position(|l| l.contains(marker))
    {
        lines.drain(..=at);
    }
    if let Some(marker) = get("end-before")
        && let Some(at) = lines.iter().position(|l| l.contains(marker))
    {
        lines.truncate(at);
    }
    let number = |name: &str| get(name).and_then(|v| v.parse::<usize>().ok());
    let start = number("start-line").unwrap_or(0).min(lines.len());
    let stop = number("end-line")
        .unwrap_or(lines.len())
        .clamp(start, lines.len());
    lines[start..stop].join("\n")
}
