//! Terminal support for Stroid.
//!
//! * [`Terminal`] / [`Screen`]: an xterm-compatible escape sequence parser and
//!   cell grid (colors, scrollback, alternate screen, scroll regions).
//! * [`Pty`] / [`Session`]: a child process attached to a pseudo-terminal, with
//!   a background thread feeding its output into a [`Terminal`].
//! * [`input`]: translation of keys and paste into the bytes a program expects.

pub mod input;
mod pty;
pub mod screen;
mod session;

pub use pty::{Pty, SpawnConfig};
pub use screen::{Cell, Color, Screen, Terminal};
pub use session::Session;
