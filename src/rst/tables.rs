//! The tables of reStructuredText: the grid tables, the simple tables, and the 2 directives that
//! hold a table, `list-table` and `csv-table`.

use super::blocks::{Block, Line, State, blocks, dedent};

type Cell = Vec<Block>;

/// The lines of a cell, cut out of the lines of a row, as blocks.
fn cell(slices: &[String], st: &mut State) -> Cell {
    let lines: Vec<Line> = slices
        .iter()
        .map(|s| {
            let text = s.trim_end().trim_start_matches(' ');
            Line {
                indent: s.trim_end().len() - text.len(),
                text: text.to_string(),
            }
        })
        .collect();
    if lines.iter().all(|l| l.text.is_empty()) {
        return Vec::new();
    }
    blocks(&dedent(&lines), st)
}

/// Several header rows as one, each column holding the cells of the rows one after the other.
fn merge(rows: Vec<Vec<Cell>>) -> Vec<Cell> {
    let mut out: Vec<Cell> = Vec::new();
    for row in rows {
        for (k, cell) in row.into_iter().enumerate() {
            if out.len() <= k {
                out.push(Vec::new());
            }
            out[k].extend(cell);
        }
    }
    out
}

fn is_grid_border(text: &str) -> bool {
    text.len() >= 3
        && text.starts_with('+')
        && text.ends_with('+')
        && text.chars().all(|c| "+-=:".contains(c))
}

/// A grid table that starts at `from`, and the line after it.
pub(super) fn grid(ls: &[Line], from: usize, st: &mut State) -> Option<(Block, usize)> {
    let indent = ls[from].indent;
    if !is_grid_border(&ls[from].text) {
        return None;
    }
    let mut end = from;
    while end < ls.len()
        && !ls[end].text.is_empty()
        && ls[end].indent == indent
        && (ls[end].text.starts_with('+') || ls[end].text.starts_with('|'))
    {
        end += 1;
    }
    if end - from < 3 || !is_grid_border(&ls[end - 1].text) {
        return None;
    }
    let first: Vec<char> = ls[from].text.chars().collect();
    let bounds: Vec<usize> = first
        .iter()
        .enumerate()
        .filter(|(_, c)| **c == '+')
        .map(|(k, _)| k)
        .collect();
    if bounds.len() < 2 {
        return None;
    }
    // The rows, as the lines between 2 borders, and how many of them make the header.
    let mut rows: Vec<Vec<Vec<char>>> = Vec::new();
    let mut header_rows = 0;
    let mut current: Vec<Vec<char>> = Vec::new();
    for l in &ls[from + 1..end] {
        if is_grid_border(&l.text) {
            if !current.is_empty() {
                rows.push(std::mem::take(&mut current));
                if l.text.contains('=') {
                    header_rows = rows.len();
                }
            }
        } else {
            current.push(l.text.chars().collect());
        }
    }
    if !current.is_empty() {
        rows.push(current);
    }
    let mut table: Vec<Vec<Cell>> = Vec::new();
    for row in &rows {
        // A boundary that some line of the row does not draw is inside a cell that spans columns.
        let drawn = |b: usize| {
            row.iter()
                .all(|line| matches!(line.get(b), Some('|' | '+')))
        };
        let mut cells = Vec::new();
        let mut k = 0;
        while k + 1 < bounds.len() {
            let mut stop = k + 1;
            while stop + 1 < bounds.len() && !drawn(bounds[stop]) {
                stop += 1;
            }
            let slices: Vec<String> = row
                .iter()
                .map(|line| {
                    line.iter()
                        .skip(bounds[k] + 1)
                        .take(bounds[stop] - bounds[k] - 1)
                        .collect()
                })
                .collect();
            cells.push(cell(&slices, st));
            for _ in k + 1..stop {
                cells.push(Vec::new());
            }
            k = stop;
        }
        table.push(cells);
    }
    let header_rows = header_rows.max(1).min(table.len());
    let body = table.split_off(header_rows);
    let header = merge(table);
    Some((Block::Table { header, rows: body }, end))
}

fn simple_columns(text: &str) -> Option<Vec<(usize, usize)>> {
    if !text.starts_with('=') || !text.contains(' ') || !text.chars().all(|c| c == '=' || c == ' ')
    {
        return None;
    }
    let chars: Vec<char> = text.chars().collect();
    let mut cols = Vec::new();
    let mut k = 0;
    while k < chars.len() {
        if chars[k] == '=' {
            let start = k;
            while k < chars.len() && chars[k] == '=' {
                k += 1;
            }
            cols.push((start, k));
        } else {
            k += 1;
        }
    }
    (cols.len() >= 2).then_some(cols)
}

/// A simple table that starts at `from`, and the line after it.
pub(super) fn simple(ls: &[Line], from: usize, st: &mut State) -> Option<(Block, usize)> {
    let indent = ls[from].indent;
    let cols = simple_columns(&ls[from].text)?;
    // The table ends at the border that is followed by a blank line or by the end of the text.
    let mut borders = vec![from];
    let mut end = None;
    let mut i = from + 1;
    while i < ls.len() {
        if ls[i].indent == indent && simple_columns(&ls[i].text).is_some() {
            borders.push(i);
            if ls.get(i + 1).is_none_or(|l| l.text.is_empty()) {
                end = Some(i + 1);
                break;
            }
        } else if !ls[i].text.is_empty() && ls[i].indent < indent {
            return None;
        }
        i += 1;
    }
    let end = end?;
    if borders.len() < 2 {
        return None;
    }
    let header_end = (borders.len() > 2).then(|| borders[1]);
    // A row is a line whose first column is not blank, with the lines that go on under it.
    let width = cols.len();
    let mut rows: Vec<Vec<Vec<String>>> = Vec::new();
    let mut header_rows = 0;
    for (n, l) in ls.iter().enumerate().take(end - 1).skip(from + 1) {
        if Some(n) == header_end {
            header_rows = rows.len();
            continue;
        }
        if l.text.is_empty() || l.text.chars().all(|c| c == '-' || c == ' ') {
            continue;
        }
        let line: Vec<char> = format!("{}{}", " ".repeat(l.indent - indent), l.text)
            .chars()
            .collect();
        let slice = |k: usize| -> String {
            let from = cols[k].0.min(line.len());
            let to = if k + 1 < width {
                cols[k + 1].0.min(line.len())
            } else {
                line.len()
            };
            line[from..to].iter().collect::<String>()
        };
        let parts: Vec<String> = (0..width).map(slice).collect();
        if parts[0].trim().is_empty() && !rows.is_empty() {
            let last = rows.last_mut().expect("a row");
            for (k, part) in parts.into_iter().enumerate() {
                last[k].push(part);
            }
        } else {
            rows.push(parts.into_iter().map(|p| vec![p]).collect());
        }
    }
    let mut table: Vec<Vec<Cell>> = rows
        .iter()
        .map(|row| row.iter().map(|lines| cell(lines, st)).collect())
        .collect();
    if table.is_empty() {
        return None;
    }
    let header_rows = header_rows.max(1).min(table.len());
    let body = table.split_off(header_rows);
    let header = merge(table);
    Some((Block::Table { header, rows: body }, end))
}

/// A table from the nested lists of a `list-table`: a list of rows, each a list of cells.
pub fn from_lists(items: Vec<Vec<Block>>, header_rows: usize) -> Option<Block> {
    let mut rows: Vec<Vec<Cell>> = Vec::new();
    for item in items {
        let mut cells = None;
        for block in item {
            if let Block::List { items, .. } = block {
                cells = Some(items);
                break;
            }
        }
        rows.push(cells?);
    }
    if rows.is_empty() {
        return None;
    }
    let header_rows = header_rows.max(1).min(rows.len());
    let body = rows.split_off(header_rows);
    let header = merge(rows);
    Some(Block::Table { header, rows: body })
}

/// The rows of comma separated values, with quotes that hold the delimiter and doubled quotes.
pub fn csv(lines: &[String], delimiter: char, quote: char) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut any = false;
    for line in lines {
        let chars: Vec<char> = line.chars().collect();
        let mut k = 0;
        while k < chars.len() {
            let c = chars[k];
            if quoted {
                if c == quote && chars.get(k + 1) == Some(&quote) {
                    field.push(quote);
                    k += 1;
                } else if c == quote {
                    quoted = false;
                } else {
                    field.push(c);
                }
            } else if c == quote {
                quoted = true;
            } else if c == delimiter {
                row.push(field.trim().to_string());
                field.clear();
            } else {
                field.push(c);
            }
            any = true;
            k += 1;
        }
        if quoted {
            field.push(' ');
        } else {
            row.push(field.trim().to_string());
            field.clear();
            if any {
                rows.push(std::mem::take(&mut row));
            }
            any = false;
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field.trim().to_string());
        rows.push(row);
    }
    rows
}

/// A table from rows of text, the first `header_rows` of them making the header.
pub fn from_text(rows: Vec<Vec<String>>, header_rows: usize) -> Option<Block> {
    let mut table: Vec<Vec<Cell>> = rows
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|text| {
                    if text.is_empty() {
                        Vec::new()
                    } else {
                        vec![Block::Para(text)]
                    }
                })
                .collect()
        })
        .collect();
    if table.is_empty() {
        return None;
    }
    let header_rows = header_rows.max(1).min(table.len());
    let body = table.split_off(header_rows);
    let header = merge(table);
    Some(Block::Table { header, rows: body })
}
