//! Text editor engine: rope buffer, multiple selections, undo/redo, search.
//!
//! * Positions are `char` offsets into the document.
//! * Lines are separated by `\n` only. A trailing `\r` is treated as ordinary
//!   text but is excluded from the "content length" of a line, so cursors never
//!   land between `\r` and `\n`.
//! * There is always at least one selection. Selections are kept sorted and
//!   non-overlapping.

use regex::{Regex, RegexBuilder};
use ropey::Rope;
use std::io::{self, Read, Write};
use std::ops::Range;
use std::path::Path;

const MAX_HISTORY: usize = 1000;

/// A selection. `anchor == head` means a plain caret.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sel {
    pub anchor: usize,
    pub head: usize,
    /// Remembered column for consecutive vertical moves.
    goal: Option<usize>,
}

impl Sel {
    pub fn new(anchor: usize, head: usize) -> Self {
        Self {
            anchor,
            head,
            goal: None,
        }
    }

    pub fn caret(pos: usize) -> Self {
        Self::new(pos, pos)
    }

    pub fn start(&self) -> usize {
        self.anchor.min(self.head)
    }

    pub fn end(&self) -> usize {
        self.anchor.max(self.head)
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }
}

/// Cursor movements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Movement {
    Left,
    Right,
    /// Move by N lines (negative = up). Used for arrows and page up/down.
    Vertical(i32),
    /// Smart home: first non-blank character, then column 0.
    LineStart,
    LineEnd,
    WordLeft,
    WordRight,
    DocStart,
    DocEnd,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SearchOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchError(pub String);

impl std::fmt::Display for SearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid search pattern: {}", self.0)
    }
}

impl std::error::Error for SearchError {}

#[derive(Clone, Debug)]
struct Edit {
    start: usize,
    removed: String,
    inserted: String,
}

#[derive(Clone, Debug)]
struct Transaction {
    id: u64,
    /// In the order they were applied to the rope.
    edits: Vec<Edit>,
    before: Vec<Sel>,
    after: Vec<Sel>,
    /// Plain typing that later typing may be merged into.
    typing: bool,
}

pub struct Editor {
    rope: Rope,
    sels: Vec<Sel>,
    undo: Vec<Transaction>,
    redo: Vec<Transaction>,
    next_id: u64,
    saved_id: u64,
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor {
    pub fn new() -> Self {
        Self::from_rope(Rope::new())
    }

    pub fn from_text(text: &str) -> Self {
        Self::from_rope(Rope::from_str(text))
    }

    /// Reads UTF-8 text. Fails with `InvalidData` for other encodings.
    pub fn from_reader<R: Read>(reader: R) -> io::Result<Self> {
        Ok(Self::from_rope(Rope::from_reader(reader)?))
    }

    pub fn open(path: &Path) -> io::Result<Self> {
        Self::from_reader(std::fs::File::open(path)?)
    }

    fn from_rope(rope: Rope) -> Self {
        Self {
            rope,
            sels: vec![Sel::caret(0)],
            undo: Vec::new(),
            redo: Vec::new(),
            next_id: 0,
            saved_id: 0,
        }
    }

    pub fn write_to<W: Write>(&self, writer: W) -> io::Result<()> {
        self.rope.write_to(writer)
    }

    /// Writes the document to `path` and marks it as saved.
    pub fn save(&mut self, path: &Path) -> io::Result<()> {
        let file = std::fs::File::create(path)?;
        self.write_to(io::BufWriter::new(file))?;
        self.mark_saved();
        Ok(())
    }

    pub fn mark_saved(&mut self) {
        if let Some(last) = self.undo.last_mut() {
            last.typing = false;
        }
        self.saved_id = self.current_id();
    }

    pub fn is_modified(&self) -> bool {
        self.current_id() != self.saved_id
    }

    fn current_id(&self) -> u64 {
        self.undo.last().map_or(0, |t| t.id)
    }

    // ---------------------------------------------------------------- reading

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    /// Text between two char offsets (clamped).
    pub fn slice(&self, start: usize, end: usize) -> String {
        let len = self.rope.len_chars();
        let (s, e) = (start.min(len), end.min(len));
        if s >= e {
            return String::new();
        }
        self.rope.slice(s..e).to_string()
    }

    /// Content of a line without its line break. Empty if out of range.
    pub fn line_text(&self, line: usize) -> String {
        if line >= self.rope.len_lines() {
            return String::new();
        }
        let start = self.rope.line_to_char(line);
        let end = start + line_content_len(&self.rope, line);
        self.rope.slice(start..end).to_string()
    }

    /// `count` consecutive lines starting at `first`, for rendering.
    pub fn lines(&self, first: usize, count: usize) -> Vec<String> {
        let last = (first + count).min(self.rope.len_lines());
        (first..last).map(|l| self.line_text(l)).collect()
    }

    pub fn line_len(&self, line: usize) -> usize {
        if line >= self.rope.len_lines() {
            0
        } else {
            line_content_len(&self.rope, line)
        }
    }

    pub fn pos_to_line_col(&self, pos: usize) -> (usize, usize) {
        line_col(&self.rope, pos.min(self.rope.len_chars()))
    }

    /// Char offset for a line/column; the column is clamped to the line.
    pub fn line_col_to_pos(&self, line: usize, col: usize) -> usize {
        let line = line.min(self.rope.len_lines() - 1);
        self.rope.line_to_char(line) + col.min(line_content_len(&self.rope, line))
    }

    pub fn selections(&self) -> &[Sel] {
        &self.sels
    }

    pub fn selected_text(&self) -> String {
        self.sels
            .iter()
            .filter(|s| !s.is_empty())
            .map(|s| self.rope.slice(s.start()..s.end()).to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    // ---------------------------------------------------------------- editing

    /// Replaces every selection with `text` (typing, paste).
    pub fn insert(&mut self, text: &str) {
        let typing = !text.contains('\n') && self.sels.iter().all(Sel::is_empty);
        let changes = self
            .sels
            .iter()
            .map(|s| (s.start()..s.end(), text.to_string()))
            .collect();
        self.apply_changes(changes, typing, true);
    }

    /// Enter key: inserts a line break and copies the current indentation.
    pub fn insert_newline(&mut self) {
        let changes = self
            .sels
            .iter()
            .map(|s| {
                let line = self.rope.char_to_line(s.start());
                let line_start = self.rope.line_to_char(line);
                let indent: String = self
                    .rope
                    .slice(line_start..s.start())
                    .chars()
                    .take_while(|c| *c == ' ' || *c == '\t')
                    .collect();
                (s.start()..s.end(), format!("\n{indent}"))
            })
            .collect();
        self.apply_changes(changes, false, true);
    }

    /// Backspace.
    pub fn delete_backward(&mut self) {
        let changes = self
            .sels
            .iter()
            .map(|s| {
                if s.is_empty() {
                    (s.start().saturating_sub(1)..s.start(), String::new())
                } else {
                    (s.start()..s.end(), String::new())
                }
            })
            .collect();
        self.apply_changes(changes, false, true);
    }

    /// Delete key.
    pub fn delete_forward(&mut self) {
        let len = self.rope.len_chars();
        let changes = self
            .sels
            .iter()
            .map(|s| {
                if s.is_empty() {
                    (s.start()..(s.start() + 1).min(len), String::new())
                } else {
                    (s.start()..s.end(), String::new())
                }
            })
            .collect();
        self.apply_changes(changes, false, true);
    }

    /// Replaces an explicit range (used by search/replace UI).
    pub fn replace_range(&mut self, range: Range<usize>, text: &str) {
        let len = self.rope.len_chars();
        let r = range.start.min(len)..range.end.min(len);
        self.apply_changes(vec![(r, text.to_string())], false, false);
    }

    /// Applies `changes` (ascending, non-overlapping) as one undo step.
    /// Afterwards each change leaves a caret after its inserted text
    /// (`multi`), or only the first one does.
    fn apply_changes(&mut self, changes: Vec<(Range<usize>, String)>, typing: bool, multi: bool) {
        if changes.iter().all(|(r, t)| r.is_empty() && t.is_empty()) {
            return;
        }
        let before = self.sels.clone();

        let mut carets = Vec::with_capacity(changes.len());
        let mut delta: isize = 0;
        for (r, t) in &changes {
            let new_len = t.chars().count();
            let start = (r.start as isize + delta) as usize;
            carets.push(start + new_len);
            delta += new_len as isize - (r.end - r.start) as isize;
        }

        let mut edits = Vec::with_capacity(changes.len());
        // Apply from the end so earlier offsets stay valid.
        for (r, t) in changes.into_iter().rev() {
            if r.is_empty() && t.is_empty() {
                continue;
            }
            let removed = self.rope.slice(r.clone()).to_string();
            if !r.is_empty() {
                self.rope.remove(r.clone());
            }
            if !t.is_empty() {
                self.rope.insert(r.start, &t);
            }
            edits.push(Edit {
                start: r.start,
                removed,
                inserted: t,
            });
        }

        if !multi {
            carets.truncate(1);
        }
        // Carets can coincide (e.g. backspace between adjacent carets).
        self.sels = normalize(carets.into_iter().map(Sel::caret).collect());
        self.redo.clear();
        self.push_transaction(edits, before, typing);
    }

    fn push_transaction(&mut self, edits: Vec<Edit>, before: Vec<Sel>, typing: bool) {
        let after = self.sels.clone();
        self.next_id += 1;
        let id = self.next_id;

        if typing {
            if let Some(last) = self.undo.last_mut() {
                if last.typing && same_places(&last.after, &before) {
                    last.edits.extend(edits);
                    last.after = after;
                    last.id = id;
                    return;
                }
            }
        }

        self.undo.push(Transaction {
            id,
            edits,
            before,
            after,
            typing,
        });
        if self.undo.len() > MAX_HISTORY {
            self.undo.remove(0);
        }
    }

    pub fn undo(&mut self) -> bool {
        let Some(tx) = self.undo.pop() else {
            return false;
        };
        for e in tx.edits.iter().rev() {
            let n = e.inserted.chars().count();
            if n > 0 {
                self.rope.remove(e.start..e.start + n);
            }
            if !e.removed.is_empty() {
                self.rope.insert(e.start, &e.removed);
            }
        }
        self.sels = tx.before.clone();
        if let Some(last) = self.undo.last_mut() {
            last.typing = false;
        }
        self.redo.push(tx);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(mut tx) = self.redo.pop() else {
            return false;
        };
        for e in &tx.edits {
            let n = e.removed.chars().count();
            if n > 0 {
                self.rope.remove(e.start..e.start + n);
            }
            if !e.inserted.is_empty() {
                self.rope.insert(e.start, &e.inserted);
            }
        }
        self.sels = tx.after.clone();
        tx.typing = false;
        self.undo.push(tx);
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    // -------------------------------------------------------------- selection

    pub fn move_selections(&mut self, movement: Movement, extend: bool) {
        let moved = self
            .sels
            .iter()
            .map(|s| move_sel(&self.rope, *s, movement, extend))
            .collect();
        self.sels = normalize(moved);
    }

    pub fn select_all(&mut self) {
        self.sels = vec![Sel::new(0, self.rope.len_chars())];
    }

    /// Collapses to a single caret.
    pub fn set_cursor(&mut self, pos: usize) {
        self.sels = vec![Sel::caret(pos.min(self.rope.len_chars()))];
    }

    /// Replaces all selections with one range.
    pub fn set_selection(&mut self, anchor: usize, head: usize) {
        let len = self.rope.len_chars();
        self.sels = vec![Sel::new(anchor.min(len), head.min(len))];
    }

    /// Adds an extra caret (long-press on touch screens).
    pub fn add_cursor(&mut self, pos: usize) {
        let mut v = self.sels.clone();
        v.push(Sel::caret(pos.min(self.rope.len_chars())));
        self.sels = normalize(v);
    }

    /// Adds a caret on the line above/below every selection.
    pub fn add_cursor_vertical(&mut self, down: bool) {
        let last_line = self.rope.len_lines() - 1;
        let mut v = self.sels.clone();
        for s in &self.sels {
            let (line, col) = line_col(&self.rope, s.head);
            let goal = s.goal.unwrap_or(col);
            let target = if down {
                if line >= last_line {
                    continue;
                }
                line + 1
            } else {
                if line == 0 {
                    continue;
                }
                line - 1
            };
            let pos =
                self.rope.line_to_char(target) + goal.min(line_content_len(&self.rope, target));
            v.push(Sel {
                anchor: pos,
                head: pos,
                goal: Some(goal),
            });
        }
        self.sels = normalize(v);
    }

    /// Selects the word around `pos`.
    pub fn select_word_at(&mut self, pos: usize) {
        let (s, e) = word_bounds(&self.rope, pos);
        self.sels = vec![Sel::new(s, e)];
    }

    /// "Select next occurrence": a caret first selects its word, later calls
    /// add the next match of the selected text (wrapping around).
    /// Returns `true` if the selection set changed.
    pub fn add_next_occurrence(&mut self) -> bool {
        let last = *self.sels.last().expect("at least one selection");
        if last.is_empty() {
            let (s, e) = word_bounds(&self.rope, last.head);
            if s == e {
                return false;
            }
            *self.sels.last_mut().expect("non-empty") = Sel::new(s, e);
            return true;
        }

        let needle = self.rope.slice(last.start()..last.end()).to_string();
        let text = self.rope.to_string();
        let from = self.rope.char_to_byte(last.end());
        let found = text[from..]
            .find(&needle)
            .map(|i| i + from)
            .or_else(|| text.find(&needle));
        let Some(byte) = found else {
            return false;
        };
        let start = self.rope.byte_to_char(byte);
        if self.sels.iter().any(|s| s.start() == start) {
            return false;
        }
        let mut v = self.sels.clone();
        v.push(Sel::new(start, start + needle.chars().count()));
        self.sels = normalize(v);
        true
    }

    // ----------------------------------------------------------------- search

    /// All non-empty matches as `(start, end)` char ranges.
    pub fn find_all(
        &self,
        query: &str,
        opts: &SearchOptions,
    ) -> Result<Vec<(usize, usize)>, SearchError> {
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let re = build_regex(query, opts)?;
        let text = self.rope.to_string();
        Ok(re
            .find_iter(&text)
            .filter(|m| !m.is_empty())
            .map(|m| {
                (
                    self.rope.byte_to_char(m.start()),
                    self.rope.byte_to_char(m.end()),
                )
            })
            .collect())
    }

    /// Replaces every match in one undo step. Returns the number replaced.
    /// In regex mode `$1`-style group references are expanded.
    pub fn replace_all(
        &mut self,
        query: &str,
        replacement: &str,
        opts: &SearchOptions,
    ) -> Result<usize, SearchError> {
        if query.is_empty() {
            return Ok(0);
        }
        let re = build_regex(query, opts)?;
        let text = self.rope.to_string();
        let mut changes = Vec::new();
        for caps in re.captures_iter(&text) {
            let m = caps.get(0).expect("group 0 always exists");
            if m.is_empty() {
                continue;
            }
            let mut out = String::new();
            if opts.regex {
                caps.expand(replacement, &mut out);
            } else {
                out.push_str(replacement);
            }
            changes.push((
                self.rope.byte_to_char(m.start())..self.rope.byte_to_char(m.end()),
                out,
            ));
        }
        let count = changes.len();
        if count > 0 {
            self.apply_changes(changes, false, false);
        }
        Ok(count)
    }
}

// ------------------------------------------------------------------- helpers

fn same_places(a: &[Sel], b: &[Sel]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| x.anchor == y.anchor && x.head == y.head)
}

fn build_regex(query: &str, opts: &SearchOptions) -> Result<Regex, SearchError> {
    let mut pattern = if opts.regex {
        query.to_string()
    } else {
        regex::escape(query)
    };
    if opts.whole_word {
        pattern = format!(r"\b(?:{pattern})\b");
    }
    RegexBuilder::new(&pattern)
        .case_insensitive(!opts.case_sensitive)
        .multi_line(true)
        .build()
        .map_err(|e| SearchError(e.to_string()))
}

fn line_col(rope: &Rope, pos: usize) -> (usize, usize) {
    let line = rope.char_to_line(pos);
    (line, pos - rope.line_to_char(line))
}

/// Chars in a line excluding `\n` and a `\r` that directly precedes it.
fn line_content_len(rope: &Rope, line: usize) -> usize {
    let l = rope.line(line);
    let mut n = l.len_chars();
    if n > 0 && l.char(n - 1) == '\n' {
        n -= 1;
        if n > 0 && l.char(n - 1) == '\r' {
            n -= 1;
        }
    }
    n
}

fn char_class(c: char) -> u8 {
    if c.is_whitespace() {
        0
    } else if c.is_alphanumeric() || c == '_' {
        1
    } else {
        2
    }
}

fn word_bounds(rope: &Rope, pos: usize) -> (usize, usize) {
    let len = rope.len_chars();
    let pos = pos.min(len);
    let probe = if pos < len && char_class(rope.char(pos)) != 0 {
        pos
    } else if pos > 0 && char_class(rope.char(pos - 1)) != 0 {
        pos - 1
    } else {
        return (pos, pos);
    };
    let class = char_class(rope.char(probe));
    let mut start = probe;
    while start > 0 && char_class(rope.char(start - 1)) == class {
        start -= 1;
    }
    let mut end = probe + 1;
    while end < len && char_class(rope.char(end)) == class {
        end += 1;
    }
    (start, end)
}

fn normalize(mut v: Vec<Sel>) -> Vec<Sel> {
    v.sort_by_key(|s| (s.start(), s.end()));
    let mut out: Vec<Sel> = Vec::with_capacity(v.len());
    for s in v {
        if let Some(last) = out.last_mut() {
            let same = s.start() == last.start() && s.end() == last.end();
            if s.start() < last.end() || same {
                let forward = last.head >= last.anchor;
                let a = last.start().min(s.start());
                let b = last.end().max(s.end());
                let goal = last.goal;
                *last = if forward {
                    Sel {
                        anchor: a,
                        head: b,
                        goal,
                    }
                } else {
                    Sel {
                        anchor: b,
                        head: a,
                        goal,
                    }
                };
                continue;
            }
        }
        out.push(s);
    }
    if out.is_empty() {
        out.push(Sel::caret(0));
    }
    out
}

fn move_sel(rope: &Rope, s: Sel, movement: Movement, extend: bool) -> Sel {
    let len = rope.len_chars();
    let mut goal = None;
    let pos = match movement {
        Movement::Left => {
            if !extend && !s.is_empty() {
                s.start()
            } else {
                s.head.saturating_sub(1)
            }
        }
        Movement::Right => {
            if !extend && !s.is_empty() {
                s.end()
            } else {
                (s.head + 1).min(len)
            }
        }
        Movement::Vertical(n) => {
            let (line, col) = line_col(rope, s.head);
            let g = s.goal.unwrap_or(col);
            goal = Some(g);
            let last = rope.len_lines() - 1;
            let target = line as i64 + n as i64;
            if target < 0 {
                0
            } else if target as usize > last {
                len
            } else {
                let t = target as usize;
                rope.line_to_char(t) + g.min(line_content_len(rope, t))
            }
        }
        Movement::LineStart => {
            let line = rope.char_to_line(s.head);
            let line_start = rope.line_to_char(line);
            let indent = rope
                .line(line)
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .count();
            let first = line_start + indent.min(line_content_len(rope, line));
            if s.head != first {
                first
            } else {
                line_start
            }
        }
        Movement::LineEnd => {
            let line = rope.char_to_line(s.head);
            rope.line_to_char(line) + line_content_len(rope, line)
        }
        Movement::WordLeft => {
            let mut p = s.head;
            while p > 0 && char_class(rope.char(p - 1)) == 0 {
                p -= 1;
            }
            if p > 0 {
                let k = char_class(rope.char(p - 1));
                while p > 0 && char_class(rope.char(p - 1)) == k {
                    p -= 1;
                }
            }
            p
        }
        Movement::WordRight => {
            let mut p = s.head;
            if p < len {
                let k = char_class(rope.char(p));
                if k != 0 {
                    while p < len && char_class(rope.char(p)) == k {
                        p += 1;
                    }
                }
                while p < len && char_class(rope.char(p)) == 0 {
                    p += 1;
                }
            }
            p
        }
        Movement::DocStart => 0,
        Movement::DocEnd => len,
    };
    Sel {
        anchor: if extend { s.anchor } else { pos },
        head: pos,
        goal,
    }
}

// --------------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    fn carets(e: &Editor) -> Vec<usize> {
        e.selections().iter().map(|s| s.head).collect()
    }

    fn ed(text: &str, caret: usize) -> Editor {
        let mut e = Editor::from_text(text);
        e.set_cursor(caret);
        e
    }

    #[test]
    fn insert_and_backspace() {
        let mut e = ed("", 0);
        e.insert("abc");
        assert_eq!(e.text(), "abc");
        e.delete_backward();
        assert_eq!(e.text(), "ab");
        e.delete_forward(); // at end: nothing to delete
        assert_eq!(e.text(), "ab");
        e.set_cursor(0);
        e.delete_forward();
        assert_eq!(e.text(), "b");
    }

    #[test]
    fn backspace_at_start_is_noop() {
        let mut e = ed("x", 0);
        e.delete_backward();
        assert_eq!(e.text(), "x");
        assert!(!e.can_undo());
    }

    #[test]
    fn typing_is_coalesced_into_one_undo_step() {
        let mut e = ed("", 0);
        for c in ["h", "e", "l", "l", "o"] {
            e.insert(c);
        }
        assert_eq!(e.text(), "hello");
        assert!(e.undo());
        assert_eq!(e.text(), "");
        assert!(!e.can_undo());
        assert!(e.redo());
        assert_eq!(e.text(), "hello");
        assert_eq!(carets(&e), vec![5]);
    }

    #[test]
    fn moving_the_cursor_breaks_coalescing() {
        let mut e = ed("", 0);
        e.insert("ab");
        e.set_cursor(0);
        e.insert("x");
        assert_eq!(e.text(), "xab");
        e.undo();
        assert_eq!(e.text(), "ab");
        e.undo();
        assert_eq!(e.text(), "");
    }

    #[test]
    fn typing_after_undo_does_not_merge_into_older_step() {
        let mut e = ed("", 0);
        e.insert("a");
        e.set_cursor(0);
        e.insert("b"); // "ba"
        e.undo(); // "a"
        e.set_cursor(1);
        e.insert("c");
        assert_eq!(e.text(), "ac");
        e.undo();
        assert_eq!(e.text(), "a");
    }

    #[test]
    fn multi_cursor_insert() {
        let mut e = ed("a\nb\nc", 0);
        e.add_cursor(2);
        e.add_cursor(4);
        e.insert("-");
        assert_eq!(e.text(), "-a\n-b\n-c");
        assert_eq!(carets(&e), vec![1, 4, 7]);
        e.undo();
        assert_eq!(e.text(), "a\nb\nc");
        assert_eq!(carets(&e), vec![0, 2, 4]);
    }

    #[test]
    fn multi_cursor_backspace_with_adjacent_carets() {
        let mut e = ed("abcd", 2);
        e.add_cursor(3);
        e.delete_backward();
        assert_eq!(e.text(), "ad");
        assert_eq!(carets(&e), vec![1]); // the two carets merged
        e.undo();
        assert_eq!(e.text(), "abcd");
        assert_eq!(carets(&e), vec![2, 3]);
    }

    #[test]
    fn replace_selection() {
        let mut e = ed("hello world", 0);
        e.set_selection(0, 5);
        e.insert("bye");
        assert_eq!(e.text(), "bye world");
    }

    #[test]
    fn newline_keeps_indentation() {
        let mut e = ed("    foo", 7);
        e.insert_newline();
        assert_eq!(e.text(), "    foo\n    ");
        assert_eq!(carets(&e), vec![12]);
    }

    #[test]
    fn newline_inside_indent_only_copies_up_to_cursor() {
        let mut e = ed("    foo", 2);
        e.insert_newline();
        assert_eq!(e.text(), "  \n    foo");
    }

    #[test]
    fn vertical_movement_remembers_goal_column() {
        let mut e = ed("abcdef\nab\nabcdef", 5);
        e.move_selections(Movement::Vertical(1), false);
        assert_eq!(e.selections()[0].head, 9); // clamped to end of "ab"
        e.move_selections(Movement::Vertical(1), false);
        assert_eq!(e.pos_to_line_col(e.selections()[0].head), (2, 5));
    }

    #[test]
    fn up_on_first_line_goes_to_start_and_down_on_last_to_end() {
        let mut e = ed("ab\ncd", 1);
        e.move_selections(Movement::Vertical(-1), false);
        assert_eq!(carets(&e), vec![0]);
        e.move_selections(Movement::Vertical(5), false);
        assert_eq!(carets(&e), vec![5]);
    }

    #[test]
    fn smart_home_toggles() {
        let mut e = ed("    foo", 7);
        e.move_selections(Movement::LineStart, false);
        assert_eq!(carets(&e), vec![4]);
        e.move_selections(Movement::LineStart, false);
        assert_eq!(carets(&e), vec![0]);
        e.move_selections(Movement::LineStart, false);
        assert_eq!(carets(&e), vec![4]);
    }

    #[test]
    fn line_end_ignores_crlf() {
        let mut e = ed("ab\r\ncd", 0);
        e.move_selections(Movement::LineEnd, false);
        assert_eq!(carets(&e), vec![2]);
        assert_eq!(e.line_text(0), "ab");
        assert_eq!(e.line_len(0), 2);
    }

    #[test]
    fn word_movement() {
        let mut e = ed("foo bar_baz  +qux", 0);
        e.move_selections(Movement::WordRight, false);
        assert_eq!(carets(&e), vec![4]);
        e.move_selections(Movement::WordRight, false);
        assert_eq!(carets(&e), vec![13]);
        e.move_selections(Movement::WordRight, false);
        assert_eq!(carets(&e), vec![14]);
        e.move_selections(Movement::WordLeft, false);
        assert_eq!(carets(&e), vec![13]);
        e.move_selections(Movement::WordLeft, false);
        assert_eq!(carets(&e), vec![4]);
    }

    #[test]
    fn extend_selection_and_collapse() {
        let mut e = ed("hello", 1);
        e.move_selections(Movement::Right, true);
        e.move_selections(Movement::Right, true);
        assert_eq!((e.selections()[0].anchor, e.selections()[0].head), (1, 3));
        assert_eq!(e.selected_text(), "el");
        e.move_selections(Movement::Left, false);
        assert_eq!(carets(&e), vec![1]);
    }

    #[test]
    fn add_cursor_below_and_above() {
        let mut e = ed("abc\nde\nfghi", 2);
        e.add_cursor_vertical(true);
        e.add_cursor_vertical(true);
        // line starts: 0, 4, 7; the short middle line clamps the column
        assert_eq!(carets(&e), vec![2, 6, 9]);
        e.add_cursor_vertical(false);
        assert_eq!(carets(&e), vec![2, 6, 9]); // already present, merged
    }

    #[test]
    fn overlapping_selections_merge() {
        let mut e = ed("abcdef", 0);
        e.set_selection(0, 3);
        let mut v = e.selections().to_vec();
        v.push(Sel::new(2, 5));
        e.sels = normalize(v);
        assert_eq!(e.selections().len(), 1);
        assert_eq!((e.selections()[0].start(), e.selections()[0].end()), (0, 5));
    }

    #[test]
    fn select_word() {
        let mut e = ed("foo bar", 0);
        e.select_word_at(5);
        assert_eq!(e.selected_text(), "bar");
        e.select_word_at(3); // on the space: takes the word before it
        assert_eq!(e.selected_text(), "foo");
    }

    #[test]
    fn add_next_occurrence_selects_word_then_adds_matches() {
        let mut e = ed("foo bar foo baz foo", 1);
        assert!(e.add_next_occurrence());
        assert_eq!(e.selected_text(), "foo");
        assert!(e.add_next_occurrence());
        assert!(e.add_next_occurrence());
        assert_eq!(e.selections().len(), 3);
        assert!(!e.add_next_occurrence()); // wraps to an existing one
        e.insert("X");
        assert_eq!(e.text(), "X bar X baz X");
    }

    #[test]
    fn undo_redo_selection_replace() {
        let mut e = ed("hello world", 0);
        e.set_selection(6, 11);
        e.insert("there");
        assert_eq!(e.text(), "hello there");
        e.undo();
        assert_eq!(e.text(), "hello world");
        assert_eq!((e.selections()[0].anchor, e.selections()[0].head), (6, 11));
        e.redo();
        assert_eq!(e.text(), "hello there");
    }

    #[test]
    fn new_edit_clears_redo() {
        let mut e = ed("", 0);
        e.insert("a");
        e.undo();
        assert!(e.can_redo());
        e.insert("b");
        assert!(!e.can_redo());
    }

    #[test]
    fn modified_tracking() {
        let mut e = ed("", 0);
        assert!(!e.is_modified());
        e.insert("a");
        assert!(e.is_modified());
        e.mark_saved();
        assert!(!e.is_modified());
        e.insert("b"); // must not merge into the saved step
        assert!(e.is_modified());
        e.undo();
        assert!(!e.is_modified());
        e.undo();
        assert!(e.is_modified());
        e.redo();
        assert!(!e.is_modified());
    }

    #[test]
    fn unicode_positions_are_chars() {
        let mut e = ed("a😀b", 0);
        assert_eq!(e.len_chars(), 3);
        e.set_cursor(2);
        e.delete_backward();
        assert_eq!(e.text(), "ab");
        e.insert("é");
        assert_eq!(e.text(), "aéb");
    }

    #[test]
    fn line_access() {
        let e = Editor::from_text("one\ntwo\n\nfour");
        assert_eq!(e.line_count(), 4);
        assert_eq!(e.lines(1, 2), vec!["two", ""]);
        assert_eq!(e.lines(3, 10), vec!["four"]);
        assert_eq!(e.line_text(99), "");
        assert_eq!(e.line_col_to_pos(1, 99), 7);
        assert_eq!(e.pos_to_line_col(5), (1, 1));
    }

    #[test]
    fn trailing_newline_adds_empty_last_line() {
        let e = Editor::from_text("a\n");
        assert_eq!(e.line_count(), 2);
        assert_eq!(e.line_text(1), "");
    }

    #[test]
    fn find_all_literal_and_options() {
        let e = Editor::from_text("Foo foo FOO food");
        let o = SearchOptions::default();
        assert_eq!(e.find_all("foo", &o).unwrap().len(), 4);
        let cs = SearchOptions {
            case_sensitive: true,
            ..o
        };
        assert_eq!(e.find_all("foo", &cs).unwrap(), vec![(4, 7), (12, 15)]);
        let ww = SearchOptions {
            whole_word: true,
            ..o
        };
        assert_eq!(e.find_all("foo", &ww).unwrap().len(), 3);
        assert!(e.find_all("", &o).unwrap().is_empty());
    }

    #[test]
    fn find_all_literal_does_not_interpret_regex() {
        let e = Editor::from_text("a.b axb");
        assert_eq!(
            e.find_all("a.b", &SearchOptions::default()).unwrap(),
            vec![(0, 3)]
        );
    }

    #[test]
    fn find_all_regex_and_invalid_pattern() {
        let e = Editor::from_text("a1 b22 c333");
        let o = SearchOptions {
            regex: true,
            ..Default::default()
        };
        assert_eq!(e.find_all(r"\d+", &o).unwrap().len(), 3);
        assert!(e.find_all("(", &o).is_err());
    }

    #[test]
    fn find_all_offsets_are_chars_with_unicode() {
        let e = Editor::from_text("😀 x 😀 x");
        assert_eq!(
            e.find_all("x", &SearchOptions::default()).unwrap(),
            vec![(2, 3), (6, 7)]
        );
    }

    #[test]
    fn replace_all_is_one_undo_step() {
        let mut e = Editor::from_text("foo bar foo");
        let n = e
            .replace_all("foo", "quux", &SearchOptions::default())
            .unwrap();
        assert_eq!(n, 2);
        assert_eq!(e.text(), "quux bar quux");
        assert_eq!(e.selections().len(), 1);
        e.undo();
        assert_eq!(e.text(), "foo bar foo");
    }

    #[test]
    fn replace_all_regex_groups() {
        let mut e = Editor::from_text("john smith, jane doe");
        let o = SearchOptions {
            regex: true,
            ..Default::default()
        };
        e.replace_all(r"(\w+) (\w+)", "$2 $1", &o).unwrap();
        assert_eq!(e.text(), "smith john, doe jane");
    }

    #[test]
    fn replace_all_literal_does_not_expand_dollars() {
        let mut e = Editor::from_text("a b");
        e.replace_all("a", "$1", &SearchOptions::default()).unwrap();
        assert_eq!(e.text(), "$1 b");
    }

    #[test]
    fn replace_range_works() {
        let mut e = Editor::from_text("hello world");
        e.replace_range(6..11, "rust");
        assert_eq!(e.text(), "hello rust");
        e.undo();
        assert_eq!(e.text(), "hello world");
    }

    #[test]
    fn open_save_roundtrip() {
        let dir = std::env::temp_dir().join(format!("stroid-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.txt");
        let mut e = Editor::from_text("héllo\nwörld\n");
        e.insert("x");
        e.save(&path).unwrap();
        assert!(!e.is_modified());
        let back = Editor::open(&path).unwrap();
        assert_eq!(back.text(), e.text());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn invalid_utf8_is_an_error() {
        let bytes: &[u8] = &[0x66, 0xff, 0xfe];
        assert!(Editor::from_reader(bytes).is_err());
    }

    #[test]
    fn large_file_stays_responsive() {
        let line =
            "the quick brown fox jumps over the lazy dog 0123456789 abcdefghijklmnopqrstuvwxyz\n";
        let text = line.repeat(130_000); // ~10.6 MB
        assert!(text.len() > 10_000_000);

        let start = std::time::Instant::now();
        let mut e = Editor::from_text(&text);
        assert_eq!(e.line_count(), 130_001);

        for i in 0..500usize {
            let line_no = (i * 263) % 130_000;
            let pos = e.line_col_to_pos(line_no, 10);
            e.set_cursor(pos);
            e.insert("x");
            let _ = e.lines(line_no.saturating_sub(20), 40);
        }
        e.set_cursor(0);
        e.insert_newline();
        while e.undo() {}
        assert_eq!(e.len_chars(), text.chars().count());
        // Generous bound for an unoptimized debug build.
        assert!(
            start.elapsed() < std::time::Duration::from_secs(10),
            "took {:?}",
            start.elapsed()
        );
    }
}
