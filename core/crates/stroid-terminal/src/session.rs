//! A running terminal: child process + PTY + emulator, with a background
//! thread that moves the program's output into the emulator.

use crate::pty::{Pty, SpawnConfig};
use crate::screen::{Screen, Terminal};
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

struct Shared {
    term: Mutex<Terminal>,
    /// Bumped whenever the screen may have changed; the UI polls it.
    generation: AtomicU64,
    /// Set once the PTY reports end of output.
    closed: AtomicBool,
}

fn lock(m: &Mutex<Terminal>) -> MutexGuard<'_, Terminal> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

pub struct Session {
    pty: Pty,
    shared: Arc<Shared>,
}

impl Session {
    pub fn spawn(cfg: &SpawnConfig) -> io::Result<Session> {
        let pty = Pty::spawn(cfg)?;
        let mut reader = pty.try_clone_master()?;
        let mut writer = pty.try_clone_master()?;
        let shared = Arc::new(Shared {
            term: Mutex::new(Terminal::new(cfg.rows as usize, cfg.cols as usize)),
            generation: AtomicU64::new(1),
            closed: AtomicBool::new(false),
        });

        let thread_shared = Arc::clone(&shared);
        std::thread::Builder::new()
            .name("stroid-pty-reader".into())
            .spawn(move || {
                let mut buf = [0u8; 16 * 1024];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            let responses = {
                                let mut term = lock(&thread_shared.term);
                                term.feed(&buf[..n]);
                                term.take_responses()
                            };
                            thread_shared.generation.fetch_add(1, Ordering::Release);
                            if !responses.is_empty() {
                                let _ = writer.write_all(&responses);
                            }
                        }
                        Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                        // EIO: the child side was closed.
                        Err(_) => break,
                    }
                }
                thread_shared.closed.store(true, Ordering::Release);
                thread_shared.generation.fetch_add(1, Ordering::Release);
            })?;

        Ok(Session { pty, shared })
    }

    /// Sends bytes (already encoded keys/paste) to the program.
    pub fn write(&self, data: &[u8]) -> io::Result<()> {
        self.pty.write_all(data)
    }

    pub fn resize(&self, rows: u16, cols: u16) -> io::Result<()> {
        self.pty.resize(rows, cols)?;
        lock(&self.shared.term).resize(rows as usize, cols as usize);
        self.shared.generation.fetch_add(1, Ordering::Release);
        Ok(())
    }

    /// Runs `f` with the current screen. Keep it short: output processing is
    /// blocked meanwhile.
    pub fn with_screen<R>(&self, f: impl FnOnce(&Screen) -> R) -> R {
        let term = lock(&self.shared.term);
        f(term.screen())
    }

    /// Changes whenever the screen may have changed.
    pub fn generation(&self) -> u64 {
        self.shared.generation.load(Ordering::Acquire)
    }

    /// True once the program closed its output (normally: it exited).
    pub fn is_closed(&self) -> bool {
        self.shared.closed.load(Ordering::Acquire)
    }

    pub fn exit_code(&self) -> Option<i32> {
        self.pty.exit_code()
    }

    pub fn pid(&self) -> u32 {
        self.pty.pid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn sh(script: &str, rows: u16, cols: u16) -> Session {
        let mut cfg = SpawnConfig::new("/bin/sh");
        cfg.args = vec!["-c".into(), script.into()];
        cfg.env = vec![("TERM".into(), "xterm-256color".into())];
        cfg.rows = rows;
        cfg.cols = cols;
        Session::spawn(&cfg).expect("spawn")
    }

    fn wait_for(session: &Session, needle: &str) -> bool {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if session.with_screen(|s| s.text().contains(needle)) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    #[test]
    fn captures_program_output() {
        let s = sh("printf 'hello pty\\n'", 24, 80);
        assert!(
            wait_for(&s, "hello pty"),
            "screen: {:?}",
            s.with_screen(|x| x.text())
        );
    }

    #[test]
    fn child_sees_a_real_terminal_with_the_requested_size() {
        let s = sh("stty size", 30, 100);
        assert!(
            wait_for(&s, "30 100"),
            "screen: {:?}",
            s.with_screen(|x| x.text())
        );
    }

    #[test]
    fn resize_reaches_the_child() {
        let s = sh("read x; stty size", 24, 80);
        s.resize(40, 120).unwrap();
        s.write(b"go\n").unwrap();
        assert!(
            wait_for(&s, "40 120"),
            "screen: {:?}",
            s.with_screen(|x| x.text())
        );
    }

    #[test]
    fn interactive_input_round_trip() {
        let s = sh("read a; echo result=$((a*2))", 24, 80);
        s.write(b"21\n").unwrap();
        assert!(
            wait_for(&s, "result=42"),
            "screen: {:?}",
            s.with_screen(|x| x.text())
        );
    }

    #[test]
    fn reports_exit_and_code() {
        let s = sh("exit 3", 24, 80);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !s.is_closed() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(s.is_closed());
        let mut code = s.exit_code();
        while code.is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
            code = s.exit_code();
        }
        assert_eq!(code, Some(3));
    }

    #[test]
    fn generation_advances_with_output() {
        let s = sh("printf x", 24, 80);
        let g0 = s.generation();
        assert!(wait_for(&s, "x"));
        assert!(s.generation() > g0);
    }

    #[test]
    fn terminal_queries_are_answered() {
        // The program asks for the cursor position (ESC [ 6 n) and reads the
        // reply ESC [ 1 ; 1 R (6 bytes) back from the terminal, dumped as hex.
        let s = sh(
            "stty raw -echo; printf '\\033[6n'; dd bs=1 count=6 2>/dev/null | od -An -tx1",
            24,
            80,
        );
        assert!(
            wait_for(&s, "1b 5b 31 3b 31 52"),
            "screen: {:?}",
            s.with_screen(|x| x.text())
        );
    }

    #[test]
    fn dropping_a_session_kills_the_child() {
        let s = sh("sleep 60", 24, 80);
        let pid = s.pid() as i32;
        drop(s);
        // SAFETY: probing for process existence with signal 0.
        let alive = unsafe { libc::kill(pid, 0) } == 0;
        assert!(!alive, "child {pid} still running");
    }

    #[test]
    fn missing_program_is_an_error() {
        let cfg = SpawnConfig::new("/definitely/not/a/program");
        assert!(Session::spawn(&cfg).is_err());
    }
}
