//! The app's `Env`: the real clock and `$HOME`, but no processes (architecture D6).
//!
//! The app never runs git or gh. A core path that would is given `Unsupported` and
//! returns its usual `Error`, which reaches the UI as a `Failure`.

use std::io::{Error, ErrorKind, Result};
use std::time::SystemTime;

use daiku_core::{Clock, Cmd, Dirs, Env, Event, Events, Output, Process};

struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
    fn utc_offset(&self, at: SystemTime) -> i32 {
        daiku_core::env::system_utc_offset(at)
    }
}

/// Runs nothing: every call is refused.
pub struct NoProcess;

fn refused() -> Error {
    Error::new(ErrorKind::Unsupported, "daiku-app runs no processes")
}

impl Process for NoProcess {
    fn output(&self, _cmd: &Cmd) -> Result<Output> {
        Err(refused())
    }
    fn run_attached(&self, _cmd: &Cmd) -> Result<i32> {
        Err(refused())
    }
}

/// The app shows no progress yet.
struct NoEvents;

impl Events for NoEvents {
    fn emit(&self, _event: Event) {}
}

/// `$HOME` for the home folder; no XDG folder; no working folder.
fn dirs() -> Dirs {
    Dirs {
        home: std::env::var_os("HOME")
            .filter(|h| !h.is_empty())
            .map(Into::into),
        xdg_config_home: None,
        cwd: None,
    }
}

pub fn app_env() -> Env {
    Env {
        clock: Box::new(SystemClock),
        process: Box::new(NoProcess),
        events: Box::new(NoEvents),
        dirs: dirs(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use daiku_core::Workspace;
    use daiku_core::scaffold::InitOptions;

    #[test]
    fn no_process_refuses_both_calls() {
        let cmd = Cmd::new("git");
        assert_eq!(
            NoProcess.output(&cmd).unwrap_err().kind(),
            ErrorKind::Unsupported
        );
        assert_eq!(
            NoProcess.run_attached(&cmd).unwrap_err().kind(),
            ErrorKind::Unsupported
        );
    }

    #[test]
    fn init_with_git_runs_no_process() {
        let dir = tempfile::TempDir::new().unwrap();
        let ws = Workspace::open(dir.path(), app_env());
        let err = ws
            .init(&InitOptions {
                no_git: false,
                force: false,
                vcs: "git".into(),
                version: "dev".into(),
            })
            .unwrap_err();
        assert!(
            err.to_string().contains("daiku-app runs no processes"),
            "{err}"
        );
        assert!(!dir.path().join(".git").exists());
    }
}
