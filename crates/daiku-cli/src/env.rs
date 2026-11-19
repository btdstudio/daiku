//! The real `Env`: the system clock, child processes, and the folders from the
//! process environment (architecture D4). Core never reads these itself.

use std::process::{Command, Stdio};
use std::time::SystemTime;

use daiku_core::{Clock, Cmd, Dirs, Env, Event, Events, Output, Process};

struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

struct ChildProcess;

fn command(cmd: &Cmd) -> Command {
    let mut c = Command::new(&cmd.program);
    c.args(&cmd.args);
    if let Some(dir) = &cmd.dir {
        c.current_dir(dir);
    }
    c.envs(cmd.env.iter().map(|(k, v)| (k, v)));
    c
}

impl Process for ChildProcess {
    fn output(&self, cmd: &Cmd) -> std::io::Result<Output> {
        let out = command(cmd).stdin(Stdio::null()).output()?;
        Ok(Output {
            code: out.status.code(),
            stdout: out.stdout,
            stderr: out.stderr,
        })
    }

    /// Runs attached to the user's terminal, as Go's `commit` and `pr` do.
    fn run_attached(&self, cmd: &Cmd) -> std::io::Result<i32> {
        Ok(command(cmd).status()?.code().unwrap_or(-1))
    }
}

/// The CLI shows no progress yet.
struct NoEvents;

impl Events for NoEvents {
    fn emit(&self, _event: Event) {}
}

/// The folders as Go finds them: `XDG_CONFIG_HOME` as set, and the home folder from
/// `$HOME` (Unix) or `%USERPROFILE%` (Windows), like `os.UserHomeDir`.
fn dirs() -> Dirs {
    let home_var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    Dirs {
        home: std::env::var_os(home_var)
            .filter(|h| !h.is_empty())
            .map(Into::into),
        xdg_config_home: std::env::var_os("XDG_CONFIG_HOME"),
    }
}

pub fn real() -> Env {
    Env {
        clock: Box::new(SystemClock),
        process: Box::new(ChildProcess),
        events: Box::new(NoEvents),
        dirs: dirs(),
    }
}
