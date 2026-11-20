//! What varies between runs and clients, passed in by the caller (ADR-0003, D4).
//!
//! Core never reads the process environment, the real clock or spawns a process
//! itself: it asks the `Env`. The CLI supplies real implementations; tests use the
//! ones in [`crate::fake`].

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::SystemTime;

/// Everything core needs from outside itself.
pub struct Env {
    pub clock: Box<dyn Clock>,
    pub process: Box<dyn Process>,
    pub events: Box<dyn Events>,
    pub dirs: Dirs,
}

/// The current time.
pub trait Clock {
    fn now(&self) -> SystemTime;
    /// The local zone's offset from UTC at `at`, in seconds east, for the dates
    /// daiku writes (`date::today`).
    fn utc_offset(&self, at: SystemTime) -> i32;
}

/// The system zone's UTC offset at `at`, in seconds east, or 0 (UTC) when the
/// zone can't be read, as Go falls back. A helper for clients' clocks: core's
/// operations never call it, so they read no zone of their own (ADR-0003).
pub fn system_utc_offset(at: SystemTime) -> i32 {
    jiff::Timestamp::try_from(at)
        .map(|ts| jiff::tz::TimeZone::system().to_offset(ts).seconds())
        .unwrap_or(0)
}

/// Runs external programs (git, gh).
pub trait Process {
    /// Runs `cmd` to completion and returns what it printed.
    fn output(&self, cmd: &Cmd) -> std::io::Result<Output>;
    /// Runs `cmd` attached to the user's terminal (for `commit` and `pr`) and
    /// returns its exit code.
    fn run_attached(&self, cmd: &Cmd) -> std::io::Result<i32>;
}

/// A program to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cmd {
    pub program: String,
    pub args: Vec<String>,
    /// The folder to run in; `None` is the caller's current folder.
    pub dir: Option<PathBuf>,
    /// Variables added to the inherited environment.
    pub env: Vec<(String, String)>,
}

impl Cmd {
    pub fn new(program: impl Into<String>) -> Cmd {
        Cmd {
            program: program.into(),
            args: Vec::new(),
            dir: None,
            env: Vec::new(),
        }
    }

    pub fn args<I, S>(mut self, args: I) -> Cmd
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn dir(mut self, dir: impl Into<PathBuf>) -> Cmd {
        self.dir = Some(dir.into());
        self
    }
}

/// What a finished program printed, and how it exited.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Output {
    /// The exit code; `None` when it was killed by a signal.
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl Output {
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

/// How Go words a failed `exec.Cmd.Run` or `Output`: `exit status 1`,
/// `signal: killed`, or, for a program that isn't there,
/// `exec: "git": executable file not found in $PATH`.
pub(crate) fn run_failure(program: &str, result: &std::io::Result<Output>) -> Option<String> {
    match result {
        Ok(out) => match out.code {
            Some(0) => None,
            Some(c) => Some(format!("exit status {c}")),
            None => Some("signal: killed".to_string()),
        },
        Err(e) => Some(spawn_failure(program, e)),
    }
}

/// The text for a program that couldn't start.
pub(crate) fn spawn_failure(program: &str, e: &std::io::Error) -> String {
    if e.kind() == std::io::ErrorKind::NotFound {
        let path = if cfg!(windows) { "%PATH%" } else { "$PATH" };
        format!("exec: \"{program}\": executable file not found in {path}")
    } else {
        e.to_string()
    }
}

/// Where core reports progress while an operation runs.
pub trait Events {
    fn emit(&self, event: Event);
}

/// Progress from an operation. Variants arrive with the operations that report them.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[non_exhaustive]
pub enum Event {
    /// `repo doctor --clone` is about to clone `repo` into `dest`.
    CloneStarted { repo: String, dest: String },
    /// That clone finished; `error` is why it failed, if it did.
    CloneFinished {
        repo: String,
        dest: String,
        error: Option<String>,
    },
}

/// The user's home and config folders, as the client found them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Dirs {
    /// The home folder (`$HOME`, or `%USERPROFILE%` on Windows).
    pub home: Option<PathBuf>,
    /// `$XDG_CONFIG_HOME` as set, even when empty.
    pub xdg_config_home: Option<OsString>,
    /// The folder the client runs in, for making a relative path absolute as Go's
    /// `filepath.Abs` does. Core never reads the process's own.
    pub cwd: Option<PathBuf>,
}
