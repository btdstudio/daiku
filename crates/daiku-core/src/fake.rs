//! Replaceable parts of an [`Env`] for tests: a fixed clock, a scripted process
//! runner and an event recorder. They're ordinary code so the CLI's tests can use
//! them too.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::time::SystemTime;

use crate::env::{Clock, Cmd, Dirs, Env, Event, Events, Output, Process};

/// A clock that always returns the same time.
pub struct FixedClock(pub SystemTime);

impl Clock for FixedClock {
    fn now(&self) -> SystemTime {
        self.0
    }
}

/// A process runner that records each command and replies from a queue. With an
/// empty queue it fails the way a missing program does.
#[derive(Default)]
pub struct ScriptedProcess {
    pub calls: RefCell<Vec<Cmd>>,
    pub replies: RefCell<VecDeque<Output>>,
}

impl ScriptedProcess {
    pub fn reply(&self, output: Output) {
        self.replies.borrow_mut().push_back(output);
    }

    fn next(&self, cmd: &Cmd) -> std::io::Result<Output> {
        self.calls.borrow_mut().push(cmd.clone());
        self.replies.borrow_mut().pop_front().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("no scripted reply for {}", cmd.program),
            )
        })
    }
}

impl Process for ScriptedProcess {
    fn output(&self, cmd: &Cmd) -> std::io::Result<Output> {
        self.next(cmd)
    }

    fn run_attached(&self, cmd: &Cmd) -> std::io::Result<i32> {
        self.next(cmd).map(|o| o.code.unwrap_or(-1))
    }
}

/// Events, kept in order.
#[derive(Default)]
pub struct RecordingEvents(pub RefCell<Vec<Event>>);

impl Events for RecordingEvents {
    fn emit(&self, event: Event) {
        self.0.borrow_mut().push(event);
    }
}

impl Env {
    /// An `Env` for tests: the Unix epoch, no scripted processes, recorded events,
    /// and the given folders.
    pub fn fake(dirs: Dirs) -> Env {
        Env {
            clock: Box::new(FixedClock(SystemTime::UNIX_EPOCH)),
            process: Box::new(ScriptedProcess::default()),
            events: Box::new(RecordingEvents::default()),
            dirs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripted_process_records_and_replies() {
        let p = ScriptedProcess::default();
        p.reply(Output {
            code: Some(0),
            stdout: b"main\n".to_vec(),
            stderr: Vec::new(),
        });
        let cmd = Cmd::new("git").args(["branch", "--show-current"]).dir("/w");
        let out = p.output(&cmd).unwrap();
        assert!(out.success());
        assert_eq!(out.stdout, b"main\n");
        assert_eq!(p.calls.borrow().as_slice(), std::slice::from_ref(&cmd));
        let err = p.run_attached(&cmd).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn fake_env_uses_the_given_dirs() {
        let dirs = Dirs {
            home: Some("/h".into()),
            xdg_config_home: None,
        };
        let env = Env::fake(dirs.clone());
        assert_eq!(env.dirs, dirs);
        assert_eq!(env.clock.now(), SystemTime::UNIX_EPOCH);
    }
}
