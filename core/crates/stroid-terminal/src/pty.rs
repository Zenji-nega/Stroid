//! A child process attached to a pseudo-terminal.

use std::ffi::CStr;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

#[derive(Clone, Debug)]
pub struct SpawnConfig {
    pub program: String,
    pub args: Vec<String>,
    /// Added to (or overriding) the inherited environment.
    pub env: Vec<(String, String)>,
    pub cwd: Option<PathBuf>,
    pub rows: u16,
    pub cols: u16,
}

impl SpawnConfig {
    pub fn new(program: impl Into<String>) -> Self {
        SpawnConfig {
            program: program.into(),
            args: Vec::new(),
            env: Vec::new(),
            cwd: None,
            rows: 24,
            cols: 80,
        }
    }
}

pub struct Pty {
    master: File,
    child: Mutex<Child>,
}

fn os_err() -> io::Error {
    io::Error::last_os_error()
}

fn set_winsize(fd: i32, rows: u16, cols: u16) -> io::Result<()> {
    let ws = libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: `fd` is a valid pty master and `ws` outlives the call.
    let rc = unsafe { libc::ioctl(fd, libc::TIOCSWINSZ as _, &ws) };
    if rc < 0 {
        Err(os_err())
    } else {
        Ok(())
    }
}

impl Pty {
    pub fn spawn(cfg: &SpawnConfig) -> io::Result<Pty> {
        // SAFETY: plain libc calls; the returned fd is owned by the `File`.
        let (master, slave_path) = unsafe {
            let fd = libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC);
            if fd < 0 {
                return Err(os_err());
            }
            let master = File::from_raw_fd(fd);
            if libc::grantpt(fd) != 0 || libc::unlockpt(fd) != 0 {
                return Err(os_err());
            }
            let mut buf = [0 as libc::c_char; 128];
            if libc::ptsname_r(fd, buf.as_mut_ptr(), buf.len()) != 0 {
                return Err(os_err());
            }
            let path = CStr::from_ptr(buf.as_ptr())
                .to_str()
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
                .to_owned();
            (master, path)
        };

        set_winsize(master.as_raw_fd(), cfg.rows, cfg.cols)?;

        let slave = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&slave_path)?;

        let mut cmd = Command::new(&cfg.program);
        cmd.args(&cfg.args);
        for (k, v) in &cfg.env {
            cmd.env(k, v);
        }
        if let Some(dir) = &cfg.cwd {
            cmd.current_dir(dir);
        }
        cmd.stdin(Stdio::from(slave.try_clone()?))
            .stdout(Stdio::from(slave.try_clone()?))
            .stderr(Stdio::from(slave));

        // SAFETY: only async-signal-safe calls (setsid, ioctl) run between
        // fork and exec.
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() < 0 {
                    return Err(os_err());
                }
                if libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 {
                    return Err(os_err());
                }
                Ok(())
            });
        }

        let child = cmd.spawn()?;
        Ok(Pty {
            master,
            child: Mutex::new(child),
        })
    }

    /// A second handle to the master side (for a reader thread).
    pub fn try_clone_master(&self) -> io::Result<File> {
        self.master.try_clone()
    }

    pub fn write_all(&self, data: &[u8]) -> io::Result<()> {
        (&self.master).write_all(data)
    }

    pub fn resize(&self, rows: u16, cols: u16) -> io::Result<()> {
        set_winsize(self.master.as_raw_fd(), rows, cols)
    }

    pub fn pid(&self) -> u32 {
        self.child.lock().unwrap_or_else(|p| p.into_inner()).id()
    }

    /// Exit code if the child has finished (`None` while running; killed by a
    /// signal reports `128 + signal`).
    pub fn exit_code(&self) -> Option<i32> {
        use std::os::unix::process::ExitStatusExt;
        let mut child = self.child.lock().unwrap_or_else(|p| p.into_inner());
        match child.try_wait() {
            Ok(Some(status)) => Some(
                status
                    .code()
                    .or_else(|| status.signal().map(|s| 128 + s))
                    .unwrap_or(-1),
            ),
            _ => None,
        }
    }

    /// Kills the whole process group of the child and reaps it.
    pub fn kill(&self) {
        let mut child = self.child.lock().unwrap_or_else(|p| p.into_inner());
        // The child called setsid(), so its pid is also its process group id.
        // SAFETY: signalling a process group we created.
        unsafe {
            libc::kill(-(child.id() as i32), libc::SIGKILL);
        }
        let _ = child.kill();
        let _ = child.wait();
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        self.kill();
    }
}
