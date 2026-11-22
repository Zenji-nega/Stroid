//! xterm-style terminal screen model driven by a `vte` parser.
//!
//! Known limitations (documented on purpose, to be addressed later):
//! zero-width / combining characters are dropped, there is no bidirectional
//! text shaping, and resizing does not reflow wrapped lines.

use std::collections::VecDeque;
use unicode_width::UnicodeWidthChar;
use vte::{Params, Parser, Perform};

pub const DEFAULT_SCROLLBACK: usize = 5000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    Default,
    /// 0-7 normal, 8-15 bright, 16-255 the xterm 256-color cube/greys.
    Indexed(u8),
    Rgb(u8, u8, u8),
}

impl Color {
    /// Packs the color into an `i32` for the JNI snapshot:
    /// `-1` default, `0..=255` palette index, `0x01RRGGBB` truecolor.
    pub fn encode(self) -> i32 {
        match self {
            Color::Default => -1,
            Color::Indexed(n) => n as i32,
            Color::Rgb(r, g, b) => (1 << 24) | ((r as i32) << 16) | ((g as i32) << 8) | b as i32,
        }
    }
}

/// Cell attribute bits.
pub mod flags {
    pub const BOLD: u8 = 1;
    pub const DIM: u8 = 2;
    pub const ITALIC: u8 = 4;
    pub const UNDERLINE: u8 = 8;
    pub const INVERSE: u8 = 16;
    pub const STRIKE: u8 = 32;
    /// Second half of a double-width character.
    pub const WIDE_TAIL: u8 = 64;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub fg: Color,
    pub bg: Color,
    pub flags: u8,
}

impl Cell {
    pub const BLANK: Cell = Cell {
        ch: ' ',
        fg: Color::Default,
        bg: Color::Default,
        flags: 0,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Pen {
    fg: Color,
    bg: Color,
    flags: u8,
}

impl Default for Pen {
    fn default() -> Self {
        Pen {
            fg: Color::Default,
            bg: Color::Default,
            flags: 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct SavedCursor {
    row: usize,
    col: usize,
    pen: Pen,
    wrap_pending: bool,
}

pub struct Screen {
    rows: usize,
    cols: usize,
    /// The active grid. While the alternate screen is shown, the primary grid
    /// is parked in `stashed`.
    grid: Vec<Vec<Cell>>,
    stashed: Vec<Vec<Cell>>,
    alt_active: bool,
    scrollback: VecDeque<Vec<Cell>>,
    scrollback_limit: usize,
    row: usize,
    col: usize,
    saved: Option<SavedCursor>,
    pen: Pen,
    scroll_top: usize,
    scroll_bottom: usize,
    wrap_pending: bool,
    autowrap: bool,
    cursor_visible: bool,
    app_cursor: bool,
    bracketed_paste: bool,
    g0_graphics: bool,
    g1_graphics: bool,
    shifted: bool,
    title: String,
    tabs: Vec<bool>,
    responses: Vec<u8>,
    bell_count: u32,
}

impl Screen {
    pub fn new(rows: usize, cols: usize) -> Self {
        let rows = rows.max(1);
        let cols = cols.max(2);
        Screen {
            rows,
            cols,
            grid: vec![vec![Cell::BLANK; cols]; rows],
            stashed: vec![vec![Cell::BLANK; cols]; rows],
            alt_active: false,
            scrollback: VecDeque::new(),
            scrollback_limit: DEFAULT_SCROLLBACK,
            row: 0,
            col: 0,
            saved: None,
            pen: Pen::default(),
            scroll_top: 0,
            scroll_bottom: rows - 1,
            wrap_pending: false,
            autowrap: true,
            cursor_visible: true,
            app_cursor: false,
            bracketed_paste: false,
            g0_graphics: false,
            g1_graphics: false,
            shifted: false,
            title: String::new(),
            tabs: default_tabs(cols),
            responses: Vec::new(),
            bell_count: 0,
        }
    }

    // --------------------------------------------------------------- queries

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    pub fn cursor(&self) -> (usize, usize) {
        (self.row, self.col)
    }

    pub fn cursor_visible(&self) -> bool {
        self.cursor_visible
    }

    /// Whether cursor keys should send `ESC O x` instead of `ESC [ x`.
    pub fn app_cursor_keys(&self) -> bool {
        self.app_cursor
    }

    pub fn bracketed_paste(&self) -> bool {
        self.bracketed_paste
    }

    pub fn alt_screen_active(&self) -> bool {
        self.alt_active
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    /// Increments each time the program rings the bell.
    pub fn bell_count(&self) -> u32 {
        self.bell_count
    }

    /// Number of lines available above the visible screen.
    pub fn scrollback_len(&self) -> usize {
        if self.alt_active {
            0
        } else {
            self.scrollback.len()
        }
    }

    pub fn cell(&self, row: usize, col: usize) -> Cell {
        self.grid
            .get(row)
            .and_then(|r| r.get(col))
            .copied()
            .unwrap_or(Cell::BLANK)
    }

    /// Visible text of one row with trailing blanks removed.
    pub fn row_text(&self, row: usize) -> String {
        row_to_string(self.grid.get(row).map(Vec::as_slice).unwrap_or(&[]))
    }

    /// Whole visible screen, one line per row.
    pub fn text(&self) -> String {
        (0..self.rows)
            .map(|r| self.row_text(r))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Text of a scrollback line (0 = oldest).
    pub fn scrollback_text(&self, idx: usize) -> String {
        self.scrollback
            .get(idx)
            .map(|r| row_to_string(r))
            .unwrap_or_default()
    }

    /// Encodes the viewport as `rows * cols * 4` ints: `[codepoint, fg, bg,
    /// flags]` per cell. `scroll_offset` is how many lines the view is scrolled
    /// back into history (clamped).
    pub fn encode_view(&self, scroll_offset: usize) -> Vec<i32> {
        let sb = self.scrollback_len();
        let offset = scroll_offset.min(sb);
        // Index into the virtual list `scrollback ++ grid` of the first visible row.
        let first = sb - offset;
        let mut out = Vec::with_capacity(self.rows * self.cols * 4);
        for i in 0..self.rows {
            let idx = first + i;
            let row: &[Cell] = if idx < sb {
                &self.scrollback[idx]
            } else {
                &self.grid[idx - sb]
            };
            for c in 0..self.cols {
                let cell = row.get(c).copied().unwrap_or(Cell::BLANK);
                out.push(cell.ch as i32);
                out.push(cell.fg.encode());
                out.push(cell.bg.encode());
                out.push(cell.flags as i32);
            }
        }
        out
    }

    // ------------------------------------------------------------- mutation

    pub fn resize(&mut self, rows: usize, cols: usize) {
        let rows = rows.max(1);
        let cols = cols.max(2);
        if rows == self.rows && cols == self.cols {
            return;
        }

        // Rows: if the cursor would fall off the bottom, scroll content up.
        while self.grid.len() > rows {
            if self.row > rows - 1 {
                let top = self.grid.remove(0);
                if !self.alt_active {
                    self.push_scrollback(top);
                }
                self.row -= 1;
            } else {
                self.grid.pop();
            }
        }
        while self.grid.len() < rows {
            self.grid.push(vec![Cell::BLANK; cols]);
        }
        self.stashed.truncate(rows);
        while self.stashed.len() < rows {
            self.stashed.push(vec![Cell::BLANK; cols]);
        }

        for row in self.grid.iter_mut().chain(self.stashed.iter_mut()) {
            row.resize(cols, Cell::BLANK);
        }

        self.rows = rows;
        self.cols = cols;
        self.scroll_top = 0;
        self.scroll_bottom = rows - 1;
        self.row = self.row.min(rows - 1);
        self.col = self.col.min(cols - 1);
        self.wrap_pending = false;
        self.tabs = default_tabs(cols);
        self.saved = None;
    }

    /// Takes bytes the terminal wants to send back to the program
    /// (cursor position reports, device attributes).
    pub fn take_responses(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.responses)
    }

    fn blank(&self) -> Cell {
        Cell {
            ch: ' ',
            fg: Color::Default,
            bg: self.pen.bg,
            flags: 0,
        }
    }

    fn blank_row(&self) -> Vec<Cell> {
        vec![self.blank(); self.cols]
    }

    fn push_scrollback(&mut self, row: Vec<Cell>) {
        self.scrollback.push_back(row);
        while self.scrollback.len() > self.scrollback_limit {
            self.scrollback.pop_front();
        }
    }

    /// Scrolls rows `top..=bottom` up by `n`; the lines that leave the top of
    /// the primary screen go to the scrollback.
    fn scroll_up_region(&mut self, top: usize, bottom: usize, n: usize) {
        let n = n.min(bottom - top + 1);
        for _ in 0..n {
            let row = self.grid.remove(top);
            if top == 0 && !self.alt_active {
                self.push_scrollback(row);
            }
            let blank = self.blank_row();
            self.grid.insert(bottom, blank);
        }
    }

    fn scroll_down_region(&mut self, top: usize, bottom: usize, n: usize) {
        let n = n.min(bottom - top + 1);
        for _ in 0..n {
            self.grid.remove(bottom);
            let blank = self.blank_row();
            self.grid.insert(top, blank);
        }
    }

    fn linefeed(&mut self) {
        if self.row == self.scroll_bottom {
            self.scroll_up_region(self.scroll_top, self.scroll_bottom, 1);
        } else if self.row + 1 < self.rows {
            self.row += 1;
        }
    }

    fn reverse_index(&mut self) {
        if self.row == self.scroll_top {
            self.scroll_down_region(self.scroll_top, self.scroll_bottom, 1);
        } else if self.row > 0 {
            self.row -= 1;
        }
    }

    fn erase_cells(&mut self, row: usize, from: usize, to: usize) {
        let blank = self.blank();
        let to = to.min(self.cols);
        if from < to {
            self.grid[row][from..to].fill(blank);
        }
        self.fix_wide_boundary(row, from, to);
    }

    /// After erasing `from..to`, drop a dangling half of a wide character.
    fn fix_wide_boundary(&mut self, row: usize, from: usize, to: usize) {
        let blank = self.blank();
        if from > 0 && from < self.cols && self.grid[row][from].flags & flags::WIDE_TAIL != 0 {
            self.grid[row][from - 1] = blank;
        }
        if to < self.cols && self.grid[row][to].flags & flags::WIDE_TAIL != 0 {
            self.grid[row][to] = blank;
        }
    }

    fn graphics_active(&self) -> bool {
        if self.shifted {
            self.g1_graphics
        } else {
            self.g0_graphics
        }
    }

    fn put_char(&mut self, ch: char) {
        let ch = if self.graphics_active() {
            map_graphics(ch)
        } else {
            ch
        };
        let width = ch.width().unwrap_or(0);
        if width == 0 {
            return;
        }

        if self.wrap_pending {
            self.wrap_pending = false;
            self.col = 0;
            self.linefeed();
        }
        if width == 2 && self.col + 1 >= self.cols {
            if self.autowrap {
                self.col = 0;
                self.linefeed();
            } else {
                return;
            }
        }

        let (r, c) = (self.row, self.col);
        // Overwriting one half of a wide character destroys the other half.
        self.fix_wide_boundary(r, c, c + width);

        self.grid[r][c] = Cell {
            ch,
            fg: self.pen.fg,
            bg: self.pen.bg,
            flags: self.pen.flags,
        };
        if width == 2 {
            self.grid[r][c + 1] = Cell {
                ch: ' ',
                fg: self.pen.fg,
                bg: self.pen.bg,
                flags: self.pen.flags | flags::WIDE_TAIL,
            };
        }

        let next = c + width;
        if next >= self.cols {
            self.col = self.cols - 1;
            self.wrap_pending = self.autowrap;
        } else {
            self.col = next;
        }
    }

    fn set_cursor(&mut self, row: usize, col: usize) {
        self.row = row.min(self.rows - 1);
        self.col = col.min(self.cols - 1);
        self.wrap_pending = false;
    }

    fn save_cursor(&mut self) {
        self.saved = Some(SavedCursor {
            row: self.row,
            col: self.col,
            pen: self.pen,
            wrap_pending: self.wrap_pending,
        });
    }

    fn restore_cursor(&mut self) {
        if let Some(s) = self.saved {
            self.row = s.row.min(self.rows - 1);
            self.col = s.col.min(self.cols - 1);
            self.pen = s.pen;
            self.wrap_pending = s.wrap_pending;
        }
    }

    fn enter_alt_screen(&mut self, save_cursor: bool) {
        if self.alt_active {
            return;
        }
        if save_cursor {
            self.save_cursor();
        }
        std::mem::swap(&mut self.grid, &mut self.stashed);
        self.alt_active = true;
        let blank = self.blank_row();
        for row in &mut self.grid {
            row.clone_from(&blank);
        }
    }

    fn leave_alt_screen(&mut self, restore_cursor: bool) {
        if !self.alt_active {
            return;
        }
        std::mem::swap(&mut self.grid, &mut self.stashed);
        self.alt_active = false;
        if restore_cursor {
            self.restore_cursor();
        }
    }

    fn full_reset(&mut self) {
        let (rows, cols) = (self.rows, self.cols);
        *self = Screen::new(rows, cols);
    }

    fn set_mode(&mut self, mode: u16, on: bool) {
        match mode {
            1 => self.app_cursor = on,
            7 => self.autowrap = on,
            25 => self.cursor_visible = on,
            47 | 1047 => {
                if on {
                    self.enter_alt_screen(false)
                } else {
                    self.leave_alt_screen(false)
                }
            }
            1049 => {
                if on {
                    self.enter_alt_screen(true)
                } else {
                    self.leave_alt_screen(true)
                }
            }
            2004 => self.bracketed_paste = on,
            _ => {}
        }
    }

    fn sgr(&mut self, params: &Params) {
        let ps: Vec<&[u16]> = params.iter().collect();
        if ps.is_empty() {
            self.pen = Pen::default();
            return;
        }
        let mut i = 0;
        while i < ps.len() {
            let code = ps[i][0];
            let mut skip = 0;
            match code {
                0 => self.pen = Pen::default(),
                1 => self.pen.flags |= flags::BOLD,
                2 => self.pen.flags |= flags::DIM,
                3 => self.pen.flags |= flags::ITALIC,
                4 | 21 => self.pen.flags |= flags::UNDERLINE,
                7 => self.pen.flags |= flags::INVERSE,
                9 => self.pen.flags |= flags::STRIKE,
                22 => self.pen.flags &= !(flags::BOLD | flags::DIM),
                23 => self.pen.flags &= !flags::ITALIC,
                24 => self.pen.flags &= !flags::UNDERLINE,
                27 => self.pen.flags &= !flags::INVERSE,
                29 => self.pen.flags &= !flags::STRIKE,
                30..=37 => self.pen.fg = Color::Indexed((code - 30) as u8),
                38 => {
                    let (color, used) = extended_color(&ps, i);
                    skip = used;
                    if let Some(c) = color {
                        self.pen.fg = c;
                    }
                }
                39 => self.pen.fg = Color::Default,
                40..=47 => self.pen.bg = Color::Indexed((code - 40) as u8),
                48 => {
                    let (color, used) = extended_color(&ps, i);
                    skip = used;
                    if let Some(c) = color {
                        self.pen.bg = c;
                    }
                }
                49 => self.pen.bg = Color::Default,
                90..=97 => self.pen.fg = Color::Indexed((code - 90 + 8) as u8),
                100..=107 => self.pen.bg = Color::Indexed((code - 100 + 8) as u8),
                _ => {}
            }
            i += 1 + skip;
        }
    }
}

/// Parses `38`/`48` extended colors. Returns the color and how many
/// *additional* parameter slots (semicolon form) were consumed.
fn extended_color(ps: &[&[u16]], i: usize) -> (Option<Color>, usize) {
    let sub = ps[i];
    if sub.len() > 1 {
        // Colon form: 38:5:n or 38:2:[colorspace]:r:g:b
        return match sub[1] {
            5 if sub.len() >= 3 => (Some(Color::Indexed(sub[2].min(255) as u8)), 0),
            2 if sub.len() >= 6 => (Some(rgb(sub[3], sub[4], sub[5])), 0),
            2 if sub.len() == 5 => (Some(rgb(sub[2], sub[3], sub[4])), 0),
            _ => (None, 0),
        };
    }
    let get = |k: usize| ps.get(i + k).map(|s| s[0]);
    match get(1) {
        Some(5) => match get(2) {
            Some(n) => (Some(Color::Indexed(n.min(255) as u8)), 2),
            None => (None, 1),
        },
        Some(2) => match (get(2), get(3), get(4)) {
            (Some(r), Some(g), Some(b)) => (Some(rgb(r, g, b)), 4),
            _ => (None, ps.len().saturating_sub(i + 1)),
        },
        Some(_) => (None, 1),
        None => (None, 0),
    }
}

fn rgb(r: u16, g: u16, b: u16) -> Color {
    Color::Rgb(r.min(255) as u8, g.min(255) as u8, b.min(255) as u8)
}

fn default_tabs(cols: usize) -> Vec<bool> {
    (0..cols).map(|c| c % 8 == 0 && c != 0).collect()
}

fn row_to_string(row: &[Cell]) -> String {
    let mut s: String = row
        .iter()
        .filter(|c| c.flags & flags::WIDE_TAIL == 0)
        .map(|c| c.ch)
        .collect();
    let trimmed = s.trim_end_matches(' ').len();
    s.truncate(trimmed);
    s
}

/// DEC Special Graphics (line drawing) character set.
fn map_graphics(c: char) -> char {
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
        other => other,
    }
}

/// First parameter, treating a missing or zero value as `default`.
fn arg(flat: &[u16], idx: usize, default: usize) -> usize {
    match flat.get(idx) {
        Some(&v) if v != 0 => v as usize,
        _ => default,
    }
}

impl Perform for Screen {
    fn print(&mut self, c: char) {
        self.put_char(c);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            0x07 => self.bell_count = self.bell_count.wrapping_add(1),
            0x08 => {
                self.col = self.col.saturating_sub(1);
                self.wrap_pending = false;
            }
            0x09 => {
                let next = (self.col + 1..self.cols).find(|&c| self.tabs[c]);
                self.col = next.unwrap_or(self.cols - 1);
                self.wrap_pending = false;
            }
            0x0A..=0x0C => {
                self.wrap_pending = false;
                self.linefeed();
            }
            0x0D => {
                self.col = 0;
                self.wrap_pending = false;
            }
            0x0E => self.shifted = true,
            0x0F => self.shifted = false,
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, action: char) {
        let flat: Vec<u16> = params.iter().map(|s| s[0]).collect();

        match (intermediates, action) {
            ([b'?'], 'h') | ([b'?'], 'l') => {
                for &m in &flat {
                    self.set_mode(m, action == 'h');
                }
                return;
            }
            ([b'>'], 'c') => {
                self.responses.extend_from_slice(b"\x1b[>0;276;0c");
                return;
            }
            ([], _) => {}
            _ => return,
        }

        let n = arg(&flat, 0, 1);
        match action {
            'A' => {
                let top = if self.row >= self.scroll_top {
                    self.scroll_top
                } else {
                    0
                };
                self.row = self.row.saturating_sub(n).max(top);
                self.wrap_pending = false;
            }
            'B' | 'e' => {
                let bottom = if self.row <= self.scroll_bottom {
                    self.scroll_bottom
                } else {
                    self.rows - 1
                };
                self.row = (self.row + n).min(bottom);
                self.wrap_pending = false;
            }
            'C' | 'a' => {
                self.col = (self.col + n).min(self.cols - 1);
                self.wrap_pending = false;
            }
            'D' => {
                self.col = self.col.saturating_sub(n);
                self.wrap_pending = false;
            }
            'E' => self.set_cursor((self.row + n).min(self.rows - 1), 0),
            'F' => self.set_cursor(self.row.saturating_sub(n), 0),
            'G' | '`' => self.set_cursor(self.row, n - 1),
            'd' => self.set_cursor(n - 1, self.col),
            'H' | 'f' => self.set_cursor(arg(&flat, 0, 1) - 1, arg(&flat, 1, 1) - 1),
            'J' => {
                let mode = flat.first().copied().unwrap_or(0);
                let (r, c) = (self.row, self.col);
                match mode {
                    0 => {
                        self.erase_cells(r, c, self.cols);
                        for row in r + 1..self.rows {
                            self.erase_cells(row, 0, self.cols);
                        }
                    }
                    1 => {
                        for row in 0..r {
                            self.erase_cells(row, 0, self.cols);
                        }
                        self.erase_cells(r, 0, c + 1);
                    }
                    2 => {
                        for row in 0..self.rows {
                            self.erase_cells(row, 0, self.cols);
                        }
                    }
                    3 => self.scrollback.clear(),
                    _ => {}
                }
            }
            'K' => {
                let mode = flat.first().copied().unwrap_or(0);
                let (r, c) = (self.row, self.col);
                match mode {
                    0 => self.erase_cells(r, c, self.cols),
                    1 => self.erase_cells(r, 0, c + 1),
                    2 => self.erase_cells(r, 0, self.cols),
                    _ => {}
                }
            }
            'L' => {
                if (self.scroll_top..=self.scroll_bottom).contains(&self.row) {
                    self.scroll_down_region(self.row, self.scroll_bottom, n);
                    self.col = 0;
                    self.wrap_pending = false;
                }
            }
            'M' => {
                if (self.scroll_top..=self.scroll_bottom).contains(&self.row) {
                    self.scroll_up_region_no_history(self.row, self.scroll_bottom, n);
                    self.col = 0;
                    self.wrap_pending = false;
                }
            }
            '@' => {
                let (r, c) = (self.row, self.col);
                let n = n.min(self.cols - c);
                let blank = self.blank();
                let row = &mut self.grid[r];
                row.truncate(self.cols - n);
                for _ in 0..n {
                    row.insert(c, blank);
                }
                self.fix_wide_boundary(r, c, c);
                self.wrap_pending = false;
            }
            'P' => {
                let (r, c) = (self.row, self.col);
                let n = n.min(self.cols - c);
                let blank = self.blank();
                let row = &mut self.grid[r];
                row.drain(c..c + n);
                row.resize(self.cols, blank);
                self.fix_wide_boundary(r, c, c);
                self.wrap_pending = false;
            }
            'X' => {
                let (r, c) = (self.row, self.col);
                self.erase_cells(r, c, c + n);
            }
            'S' => self.scroll_up_region(self.scroll_top, self.scroll_bottom, n),
            'T' => self.scroll_down_region(self.scroll_top, self.scroll_bottom, n),
            'm' => self.sgr(params),
            'r' => {
                let top = arg(&flat, 0, 1) - 1;
                let bottom = arg(&flat, 1, self.rows) - 1;
                if top < bottom && bottom < self.rows {
                    self.scroll_top = top;
                    self.scroll_bottom = bottom;
                    self.set_cursor(0, 0);
                }
            }
            's' => self.save_cursor(),
            'u' => self.restore_cursor(),
            'g' => match flat.first().copied().unwrap_or(0) {
                0 => self.tabs[self.col] = false,
                3 => self.tabs.iter_mut().for_each(|t| *t = false),
                _ => {}
            },
            'n' => match flat.first().copied().unwrap_or(0) {
                5 => self.responses.extend_from_slice(b"\x1b[0n"),
                6 => {
                    let reply = format!("\x1b[{};{}R", self.row + 1, self.col + 1);
                    self.responses.extend_from_slice(reply.as_bytes());
                }
                _ => {}
            },
            'c' if flat.first().copied().unwrap_or(0) == 0 => {
                self.responses.extend_from_slice(b"\x1b[?1;2c");
            }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], _ignore: bool, byte: u8) {
        match (intermediates, byte) {
            ([b'('], b'0') => self.g0_graphics = true,
            ([b'('], _) => self.g0_graphics = false,
            ([b')'], b'0') => self.g1_graphics = true,
            ([b')'], _) => self.g1_graphics = false,
            ([], b'7') => self.save_cursor(),
            ([], b'8') => self.restore_cursor(),
            ([], b'D') => {
                self.wrap_pending = false;
                self.linefeed();
            }
            ([], b'E') => {
                self.col = 0;
                self.wrap_pending = false;
                self.linefeed();
            }
            ([], b'M') => {
                self.wrap_pending = false;
                self.reverse_index();
            }
            ([], b'H') => self.tabs[self.col] = true,
            ([], b'c') => self.full_reset(),
            _ => {}
        }
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if let [code, text, ..] = params {
            if *code == b"0" || *code == b"2" {
                self.title = String::from_utf8_lossy(text).into_owned();
            }
        }
    }
}

impl Screen {
    /// Like `scroll_up_region` but lines are discarded, never kept as history
    /// (delete-line inside a region is not scrolling).
    fn scroll_up_region_no_history(&mut self, top: usize, bottom: usize, n: usize) {
        let n = n.min(bottom - top + 1);
        for _ in 0..n {
            self.grid.remove(top);
            let blank = self.blank_row();
            self.grid.insert(bottom, blank);
        }
    }
}

/// A parser plus the screen it drives.
pub struct Terminal {
    parser: Parser,
    screen: Screen,
}

impl Terminal {
    pub fn new(rows: usize, cols: usize) -> Self {
        Terminal {
            parser: Parser::new(),
            screen: Screen::new(rows, cols),
        }
    }

    /// Feeds program output into the emulator.
    pub fn feed(&mut self, bytes: &[u8]) {
        self.parser.advance(&mut self.screen, bytes);
    }

    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    pub fn resize(&mut self, rows: usize, cols: usize) {
        self.screen.resize(rows, cols);
    }

    pub fn take_responses(&mut self) -> Vec<u8> {
        self.screen.take_responses()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn term(rows: usize, cols: usize) -> Terminal {
        Terminal::new(rows, cols)
    }

    fn feed(t: &mut Terminal, s: &str) {
        t.feed(s.as_bytes());
    }

    #[test]
    fn prints_text_and_moves_cursor() {
        let mut t = term(3, 10);
        feed(&mut t, "hello");
        assert_eq!(t.screen().row_text(0), "hello");
        assert_eq!(t.screen().cursor(), (0, 5));
    }

    #[test]
    fn crlf_and_scrolling_fill_scrollback() {
        let mut t = term(3, 10);
        feed(&mut t, "a\r\nb\r\nc\r\nd\r\ne");
        assert_eq!(t.screen().text(), "c\nd\ne");
        assert_eq!(t.screen().scrollback_len(), 2);
        assert_eq!(t.screen().scrollback_text(0), "a");
        assert_eq!(t.screen().scrollback_text(1), "b");
    }

    #[test]
    fn autowrap_uses_deferred_wrap() {
        let mut t = term(3, 4);
        feed(&mut t, "abcd");
        assert_eq!(t.screen().cursor(), (0, 3)); // pending wrap, still on row 0
        feed(&mut t, "e");
        assert_eq!(t.screen().row_text(0), "abcd");
        assert_eq!(t.screen().row_text(1), "e");
    }

    #[test]
    fn no_autowrap_overwrites_last_column() {
        let mut t = term(2, 4);
        feed(&mut t, "\x1b[?7labcdef");
        assert_eq!(t.screen().row_text(0), "abcf");
        assert_eq!(t.screen().row_text(1), "");
    }

    #[test]
    fn cursor_positioning() {
        let mut t = term(5, 10);
        feed(&mut t, "\x1b[3;4HX");
        assert_eq!(t.screen().cell(2, 3).ch, 'X');
        feed(&mut t, "\x1b[2AY");
        assert_eq!(t.screen().cell(0, 4).ch, 'Y');
        feed(&mut t, "\x1b[HZ");
        assert_eq!(t.screen().cell(0, 0).ch, 'Z');
    }

    #[test]
    fn cursor_movement_is_clamped() {
        let mut t = term(3, 5);
        feed(&mut t, "\x1b[99;99H");
        assert_eq!(t.screen().cursor(), (2, 4));
        feed(&mut t, "\x1b[99A\x1b[99D");
        assert_eq!(t.screen().cursor(), (0, 0));
    }

    #[test]
    fn erase_in_line_and_display() {
        let mut t = term(3, 6);
        feed(&mut t, "abcdef\r\nghijkl\r\nmnopqr");
        feed(&mut t, "\x1b[2;3H\x1b[K");
        assert_eq!(t.screen().row_text(1), "gh");
        feed(&mut t, "\x1b[2;3H\x1b[1K");
        assert_eq!(t.screen().row_text(1), "");
        feed(&mut t, "\x1b[2J");
        assert_eq!(t.screen().text(), "\n\n");
    }

    #[test]
    fn erase_below_cursor() {
        let mut t = term(3, 4);
        feed(&mut t, "aaaa\r\nbbbb\r\ncccc\x1b[2;3H\x1b[J");
        assert_eq!(t.screen().text(), "aaaa\nbb\n");
    }

    #[test]
    fn sgr_colors_and_attributes() {
        let mut t = term(2, 10);
        feed(&mut t, "\x1b[1;31;44mA\x1b[0mB");
        let a = t.screen().cell(0, 0);
        assert_eq!(a.fg, Color::Indexed(1));
        assert_eq!(a.bg, Color::Indexed(4));
        assert_eq!(a.flags & flags::BOLD, flags::BOLD);
        let b = t.screen().cell(0, 1);
        assert_eq!((b.fg, b.bg, b.flags), (Color::Default, Color::Default, 0));
    }

    #[test]
    fn sgr_256_and_truecolor_both_syntaxes() {
        let mut t = term(2, 10);
        feed(&mut t, "\x1b[38;5;200mA\x1b[48;2;1;2;3mB\x1b[38:2::9:8:7mC");
        assert_eq!(t.screen().cell(0, 0).fg, Color::Indexed(200));
        assert_eq!(t.screen().cell(0, 1).bg, Color::Rgb(1, 2, 3));
        assert_eq!(t.screen().cell(0, 2).fg, Color::Rgb(9, 8, 7));
    }

    #[test]
    fn sgr_bright_and_resets() {
        let mut t = term(2, 10);
        feed(&mut t, "\x1b[91;102mA\x1b[39;49mB");
        assert_eq!(t.screen().cell(0, 0).fg, Color::Indexed(9));
        assert_eq!(t.screen().cell(0, 0).bg, Color::Indexed(10));
        assert_eq!(t.screen().cell(0, 1).fg, Color::Default);
        assert_eq!(t.screen().cell(0, 1).bg, Color::Default);
    }

    #[test]
    fn color_encoding() {
        assert_eq!(Color::Default.encode(), -1);
        assert_eq!(Color::Indexed(5).encode(), 5);
        assert_eq!(Color::Rgb(1, 2, 3).encode(), 0x0101_0203);
    }

    #[test]
    fn erase_uses_current_background() {
        let mut t = term(2, 4);
        feed(&mut t, "\x1b[41m\x1b[2J");
        assert_eq!(t.screen().cell(1, 3).bg, Color::Indexed(1));
    }

    #[test]
    fn scroll_region_keeps_outside_rows() {
        let mut t = term(5, 4);
        feed(&mut t, "1\r\n2\r\n3\r\n4\r\n5");
        feed(&mut t, "\x1b[2;4r"); // rows 2..4, cursor homes
        feed(&mut t, "\x1b[4;1H\n");
        assert_eq!(t.screen().text(), "1\n3\n4\n\n5");
        assert_eq!(t.screen().scrollback_len(), 0); // region scroll is not history
    }

    #[test]
    fn insert_and_delete_lines() {
        let mut t = term(4, 3);
        feed(&mut t, "a\r\nb\r\nc\r\nd");
        feed(&mut t, "\x1b[2;1H\x1b[L");
        assert_eq!(t.screen().text(), "a\n\nb\nc");
        feed(&mut t, "\x1b[M");
        assert_eq!(t.screen().text(), "a\nb\nc\n");
    }

    #[test]
    fn insert_delete_erase_characters() {
        let mut t = term(2, 6);
        feed(&mut t, "abcdef\x1b[1;3H\x1b[2@");
        assert_eq!(t.screen().row_text(0), "ab  cd");
        feed(&mut t, "\x1b[2P");
        assert_eq!(t.screen().row_text(0), "abcd");
        feed(&mut t, "\x1b[2X");
        assert_eq!(t.screen().row_text(0), "ab");
    }

    #[test]
    fn reverse_index_scrolls_down_at_top() {
        let mut t = term(3, 3);
        feed(&mut t, "a\r\nb\r\nc\x1b[H\x1bM");
        assert_eq!(t.screen().text(), "\na\nb");
    }

    #[test]
    fn alternate_screen_restores_primary_and_cursor() {
        let mut t = term(3, 6);
        feed(&mut t, "main\x1b[?1049h");
        assert!(t.screen().alt_screen_active());
        assert_eq!(t.screen().text(), "\n\n");
        feed(&mut t, "\x1b[Halt");
        assert_eq!(t.screen().row_text(0), "alt");
        assert_eq!(t.screen().scrollback_len(), 0);
        feed(&mut t, "\x1b[?1049l");
        assert!(!t.screen().alt_screen_active());
        assert_eq!(t.screen().row_text(0), "main");
        assert_eq!(t.screen().cursor(), (0, 4));
    }

    #[test]
    fn alt_screen_scrolling_does_not_touch_history() {
        let mut t = term(2, 4);
        feed(&mut t, "\x1b[?1049h1\r\n2\r\n3\r\n4");
        feed(&mut t, "\x1b[?1049l");
        assert_eq!(t.screen().scrollback_len(), 0);
    }

    #[test]
    fn save_and_restore_cursor() {
        let mut t = term(3, 10);
        feed(&mut t, "\x1b[2;5H\x1b7\x1b[1;1H\x1b8X");
        assert_eq!(t.screen().cell(1, 4).ch, 'X');
    }

    #[test]
    fn modes_are_tracked() {
        let mut t = term(2, 4);
        feed(&mut t, "\x1b[?1h\x1b[?2004h\x1b[?25l");
        assert!(t.screen().app_cursor_keys());
        assert!(t.screen().bracketed_paste());
        assert!(!t.screen().cursor_visible());
        feed(&mut t, "\x1b[?1l\x1b[?2004l\x1b[?25h");
        assert!(!t.screen().app_cursor_keys());
        assert!(!t.screen().bracketed_paste());
        assert!(t.screen().cursor_visible());
    }

    #[test]
    fn device_status_and_attribute_responses() {
        let mut t = term(5, 10);
        feed(&mut t, "\x1b[3;7H\x1b[6n");
        assert_eq!(t.take_responses(), b"\x1b[3;7R");
        feed(&mut t, "\x1b[c");
        assert_eq!(t.take_responses(), b"\x1b[?1;2c");
        assert!(t.take_responses().is_empty());
    }

    #[test]
    fn title_via_osc() {
        let mut t = term(2, 10);
        feed(&mut t, "\x1b]0;my title\x07");
        assert_eq!(t.screen().title(), "my title");
        feed(&mut t, "\x1b]2;second\x1b\\");
        assert_eq!(t.screen().title(), "second");
    }

    #[test]
    fn tabs() {
        let mut t = term(2, 20);
        feed(&mut t, "a\tb");
        assert_eq!(t.screen().cell(0, 8).ch, 'b');
    }

    #[test]
    fn backspace_and_bell() {
        let mut t = term(2, 10);
        feed(&mut t, "ab\x08c\x07");
        assert_eq!(t.screen().row_text(0), "ac");
        assert_eq!(t.screen().bell_count(), 1);
    }

    #[test]
    fn wide_characters_take_two_cells() {
        let mut t = term(2, 6);
        feed(&mut t, "a世b");
        assert_eq!(t.screen().cell(0, 1).ch, '世');
        assert_eq!(
            t.screen().cell(0, 2).flags & flags::WIDE_TAIL,
            flags::WIDE_TAIL
        );
        assert_eq!(t.screen().cell(0, 3).ch, 'b');
        assert_eq!(t.screen().row_text(0), "a世b");
    }

    #[test]
    fn wide_character_wraps_when_it_does_not_fit() {
        let mut t = term(2, 3);
        feed(&mut t, "ab世");
        assert_eq!(t.screen().row_text(0), "ab");
        assert_eq!(t.screen().row_text(1), "世");
    }

    #[test]
    fn overwriting_half_of_a_wide_character_clears_the_rest() {
        let mut t = term(2, 6);
        feed(&mut t, "世\x1b[1;2HX");
        assert_eq!(t.screen().cell(0, 0).ch, ' ');
        assert_eq!(t.screen().cell(0, 1).ch, 'X');
    }

    #[test]
    fn utf8_split_across_feeds() {
        let mut t = term(2, 6);
        let bytes = "é".as_bytes();
        t.feed(&bytes[..1]);
        t.feed(&bytes[1..]);
        assert_eq!(t.screen().row_text(0), "é");
    }

    #[test]
    fn escape_sequence_split_across_feeds() {
        let mut t = term(2, 6);
        t.feed(b"\x1b[3");
        t.feed(b"1mX");
        assert_eq!(t.screen().cell(0, 0).fg, Color::Indexed(1));
    }

    #[test]
    fn line_drawing_charset() {
        let mut t = term(2, 6);
        feed(&mut t, "\x1b(0lqk\x1b(Bq");
        assert_eq!(t.screen().row_text(0), "┌─┐q");
    }

    #[test]
    fn full_reset_clears_everything() {
        let mut t = term(2, 6);
        feed(&mut t, "abc\x1b[31m\x1bc");
        assert_eq!(t.screen().text(), "\n");
        feed(&mut t, "x");
        assert_eq!(t.screen().cell(0, 0).fg, Color::Default);
    }

    #[test]
    fn resize_keeps_content_and_clamps_cursor() {
        let mut t = term(4, 8);
        feed(&mut t, "one\r\ntwo\r\nthree\r\nfour");
        t.resize(2, 5);
        assert_eq!(t.screen().rows(), 2);
        assert_eq!(t.screen().cols(), 5);
        assert_eq!(t.screen().text(), "three\nfour");
        assert_eq!(t.screen().scrollback_len(), 2);
        assert!(t.screen().cursor().0 < 2 && t.screen().cursor().1 < 5);
        t.resize(6, 12);
        assert_eq!(t.screen().rows(), 6);
        feed(&mut t, "\r\nx");
        assert_eq!(t.screen().rows(), 6);
    }

    #[test]
    fn encode_view_layout_and_scrollback_window() {
        let mut t = term(2, 3);
        feed(&mut t, "a\r\nb\r\nc"); // history: a ; screen: b, c
        let v = t.screen().encode_view(0);
        assert_eq!(v.len(), 2 * 3 * 4);
        assert_eq!(v[0], 'b' as i32);
        assert_eq!(v[1], -1);
        assert_eq!(v[3], 0);
        assert_eq!(v[3 * 4], 'c' as i32);
        let back = t.screen().encode_view(1);
        assert_eq!(back[0], 'a' as i32);
        assert_eq!(back[3 * 4], 'b' as i32);
        // Offsets beyond the history are clamped.
        assert_eq!(t.screen().encode_view(99), back);
    }
}
