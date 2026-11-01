//! The content that a widget takes from a file instead of from the document.
//!
//! `<!-- site: widget replay src=session.cast -->` takes the session from the file, written
//! relative to the document, and replaces the blocks that the widget holds.
//! The file is plain text, or an asciicast recording of version 2.
//! A recording that takes the whole screen becomes a series of SVG snapshots of the screen.
//! Another recording gives its lines, not its timing: the widget types a line that starts with
//! `$ ` and shows the others one after the other.
//!
//! `<!-- site: widget table src=rules.tsv -->` builds the table from a file of values separated
//! by tabs or by commas, whose first row gives the headings, or from a JSON or a YAML file.
//! `columns="a,b"` keeps the columns of those headings, in that order.

use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

use super::snapshots;
use crate::annotation::{self, Kind};

/// The document with the sessions of its `replay` widgets and the tables of its `table` widgets
/// read from their files.
pub fn expand(markdown: &str, dir: Option<&Path>) -> Result<String> {
    let mut out = String::with_capacity(markdown.len());
    let mut lines = markdown.split_inclusive('\n');
    // The number of snapshot series so far, which names their rows apart.
    let mut series = 0;
    // The fence of the code block that is open, whose lines are shown as they are.
    let mut fence: Option<String> = None;
    while let Some(line) = lines.next() {
        out.push_str(line);
        if let Some(open) = &fence {
            if closes(line, open) {
                fence = None;
            }
            continue;
        }
        if let Some(opened) = opens(line) {
            fence = Some(opened);
            continue;
        }
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
            match session(&raw).with_context(|| format!("cannot read the session {src}"))? {
                Session::Text(text) => {
                    let fence = fence_for(&text);
                    format!("{fence}\n{text}\n{fence}\n")
                }
                Session::Screen(recording) => {
                    series += 1;
                    let prefix = format!("dkt-s{:08x}-{series}", fnv(&src));
                    let html = snapshots::html(&recording, &prefix)
                        .with_context(|| format!("cannot play the session {src}"))?;
                    format!("\n{html}\n")
                }
            }
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

/// The fence that a line opens: its run of 3 or more backticks or tildes.
fn opens(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let c = trimmed.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let run: String = trimmed.chars().take_while(|&d| d == c).collect();
    (run.len() >= 3 && line.len() - trimmed.len() < 4).then_some(run)
}

/// Whether a line closes the code block that `fence` opened: a run of its character, at least
/// as long, and nothing after it.
fn closes(line: &str, fence: &str) -> bool {
    let trimmed = line.trim();
    let c = fence.chars().next().unwrap_or('`');
    trimmed.len() >= fence.len() && trimmed.chars().all(|d| d == c)
}

/// Reads the file that a `table` or a `replay` widget names, as the site would.
pub(crate) fn check(widget: &str, path: &Path, columns: Option<&str>) -> Result<()> {
    let raw = std::fs::read_to_string(path)?;
    if widget == "table" {
        table(&raw, path, columns)?;
    } else if let Session::Screen(recording) = session(&raw)? {
        snapshots::html(&recording, "check")?;
    }
    Ok(())
}

/// The 32 bits FNV-1a hash of a text.
fn fnv(text: &str) -> u32 {
    text.bytes().fold(0x811c_9dc5, |hash, b| {
        (hash ^ u32::from(b)).wrapping_mul(0x0100_0193)
    })
}

/// The widget, the file and the columns that a line names, when the line is a `replay` or a
/// `table` widget with a `src`.
pub(crate) fn file_widget(line: &str) -> Option<(&'static str, String, Option<String>)> {
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

/// The Markdown table of a file, with the columns that `columns` names.
///
/// The extension of the file gives its format: `.json`, `.yaml` or `.yml`, `.csv`, and values
/// separated by tabs for any other.
fn table(raw: &str, path: &Path, columns: Option<&str>) -> Result<String> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let rows: Vec<Vec<String>> = match extension.as_str() {
        "json" => rows_of(serde_json::from_str::<Value>(raw)?)?,
        "yaml" | "yml" => rows_of(yaml(raw)?)?,
        _ => raw
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                if extension == "csv" {
                    split_csv(l)
                } else {
                    l.split('\t').map(str::to_string).collect()
                }
            })
            .collect(),
    };
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

/// A value of a JSON or a YAML file, with the keys of an object in the order of the file.
///
/// A single value is held as the text of a cell: a number as written, a boolean as `true` or
/// `false`, and null as an empty text.
#[derive(Debug, PartialEq)]
enum Value {
    Single(String),
    List(Vec<Value>),
    Object(Vec<(String, Value)>),
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(ValueVisitor)
    }
}

struct ValueVisitor;

impl<'de> Visitor<'de> for ValueVisitor {
    type Value = Value;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a JSON value")
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Single(v.to_string()))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Value, E> {
        Ok(Value::Single(v.to_string()))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Value, E> {
        Ok(Value::Single(v.to_string()))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Value, E> {
        Ok(Value::Single(v.to_string()))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Value, E> {
        Ok(Value::Single(v.to_string()))
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<Value, E> {
        Ok(Value::Single(v))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Single(String::new()))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Value::List(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut entries = Vec::new();
        while let Some(entry) = map.next_entry::<String, Value>()? {
            entries.push(entry);
        }
        Ok(Value::Object(entries))
    }
}

/// The first document of a YAML file, as a value.
fn yaml(raw: &str) -> Result<Value> {
    let docs = yaml_rust2::YamlLoader::load_from_str(raw).map_err(|e| {
        anyhow::anyhow!(
            "{} at line {} column {}",
            e.info(),
            e.marker().line(),
            e.marker().col() + 1
        )
    })?;
    let Some(doc) = docs.into_iter().next() else {
        bail!("the file is empty");
    };
    from_yaml(doc)
}

fn from_yaml(node: yaml_rust2::Yaml) -> Result<Value> {
    use yaml_rust2::Yaml;
    Ok(match node {
        Yaml::Real(text) | Yaml::String(text) => Value::Single(text),
        Yaml::Integer(n) => Value::Single(n.to_string()),
        Yaml::Boolean(b) => Value::Single(b.to_string()),
        Yaml::Null => Value::Single(String::new()),
        Yaml::Array(items) => Value::List(items.into_iter().map(from_yaml).collect::<Result<_>>()?),
        Yaml::Hash(entries) => Value::Object(
            entries
                .into_iter()
                .map(|(key, value)| match from_yaml(key)? {
                    Value::Single(key) => Ok((key, from_yaml(value)?)),
                    _ => bail!("a key is a list or a mapping"),
                })
                .collect::<Result<_>>()?,
        ),
        Yaml::Alias(_) | Yaml::BadValue => {
            bail!("a value names an anchor that the file does not hold")
        }
    })
}

/// The rows of a table, headings first, that a value holds.
///
/// The value is a list of objects, whose keys give the headings in the order they first appear,
/// or a list of lists, whose first one gives the headings.
/// An object that holds 1 list stands for that list.
fn rows_of(value: Value) -> Result<Vec<Vec<String>>> {
    let items = match value {
        Value::List(items) => items,
        Value::Object(entries) => {
            let mut lists: Vec<(String, Vec<Value>)> = entries
                .into_iter()
                .filter_map(|(key, value)| match value {
                    Value::List(items) => Some((key, items)),
                    _ => None,
                })
                .collect();
            match lists.len() {
                0 => bail!("the file holds an object with no list"),
                1 => lists.remove(0).1,
                _ => {
                    let keys: Vec<String> = lists.into_iter().map(|(k, _)| k).collect();
                    bail!(
                        "the file holds an object with several lists, {}, and a table takes 1",
                        keys.join(", ")
                    )
                }
            }
        }
        Value::Single(_) => bail!("the file holds a single value, not a list"),
    };
    let single = |value: Value, place: String| match value {
        Value::Single(text) => Ok(text),
        _ => bail!("{place} holds a list or an object, and a cell holds a single value"),
    };
    match items.first() {
        None => bail!("the file is empty"),
        Some(Value::Object(_)) => {
            let mut head: Vec<String> = Vec::new();
            for item in &items {
                if let Value::Object(entries) = item {
                    for (key, _) in entries {
                        if !head.contains(key) {
                            head.push(key.clone());
                        }
                    }
                }
            }
            let mut rows = vec![head.clone()];
            for (n, item) in items.into_iter().enumerate() {
                let Value::Object(mut entries) = item else {
                    bail!("item {} is not an object like the first", n + 1);
                };
                let mut row = Vec::with_capacity(head.len());
                for key in &head {
                    let text = match entries.iter().position(|(k, _)| k == key) {
                        Some(at) => single(
                            entries.swap_remove(at).1,
                            format!("the cell '{key}' of item {}", n + 1),
                        )?,
                        None => String::new(),
                    };
                    row.push(text);
                }
                rows.push(row);
            }
            Ok(rows)
        }
        Some(Value::List(_)) => items
            .into_iter()
            .enumerate()
            .map(|(n, item)| match item {
                Value::List(cells) => cells
                    .into_iter()
                    .enumerate()
                    .map(|(m, cell)| single(cell, format!("the cell {} of row {}", m + 1, n + 1)))
                    .collect(),
                _ => bail!("row {} is not a list like the first", n + 1),
            })
            .collect(),
        Some(Value::Single(_)) => {
            bail!("the items of the list are single values, not objects or rows")
        }
    }
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

/// What a session file gives: lines of text, or a recording that takes the whole screen.
enum Session {
    Text(String),
    Screen(Recording),
}

/// An asciicast recording of version 2: the size of its screen, its longest pause and its
/// events.
pub(crate) struct Recording {
    pub cols: usize,
    pub rows: usize,
    /// The longest pause between 2 events, in seconds, that the recording sets.
    pub idle: Option<f64>,
    pub events: Vec<Event>,
}

/// An event of a recording, at its time in seconds from the start.
pub(crate) enum Event {
    /// Text that the program writes.
    Output(f64, String),
    /// A new size of the screen, in columns and rows.
    Resize(f64, usize, usize),
}

impl Event {
    pub fn time(&self) -> f64 {
        match self {
            Event::Output(t, _) | Event::Resize(t, _, _) => *t,
        }
    }
}

/// The session of a file: the file itself, a recording that takes the screen, or the output of
/// another recording without its escapes.
fn session(raw: &str) -> Result<Session> {
    if !raw.trim_start().starts_with('{') {
        return Ok(Session::Text(raw.trim_end().to_string()));
    }
    let recording = recording(raw)?;
    if snapshots::takes_the_screen(&recording) {
        return Ok(Session::Screen(recording));
    }
    let mut text = String::new();
    for event in &recording.events {
        if let Event::Output(_, data) = event {
            text.push_str(data);
        }
    }
    let text = strip_escapes(&text).replace("\r\n", "\n").replace('\r', "");
    Ok(Session::Text(text.trim_end().to_string()))
}

/// Reads an asciicast recording: a header object on the first line, then 1 event per line,
/// `[time, code, data]`.
///
/// The code `o` is output and `r` is a new size written `COLSxROWS`.
/// The other events are left out.
fn recording(raw: &str) -> Result<Recording> {
    let mut lines = raw
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty());
    let (_, first) = lines.next().context("the file is empty")?;
    let header: serde_json::Value =
        serde_json::from_str(first).context("line 1: the header is not a JSON object")?;
    if header.get("version").is_none() {
        bail!("the file starts like a recording and has no version");
    }
    let size = |key: &str| {
        header
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .map(|n| n as usize)
            .with_context(|| format!("line 1: the header gives no {key}"))
    };
    let mut recording = Recording {
        cols: size("width")?,
        rows: size("height")?,
        idle: header
            .get("idle_time_limit")
            .and_then(serde_json::Value::as_f64),
        events: Vec::new(),
    };
    for (n, line) in lines {
        let (time, code, data): (f64, String, String) = serde_json::from_str(line)
            .with_context(|| format!("line {}: an event is not [time, code, data]", n + 1))?;
        match code.as_str() {
            "o" => recording.events.push(Event::Output(time, data)),
            "r" => {
                let (cols, rows) = data
                    .split_once('x')
                    .and_then(|(c, r)| Some((c.trim().parse().ok()?, r.trim().parse().ok()?)))
                    .with_context(|| {
                        format!("line {}: a new size is not written COLSxROWS", n + 1)
                    })?;
                recording.events.push(Event::Resize(time, cols, rows));
            }
            _ => {}
        }
    }
    Ok(recording)
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

    /// The text of a session that is not a full screen one.
    fn text(raw: &str) -> String {
        match session(raw).unwrap() {
            Session::Text(text) => text,
            Session::Screen(_) => panic!("a full screen session"),
        }
    }

    #[test]
    fn a_plain_file_is_the_session() {
        assert_eq!(text("$ ls\na b\n\n"), "$ ls\na b");
    }

    #[test]
    fn a_recording_gives_its_output_without_escapes() {
        let cast = "{\"version\": 2, \"width\": 80, \"height\": 24}\n\
                    [0.1, \"o\", \"$ ls\\r\\n\"]\n\
                    [0.4, \"i\", \"x\"]\n\
                    [0.9, \"o\", \"\\u001b[32ma\\u001b[0m b\\r\\n\"]\n";
        assert_eq!(text(cast), "$ ls\na b");
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
        let cast =
            "{\"version\": 2, \"width\": 8, \"height\": 2}\n[0, \"o\", \"\\ud83d\\ude00\"]\n";
        assert_eq!(text(cast), "\u{1F600}");
    }

    #[test]
    fn a_bad_recording_names_its_line() {
        let cast = "{\"version\": 2, \"width\": 8, \"height\": 2}\n[0, \"o\", \"a\"]\n[1, \"o\"\n";
        let err = format!("{:#}", session(cast).err().unwrap());
        assert!(err.starts_with("line 3: an event is not"), "{err}");
        let err = format!("{:#}", session("{\"version\": 2}\n").err().unwrap());
        assert!(err.contains("no width"), "{err}");
    }

    #[test]
    fn a_full_screen_recording_is_a_series_of_snapshots() {
        let cast = "{\"version\": 2, \"width\": 20, \"height\": 4}\n\
                    [0.1, \"o\", \"\\u001b[?1049h\\u001b[1;1H\\u001b[7m menu \\u001b[0m\"]\n\
                    [0.5, \"r\", \"30x5\"]\n\
                    [0.9, \"o\", \"\\u001b[3;2Hitem\"]\n";
        assert!(matches!(session(cast).unwrap(), Session::Screen(_)));
        let dir = std::env::temp_dir().join(format!("dkt-screen-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("s.cast"), cast).unwrap();
        let md = "<!-- site: widget replay src=s.cast -->\n<!-- site: end -->\n";
        let got = expand(md, Some(&dir)).unwrap();
        assert!(got.contains("<div class=\"dkt-snaps\">"), "{got}");
        assert_eq!(got.matches("<svg class=\"dkt-snap\"").count(), 3, "{got}");
        assert!(got.trim_end().ends_with("<!-- site: end -->"), "{got}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_json_or_a_yaml_file_is_a_table() {
        let json =
            r#"[{"name": "a", "size": 1}, {"name": "b|c", "on": true, "size": 2.5, "note": null}]"#;
        let want = "| name | size | on | note |\n| --- | --- | --- | --- |\n| a | 1 |  |  |\n| b\\|c | 2.5 | true |  |\n";
        assert_eq!(table(json, Path::new("t.json"), None).unwrap(), want);
        let yaml = "- name: a\n  size: 1\n- name: b|c\n  on: true\n  size: 2.5\n  note: ~\n";
        assert_eq!(table(yaml, Path::new("t.yaml"), None).unwrap(), want);
        let rows = r#"{"title": "x", "rows": [["k", "v"], [1, false]]}"#;
        assert_eq!(
            table(rows, Path::new("t.JSON"), Some("v")).unwrap(),
            "| v |\n| --- |\n| false |\n"
        );
        let yml = "rows:\n  - [k, v]\n  - [1, 2]\n";
        assert_eq!(
            table(yml, Path::new("t.yml"), None).unwrap(),
            "| k | v |\n| --- | --- |\n| 1 | 2 |\n"
        );
    }

    #[test]
    fn a_json_or_a_yaml_file_that_does_not_hold_a_table_is_refused() {
        let err = |raw: &str, name: &str| {
            format!("{:#}", table(raw, Path::new(name), None).err().unwrap())
        };
        assert!(err("[{\"a\": 1,}]", "t.json").contains("line 1 column"));
        assert!(err("a: [1\n", "t.yaml").contains("line"));
        assert_eq!(
            err(r#"[{"a": 1}, {"a": [1]}]"#, "t.json"),
            "the cell 'a' of item 2 holds a list or an object, and a cell holds a single value"
        );
        assert_eq!(
            err("- [k]\n- [{x: 1}]\n", "t.yaml"),
            "the cell 1 of row 2 holds a list or an object, and a cell holds a single value"
        );
        assert!(err(r#"{"a": [], "b": []}"#, "t.json").contains("several lists, a, b"));
        assert!(err("[1, 2]", "t.json").contains("single values"));
        assert!(err("[]", "t.json").contains("empty"));
        assert!(err("[{\"a\": 1}, [1]]", "t.json").contains("item 2 is not an object"));
    }

    #[test]
    fn the_session_replaces_what_the_widget_held() {
        let dir = std::env::temp_dir().join(format!("dkt-session-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("s.txt"), "$ run\nok\n").unwrap();
        let md = "text\n<!-- site: widget replay src=s.txt -->\n```\nold\n```\n<!-- site: end -->\nafter\n";
        // A widget written in a code block is shown, not read.
        let shown = "~~~\n<!-- site: widget replay src=nope.txt -->\n<!-- site: end -->\n~~~\n";
        assert_eq!(expand(shown, Some(&dir)).unwrap(), shown);
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
