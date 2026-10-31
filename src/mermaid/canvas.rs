//! Character grid on which a diagram is drawn, and its conversion to styled lines.

use unicode_width::UnicodeWidthChar;

use crate::model::{Line, Segment};
use crate::options::Glyphs;
use crate::style::Style;
use crate::table::Borders;

/// Line segment bits of a cell.
pub const N: u8 = 1;
pub const E: u8 = 2;
pub const S: u8 = 4;
pub const W: u8 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    N,
    E,
    S,
    W,
}

impl Dir {
    pub const ALL: [Dir; 4] = [Dir::N, Dir::E, Dir::S, Dir::W];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn bit(self) -> u8 {
        1 << self.index()
    }

    pub fn opposite(self) -> Dir {
        Dir::ALL[(self.index() + 2) % 4]
    }

    /// The cell next to `(x, y)` in this direction. The caller keeps a margin around the grid.
    pub fn step(self, (x, y): (usize, usize)) -> (usize, usize) {
        match self {
            Dir::N => (x, y - 1),
            Dir::E => (x + 1, y),
            Dir::S => (x, y + 1),
            Dir::W => (x - 1, y),
        }
    }

    /// Direction of the move from `a` to the adjacent cell `b`.
    pub fn between(a: (usize, usize), b: (usize, usize)) -> Dir {
        if b.1 < a.1 {
            Dir::N
        } else if b.0 > a.0 {
            Dir::E
        } else if b.1 > a.1 {
            Dir::S
        } else {
            Dir::W
        }
    }
}

/// How the lines of a cell are drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weight {
    Normal,
    Dotted,
    Thick,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cell {
    Empty,
    /// Line segments leaving the cell: a mask of `N`, `E`, `S` and `W`.
    Lines(u8, Weight),
    /// A drawing character that is not part of a line.
    Glyph(char),
    Text(char),
    /// Right half of a double-width `Text`.
    Cont,
}

pub struct Canvas {
    w: usize,
    h: usize,
    cells: Vec<Cell>,
}

impl Canvas {
    pub fn new(w: usize, h: usize) -> Canvas {
        Canvas {
            w,
            h,
            cells: vec![Cell::Empty; w * h],
        }
    }

    pub fn width(&self) -> usize {
        self.w
    }

    pub fn height(&self) -> usize {
        self.h
    }

    /// Line bits of a cell, 0 when it holds none.
    pub fn lines(&self, x: usize, y: usize) -> u8 {
        match self.cells[y * self.w + x] {
            Cell::Lines(m, _) => m,
            _ => 0,
        }
    }

    /// True when a line may pass through the cell.
    pub fn is_free(&self, x: usize, y: usize) -> bool {
        matches!(self.cells[y * self.w + x], Cell::Empty | Cell::Lines(..))
    }

    /// True when nothing is drawn in the cell.
    pub fn is_empty(&self, x: usize, y: usize) -> bool {
        self.cells[y * self.w + x] == Cell::Empty
    }

    /// Adds line segments to a cell. A weight other than `Normal` replaces the cell's.
    pub fn add_lines(&mut self, x: usize, y: usize, mask: u8, weight: Weight) {
        let cell = &mut self.cells[y * self.w + x];
        match *cell {
            Cell::Empty => *cell = Cell::Lines(mask, weight),
            Cell::Lines(m, old) => {
                let weight = if weight == Weight::Normal {
                    old
                } else {
                    weight
                };
                *cell = Cell::Lines(m | mask, weight);
            }
            _ => {}
        }
    }

    pub fn set_glyph(&mut self, x: usize, y: usize, ch: char) {
        self.cells[y * self.w + x] = Cell::Glyph(ch);
    }

    /// Writes `s` from `(x, y)` rightwards and returns the columns used. Stops at the right edge.
    pub fn text(&mut self, x: usize, y: usize, s: &str) -> usize {
        let mut col = x;
        for ch in s.chars() {
            let width = ch.width().unwrap_or(0);
            if width == 0 || col + width > self.w {
                continue;
            }
            self.cells[y * self.w + col] = Cell::Text(ch);
            if width == 2 {
                self.cells[y * self.w + col + 1] = Cell::Cont;
            }
            col += width;
        }
        col - x
    }

    /// Empties a rectangle of cells.
    pub fn clear_rect(&mut self, x: usize, y: usize, w: usize, h: usize) {
        for cy in y..(y + h).min(self.h) {
            for cx in x..(x + w).min(self.w) {
                self.cells[cy * self.w + cx] = Cell::Empty;
            }
        }
    }

    /// Draws a rectangle border of `w` by `h` cells, at least 2 by 2.
    pub fn draw_box(&mut self, x: usize, y: usize, w: usize, h: usize) {
        for cx in x + 1..x + w - 1 {
            self.add_lines(cx, y, E | W, Weight::Normal);
            self.add_lines(cx, y + h - 1, E | W, Weight::Normal);
        }
        for cy in y + 1..y + h - 1 {
            self.add_lines(x, cy, N | S, Weight::Normal);
            self.add_lines(x + w - 1, cy, N | S, Weight::Normal);
        }
        self.add_lines(x, y, S | E, Weight::Normal);
        self.add_lines(x + w - 1, y, S | W, Weight::Normal);
        self.add_lines(x, y + h - 1, N | E, Weight::Normal);
        self.add_lines(x + w - 1, y + h - 1, N | W, Weight::Normal);
    }

    /// One line per row, without blank rows at the ends nor blank columns on the left.
    pub fn render_lines(&self, glyphs: &Glyphs, line: Style, text: Style) -> Vec<Line> {
        let rows: Vec<&[Cell]> = self.cells.chunks(self.w.max(1)).collect();
        let used = |row: &&[Cell]| row.iter().any(|c| *c != Cell::Empty);
        let Some(first) = rows.iter().position(used) else {
            return Vec::new();
        };
        let last = rows.iter().rposition(used).unwrap_or(first);
        let left = rows[first..=last]
            .iter()
            .filter_map(|r| r.iter().position(|c| *c != Cell::Empty))
            .min()
            .unwrap_or(0);
        rows[first..=last]
            .iter()
            .map(|row| {
                let end = row
                    .iter()
                    .rposition(|c| *c != Cell::Empty)
                    .map_or(0, |i| i + 1);
                let mut segs: Vec<Segment> = Vec::new();
                let mut push = |ch: char, style: Style| match segs.last_mut() {
                    Some(seg) if seg.style == style => seg.text.push(ch),
                    _ => segs.push(Segment::new(ch.to_string(), style)),
                };
                for cell in &row[left.min(end)..end] {
                    match *cell {
                        Cell::Empty => push(' ', text),
                        Cell::Lines(m, weight) => push(lines_glyph(m, weight, glyphs), line),
                        Cell::Glyph(ch) => push(ch, line),
                        Cell::Text(ch) => push(ch, text),
                        Cell::Cont => {}
                    }
                }
                segs
            })
            .collect()
    }
}

/// The character joining the line segments of `mask`.
fn lines_glyph(mask: u8, weight: Weight, glyphs: &Glyphs) -> char {
    let b = if weight == Weight::Thick {
        &glyphs.thick
    } else {
        &glyphs.borders
    };
    let joined = joined_glyph(mask, b);
    match weight {
        Weight::Dotted if mask & (E | W) == 0 => glyphs.dotted[1],
        Weight::Dotted if mask & (N | S) == 0 => glyphs.dotted[0],
        _ => joined,
    }
}

fn joined_glyph(mask: u8, b: &Borders) -> char {
    match (mask & N != 0, mask & E != 0, mask & S != 0, mask & W != 0) {
        (_, false, _, false) => b.v,
        (false, _, false, _) => b.h,
        (false, true, true, false) => b.top_left,
        (false, false, true, true) => b.top_right,
        (true, true, false, false) => b.bottom_left,
        (true, false, false, true) => b.bottom_right,
        (true, true, true, false) => b.mid_left,
        (true, false, true, true) => b.mid_right,
        (false, true, true, true) => b.top_mid,
        (true, true, false, true) => b.bottom_mid,
        (true, true, true, true) => b.cross,
    }
}

/// Width and height of a cell in the drawing of a diagram, in the units of its `viewBox`.
const CELL_W: f64 = 7.8;
const CELL_H: f64 = 16.0;
/// Where the base line of a row of text lies in its cell.
const BASELINE: f64 = 11.5;

fn number(x: f64) -> String {
    let s = format!("{x:.1}");
    s.strip_suffix(".0").map_or(s.clone(), str::to_string)
}

/// The path of a triangle with its tip toward `heading`, in the cell at `(x, y)`.
fn triangle(heading: char, x: f64, y: f64) -> String {
    let (cx, cy) = (x + CELL_W / 2.0, y + CELL_H / 2.0);
    let (a, b) = (3.6, 4.4);
    let points = match heading {
        '^' => [(cx, cy - b), (cx - a, cy + b), (cx + a, cy + b)],
        '>' => [(cx + a, cy), (cx - a, cy - b), (cx - a, cy + b)],
        'v' => [(cx, cy + b), (cx - a, cy - b), (cx + a, cy - b)],
        _ => [(cx - a, cy), (cx + a, cy - b), (cx + a, cy + b)],
    };
    format!(
        "M{} {}L{} {}L{} {}Z",
        number(points[0].0),
        number(points[0].1),
        number(points[1].0),
        number(points[1].1),
        number(points[2].0),
        number(points[2].1)
    )
}

impl Canvas {
    /// The drawing as an SVG element, which holds `source` for those who cannot see it.
    /// Lines are drawn with the colour of the text, so that the drawing follows the theme.
    pub fn to_svg(&self, source: &str) -> String {
        let used = |c: &Cell| *c != Cell::Empty;
        let rows: Vec<&[Cell]> = self.cells.chunks(self.w.max(1)).collect();
        let Some(first) = rows.iter().position(|r| r.iter().any(used)) else {
            return String::new();
        };
        let last = rows
            .iter()
            .rposition(|r| r.iter().any(used))
            .unwrap_or(first);
        let left = rows[first..=last]
            .iter()
            .filter_map(|r| r.iter().position(used))
            .min()
            .unwrap_or(0);
        let end = rows[first..=last]
            .iter()
            .filter_map(|r| r.iter().rposition(used))
            .max()
            .map_or(left, |e| e + 1);
        let (w, h) = (
            (end - left) as f64 * CELL_W,
            (last - first + 1) as f64 * CELL_H,
        );
        let (mut normal, mut thick, mut dotted) = (String::new(), String::new(), String::new());
        let mut shapes = String::new();
        let mut words = String::new();
        let seg = |d: &mut String, x1: f64, y1: f64, x2: f64, y2: f64| {
            d.push_str(&format!(
                "M{} {}L{} {}",
                number(x1),
                number(y1),
                number(x2),
                number(y2)
            ));
        };
        for (r, row) in rows[first..=last].iter().enumerate() {
            let y = r as f64 * CELL_H;
            let cy = y + CELL_H / 2.0;
            let mut run: Vec<(usize, char)> = Vec::new();
            let flush = |run: &mut Vec<(usize, char)>, words: &mut String| {
                if run.is_empty() {
                    return;
                }
                let xs: Vec<String> = run
                    .iter()
                    .map(|(c, _)| number(*c as f64 * CELL_W))
                    .collect();
                let text: String = run.iter().map(|(_, ch)| *ch).collect();
                words.push_str(&format!(
                    "<text y=\"{}\"><tspan x=\"{}\">{}</tspan></text>",
                    number(y + BASELINE),
                    xs.join(" "),
                    crate::escape::html(&text)
                ));
                run.clear();
            };
            for (c, cell) in row[left.min(row.len())..end.min(row.len())]
                .iter()
                .enumerate()
            {
                let x = c as f64 * CELL_W;
                let cx = x + CELL_W / 2.0;
                if let Cell::Text(ch) = cell {
                    run.push((c, *ch));
                    continue;
                }
                if matches!(cell, Cell::Cont) {
                    continue;
                }
                flush(&mut run, &mut words);
                match *cell {
                    Cell::Lines(mask, weight) => {
                        let d = match weight {
                            Weight::Normal => &mut normal,
                            Weight::Thick => &mut thick,
                            Weight::Dotted => &mut dotted,
                        };
                        let (n, e, s, wst) =
                            (mask & N != 0, mask & E != 0, mask & S != 0, mask & W != 0);
                        if n && s {
                            seg(d, cx, y, cx, y + CELL_H);
                        } else if n {
                            seg(d, cx, y, cx, cy);
                        } else if s {
                            seg(d, cx, cy, cx, y + CELL_H);
                        }
                        if e && wst {
                            seg(d, x, cy, x + CELL_W, cy);
                        } else if e {
                            seg(d, cx, cy, x + CELL_W, cy);
                        } else if wst {
                            seg(d, x, cy, cx, cy);
                        }
                    }
                    Cell::Glyph(ch) => {
                        let r = CELL_W / 2.0;
                        match ch {
                            '\u{25b2}' => shapes.push_str(&format!("<path d=\"{}\" fill=\"currentColor\"/>", triangle('^', x, y))),
                            '\u{25ba}' => shapes.push_str(&format!("<path d=\"{}\" fill=\"currentColor\"/>", triangle('>', x, y))),
                            '\u{25bc}' => shapes.push_str(&format!("<path d=\"{}\" fill=\"currentColor\"/>", triangle('v', x, y))),
                            '\u{25c4}' => shapes.push_str(&format!("<path d=\"{}\" fill=\"currentColor\"/>", triangle('<', x, y))),
                            '\u{25b3}' => seg_path(&mut shapes, &triangle('^', x, y)),
                            '\u{25b7}' => seg_path(&mut shapes, &triangle('>', x, y)),
                            '\u{25bd}' => seg_path(&mut shapes, &triangle('v', x, y)),
                            '\u{25c1}' => seg_path(&mut shapes, &triangle('<', x, y)),
                            '\u{25c6}' | '\u{25c7}' => {
                                let d = format!(
                                    "M{} {}L{} {}L{} {}L{} {}Z",
                                    number(cx),
                                    number(cy - 4.6),
                                    number(cx + r),
                                    number(cy),
                                    number(cx),
                                    number(cy + 4.6),
                                    number(cx - r),
                                    number(cy)
                                );
                                if ch == '\u{25c6}' {
                                    shapes.push_str(&format!("<path d=\"{d}\" fill=\"currentColor\"/>"));
                                } else {
                                    seg_path(&mut shapes, &d);
                                }
                            }
                            '\u{256d}' => normal.push_str(&format!("M{} {}L{} {}Q{} {} {} {}L{} {}", number(cx), number(y + CELL_H), number(cx), number(cy + r), number(cx), number(cy), number(cx + r), number(cy), number(x + CELL_W), number(cy))),
                            '\u{256e}' => normal.push_str(&format!("M{} {}L{} {}Q{} {} {} {}L{} {}", number(cx), number(y + CELL_H), number(cx), number(cy + r), number(cx), number(cy), number(cx - r), number(cy), number(x), number(cy))),
                            '\u{2570}' => normal.push_str(&format!("M{} {}L{} {}Q{} {} {} {}L{} {}", number(cx), number(y), number(cx), number(cy - r), number(cx), number(cy), number(cx + r), number(cy), number(x + CELL_W), number(cy))),
                            '\u{256f}' => normal.push_str(&format!("M{} {}L{} {}Q{} {} {} {}L{} {}", number(cx), number(y), number(cx), number(cy - r), number(cx), number(cy), number(cx - r), number(cy), number(x), number(cy))),
                            '\u{2571}' => seg(&mut normal, x + CELL_W, y, x, y + CELL_H),
                            '\u{2572}' => seg(&mut normal, x, y, x + CELL_W, y + CELL_H),
                            '\u{25cf}' => shapes.push_str(&format!("<circle cx=\"{}\" cy=\"{}\" r=\"4\" fill=\"currentColor\"/>", number(cx), number(cy))),
                            '\u{25c9}' => shapes.push_str(&format!("<circle cx=\"{}\" cy=\"{}\" r=\"4.4\" fill=\"none\" stroke=\"currentColor\"/><circle cx=\"{}\" cy=\"{}\" r=\"2\" fill=\"currentColor\"/>", number(cx), number(cy), number(cx), number(cy))),
                            other => run.push((c, other)),
                        }
                    }
                    _ => {}
                }
            }
            flush(&mut run, &mut words);
        }
        let stroke = |d: &str, width: &str, dash: &str| {
            if d.is_empty() {
                String::new()
            } else {
                format!(
                    "<path d=\"{d}\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"{width}\"{dash} stroke-linecap=\"round\" stroke-linejoin=\"round\"/>"
                )
            }
        };
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w} {h}\" width=\"{w}\" height=\"{h}\" role=\"img\" aria-label=\"Diagram\" font-family=\"ui-monospace, SFMono-Regular, Menlo, Consolas, 'DejaVu Sans Mono', 'Liberation Mono', monospace\" font-size=\"13\"><title>Diagram</title><desc>{}</desc>{}{}{}{}<g fill=\"currentColor\" xml:space=\"preserve\">{words}</g></svg>",
            crate::escape::html(source.trim_end()),
            stroke(&normal, "1.2", ""),
            stroke(&thick, "2.4", ""),
            stroke(&dotted, "1.2", " stroke-dasharray=\"2 3\""),
            shapes,
            w = number(w),
            h = number(h),
        )
    }
}

/// An outline in the colour of the text, with nothing inside.
fn seg_path(out: &mut String, d: &str) {
    out.push_str(&format!(
        "<path d=\"{d}\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.2\" stroke-linejoin=\"round\"/>"
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(canvas: Canvas) -> Vec<String> {
        canvas
            .render_lines(&Glyphs::ascii(), Style::PLAIN, Style::PLAIN)
            .iter()
            .map(|l| l.iter().map(|s| s.text.as_str()).collect())
            .collect()
    }

    #[test]
    fn box_with_text() {
        let mut c = Canvas::new(10, 6);
        c.draw_box(2, 1, 7, 3);
        c.text(4, 2, "ab");
        assert_eq!(plain(c), ["+-----+", "| ab  |", "+-----+"]);
    }

    #[test]
    fn lines_join_into_a_junction() {
        let mut c = Canvas::new(5, 3);
        c.add_lines(1, 1, E | W, Weight::Normal);
        c.add_lines(2, 1, E | W | S, Weight::Normal);
        c.add_lines(2, 2, N, Weight::Normal);
        assert_eq!(plain(c), ["-+", " |"]);
    }

    #[test]
    fn wide_text_takes_two_cells() {
        let mut c = Canvas::new(6, 1);
        assert_eq!(c.text(0, 0, "a\u{4e2d}b"), 4);
        assert_eq!(plain(c), ["a\u{4e2d}b"]);
    }
}
