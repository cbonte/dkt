//! Tables drawn as a grid, with borders made of plus and dash signs and cells between pipes.
//!
//! Cells that span several columns, as in a header that groups columns, are read against the
//! finest border: each column of the table is named after the cells that cover it.

use super::text::Line;

/// Whether a line is a full border, such as `+----+-----+`.
pub fn is_border(text: &str) -> bool {
    text.len() >= 3
        && text.starts_with('+')
        && text.bytes().all(|b| b"+-=".contains(&b))
        && text.contains(['-', '='])
}

/// Whether a line is a border that starts inside the grid, such as `|     +----+----+`.
fn is_partial_border(text: &str) -> bool {
    text.contains("+-")
        && text.bytes().all(|b| b"+-=| ".contains(&b))
        && text.starts_with(['|', '+'])
}

fn is_row(text: &str) -> bool {
    text.starts_with('|') && !is_partial_border(text)
}

/// The columns, counted from the start of the line, of the signs in `line` that are in `signs`.
fn columns(line: &Line<'_>, signs: &[char]) -> Vec<usize> {
    line.text
        .chars()
        .enumerate()
        .filter(|(_, c)| signs.contains(c))
        .map(|(p, _)| line.indent + p)
        .collect()
}

/// The text of a row between two columns, trimmed.
fn between(line: &Line<'_>, from: usize, to: usize) -> String {
    let full: Vec<char> = format!("{}{}", " ".repeat(line.indent), line.text)
        .chars()
        .collect();
    let from = (from + 1).min(full.len());
    let to = to.min(full.len()).max(from);
    full[from..to].iter().collect::<String>().trim().to_string()
}

/// The longest text that a column label has.
const MAX_LABEL: usize = 60;

/// A grid table that starts at the first line, its header and its rows, and the number of lines it
/// takes.
pub fn table(run: &[Line<'_>]) -> Option<(Vec<String>, Vec<Vec<String>>, usize)> {
    if !is_border(run.first()?.text) {
        return None;
    }
    let end = run
        .iter()
        .position(|l| !(is_border(l.text) || is_partial_border(l.text) || is_row(l.text)))
        .unwrap_or(run.len());
    let lines = &run[..end];
    // The finest border gives the columns of the table.
    let finest = lines
        .iter()
        .filter(|l| !is_row(l.text))
        .max_by_key(|l| columns(l, &['+']).len())?;
    let cuts = columns(finest, &['+']);
    if cuts.len() < 3 {
        return None;
    }
    let width = cuts.len() - 1;
    let full = cuts.len();
    // Rows are the lines between borders, and a header ends at the first full border that follows
    // a row, or at a border drawn with equal signs.
    let mut groups: Vec<Vec<&Line<'_>>> = Vec::new();
    let mut header_groups = 0;
    let mut current: Vec<&Line<'_>> = Vec::new();
    let mut header_done = false;
    for l in lines {
        if is_row(l.text) {
            current.push(l);
            continue;
        }
        if !current.is_empty() {
            groups.push(std::mem::take(&mut current));
        }
        let strong = l.text.contains('=') && is_border(l.text);
        let complete = columns(l, &['+']).len() == full && is_border(l.text);
        if !header_done && !groups.is_empty() && (strong || complete) {
            header_groups = groups.len();
            header_done = true;
        }
    }
    if !current.is_empty() {
        groups.push(current);
    }
    if groups.is_empty() {
        return None;
    }
    // Without an inner full border, the table has no header.
    let segment_text = |group: &[&Line<'_>], c: usize, spread: bool| -> String {
        let mid = (cuts[c] + cuts[c + 1]) / 2;
        let mut parts: Vec<String> = Vec::new();
        for l in group {
            let pipes = columns(l, &['|', '+']);
            let Some(k) = pipes.windows(2).position(|w| w[0] < mid && mid < w[1]) else {
                continue;
            };
            // A cell that spans columns is given to the first of them, or to all when asked.
            let first_col = (0..width).find(|&d| {
                let m = (cuts[d] + cuts[d + 1]) / 2;
                pipes[k] < m && m < pipes[k + 1]
            });
            if spread || first_col == Some(c) {
                let t = between(l, pipes[k], pipes[k + 1]);
                if !t.is_empty() {
                    parts.push(t);
                }
            }
        }
        parts.join(" ")
    };
    // A first row whose cells are sentences is data: the grid has no header.
    if header_groups == 1
        && (0..width).any(|c| segment_text(&groups[0], c, true).chars().count() > MAX_LABEL)
    {
        header_groups = 0;
    }
    let mut header = vec![String::new(); width];
    for (c, label) in header.iter_mut().enumerate() {
        let mut names: Vec<String> = Vec::new();
        for group in &groups[..header_groups] {
            let t = segment_text(group, c, true);
            if !t.is_empty() && names.last() != Some(&t) {
                names.push(t);
            }
        }
        *label = names.join(" ");
    }
    // Between borders, a line goes on with the row above when it fills one cell, which the row above
    // fills too. Each line is a row otherwise.
    let filled = |l: &Line<'_>| -> Vec<bool> {
        (0..width)
            .map(|c| !segment_text(&[l], c, false).is_empty())
            .collect()
    };
    let mut data: Vec<Vec<&Line<'_>>> = Vec::new();
    for group in &groups[header_groups..] {
        let mut previous: Option<Vec<bool>> = None;
        for l in group {
            let now = filled(l);
            let goes_on = previous.as_ref().is_some_and(|p| {
                let (a, b) = (
                    now.iter().filter(|x| **x).count(),
                    p.iter().filter(|x| **x).count(),
                );
                a == 1 && a < b && now.iter().zip(p).all(|(n, o)| !*n || *o)
            });
            if goes_on && let Some(row) = data.last_mut() {
                row.push(l);
            } else {
                data.push(vec![l]);
                previous = Some(now);
            }
        }
    }
    // A line of empty cells that closes a row is not a row.
    let rows: Vec<Vec<String>> = data
        .iter()
        .map(|group| {
            (0..width)
                .map(|c| segment_text(group, c, false))
                .collect::<Vec<_>>()
        })
        .filter(|row| row.iter().any(|cell| !cell.is_empty()))
        .collect();
    if rows.is_empty() {
        return None;
    }
    Some((header, rows, end))
}

#[cfg(test)]
mod tests {
    use super::super::text::{lines, normalise};
    use super::*;

    fn read(text: &str) -> Option<(Vec<String>, Vec<Vec<String>>, usize)> {
        let t = normalise(text);
        let l = lines(&t);
        table(&l)
    }

    #[test]
    fn a_grid_has_a_header_and_rows() {
        let (h, r, used) = read("+----+-------+\n| id | name  |\n+----+-------+\n|  1 | one   |\n|  2 | two   |\n+----+-------+\n\nafter\n").unwrap();
        assert_eq!(h, ["id", "name"]);
        assert_eq!(r, [vec!["1", "one"], vec!["2", "two"]]);
        assert_eq!(used, 6);
    }

    #[test]
    fn a_grid_whose_first_row_is_sentences_has_no_header_and_no_empty_rows() {
        let long = "x".repeat(40);
        let text = format!(
            "+------+------------------------------------------------------------------+\n| A    | {long} {long} |\n|      |                                                                  |\n+------+------------------------------------------------------------------+\n| B    | two                                                              |\n|      |                                                                  |\n+------+------------------------------------------------------------------+\n"
        );
        let (h, r, _) = read(&text).unwrap();
        assert!(h.iter().all(String::is_empty), "{h:?}");
        assert_eq!(r.len(), 2, "{r:?}");
        assert_eq!(r[1], ["B", "two"]);
    }

    #[test]
    fn a_header_that_groups_columns_names_each_column_after_its_group() {
        let text = "+---------+-----------+\n|  name   |  usable   |\n|         +-----+-----+\n|         |  M  |  W  |\n+---------+-----+-----+\n| A       |  X  |     |\n| B       |     |  X  |\n+---------+-----+-----+\n";
        let (h, r, _) = read(text).unwrap();
        assert_eq!(h, ["name", "usable M", "usable W"]);
        assert_eq!(r, [vec!["A", "X", ""], vec!["B", "", "X"]]);
    }

    #[test]
    fn a_cell_that_runs_over_several_lines_is_joined() {
        let (_, r, _) = read("+---+------+\n| k | v    |\n+---+------+\n| a | one  |\n|   | more |\n+---+------+\n| b | two  |\n+---+------+\n").unwrap();
        assert_eq!(r, [vec!["a", "one more"], vec!["b", "two"]]);
    }

    #[test]
    fn a_figure_is_not_a_table() {
        assert!(read("+---+\n| a |\n+---+\n").is_none());
        assert!(read("text\n+---+---+\n").is_none());
    }
}
