//! Translating keys and pasted text into the bytes terminal programs expect
//! (xterm conventions).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    Delete,
    Enter,
    Tab,
    Backspace,
    Escape,
    /// Function keys F1..=F12.
    F(u8),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
}

impl Mods {
    pub const NONE: Mods = Mods {
        shift: false,
        alt: false,
        ctrl: false,
    };

    fn any(self) -> bool {
        self.shift || self.alt || self.ctrl
    }

    /// xterm modifier parameter: 1 + shift(1) + alt(2) + ctrl(4).
    fn param(self) -> u8 {
        1 + self.shift as u8 + 2 * self.alt as u8 + 4 * self.ctrl as u8
    }
}

/// `ESC [ <final>` / `ESC [ 1 ; <mod> <final>` / `ESC O <final>`.
fn csi_letter(letter: u8, mods: Mods, ss3: bool) -> Vec<u8> {
    if mods.any() {
        format!("\x1b[1;{}{}", mods.param(), letter as char).into_bytes()
    } else if ss3 {
        vec![0x1b, b'O', letter]
    } else {
        vec![0x1b, b'[', letter]
    }
}

/// `ESC [ <n> ~` / `ESC [ <n> ; <mod> ~`.
fn csi_tilde(n: u8, mods: Mods) -> Vec<u8> {
    if mods.any() {
        format!("\x1b[{};{}~", n, mods.param()).into_bytes()
    } else {
        format!("\x1b[{}~", n).into_bytes()
    }
}

/// Bytes for a non-character key. `app_cursor` is
/// [`Screen::app_cursor_keys`](crate::Screen::app_cursor_keys).
pub fn encode_key(key: Key, mods: Mods, app_cursor: bool) -> Vec<u8> {
    match key {
        Key::Up => csi_letter(b'A', mods, app_cursor),
        Key::Down => csi_letter(b'B', mods, app_cursor),
        Key::Right => csi_letter(b'C', mods, app_cursor),
        Key::Left => csi_letter(b'D', mods, app_cursor),
        Key::Home => csi_letter(b'H', mods, app_cursor),
        Key::End => csi_letter(b'F', mods, app_cursor),
        Key::PageUp => csi_tilde(5, mods),
        Key::PageDown => csi_tilde(6, mods),
        Key::Insert => csi_tilde(2, mods),
        Key::Delete => csi_tilde(3, mods),
        Key::Enter => alt_prefix(b"\r", mods),
        Key::Tab => {
            if mods.shift {
                b"\x1b[Z".to_vec()
            } else {
                alt_prefix(b"\t", mods)
            }
        }
        Key::Backspace => {
            if mods.ctrl {
                vec![0x08]
            } else {
                alt_prefix(&[0x7f], mods)
            }
        }
        Key::Escape => vec![0x1b],
        Key::F(n) => match n {
            1..=4 => {
                let letter = b'P' + (n - 1);
                csi_letter(letter, mods, true)
            }
            5 => csi_tilde(15, mods),
            6 => csi_tilde(17, mods),
            7 => csi_tilde(18, mods),
            8 => csi_tilde(19, mods),
            9 => csi_tilde(20, mods),
            10 => csi_tilde(21, mods),
            11 => csi_tilde(23, mods),
            12 => csi_tilde(24, mods),
            _ => Vec::new(),
        },
    }
}

fn alt_prefix(bytes: &[u8], mods: Mods) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + 1);
    if mods.alt {
        out.push(0x1b);
    }
    out.extend_from_slice(bytes);
    out
}

/// Bytes for a typed character with modifiers (Ctrl combinations become
/// control codes, Alt adds an ESC prefix).
pub fn encode_char(c: char, mods: Mods) -> Vec<u8> {
    let mut out = Vec::new();
    if mods.alt {
        out.push(0x1b);
    }
    if mods.ctrl {
        let code = match c {
            'a'..='z' => Some(c as u8 - b'a' + 1),
            'A'..='Z' => Some(c as u8 - b'A' + 1),
            '@' | ' ' | '2' => Some(0),
            '[' | '3' => Some(0x1b),
            '\\' | '4' => Some(0x1c),
            ']' | '5' => Some(0x1d),
            '^' | '6' => Some(0x1e),
            '_' | '7' | '/' => Some(0x1f),
            '?' | '8' => Some(0x7f),
            _ => None,
        };
        if let Some(b) = code {
            out.push(b);
            return out;
        }
    }
    let mut buf = [0u8; 4];
    out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
    out
}

/// Bytes for pasted text. With bracketed paste the text is wrapped so the
/// program can tell it from typing; otherwise newlines become carriage
/// returns like Enter would send.
pub fn encode_paste(text: &str, bracketed: bool) -> Vec<u8> {
    if bracketed {
        // A pasted end marker must not be able to end the paste early.
        let safe = text.replace("\x1b[201~", "");
        let mut out = Vec::with_capacity(safe.len() + 12);
        out.extend_from_slice(b"\x1b[200~");
        out.extend_from_slice(safe.as_bytes());
        out.extend_from_slice(b"\x1b[201~");
        out
    } else {
        text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NO: Mods = Mods::NONE;

    fn ctrl() -> Mods {
        Mods {
            ctrl: true,
            ..Mods::NONE
        }
    }

    fn alt() -> Mods {
        Mods {
            alt: true,
            ..Mods::NONE
        }
    }

    #[test]
    fn arrows_normal_and_application_mode() {
        assert_eq!(encode_key(Key::Up, NO, false), b"\x1b[A");
        assert_eq!(encode_key(Key::Up, NO, true), b"\x1bOA");
        assert_eq!(encode_key(Key::Left, NO, false), b"\x1b[D");
    }

    #[test]
    fn modified_arrows_use_csi_parameters() {
        assert_eq!(encode_key(Key::Right, ctrl(), false), b"\x1b[1;5C");
        assert_eq!(encode_key(Key::Right, ctrl(), true), b"\x1b[1;5C");
        assert_eq!(encode_key(Key::Left, alt(), false), b"\x1b[1;3D");
        let shift = Mods { shift: true, ..NO };
        assert_eq!(encode_key(Key::Up, shift, false), b"\x1b[1;2A");
    }

    #[test]
    fn editing_keys() {
        assert_eq!(encode_key(Key::PageUp, NO, false), b"\x1b[5~");
        assert_eq!(encode_key(Key::Delete, NO, false), b"\x1b[3~");
        assert_eq!(encode_key(Key::Delete, ctrl(), false), b"\x1b[3;5~");
        assert_eq!(encode_key(Key::Home, NO, false), b"\x1b[H");
        assert_eq!(encode_key(Key::End, NO, true), b"\x1bOF");
    }

    #[test]
    fn simple_keys() {
        assert_eq!(encode_key(Key::Enter, NO, false), b"\r");
        assert_eq!(encode_key(Key::Enter, alt(), false), b"\x1b\r");
        assert_eq!(encode_key(Key::Tab, NO, false), b"\t");
        assert_eq!(
            encode_key(Key::Tab, Mods { shift: true, ..NO }, false),
            b"\x1b[Z"
        );
        assert_eq!(encode_key(Key::Backspace, NO, false), [0x7f]);
        assert_eq!(encode_key(Key::Escape, NO, false), [0x1b]);
    }

    #[test]
    fn function_keys() {
        assert_eq!(encode_key(Key::F(1), NO, false), b"\x1bOP");
        assert_eq!(encode_key(Key::F(4), NO, false), b"\x1bOS");
        assert_eq!(encode_key(Key::F(5), NO, false), b"\x1b[15~");
        assert_eq!(encode_key(Key::F(12), NO, false), b"\x1b[24~");
        assert_eq!(encode_key(Key::F(1), ctrl(), false), b"\x1b[1;5P");
        assert!(encode_key(Key::F(13), NO, false).is_empty());
    }

    #[test]
    fn control_characters() {
        assert_eq!(encode_char('c', ctrl()), [3]);
        assert_eq!(encode_char('C', ctrl()), [3]);
        assert_eq!(encode_char('d', ctrl()), [4]);
        assert_eq!(encode_char('[', ctrl()), [0x1b]);
        assert_eq!(encode_char(' ', ctrl()), [0]);
        assert_eq!(encode_char('?', ctrl()), [0x7f]);
    }

    #[test]
    fn alt_and_plain_characters() {
        assert_eq!(encode_char('x', alt()), b"\x1bx");
        assert_eq!(encode_char('x', NO), b"x");
        assert_eq!(encode_char('é', NO), "é".as_bytes());
        assert_eq!(encode_char('1', ctrl()), b"1"); // no control mapping
    }

    #[test]
    fn paste_plain_converts_newlines() {
        assert_eq!(encode_paste("a\nb\r\nc", false), b"a\rb\rc");
    }

    #[test]
    fn paste_bracketed_wraps_and_sanitizes() {
        assert_eq!(encode_paste("hi", true), b"\x1b[200~hi\x1b[201~");
        assert_eq!(
            encode_paste("a\x1b[201~rm -rf", true),
            b"\x1b[200~arm -rf\x1b[201~"
        );
    }
}
