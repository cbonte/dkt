//! The screen of a terminal, as a grid of cells that the output of a program writes.
//!
//! `Screen::feed` reads text with the control characters and the escape sequences of a VT
//! terminal: cursor moves, erases, inserted and deleted lines and characters, a scroll region,
//! the look of the text, the alternate screen and the saved cursor.
//! A sequence that the screen does not know is read to its end and left out.

use unicode_width::UnicodeWidthChar;

/// The colour of a cell, as the program set it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Color {
    #[default]
    Default,
    Index(u8),
    Rgb(u8, u8, u8),
}

/// The look of a cell.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Style {
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub reverse: bool,
    pub hidden: bool,
}

/// One cell of the grid.
///
/// A wide character takes its cell and the next one, which holds `'\0'`.
/// The characters of no width that follow a character are kept in `extra`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Cell {
    pub ch: char,
    pub extra: Option<Box<str>>,
    pub style: Style,
}

impl Cell {
    /// An empty cell with the background of `style`, as an erase leaves it.
    fn blank(style: Style) -> Cell {
        Cell {
            ch: ' ',
            extra: None,
            style: Style {
                bg: style.bg,
                ..Style::default()
            },
        }
    }

    /// The second half of a wide character.
    pub fn is_continuation(&self) -> bool {
        self.ch == '\0'
    }
}

/// The cursor, the look and the character sets that `ESC 7` saves and `ESC 8` restores.
#[derive(Clone, Copy, Debug, Default)]
struct Saved {
    row: usize,
    col: usize,
    pen: Style,
    origin: bool,
    charsets: [bool; 2],
    shifted: bool,
}

/// Where the reader of the input stands, between 2 characters.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum Mode {
    #[default]
    Ground,
    Escape,
    /// After `ESC` and an intermediate character, such as `(` that designates a character set.
    EscapeWith(char),
    Csi,
    /// A string that ends with `ST` or `BEL`: an OSC, a DCS, an APC, a PM or a SOS.
    Text,
    /// An `ESC` inside such a string, which starts its `ST`.
    TextEscape,
}

/// The most parameters that a control sequence keeps.
const MAX_PARAMS: usize = 32;

/// The distance between 2 tab stops.
const TAB: usize = 8;

/// A terminal screen of a fixed size, which output changes.
pub struct Screen {
    cols: usize,
    rows: usize,
    grid: Vec<Vec<Cell>>,
    /// The other screen: the main one while the alternate one is shown, and the other way.
    other: Vec<Vec<Cell>>,
    alternate: bool,
    row: usize,
    col: usize,
    /// The cursor stands past the last column, and the next character goes to the next line.
    pending_wrap: bool,
    pen: Style,
    top: usize,
    bottom: usize,
    origin: bool,
    autowrap: bool,
    insert: bool,
    cursor_visible: bool,
    saved: Saved,
    /// The cursor that `?1049h` saves, apart from the one of `ESC 7`.
    saved_alternate: Saved,
    /// Whether G0 and G1 draw lines, as `ESC ( 0` and `ESC ) 0` set them.
    charsets: [bool; 2],
    /// G1 is in use, after `SO`.
    shifted: bool,
    last: Option<char>,
    mode: Mode,
    params: String,
    /// The program placed the cursor, cleared the screen or showed the alternate screen.
    addressed: bool,
}

impl Screen {
    /// An empty screen of `cols` columns and `rows` rows, at least 1 of each.
    pub fn new(cols: usize, rows: usize) -> Screen {
        let (cols, rows) = (cols.max(1), rows.max(1));
        let blank = vec![vec![Cell::blank(Style::default()); cols]; rows];
        Screen {
            cols,
            rows,
            grid: blank.clone(),
            other: blank,
            alternate: false,
            row: 0,
            col: 0,
            pending_wrap: false,
            pen: Style::default(),
            top: 0,
            bottom: rows - 1,
            origin: false,
            autowrap: true,
            insert: false,
            cursor_visible: true,
            saved: Saved::default(),
            saved_alternate: Saved::default(),
            charsets: [false; 2],
            shifted: false,
            last: None,
            mode: Mode::Ground,
            params: String::new(),
            addressed: false,
        }
    }

    /// The rows of the screen in view.
    pub fn lines(&self) -> &[Vec<Cell>] {
        &self.grid
    }

    pub fn size(&self) -> (usize, usize) {
        (self.cols, self.rows)
    }

    /// The row and the column of the cursor, when the program shows it.
    pub fn cursor(&self) -> Option<(usize, usize)> {
        self.cursor_visible.then_some((self.row, self.col))
    }

    /// Whether the output placed the cursor, cleared the whole screen or showed the alternate
    /// screen, as a program that takes the whole screen does.
    pub fn addressed(&self) -> bool {
        self.addressed
    }

    /// Changes the size, keeping the cells at the top left that still fit.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        let (cols, rows) = (cols.max(1), rows.max(1));
        for grid in [&mut self.grid, &mut self.other] {
            grid.resize(rows, vec![Cell::blank(Style::default()); cols]);
            for line in grid.iter_mut() {
                line.resize(cols, Cell::blank(Style::default()));
                // A wide character cut in half by the new edge is erased.
                if line.last().is_some_and(|c| c.ch.width().unwrap_or(1) == 2) {
                    line[cols - 1] = Cell::blank(Style::default());
                }
            }
        }
        self.cols = cols;
        self.rows = rows;
        self.top = 0;
        self.bottom = rows - 1;
        self.row = self.row.min(rows - 1);
        self.col = self.col.min(cols - 1);
        self.pending_wrap = false;
    }

    /// Reads the output of a program.
    pub fn feed(&mut self, text: &str) {
        for c in text.chars() {
            self.step(c);
        }
    }

    fn step(&mut self, c: char) {
        match self.mode {
            Mode::Ground => self.ground(c),
            Mode::Escape => self.escape(c),
            Mode::EscapeWith(first) => {
                self.mode = Mode::Ground;
                let slot = match first {
                    '(' => Some(0),
                    ')' => Some(1),
                    _ => None,
                };
                if let Some(slot) = slot {
                    self.charsets[slot] = c == '0';
                }
            }
            Mode::Csi => {
                if ('@'..='~').contains(&c) {
                    self.mode = Mode::Ground;
                    let params = std::mem::take(&mut self.params);
                    self.csi(&params, c);
                } else if c == '\u{1b}' {
                    self.params.clear();
                    self.mode = Mode::Escape;
                } else if c.is_control() {
                    self.control(c);
                } else {
                    self.params.push(c);
                }
            }
            Mode::Text => match c {
                '\u{7}' => self.mode = Mode::Ground,
                '\u{1b}' => self.mode = Mode::TextEscape,
                _ => {}
            },
            Mode::TextEscape => {
                self.mode = if c == '\\' { Mode::Ground } else { Mode::Text };
            }
        }
    }

    fn ground(&mut self, c: char) {
        if c == '\u{1b}' {
            self.mode = Mode::Escape;
        } else if c.is_control() {
            self.control(c);
        } else {
            self.print(c);
        }
    }

    fn control(&mut self, c: char) {
        match c {
            '\n' | '\u{b}' | '\u{c}' => self.linefeed(),
            '\r' => {
                self.col = 0;
                self.pending_wrap = false;
            }
            '\u{8}' => {
                self.col = self.col.saturating_sub(1);
                self.pending_wrap = false;
            }
            '\t' => self.tab(1),
            '\u{e}' => self.shifted = true,
            '\u{f}' => self.shifted = false,
            _ => {}
        }
    }

    fn escape(&mut self, c: char) {
        self.mode = Mode::Ground;
        match c {
            '[' => {
                self.params.clear();
                self.mode = Mode::Csi;
            }
            ']' | 'P' | '_' | '^' | 'X' => self.mode = Mode::Text,
            '(' | ')' | '*' | '+' | '#' | '%' | ' ' => self.mode = Mode::EscapeWith(c),
            'D' => self.index(),
            'E' => {
                self.col = 0;
                self.index();
            }
            'M' => self.reverse_index(),
            '7' => self.saved = self.save(),
            '8' => self.restore(self.saved),
            'c' => {
                let addressed = self.addressed;
                *self = Screen::new(self.cols, self.rows);
                self.addressed = addressed;
            }
            _ => {}
        }
    }

    /// Writes a character at the cursor and moves the cursor past it.
    fn print(&mut self, c: char) {
        let c = if self.charsets[usize::from(self.shifted)] {
            line_drawing(c)
        } else {
            c
        };
        let width = c.width().unwrap_or(0);
        if width == 0 {
            self.combine(c);
            return;
        }
        if self.pending_wrap && self.autowrap {
            self.col = 0;
            self.linefeed();
        }
        self.pending_wrap = false;
        if width == 2 && self.col + 1 >= self.cols {
            if !self.autowrap || self.cols < 2 {
                return;
            }
            self.grid[self.row][self.col] = Cell::blank(self.pen);
            self.col = 0;
            self.linefeed();
        }
        if self.insert {
            self.insert_chars(width);
        }
        self.clear_halves(self.row, self.col, width);
        let style = self.pen;
        self.grid[self.row][self.col] = Cell {
            ch: c,
            extra: None,
            style,
        };
        if width == 2 {
            self.grid[self.row][self.col + 1] = Cell {
                ch: '\0',
                extra: None,
                style,
            };
        }
        self.last = Some(c);
        self.col += width;
        if self.col >= self.cols {
            self.col = self.cols - 1;
            self.pending_wrap = self.autowrap;
        }
    }

    /// Adds a character of no width to the character before the cursor.
    fn combine(&mut self, c: char) {
        let line = &mut self.grid[self.row];
        let mut at = if self.pending_wrap {
            self.col
        } else {
            self.col.saturating_sub(1)
        };
        if at > 0 && line[at].is_continuation() {
            at -= 1;
        }
        let cell = &mut line[at];
        let mut extra = cell.extra.take().map(String::from).unwrap_or_default();
        extra.push(c);
        cell.extra = Some(extra.into_boxed_str());
    }

    /// Erases the other half of a wide character that a write of `width` cells at `col` cuts.
    fn clear_halves(&mut self, row: usize, col: usize, width: usize) {
        let line = &mut self.grid[row];
        if line[col].is_continuation() && col > 0 {
            line[col - 1] = Cell::blank(Style::default());
        }
        let end = col + width;
        if end < self.cols && line[end].is_continuation() {
            line[end] = Cell::blank(Style::default());
        }
    }

    fn linefeed(&mut self) {
        self.pending_wrap = false;
        self.index();
    }

    /// Moves the cursor down, scrolling the region when it stands on its last row.
    fn index(&mut self) {
        self.pending_wrap = false;
        if self.row == self.bottom {
            self.scroll_up(1);
        } else if self.row + 1 < self.rows {
            self.row += 1;
        }
    }

    /// Moves the cursor up, scrolling the region down when it stands on its first row.
    fn reverse_index(&mut self) {
        self.pending_wrap = false;
        if self.row == self.top {
            self.scroll_down(1);
        } else {
            self.row = self.row.saturating_sub(1);
        }
    }

    fn blank_line(&self) -> Vec<Cell> {
        vec![Cell::blank(self.pen); self.cols]
    }

    /// Moves the rows of the scroll region up by `n`, with blank rows at its bottom.
    fn scroll_up(&mut self, n: usize) {
        let n = n.min(self.bottom - self.top + 1);
        let blank = self.blank_line();
        self.grid[self.top..=self.bottom].rotate_left(n);
        for line in &mut self.grid[self.bottom + 1 - n..=self.bottom] {
            line.clone_from(&blank);
        }
    }

    /// Moves the rows of the scroll region down by `n`, with blank rows at its top.
    fn scroll_down(&mut self, n: usize) {
        let n = n.min(self.bottom - self.top + 1);
        let blank = self.blank_line();
        self.grid[self.top..=self.bottom].rotate_right(n);
        for line in &mut self.grid[self.top..self.top + n] {
            line.clone_from(&blank);
        }
    }

    fn tab(&mut self, count: usize) {
        for _ in 0..count {
            self.col = ((self.col / TAB + 1) * TAB).min(self.cols - 1);
        }
        self.pending_wrap = false;
    }

    fn back_tab(&mut self, count: usize) {
        for _ in 0..count {
            self.col = self.col.saturating_sub(1) / TAB * TAB;
        }
        self.pending_wrap = false;
    }

    fn save(&self) -> Saved {
        Saved {
            row: self.row,
            col: self.col,
            pen: self.pen,
            origin: self.origin,
            charsets: self.charsets,
            shifted: self.shifted,
        }
    }

    fn restore(&mut self, saved: Saved) {
        self.row = saved.row.min(self.rows - 1);
        self.col = saved.col.min(self.cols - 1);
        self.pen = saved.pen;
        self.origin = saved.origin;
        self.charsets = saved.charsets;
        self.shifted = saved.shifted;
        self.pending_wrap = false;
    }

    /// Places the cursor at a row and a column counted from 0, inside the region in origin mode.
    fn go(&mut self, row: usize, col: usize) {
        let (first, last) = if self.origin {
            (self.top, self.bottom)
        } else {
            (0, self.rows - 1)
        };
        self.row = (first + row).min(last);
        self.col = col.min(self.cols - 1);
        self.pending_wrap = false;
    }

    /// Erases the cells `from..to` of a row.
    fn erase(&mut self, row: usize, from: usize, to: usize) {
        let to = to.min(self.cols);
        if from >= to {
            return;
        }
        if from > 0 && self.grid[row][from].is_continuation() {
            self.grid[row][from - 1] = Cell::blank(self.pen);
        }
        if to < self.cols && self.grid[row][to].is_continuation() {
            self.grid[row][to] = Cell::blank(self.pen);
        }
        let blank = Cell::blank(self.pen);
        for cell in &mut self.grid[row][from..to] {
            cell.clone_from(&blank);
        }
    }

    fn insert_chars(&mut self, n: usize) {
        let (row, col) = (self.row, self.col);
        let n = n.min(self.cols - col);
        self.clear_halves(row, col, 0);
        let blank = Cell::blank(self.pen);
        let line = &mut self.grid[row];
        line[col..].rotate_right(n);
        for cell in &mut line[col..col + n] {
            cell.clone_from(&blank);
        }
        if line[self.cols - 1].ch.width().unwrap_or(1) == 2 {
            line[self.cols - 1] = blank;
        }
    }

    fn delete_chars(&mut self, n: usize) {
        let (row, col) = (self.row, self.col);
        let n = n.min(self.cols - col);
        self.clear_halves(row, col, n);
        let blank = Cell::blank(self.pen);
        let line = &mut self.grid[row];
        line[col..].rotate_left(n);
        let cols = line.len();
        for cell in &mut line[cols - n..] {
            cell.clone_from(&blank);
        }
    }

    /// Inserts `n` blank rows at the cursor, inside the scroll region.
    fn insert_lines(&mut self, n: usize) {
        if !(self.top..=self.bottom).contains(&self.row) {
            return;
        }
        let n = n.min(self.bottom - self.row + 1);
        let blank = self.blank_line();
        self.grid[self.row..=self.bottom].rotate_right(n);
        for line in &mut self.grid[self.row..self.row + n] {
            line.clone_from(&blank);
        }
        self.col = 0;
        self.pending_wrap = false;
    }

    /// Deletes `n` rows at the cursor, inside the scroll region.
    fn delete_lines(&mut self, n: usize) {
        if !(self.top..=self.bottom).contains(&self.row) {
            return;
        }
        let n = n.min(self.bottom - self.row + 1);
        let blank = self.blank_line();
        self.grid[self.row..=self.bottom].rotate_left(n);
        for line in &mut self.grid[self.bottom + 1 - n..=self.bottom] {
            line.clone_from(&blank);
        }
        self.col = 0;
        self.pending_wrap = false;
    }

    /// Shows the alternate screen, cleared, or the main screen again.
    fn set_alternate(&mut self, on: bool) {
        if on == self.alternate {
            return;
        }
        std::mem::swap(&mut self.grid, &mut self.other);
        self.alternate = on;
        if on {
            let blank = vec![Cell::blank(Style::default()); self.cols];
            for line in &mut self.grid {
                line.clone_from(&blank);
            }
            self.addressed = true;
        }
    }

    fn set_mode(&mut self, private: bool, params: &[u32], on: bool) {
        for &p in params {
            match (private, p) {
                (false, 4) => self.insert = on,
                (true, 6) => {
                    self.origin = on;
                    self.go(0, 0);
                }
                (true, 7) => self.autowrap = on,
                (true, 25) => self.cursor_visible = on,
                (true, 47 | 1047) => self.set_alternate(on),
                (true, 1048) => {
                    if on {
                        self.saved_alternate = self.save();
                    } else {
                        self.restore(self.saved_alternate);
                    }
                }
                (true, 1049) => {
                    if on {
                        self.saved_alternate = self.save();
                        self.set_alternate(true);
                    } else {
                        self.set_alternate(false);
                        self.restore(self.saved_alternate);
                    }
                }
                _ => {}
            }
        }
    }

    /// Runs a control sequence, `ESC [`, its parameters and its final character.
    fn csi(&mut self, raw: &str, final_char: char) {
        let private = raw.starts_with(['?', '>', '<', '=']);
        let body = raw.trim_start_matches(['?', '>', '<', '=']);
        // An intermediate character, such as the `!` of `CSI ! p`, makes another sequence.
        let intermediate = body.chars().find(|c| (' '..='/').contains(c));
        let body: String = body.chars().filter(|c| !(' '..='/').contains(c)).collect();
        let groups: Vec<Vec<Option<u32>>> = body
            .split(';')
            .take(MAX_PARAMS)
            .map(|g| g.split(':').map(|n| n.parse().ok()).collect())
            .collect();
        let params: Vec<u32> = groups
            .iter()
            .map(|g| g.first().copied().flatten().unwrap_or(0))
            .collect();
        // The first parameter, where 0 or nothing stands for 1.
        let n = params.first().copied().unwrap_or(0).max(1) as usize;
        let second = params.get(1).copied().unwrap_or(0).max(1) as usize;
        if intermediate.is_some() {
            if intermediate == Some('!') && final_char == 'p' {
                self.soft_reset();
            }
            return;
        }
        if private {
            match final_char {
                'h' => self.set_mode(raw.starts_with('?'), &params, true),
                'l' => self.set_mode(raw.starts_with('?'), &params, false),
                _ => {}
            }
            return;
        }
        match final_char {
            'A' => {
                let floor = if self.row >= self.top { self.top } else { 0 };
                self.row = self.row.saturating_sub(n).max(floor);
                self.pending_wrap = false;
            }
            'B' | 'e' => {
                let ceiling = if self.row <= self.bottom {
                    self.bottom
                } else {
                    self.rows - 1
                };
                self.row = (self.row + n).min(ceiling);
                self.pending_wrap = false;
            }
            'C' | 'a' => {
                self.col = (self.col + n).min(self.cols - 1);
                self.pending_wrap = false;
            }
            'D' => {
                self.col = self.col.saturating_sub(n);
                self.pending_wrap = false;
            }
            'E' => {
                self.row = (self.row + n).min(self.rows - 1);
                self.col = 0;
                self.pending_wrap = false;
            }
            'F' => {
                self.row = self.row.saturating_sub(n);
                self.col = 0;
                self.pending_wrap = false;
            }
            'G' | '`' => {
                self.col = (n - 1).min(self.cols - 1);
                self.pending_wrap = false;
            }
            'd' => {
                let col = self.col;
                self.go(n - 1, col);
            }
            'H' | 'f' => {
                if !raw.is_empty() {
                    self.addressed = true;
                }
                self.go(n - 1, second - 1);
            }
            'I' => self.tab(n),
            'Z' => self.back_tab(n),
            'J' => {
                let (row, col) = (self.row, self.col);
                match params.first().copied().unwrap_or(0) {
                    0 => {
                        self.erase(row, col, self.cols);
                        for r in row + 1..self.rows {
                            self.erase(r, 0, self.cols);
                        }
                    }
                    1 => {
                        for r in 0..row {
                            self.erase(r, 0, self.cols);
                        }
                        self.erase(row, 0, col + 1);
                    }
                    2 | 3 => {
                        self.addressed = true;
                        for r in 0..self.rows {
                            self.erase(r, 0, self.cols);
                        }
                    }
                    _ => {}
                }
            }
            'K' => {
                let (row, col) = (self.row, self.col);
                match params.first().copied().unwrap_or(0) {
                    0 => self.erase(row, col, self.cols),
                    1 => self.erase(row, 0, col + 1),
                    2 => self.erase(row, 0, self.cols),
                    _ => {}
                }
            }
            'X' => {
                let (row, col) = (self.row, self.col);
                self.erase(row, col, col + n);
            }
            '@' => self.insert_chars(n),
            'P' => self.delete_chars(n),
            'L' => self.insert_lines(n),
            'M' => self.delete_lines(n),
            'S' => self.scroll_up(n),
            // `CSI T` with more parameters starts a mouse tracking, not a scroll.
            'T' if params.len() <= 1 => self.scroll_down(n),
            'b' => {
                if let Some(c) = self.last {
                    for _ in 0..n.min(self.cols * self.rows) {
                        self.print(c);
                    }
                }
            }
            'r' => {
                let top = n - 1;
                let bottom = match params.get(1).copied().unwrap_or(0) {
                    0 => self.rows - 1,
                    b => (b as usize - 1).min(self.rows - 1),
                };
                if top < bottom {
                    self.top = top;
                    self.bottom = bottom;
                    self.go(0, 0);
                }
            }
            's' => self.saved = self.save(),
            'u' => self.restore(self.saved),
            'h' => self.set_mode(false, &params, true),
            'l' => self.set_mode(false, &params, false),
            'm' => self.sgr(&groups),
            _ => {}
        }
    }

    fn soft_reset(&mut self) {
        self.pen = Style::default();
        self.top = 0;
        self.bottom = self.rows - 1;
        self.origin = false;
        self.autowrap = true;
        self.insert = false;
        self.cursor_visible = true;
        self.charsets = [false; 2];
        self.shifted = false;
    }

    /// Applies the parameters of a `m` sequence to the look of the next characters.
    ///
    /// A colour of 256 or a true colour is read with `;` or with `:` between its parts.
    fn sgr(&mut self, groups: &[Vec<Option<u32>>]) {
        let pen = &mut self.pen;
        let mut i = 0;
        while i < groups.len() {
            let group = &groups[i];
            let p = group.first().copied().flatten().unwrap_or(0);
            match p {
                0 => *pen = Style::default(),
                1 => pen.bold = true,
                2 => pen.dim = true,
                3 => pen.italic = true,
                4 => pen.underline = group.get(1).copied().flatten() != Some(0),
                7 => pen.reverse = true,
                8 => pen.hidden = true,
                9 => pen.strike = true,
                21 => pen.underline = true,
                22 => {
                    pen.bold = false;
                    pen.dim = false;
                }
                23 => pen.italic = false,
                24 => pen.underline = false,
                27 => pen.reverse = false,
                28 => pen.hidden = false,
                29 => pen.strike = false,
                30..=37 => pen.fg = Color::Index((p - 30) as u8),
                39 => pen.fg = Color::Default,
                40..=47 => pen.bg = Color::Index((p - 40) as u8),
                49 => pen.bg = Color::Default,
                90..=97 => pen.fg = Color::Index((p - 90 + 8) as u8),
                100..=107 => pen.bg = Color::Index((p - 100 + 8) as u8),
                38 | 48 | 58 => {
                    let (color, used) = if group.len() > 1 {
                        (extended(&group[1..]), 0)
                    } else {
                        let rest: Vec<Option<u32>> = groups[i + 1..]
                            .iter()
                            .take(4)
                            .map(|g| g.first().copied().flatten())
                            .collect();
                        let used = match rest.first().copied().flatten() {
                            Some(5) => 2,
                            Some(2) => 4,
                            _ => 0,
                        };
                        (extended(&rest), used)
                    };
                    i += used.min(groups.len() - i - 1);
                    match (p, color) {
                        (38, Some(c)) => pen.fg = c,
                        (48, Some(c)) => pen.bg = c,
                        _ => {}
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }
}

/// The colour that the parts after a `38` or a `48` give: `5;n`, or `2;r;g;b`.
///
/// The colon form may hold a colour space before the red, as in `2::r:g:b`.
fn extended(parts: &[Option<u32>]) -> Option<Color> {
    let byte = |v: Option<u32>| v.unwrap_or(0).min(255) as u8;
    match parts.first().copied().flatten() {
        Some(5) => parts.get(1).map(|&n| Color::Index(byte(n))),
        Some(2) => {
            let rgb = if parts.len() >= 5 {
                &parts[2..5]
            } else {
                parts.get(1..4)?
            };
            Some(Color::Rgb(byte(rgb[0]), byte(rgb[1]), byte(rgb[2])))
        }
        _ => None,
    }
}

/// The character of the DEC line drawing set that stands for `c`.
fn line_drawing(c: char) -> char {
    match c {
        '`' => '◆',
        'a' => '▒',
        'f' => '°',
        'g' => '±',
        'j' => '┘',
        'k' => '┐',
        'l' => '┌',
        'm' => '└',
        'n' => '┼',
        'o' => '⎺',
        'p' => '⎻',
        'q' => '─',
        'r' => '⎼',
        's' => '⎽',
        't' => '├',
        'u' => '┤',
        'v' => '┴',
        'w' => '┬',
        'x' => '│',
        'y' => '≤',
        'z' => '≥',
        '{' => 'π',
        '|' => '≠',
        '}' => '£',
        '~' => '·',
        c => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The text of the rows, without the spaces at their end.
    fn text(screen: &Screen) -> Vec<String> {
        screen
            .lines()
            .iter()
            .map(|line| {
                line.iter()
                    .filter(|c| !c.is_continuation())
                    .map(|c| c.ch)
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    fn screen(cols: usize, rows: usize, input: &str) -> Screen {
        let mut s = Screen::new(cols, rows);
        s.feed(input);
        s
    }

    #[test]
    fn text_wraps_and_scrolls() {
        let s = screen(4, 2, "abcdef\r\ngh");
        assert_eq!(text(&s), ["ef", "gh"]);
        assert!(!s.addressed());
    }

    #[test]
    fn the_cursor_moves_and_erases() {
        let s = screen(
            6,
            3,
            "\x1b[2;3Hx\x1b[1;1Habc\x1b[2D\x1b[K\x1b[3;1Hzzzz\x1b[1G\x1b[2X",
        );
        assert_eq!(text(&s), ["a", "  x", "  zz"]);
        assert!(s.addressed());
        let s = screen(4, 3, "aaaa\r\nbbbb\r\ncccc\x1b[2;2H\x1b[1J");
        assert_eq!(text(&s), ["", "  bb", "cccc"]);
        let s = screen(4, 2, "ab\x1b[2Jx");
        assert_eq!(text(&s), ["  x", ""]);
    }

    #[test]
    fn up_down_forward_back_and_absolute_moves() {
        let s = screen(
            5,
            3,
            "\x1b[3B\x1b[9Cx\x1b[A\x1b[3Dy\x1b[1;1H\x1b[2;3fz\x1b[3dw",
        );
        assert_eq!(text(&s), ["", " yz", "   wx"]);
        let s = screen(5, 2, "a\x1b[3G\x1b[4`b\x1b[2Ec");
        assert_eq!(text(&s), ["a  b", "c"]);
    }

    #[test]
    fn lines_and_characters_are_inserted_and_deleted() {
        let s = screen(5, 3, "abcde\x1b[1;2H\x1b[2@");
        assert_eq!(text(&s), ["a  bc", "", ""]);
        let s = screen(5, 3, "abcde\x1b[1;2H\x1b[2P");
        assert_eq!(text(&s), ["ade", "", ""]);
        let s = screen(3, 3, "1\r\n2\r\n3\x1b[1;1H\x1b[L");
        assert_eq!(text(&s), ["", "1", "2"]);
        let s = screen(3, 3, "1\r\n2\r\n3\x1b[1;1H\x1b[M");
        assert_eq!(text(&s), ["2", "3", ""]);
    }

    #[test]
    fn the_scroll_region_holds_the_scrolls() {
        let s = screen(3, 4, "top\x1b[2;3r\x1b[2;1Ha\r\nb\r\nc\x1b[4;1Hend");
        assert_eq!(text(&s), ["top", "b", "c", "end"]);
        let s = screen(3, 4, "1\r\n2\r\n3\r\n4\x1b[2;3r\x1b[2;1H\x1bM");
        assert_eq!(text(&s), ["1", "", "2", "4"]);
        let s = screen(3, 3, "1\r\n2\r\n3\x1b[S");
        assert_eq!(text(&s), ["2", "3", ""]);
        let s = screen(3, 3, "1\r\n2\r\n3\x1b[T");
        assert_eq!(text(&s), ["", "1", "2"]);
        let s = screen(3, 3, "1\x1bE2\x1bD3");
        assert_eq!(text(&s), ["1", "2", " 3"]);
    }

    #[test]
    fn the_alternate_screen_keeps_the_main_one() {
        let s = screen(5, 2, "$ top\x1b[?1049h\x1b[Hbusy\x1b[?1049l");
        assert_eq!(text(&s), ["$ top", ""]);
        assert_eq!(s.cursor(), Some((0, 4)));
        let s = screen(5, 2, "$ top\x1b[?1049h\x1b[Hbusy");
        assert_eq!(text(&s), ["busy", ""]);
        let s = screen(5, 2, "a\x1b[?47hb\x1b[?47l");
        assert_eq!(text(&s), ["a", ""]);
    }

    #[test]
    fn the_look_follows_the_sgr_parameters() {
        let s = screen(
            8,
            1,
            "\x1b[1;31ma\x1b[38;5;200mb\x1b[38;2;1;2;3mc\x1b[48:2::4:5:6md\x1b[0;7;4me\x1b[22;27;24;2mf\x1b[93;104mg",
        );
        let cells = &s.lines()[0];
        assert_eq!(cells[0].style.fg, Color::Index(1));
        assert!(cells[0].style.bold);
        assert_eq!(cells[1].style.fg, Color::Index(200));
        assert_eq!(cells[2].style.fg, Color::Rgb(1, 2, 3));
        assert_eq!(cells[3].style.bg, Color::Rgb(4, 5, 6));
        assert_eq!(cells[3].style.fg, Color::Rgb(1, 2, 3));
        assert!(cells[4].style.reverse && cells[4].style.underline);
        assert_eq!(cells[4].style.fg, Color::Default);
        assert!(cells[5].style.dim && !cells[5].style.reverse);
        assert_eq!(cells[6].style.fg, Color::Index(11));
        assert_eq!(cells[6].style.bg, Color::Index(12));
    }

    #[test]
    fn an_erase_takes_the_background() {
        let s = screen(3, 1, "\x1b[44m\x1b[K");
        assert!(s.lines()[0].iter().all(|c| c.style.bg == Color::Index(4)));
    }

    #[test]
    fn wide_characters_take_2_cells() {
        let s = screen(5, 2, "a日本");
        assert_eq!(text(&s), ["a日本", ""]);
        assert!(s.lines()[0][2].is_continuation());
        // A wide character that does not fit at the end of a row goes to the next one.
        let s = screen(4, 2, "abc日");
        assert_eq!(text(&s), ["abc", "日"]);
        // Writing over half of a wide character erases the other half.
        let s = screen(4, 1, "日本\x1b[1;2Hx");
        assert_eq!(text(&s), [" x本"]);
        // A combining mark joins the character before it.
        let s = screen(4, 1, "e\u{301}x");
        assert_eq!(s.lines()[0][0].extra.as_deref(), Some("\u{301}"));
        assert_eq!(s.lines()[0][1].ch, 'x');
    }

    #[test]
    fn the_saved_cursor_comes_back() {
        let s = screen(5, 2, "ab\x1b7\x1b[2;4Hx\x1b8c\x1b[s\x1b[2;1Hy\x1b[uz");
        assert_eq!(text(&s), ["abcz", "y  x"]);
    }

    #[test]
    fn lines_are_drawn_with_the_line_set() {
        let s = screen(4, 1, "\x1b(0lqqk\x1b(B");
        assert_eq!(text(&s), ["┌──┐"]);
        let s = screen(4, 1, "\x1b)0a\x0eq\x0fq");
        assert_eq!(text(&s), ["a─q"]);
    }

    #[test]
    fn the_strings_of_a_terminal_are_left_out() {
        let s = screen(
            6,
            1,
            "\x1b]0;title\x07a\x1b]8;;http://x\x1b\\b\x1bPq#0\x1b\\c",
        );
        assert_eq!(text(&s), ["abc"]);
    }

    #[test]
    fn a_resize_keeps_what_fits() {
        let mut s = screen(4, 2, "abcd\r\nefgh");
        s.resize(2, 3);
        assert_eq!(text(&s), ["ab", "ef", ""]);
        assert_eq!(s.size(), (2, 3));
    }

    #[test]
    fn the_hidden_cursor_is_none() {
        assert_eq!(screen(3, 1, "\x1b[?25l").cursor(), None);
        assert_eq!(screen(3, 1, "ab").cursor(), Some((0, 2)));
    }

    #[test]
    fn repeat_and_tabs() {
        let s = screen(20, 1, "x\x1b[3b\tz\x1b[Zy");
        assert_eq!(text(&s), ["xxxx    y"]);
    }
}
