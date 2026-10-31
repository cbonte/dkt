//! Drawings made of characters, as SVG.
//!
//! A fenced block marked `diagram` stays readable as text on GitHub and in the terminal, and the
//! pages of a site show it as a drawing. Each character of the block is a cell of a grid. The
//! characters that draw lines, arrows and corners become strokes, and the others stay text.

use std::collections::HashSet;

const CELL_W: f64 = 7.8;
const CELL_H: f64 = 16.0;
const FONT: f64 = 13.0;

/// What a cell of the grid is drawn as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cell {
    Blank,
    Text(char),
    Horizontal,
    Double,
    Under,
    Vertical,
    Dotted,
    Cross,
    Corner,
    Rise,
    Fall,
    Left,
    Right,
    Up,
    Down,
}

struct Grid {
    rows: Vec<Vec<char>>,
    width: usize,
}

impl Grid {
    fn new(text: &str) -> Grid {
        let rows: Vec<Vec<char>> = text.lines().map(|l| l.chars().collect()).collect();
        let width = rows.iter().map(Vec::len).max().unwrap_or(0);
        Grid { rows, width }
    }

    fn at(&self, r: isize, c: isize) -> char {
        if r < 0 || c < 0 {
            return ' ';
        }
        self.rows
            .get(r as usize)
            .and_then(|row| row.get(c as usize))
            .copied()
            .unwrap_or(' ')
    }
}

fn word(c: char) -> bool {
    c.is_alphanumeric()
}

/// What the character at a cell is drawn as, from the characters around it.
fn classify(g: &Grid, r: isize, c: isize) -> Cell {
    let ch = g.at(r, c);
    let (a, b, u, d) = (
        g.at(r, c - 1),
        g.at(r, c + 1),
        g.at(r - 1, c),
        g.at(r + 1, c),
    );
    let blank = |x: char| x == ' ';
    let horizontal = |x: char| matches!(x, '-' | '_' | '=' | '+' | '<' | '>');
    // What a line that turns hangs from: a bar, or the end of a span drawn right above it.
    let hangs = |x: char| matches!(x, '|' | '-' | '+' | '<' | '>');
    // A character of a command, of a path or of a pattern stays text: a stroke has to join other
    // strokes, or stand alone between spaces.
    match ch {
        ' ' => Cell::Blank,
        '-' if horizontal(a)
            || horizontal(b)
            || a == '|'
            || b == '|'
            || (a == '`' && hangs(g.at(r - 1, c - 1))) =>
        {
            Cell::Horizontal
        }
        '=' if matches!(a, '=' | '<' | '|' | '+') || matches!(b, '=' | '>' | '|' | '+') => {
            Cell::Double
        }
        '_' if !(word(a) && word(b))
            && (matches!(a, '_' | '/' | '\\' | '|') || matches!(b, '_' | '/' | '\\' | '|')) =>
        {
            Cell::Under
        }
        '|' if matches!(a, '-' | '+' | '|' | '<' | '>')
            || matches!(b, '-' | '+' | '|' | '<' | '>')
            || (blank(a) && blank(b)) =>
        {
            Cell::Vertical
        }
        '+' if matches!(a, '-' | '+' | '=')
            || matches!(b, '-' | '+' | '=')
            || (blank(a) && blank(b) && (matches!(u, '|' | '+') || matches!(d, '|' | '+'))) =>
        {
            Cell::Cross
        }
        '/' if a == '_'
            || matches!(g.at(r - 1, c + 1), '/' | '_')
            || g.at(r + 1, c - 1) == '/'
            || (blank(a) && blank(b)) =>
        {
            Cell::Rise
        }
        '\\' if b == '_'
            || matches!(g.at(r - 1, c - 1), '\\' | '_')
            || g.at(r + 1, c + 1) == '\\'
            || (blank(a) && blank(b)) =>
        {
            Cell::Fall
        }
        ':' if !word(a)
            && (matches!(b, '<' | '-' | '>' | ' ')
                || matches!(u, ':' | '|')
                || matches!(d, ':' | '|'))
            && !word(b) =>
        {
            Cell::Dotted
        }
        // The foot of a vertical line that turns right to a label.
        '`' if hangs(u) && b == '-' => Cell::Corner,
        '<' if matches!(b, '-' | '=') => Cell::Left,
        '>' if matches!(a, '-' | '=') => Cell::Right,
        '^' if matches!(d, '|' | '/' | '\\') && !word(a) && !word(b) => Cell::Up,
        'v' if u == '|' && !word(a) && !word(b) => Cell::Down,
        c => Cell::Text(c),
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn number(x: f64) -> String {
    let s = format!("{x:.1}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A line that a label points at: the span between two bars, or a run of dashes.
struct Span {
    x1: f64,
    x2: f64,
    y: f64,
}

/// The spans of a grid: the dashes between two bars, from the middle of one bar to the middle of
/// the next, and the runs of dashes that no bar closes.
fn spans(g: &Grid) -> Vec<Span> {
    let mut out = Vec::new();
    for r in 0..g.rows.len() {
        let cells: Vec<Cell> = (0..g.width)
            .map(|c| classify(g, r as isize, c as isize))
            .collect();
        let y = r as f64 * CELL_H + CELL_H / 2.0;
        let mut c = 0;
        while c < cells.len() {
            if cells[c] == Cell::Vertical {
                let mut e = c + 1;
                while e < cells.len() && cells[e] == Cell::Horizontal {
                    e += 1;
                }
                if e > c + 1 && e < cells.len() && cells[e] == Cell::Vertical {
                    out.push(Span {
                        x1: (c as f64 + 0.5) * CELL_W,
                        x2: (e as f64 + 0.5) * CELL_W,
                        y,
                    });
                    c = e;
                    continue;
                }
            }
            c += 1;
        }
        let mut c = 0;
        while c < cells.len() {
            if cells[c] == Cell::Horizontal {
                let start = c;
                while c < cells.len() && cells[c] == Cell::Horizontal {
                    c += 1;
                }
                let closed = (start > 0 && cells[start - 1] == Cell::Vertical)
                    || (c < cells.len() && cells[c] == Cell::Vertical);
                if !closed && c - start >= 2 {
                    out.push(Span {
                        x1: start as f64 * CELL_W,
                        x2: c as f64 * CELL_W,
                        y,
                    });
                }
            } else {
                c += 1;
            }
        }
    }
    out
}

/// The span above a point that the point is under, or the closest to it when 2 are within reach of
/// a cell: a connector that starts on a bar belongs to the span whose range holds it.
fn nearest_span(spans: &[Span], x: f64, below: f64) -> Option<&Span> {
    let gap = |s: &Span| (s.x1 - x).max(x - s.x2).max(0.0);
    spans
        .iter()
        .filter(|s| s.y < below && gap(s) <= CELL_W)
        .min_by(|a, b| gap(a).total_cmp(&gap(b)).then(b.y.total_cmp(&a.y)))
}

/// The path of segments, where each segment that starts at the end of another goes on in the same
/// stroke, so that `_/` is one line that turns. A stroke that ends in diagonals rises to the middle
/// of the span above its end, as a label points at the parameter it names.
fn chain(
    segments: &[[f64; 4]],
    rises: &HashSet<usize>,
    falls: &HashSet<usize>,
    spans: &[Span],
) -> String {
    let key = |x: f64, y: f64| ((x * 10.0).round() as i64, (y * 10.0).round() as i64);
    let mut used = vec![false; segments.len()];
    let mut out = String::new();
    // A stroke starts at a segment that no other segment ends at, so that it is read from its
    // beginning whatever the order of the cells.
    let follows = |j: usize| {
        (0..segments.len()).any(|k| {
            k != j && key(segments[k][2], segments[k][3]) == key(segments[j][0], segments[j][1])
        })
    };
    let order: Vec<usize> = (0..segments.len())
        .filter(|&j| !follows(j))
        .chain(0..segments.len())
        .collect();
    for i in order {
        if used[i] {
            continue;
        }
        used[i] = true;
        let mut chain = vec![i];
        loop {
            let [_, _, x2, y2] = segments[*chain.last().unwrap_or(&i)];
            match (0..segments.len())
                .find(|&j| !used[j] && key(segments[j][0], segments[j][1]) == key(x2, y2))
            {
                Some(next) => {
                    used[next] = true;
                    chain.push(next);
                }
                None => break,
            }
        }
        let mut points = vec![(segments[i][0], segments[i][1])];
        let first_rise = chain.iter().position(|j| rises.contains(j));
        let end = segments[*chain.last().unwrap_or(&i)];
        let target = first_rise
            .filter(|&k| chain[k..].iter().all(|j| rises.contains(j)))
            .and_then(|k| {
                nearest_span(spans, end[2], end[3]).map(|s| (k, ((s.x1 + s.x2) / 2.0, s.y)))
            });
        // The mirror of a rising line: it starts with backslashes that go down to underscores, and
        // its tip, where it starts, goes to the middle of the span above.
        let lead = chain.iter().take_while(|j| falls.contains(j)).count();
        let mirrored = (lead > 0
            && lead < chain.len()
            && chain[lead..]
                .iter()
                .all(|j| !falls.contains(j) && !rises.contains(j)))
        .then(|| {
            let start = (segments[i][0], segments[i][1]);
            let span = nearest_span(spans, start.0, start.1)?;
            let tip = ((span.x1 + span.x2) / 2.0, span.y);
            let corner_y = segments[chain[lead - 1]][3];
            let corner_x = tip.0 + (corner_y - tip.1) * CELL_W / CELL_H;
            let rest: Vec<(f64, f64)> = chain[lead..]
                .iter()
                .map(|&j| (segments[j][2], segments[j][3]))
                .collect();
            let room = rest.last().is_some_and(|p| corner_x <= p.0);
            room.then(|| {
                let mut points = vec![tip, (corner_x, corner_y)];
                points.extend(rest.into_iter().filter(|p| p.0 > corner_x));
                points
            })
        })
        .flatten();
        if let Some(points) = mirrored {
            for (n, (x, y)) in points.iter().enumerate() {
                out.push_str(&format!(
                    "{}{} {}",
                    if n == 0 { "M" } else { "L" },
                    number(*x),
                    number(*y)
                ));
            }
            continue;
        }
        match target {
            Some((k, tip)) => {
                points.extend(chain[..k].iter().map(|&j| (segments[j][2], segments[j][3])));
                // Every rising line has the slope of a slash, so they are all parallel: the corner
                // where the line leaves the underscores is placed to make it land on the tip.
                if k > 0 {
                    let corner_y = segments[chain[k]][1];
                    let corner_x = tip.0 - (corner_y - tip.1) * CELL_W / CELL_H;
                    if corner_x >= points[0].0 {
                        // The underscores stop at the corner, or go on to it.
                        points.retain(|p| p.0 < corner_x);
                        points.push((corner_x, corner_y));
                    }
                }
                points.push(tip);
            }
            None => points.extend(chain.iter().map(|&j| (segments[j][2], segments[j][3]))),
        }
        for (n, (x, y)) in points.iter().enumerate() {
            out.push_str(&format!(
                "{}{} {}",
                if n == 0 { "M" } else { "L" },
                number(*x),
                number(*y)
            ));
        }
    }
    out
}

/// The text as a figure that holds an SVG drawing and the text for those who cannot see it.
pub fn to_svg(text: &str) -> String {
    let g = Grid::new(text.trim_end_matches('\n'));
    let (w, h) = (g.width as f64 * CELL_W, g.rows.len() as f64 * CELL_H);
    let mut segments: Vec<[f64; 4]> = Vec::new();
    let mut rises: HashSet<usize> = HashSet::new();
    let mut falls: HashSet<usize> = HashSet::new();
    let tips = spans(&g);
    let mut dotted = String::new();
    let mut heads = String::new();
    let mut words = String::new();
    let line = |d: &mut String, x1: f64, y1: f64, x2: f64, y2: f64| {
        d.push_str(&format!(
            "M{} {}L{} {}",
            number(x1),
            number(y1),
            number(x2),
            number(y2)
        ));
    };
    // A line that ends at a bar or a corner goes on to the middle of it. A slash or a backslash
    // is a line of its own that starts where the other ends.
    let joins = |x: char| matches!(x, '|' | '+' | ':');
    for r in 0..g.rows.len() {
        let top = r as f64 * CELL_H;
        let (mid, bottom) = (top + CELL_H / 2.0, top + CELL_H);
        let mut run = String::new();
        let mut run_at = 0;
        let mut row_words: Vec<String> = Vec::new();
        let flush = |run: &mut String, at: usize, row_words: &mut Vec<String>| {
            if !run.is_empty() {
                // Each character has its own position, so that a font whose letters are not all
                // as wide as a cell still keeps every letter under its cell.
                let xs: Vec<String> = (0..run.chars().count())
                    .map(|k| number((at + k) as f64 * CELL_W))
                    .collect();
                row_words.push(format!(
                    "<tspan x=\"{}\">{}</tspan>",
                    xs.join(" "),
                    escape(run)
                ));
                run.clear();
            }
        };
        for c in 0..g.rows[r].len() {
            let cell = classify(&g, r as isize, c as isize);
            let (left, right) = (c as f64 * CELL_W, (c + 1) as f64 * CELL_W);
            let center = (left + right) / 2.0;
            let (a, b) = (
                g.at(r as isize, c as isize - 1),
                g.at(r as isize, c as isize + 1),
            );
            let x1 = if joins(a) { left - CELL_W / 2.0 } else { left };
            let x2 = if joins(b) {
                right + CELL_W / 2.0
            } else {
                right
            };
            if let Cell::Text(ch) = cell {
                if run.is_empty() {
                    run_at = c;
                }
                run.push(ch);
                continue;
            }
            flush(&mut run, run_at, &mut row_words);
            match cell {
                Cell::Blank | Cell::Text(_) => {}
                Cell::Horizontal => segments.push([x1, mid, x2, mid]),
                Cell::Double => {
                    segments.push([x1, mid - 2.0, x2, mid - 2.0]);
                    segments.push([x1, mid + 2.0, x2, mid + 2.0]);
                }
                Cell::Under => segments.push([x1, bottom - 2.0, x2, bottom - 2.0]),
                Cell::Vertical => segments.push([center, top, center, bottom]),
                Cell::Corner => {
                    segments.push([center, top, center, mid]);
                    segments.push([center, mid, x2, mid]);
                }
                Cell::Dotted => line(&mut dotted, center, top, center, bottom),
                Cell::Cross => {
                    segments.push([x1, mid, x2, mid]);
                    segments.push([center, top, center, bottom]);
                }
                Cell::Rise => {
                    // It starts where an underscore on its left ends, and ends where one above ends.
                    let from = if a == '_' { bottom - 2.0 } else { bottom };
                    let to = if g.at(r as isize - 1, c as isize + 1) == '_' {
                        top - 2.0
                    } else {
                        top
                    };
                    rises.insert(segments.len());
                    segments.push([left, from, right, to]);
                }
                Cell::Fall => {
                    falls.insert(segments.len());
                    let from = if g.at(r as isize - 1, c as isize - 1) == '_' {
                        top - 2.0
                    } else {
                        top
                    };
                    let to = if b == '_' { bottom - 2.0 } else { bottom };
                    segments.push([left, from, right, to]);
                }
                Cell::Left => heads.push_str(&format!(
                    "<path d=\"M{} {}L{} {}L{} {}z\"/>",
                    number(left),
                    number(mid),
                    number(left + CELL_W * 0.9),
                    number(mid - 3.5),
                    number(left + CELL_W * 0.9),
                    number(mid + 3.5)
                )),
                Cell::Right => heads.push_str(&format!(
                    "<path d=\"M{} {}L{} {}L{} {}z\"/>",
                    number(right),
                    number(mid),
                    number(right - CELL_W * 0.9),
                    number(mid - 3.5),
                    number(right - CELL_W * 0.9),
                    number(mid + 3.5)
                )),
                Cell::Up => heads.push_str(&format!(
                    "<path d=\"M{} {}L{} {}L{} {}z\"/>",
                    number(center),
                    number(top),
                    number(center - 3.5),
                    number(top + 7.0),
                    number(center + 3.5),
                    number(top + 7.0)
                )),
                Cell::Down => heads.push_str(&format!(
                    "<path d=\"M{} {}L{} {}L{} {}z\"/>",
                    number(center),
                    number(bottom),
                    number(center - 3.5),
                    number(bottom - 7.0),
                    number(center + 3.5),
                    number(bottom - 7.0)
                )),
            }
        }
        flush(&mut run, run_at, &mut row_words);
        if !row_words.is_empty() {
            words.push_str(&format!(
                "<text y=\"{}\">{}</text>",
                number(top + CELL_H * 0.72),
                row_words.join(" ")
            ));
        }
    }
    let mut out = String::new();
    out.push_str(&format!(
        "<figure class=\"diagram\"><svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" width=\"{}\" height=\"{}\" role=\"img\" aria-label=\"Diagram\" font-family=\"ui-monospace, SFMono-Regular, Menlo, Consolas, 'DejaVu Sans Mono', 'Liberation Mono', monospace\" font-size=\"{}\">",
        number(w),
        number(h),
        number(w),
        number(h),
        number(FONT)
    ));
    out.push_str(&format!(
        "<title>Diagram</title><desc>{}</desc>",
        escape(text.trim_end_matches('\n'))
    ));
    let strokes = chain(&segments, &rises, &falls, &tips);
    if !strokes.is_empty() {
        out.push_str(&format!("<path d=\"{strokes}\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>"));
    }
    if !dotted.is_empty() {
        out.push_str(&format!("<path d=\"{dotted}\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.2\" stroke-dasharray=\"2 3\"/>"));
    }
    if !heads.is_empty() {
        out.push_str(&format!("<g fill=\"currentColor\">{heads}</g>"));
    }
    out.push_str(&format!(
        "<g fill=\"currentColor\" xml:space=\"preserve\">{words}</g></svg></figure>\n"
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells(text: &str, r: isize) -> Vec<Cell> {
        let g = Grid::new(text);
        (0..g.width as isize).map(|c| classify(&g, r, c)).collect()
    }

    #[test]
    fn a_bar_inside_a_word_stays_text_and_a_bracket_is_drawn() {
        let c = cells("(foo|bar)", 0);
        assert!(c.iter().all(|x| matches!(x, Cell::Text(_))));
        let c = cells("|---|", 0);
        assert_eq!(
            c,
            [
                Cell::Vertical,
                Cell::Horizontal,
                Cell::Horizontal,
                Cell::Horizontal,
                Cell::Vertical
            ]
        );
    }

    #[test]
    fn a_dash_between_letters_and_an_underscore_in_a_name_stay_text() {
        assert!(
            cells("keep-alive srv_conn", 0)
                .iter()
                .all(|x| matches!(x, Cell::Text(_) | Cell::Blank))
        );
        assert_eq!(cells("arg1 _/", 0)[5], Cell::Under);
        assert_eq!(cells("arg1 _/", 0)[6], Cell::Rise);
    }

    #[test]
    fn arrows_and_slashes_in_paths() {
        let c = cells("<-- Tq -->", 0);
        assert_eq!(c[0], Cell::Left);
        assert_eq!(c[9], Cell::Right);
        assert!(
            cells("HTTP/1.1 /path", 0)
                .iter()
                .all(|x| !matches!(x, Cell::Rise))
        );
    }

    #[test]
    fn an_underscore_and_the_slash_after_it_are_one_stroke() {
        let svg = to_svg("a __/\n");
        let path = svg.split("<path d=\"").nth(1).expect("a path");
        let d = path.split('"').next().expect("its data");
        assert_eq!(d.matches('M').count(), 1, "{d}");
        assert_eq!(d.matches('L').count(), 3, "{d}");
        // the diagonal starts at the height of the underscore
        let y = d
            .split('L')
            .nth(1)
            .and_then(|p| p.split(' ').nth(1))
            .expect("a height");
        let from = d
            .split('L')
            .nth(3)
            .and_then(|p| p.split(' ').nth(1))
            .expect("a height");
        assert_ne!(y, "16", "{d}");
        assert_eq!(from, "0", "{d}");
    }

    #[test]
    fn a_connector_rises_to_the_middle_of_the_span_it_points_at() {
        let svg = to_svg("cmd\n|-----|---|\n        _/\n");
        let path = svg.split("<path d=\"").nth(1).expect("a path");
        let d = path.split('"').next().expect("its data");
        // the second span runs from the middle of the bar at 6 to the one at 10
        let tip = format!("L{} 24", number((6.5 + 10.5) / 2.0 * CELL_W));
        assert!(d.contains(&tip), "{d} lacks {tip}");
    }

    #[test]
    fn a_command_keeps_its_slashes_bars_and_backslashes_as_text() {
        let text = "http-request set-path '%[path,regsub(\"^/(here)(/|$)\",\"my/\\1\",g)]'\n                                    |---------||----|-|\n";
        let g = Grid::new(text);
        let first: Vec<Cell> = (0..g.width as isize).map(|c| classify(&g, 0, c)).collect();
        assert!(
            first
                .iter()
                .all(|x| matches!(x, Cell::Text(_) | Cell::Blank)),
            "{first:?}"
        );
        let svg = to_svg(text);
        assert!(
            svg.contains("regsub(&quot;^/(here)(/|$)&quot;,&quot;my/\\1&quot;"),
            "{svg}"
        );
        assert!(
            svg.contains("</tspan> <tspan"),
            "the words are separated by a space for those who copy the text"
        );
    }

    #[test]
    fn a_connector_that_comes_down_from_the_left_goes_to_the_middle_of_its_span() {
        let bars = format!("|{}|{}|", "-".repeat(20), "-".repeat(20));
        let text = format!("{bars}\n   \\\n    \\\n     \\{} a\n", "_".repeat(20));
        let svg = to_svg(&text);
        let path = svg.split("<path d=\"").nth(1).expect("a path");
        let d = path.split('"').next().expect("its data");
        // the span between the first 2 bars is 3.9 to 167.7, so its middle is 85.8
        assert!(d.contains("M85.8 8L"), "{d}");
    }

    #[test]
    fn a_connector_that_starts_on_a_bar_goes_to_the_span_that_holds_it() {
        let spans = [
            Span {
                x1: 3.9,
                x2: 35.1,
                y: 24.0,
            },
            Span {
                x1: 35.1,
                x2: 50.7,
                y: 24.0,
            },
        ];
        // a point on the shared bar belongs to the span on its right only if it is closer to it
        let first = nearest_span(&spans, 30.0, 40.0).expect("a span");
        let second = nearest_span(&spans, 36.0, 40.0).expect("a span");
        assert_eq!(first.x1, 3.9);
        assert_eq!(second.x1, 35.1);
    }

    #[test]
    fn spans_between_arrows_and_feet_that_turn_are_strokes() {
        let text = " a b c\n<-><--->\n |   |\n |   `- label\n `------- name\n";
        let svg = to_svg(text);
        assert!(svg.contains("<path"), "{svg}");
        // the text of a label stays text and the backtick is not printed
        assert!(svg.contains("label"), "{svg}");
        assert!(
            !svg.split("</desc>")
                .nth(1)
                .unwrap_or_default()
                .contains('`'),
            "{svg}"
        );
        // the foot of the 2nd bar turns right and goes on into the dash after it
        let d = svg
            .split("<path d=\"")
            .nth(1)
            .and_then(|p| p.split('"').next())
            .expect("path");
        assert!(d.contains("L"), "{d}");
    }

    #[test]
    fn every_connector_rises_with_the_same_slope() {
        let bars = format!("|{}|{}|", "-".repeat(20), "-".repeat(20));
        let text = format!(
            "cmd\n{bars}\na  {}/\nb  {}/\n",
            "_".repeat(10),
            "_".repeat(30)
        );
        let svg = to_svg(&text);
        let path = svg.split("<path d=\"").nth(1).expect("a path");
        let d = path.split('"').next().expect("its data");
        let mut slopes = Vec::new();
        for stroke in d.split('M').filter(|p| !p.is_empty()) {
            let points: Vec<(f64, f64)> = stroke
                .split('L')
                .filter_map(|p| {
                    let mut n = p.split(' ');
                    Some((n.next()?.parse().ok()?, n.next()?.parse().ok()?))
                })
                .collect();
            if let [.., (x1, y1), (x2, y2)] = points[..]
                && y1 > y2
                && x2 > x1
            {
                slopes.push((x2 - x1) / (y1 - y2));
            }
        }
        assert_eq!(slopes.len(), 2, "{d}");
        for s in &slopes {
            assert!((s - CELL_W / CELL_H).abs() < 0.02, "{slopes:?} in {d}");
        }
    }

    #[test]
    fn the_svg_has_strokes_text_and_the_text_for_those_who_cannot_see_it() {
        let svg = to_svg("cmd arg\n|---|\n  a _/\n");
        assert!(svg.starts_with("<figure class=\"diagram\"><svg"));
        assert!(svg.contains("<text"));
        assert!(svg.contains("<path d=\"M"));
        assert!(svg.contains("<desc>cmd arg\n|---|\n  a _/</desc>"));
        assert!(svg.ends_with("</figure>\n"));
    }
}
