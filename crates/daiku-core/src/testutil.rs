//! Helpers for core's unit tests.

use std::path::{Path, PathBuf};

/// A temporary folder, removed when dropped, so core's tests need no extra crate.
pub struct Temp(pub PathBuf);

impl Temp {
    pub fn new() -> Temp {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!(
            "daiku-core-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&p).unwrap();
        Temp(p)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn p(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }

    /// Writes `rel`, creating its folders.
    pub fn write(&self, rel: &str, data: &str) {
        let p = self.p(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, data).unwrap();
    }

    pub fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.p(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
    }

    pub fn exists(&self, rel: &str) -> bool {
        std::fs::symlink_metadata(self.p(rel)).is_ok()
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Runs programs for real, for tests that need git. Git gets a fixed identity and
/// no user or system config.
pub struct RealProcess;

impl crate::env::Process for RealProcess {
    fn output(&self, cmd: &crate::env::Cmd) -> std::io::Result<crate::env::Output> {
        let mut c = std::process::Command::new(&cmd.program);
        c.args(&cmd.args);
        if let Some(d) = &cmd.dir {
            c.current_dir(d);
        }
        for (k, v) in GIT_ENV {
            c.env(k, v);
        }
        for (k, v) in &cmd.env {
            c.env(k, v);
        }
        let out = c.output()?;
        Ok(crate::env::Output {
            code: out.status.code(),
            stdout: out.stdout,
            stderr: out.stderr,
        })
    }

    fn run_attached(&self, cmd: &crate::env::Cmd) -> std::io::Result<i32> {
        self.output(cmd).map(|o| o.code.unwrap_or(-1))
    }
}

const GIT_ENV: [(&str, &str); 6] = [
    ("GIT_AUTHOR_NAME", "t"),
    ("GIT_AUTHOR_EMAIL", "t@example.com"),
    ("GIT_COMMITTER_NAME", "t"),
    ("GIT_COMMITTER_EMAIL", "t@example.com"),
    ("GIT_CONFIG_NOSYSTEM", "1"),
    ("GIT_CONFIG_GLOBAL", NULL_DEVICE),
];

#[cfg(windows)]
const NULL_DEVICE: &str = "NUL";
#[cfg(not(windows))]
const NULL_DEVICE: &str = "/dev/null";

/// An `Env` for a test workspace in `t`: a fixed clock (2026-11-07T01:02:03.004Z),
/// `home` inside the temp folder, and the given process runner.
pub fn env(t: &Temp, process: Box<dyn crate::env::Process>) -> crate::Env {
    use std::time::{Duration, UNIX_EPOCH};
    let mut e = crate::Env::fake(crate::env::Dirs {
        home: Some(t.p("home")),
        xdg_config_home: None,
        cwd: Some(t.path().to_path_buf()),
    });
    e.clock = Box::new(crate::fake::FixedClock(
        UNIX_EPOCH + Duration::from_millis(1_794_013_323_004),
    ));
    e.process = process;
    e
}

/// Runs git in `dir` for a test, panicking on failure; returns its stdout.
pub fn git(dir: &Path, args: &[&str]) -> String {
    use crate::env::Process;
    let cmd = crate::env::Cmd::new("git")
        .args(args.iter().copied())
        .dir(dir);
    let out = RealProcess.output(&cmd).unwrap();
    assert!(
        out.success(),
        "git {args:?}: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// `t` as a git repo with one commit.
pub fn git_repo(t: &Temp) {
    git(t.path(), &["init", "-q"]);
    t.write("README.md", "hi\n");
    git(t.path(), &["add", "README.md"]);
    git(t.path(), &["commit", "-q", "-m", "init"]);
}
