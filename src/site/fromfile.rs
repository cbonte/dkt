//! The content that a widget takes from a file instead of from the document.
//!
//! `<!-- site: widget replay src=session.cast -->` takes the session from the file, written
//! relative to the document, and replaces the blocks that the widget holds.
//! The file is plain text, or an asciicast recording of version 2.
//! The recording gives its lines, not its timing: the widget types a line that starts with `$ `
//! and shows the others one after the other.
//!
//! `<!-- site: widget table src=rules.tsv -->` builds the table from a file of values separated
//! by tabs or by commas, whose first row gives the headings.
//! `columns="a,b"` keeps the columns of those headings, in that order.

use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::annotation::{self, Kind};

/// The document with the sessions of its `replay` widgets read from their files.
pub fn expand(markdown: &str, dir: Option<&Path>) -> Result<String> {
    let mut out = String::with_capacity(markdown.len());
    let mut lines = markdown.split_inclusive('\n');
    while let Some(line) = lines.next() {
        out.push_str(line);
        let Some((widget, src, columns)) = file_widget(line) else {
            continue;
        };
        let path = dir.map_or_else(|| src.clone().into(), |d| d.join(&src));
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("the {widget} names {src}, which cannot be read"))?;
        let body = if widget == "table" {
            table(&raw, &path, columns.as_deref())
                .with_context(|| format!("cannot read the table {src}"))?
        } else {
            let session =
                session(&raw).with_context(|| format!("cannot read the session {src}"))?;
            let fence = fence_for(&session);
            format!("{fence}\n{session}\n{fence}\n")
        };
        out.push_str(&body);
        // What the widget held is replaced, up to its `end`.
        for held in lines.by_ref() {
            if is_end(held) {
                out.push_str(held);
                break;
            }
        }
    }
    Ok(out)
}

/// The widget, the file and the columns that a line names, when the line is a `replay` or a
/// `table` widget with a `src`.
fn file_widget(line: &str) -> Option<(&'static str, String, Option<String>)> {
    let body = annotation::comment(line)?;
    let Ok(note) = annotation::parse(body)? else {
        return None;
    };
    if note.kind != Kind::Widget {
        return None;
    }
    let widget = match note.get("widget")? {
        "replay" => "replay",
        "table" => "table",
        _ => return None,
    };
    let src = note.get("src")?.to_string();
    Some((widget, src, note.get("columns").map(str::to_string)))
}

fn is_end(line: &str) -> bool {
    annotation::comment(line)
        .and_then(annotation::parse)
        .is_some_and(|r| r.is_ok_and(|n| n.kind == Kind::End))
}

/// The Markdown table of a file of values, with the columns that `columns` names.
fn table(raw: &str, path: &Path, columns: Option<&str>) -> Result<String> {
    let comma = path.extension().is_some_and(|e| e == "csv");
    let rows: Vec<Vec<String>> = raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            if comma {
                split_csv(l)
            } else {
                l.split('\t').map(str::to_string).collect()
            }
        })
        .collect();
    let Some(head) = rows.first() else {
        bail!("the file is empty");
    };
    let picked: Vec<usize> = match columns {
        None => (0..head.len()).collect(),
        Some(names) => names
            .split(',')
            .map(|name| {
                head.iter()
                    .position(|h| h.trim() == name.trim())
                    .with_context(|| format!("the file has no column '{}'", name.trim()))
            })
            .collect::<Result<_>>()?,
    };
    let cell = |row: &[String], i: usize| {
        row.get(i)
            .map(|c| c.trim().replace('|', "\\|").replace('\n', " "))
            .unwrap_or_default()
    };
    let line = |row: &[String]| {
        let cells: Vec<String> = picked.iter().map(|&i| cell(row, i)).collect();
        format!("| {} |\n", cells.join(" | "))
    };
    let mut out = line(head);
    out.push_str(&format!("|{}\n", " --- |".repeat(picked.len())));
    for row in &rows[1..] {
        out.push_str(&line(row));
    }
    Ok(out)
}

/// The fields of a line of comma separated values, a double-quoted field keeping its commas.
fn split_csv(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let (mut current, mut quoted) = (String::new(), false);
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                current.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => fields.push(std::mem::take(&mut current)),
            c => current.push(c),
        }
    }
    fields.push(current);
    fields
}

/// A fence of backticks longer than any run in the session.
fn fence_for(session: &str) -> String {
    let mut longest = 0;
    let mut run = 0;
    for c in session.chars() {
        run = if c == '`' { run + 1 } else { 0 };
        longest = longest.max(run);
    }
    "`".repeat((longest + 1).max(3))
}

/// The lines of a recording: the file itself, or the output of an asciicast without its escapes.
fn session(raw: &str) -> Result<String> {
    if !raw.trim_start().starts_with('{') {
        return Ok(raw.trim_end().to_string());
    }
    let mut lines = raw.lines();
    let header = lines.next().unwrap_or_default();
    if !header.contains("\"version\"") {
        bail!("the file starts like a recording and has no version");
    }
    let mut text = String::new();
    for line in lines.filter(|l| !l.trim().is_empty()) {
        if let Some(data) = output_of(line)? {
            text.push_str(&data);
        }
    }
    let text = strip_escapes(&text).replace("\r\n", "\n").replace('\r', "");
    Ok(text.trim_end().to_string())
}

/// The text of an output event, `[time, "o", "text"]`, and nothing for the other events.
fn output_of(line: &str) -> Result<Option<String>> {
    let inner = line
        .trim()
        .strip_prefix('[')
        .and_then(|l| l.strip_suffix(']'))
        .context("an event is not a list")?;
    let mut parts = inner.splitn(3, ',');
    parts.next();
    if parts.next().map(str::trim) != Some("\"o\"") {
        return Ok(None);
    }
    let quoted = parts.next().context("an event has no text")?.trim();
    unquote(quoted).map(Some)
}

/// The text of a JSON string.
fn unquote(quoted: &str) -> Result<String> {
    let body = quoted
        .strip_prefix('"')
        .and_then(|q| q.strip_suffix('"'))
        .context("the text of an event is not a string")?;
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next().context("a string ends inside an escape")? {
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'b' | 'f' => {}
            'u' => {
                let hex: String = chars.by_ref().take(4).collect();
                let mut code = u32::from_str_radix(&hex, 16).context("a bad \\u escape")?;
                if (0xD800..0xDC00).contains(&code) {
                    let rest: String = chars.by_ref().take(6).collect();
                    let low = u32::from_str_radix(rest.trim_start_matches("\\u"), 16)
                        .context("a bad surrogate pair")?;
                    code = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                }
                out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
            }
            other => out.push(other),
        }
    }
    Ok(out)
}

/// The text without the escape sequences that colour or move the cursor.
fn strip_escapes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('[') => {
                for d in chars.by_ref() {
                    if ('@'..='~').contains(&d) {
                        break;
                    }
                }
            }
            Some(']') => {
                while let Some(d) = chars.next() {
                    if d == '\u{7}' || (d == '\u{1b}' && chars.next() == Some('\\')) {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_file_is_the_session() {
        assert_eq!(session("$ ls\na b\n\n").unwrap(), "$ ls\na b");
    }

    #[test]
    fn a_recording_gives_its_output_without_escapes() {
        let cast = "{\"version\": 2, \"width\": 80, \"height\": 24}\n\
                    [0.1, \"o\", \"$ ls\\r\\n\"]\n\
                    [0.4, \"i\", \"x\"]\n\
                    [0.9, \"o\", \"\\u001b[32ma\\u001b[0m b\\r\\n\"]\n";
        assert_eq!(session(cast).unwrap(), "$ ls\na b");
    }

    #[test]
    fn a_file_of_values_is_a_table() {
        let tsv = "name\tdefault\tnote\na\t1\tx|y\nb\t2\t\n";
        assert_eq!(
            table(tsv, Path::new("t.tsv"), None).unwrap(),
            "| name | default | note |\n| --- | --- | --- |\n| a | 1 | x\\|y |\n| b | 2 |  |\n"
        );
        assert_eq!(
            table(tsv, Path::new("t.tsv"), Some("note, name")).unwrap(),
            "| note | name |\n| --- | --- |\n| x\\|y | a |\n|  | b |\n"
        );
        assert!(table(tsv, Path::new("t.tsv"), Some("missing")).is_err());
        let csv = "a,b\n\"1,5\",\"say \"\"hi\"\"\"\n";
        assert_eq!(
            table(csv, Path::new("t.csv"), None).unwrap(),
            "| a | b |\n| --- | --- |\n| 1,5 | say \"hi\" |\n"
        );
    }

    #[test]
    fn a_surrogate_pair_is_one_character() {
        assert_eq!(unquote("\"\\ud83d\\ude00\"").unwrap(), "\u{1F600}");
    }

    #[test]
    fn the_session_replaces_what_the_widget_held() {
        let dir = std::env::temp_dir().join(format!("dkt-session-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("s.txt"), "$ run\nok\n").unwrap();
        let md = "text\n<!-- site: widget replay src=s.txt -->\n```\nold\n```\n<!-- site: end -->\nafter\n";
        let got = expand(md, Some(&dir)).unwrap();
        assert_eq!(
            got,
            "text\n<!-- site: widget replay src=s.txt -->\n```\n$ run\nok\n```\n<!-- site: end -->\nafter\n"
        );
        assert!(
            expand(
                "<!-- site: widget replay src=nope.txt -->\n<!-- site: end -->\n",
                Some(&dir)
            )
            .is_err()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
