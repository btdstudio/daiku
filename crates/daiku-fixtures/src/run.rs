//! The sandbox: temp folders, a fixed environment, an optional git base, and the run.

use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use tempfile::TempDir;

use crate::case::no_symlink;
use crate::observe::{self, Observation};
use crate::{Case, Error, normalize};

/// How long one run may take.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// The marker that keeps an otherwise empty folder in `before/`.
pub const KEEP_MARKER: &str = ".fixture-keep";

const GIT_NAME: &str = "daiku-fixtures";
const GIT_EMAIL: &str = "daiku-fixtures@example.invalid";
const GIT_DATE: &str = "2026-01-01T00:00:00Z";
const BASE_SUBJECT: &str = "fixture: before";

/// The three temp folders of one run. The folders are deleted when this drops.
pub(crate) struct Sandbox {
    _dirs: [TempDir; 3],
    /// Workspace, home and config, canonicalised.
    pub root: PathBuf,
    pub home: PathBuf,
    pub config: PathBuf,
    /// The same three as the OS gave them, before canonicalising.
    pub given: [PathBuf; 3],
}

impl Sandbox {
    fn new() -> Result<Sandbox, Error> {
        let mk = |prefix: &str| {
            tempfile::Builder::new()
                .prefix(prefix)
                .tempdir()
                .map_err(|e| Error::msg(format!("temp folder: {e}")))
        };
        let dirs = [mk("fx-root-")?, mk("fx-home-")?, mk("fx-config-")?];
        // Canonical, so macOS's /var symlink and Windows' 8.3 short names resolve, but
        // without Windows' `\\?\` prefix, which git and Go handle badly.
        let canon = |d: &TempDir| {
            std::fs::canonicalize(d.path())
                .map(strip_verbatim)
                .map_err(|e| Error::at(d.path(), e))
        };
        let (root, home, config) = (canon(&dirs[0])?, canon(&dirs[1])?, canon(&dirs[2])?);
        let given = [
            dirs[0].path().to_path_buf(),
            dirs[1].path().to_path_buf(),
            dirs[2].path().to_path_buf(),
        ];
        Ok(Sandbox {
            _dirs: dirs,
            root,
            home,
            config,
            given,
        })
    }

    /// Replaces `{ROOT}`, `{HOME}` and `{CONFIG}` with this run's paths.
    fn expand(&self, s: &str) -> String {
        s.replace("{ROOT}", &self.root.to_string_lossy())
            .replace("{HOME}", &self.home.to_string_lossy())
            .replace("{CONFIG}", &self.config.to_string_lossy())
    }

    /// The environment every process in the run gets (D7, D8), before the case's own.
    fn base_env(&self, git: bool) -> Vec<(String, OsString)> {
        let mut env: Vec<(String, OsString)> = Vec::new();
        if let Some(path) = std::env::var_os("PATH") {
            env.push(("PATH".into(), path));
        }
        env.push(("HOME".into(), self.home.clone().into()));
        env.push(("XDG_CONFIG_HOME".into(), self.config.clone().into()));
        env.push(("TZ".into(), "UTC".into()));
        env.push(("LANG".into(), "C".into()));
        env.push(("LC_ALL".into(), "C".into()));
        if cfg!(windows) {
            env.push(("USERPROFILE".into(), self.home.clone().into()));
            if let Some(root) = std::env::var_os("SystemRoot") {
                env.push(("SystemRoot".into(), root));
            }
        }
        if git {
            for (k, v) in [
                ("GIT_AUTHOR_NAME", GIT_NAME),
                ("GIT_AUTHOR_EMAIL", GIT_EMAIL),
                ("GIT_AUTHOR_DATE", GIT_DATE),
                ("GIT_COMMITTER_NAME", GIT_NAME),
                ("GIT_COMMITTER_EMAIL", GIT_EMAIL),
                ("GIT_COMMITTER_DATE", GIT_DATE),
                ("GIT_CONFIG_NOSYSTEM", "1"),
            ] {
                env.push((k.into(), v.into()));
            }
        }
        env
    }
}

/// `\\?\C:\x` becomes `C:\x`. Any other path, including a verbatim UNC path
/// (`\\?\UNC\…`), is returned unchanged.
fn strip_verbatim(p: PathBuf) -> PathBuf {
    if let Some(rest) = p.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        let b = rest.as_bytes();
        if b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\' {
            return PathBuf::from(rest);
        }
    }
    p
}

/// What the binary did, before observing the workspace.
pub(crate) struct RawRun {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit: i32,
}

/// Runs one case and returns its normalised observation (design steps 2–7).
pub fn run_case(bin: &Path, case: &Case) -> Result<Observation, Error> {
    run_case_with_timeout(bin, case, DEFAULT_TIMEOUT)
}

/// `run_case` with a chosen timeout. For the runner's own tests.
#[doc(hidden)]
pub fn run_case_with_timeout(
    bin: &Path,
    case: &Case,
    timeout: Duration,
) -> Result<Observation, Error> {
    let bin = std::path::absolute(bin).map_err(|e| Error::at(bin, e))?;
    let start_secs = normalize::unix_now();
    let sb = Sandbox::new()?;
    copy_before(&case.before_dir(), &sb.root)?;
    let env = sb.base_env(case.spec.git);

    let base = if case.spec.git {
        Some(git_base(&sb.root, &env)?)
    } else {
        None
    };

    let mut cmd = Command::new(&bin);
    cmd.args(case.spec.args.iter().map(|a| sb.expand(a)))
        .current_dir(&sb.root)
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        .envs(case.spec.env.iter().map(|(k, v)| (k, sb.expand(v))));
    let raw = spawn(cmd, timeout).map_err(|e| Error::at(&case.dir, e))?;
    let end_secs = normalize::unix_now();

    let commits_env = if case.spec.git { Some(&env[..]) } else { None };
    let obs = observe::observe(&sb.root, raw, base.as_deref(), commits_env)
        .map_err(|e| Error::at(&case.dir, e))?;
    let norm = normalize::Normalizer::new(&sb, &[start_secs, end_secs], &obs.commits);
    Ok(obs.normalized(&norm))
}

/// Copies `before/` into the workspace. Folders holding the keep marker are created;
/// the marker itself isn't copied (D5). A missing `before/` is an empty workspace.
/// `before` itself may not be a symlink, and a symlink inside it must stay inside it.
///
/// Links are checked twice: by their text as they're copied (a clear, early error),
/// then, once everything is copied, by resolving each copy. Only the second catches
/// links that climb out through other links (`d -> .`, `e -> d/..`).
fn copy_before(from: &Path, to: &Path) -> Result<(), Error> {
    if no_symlink(from)?.is_none() {
        return Ok(());
    }
    let mut links = Vec::new();
    copy_dir(from, to, 0, &mut links)?;
    for (src, dst) in links {
        let refuse = |why: String| {
            Err(Error::at(
                &src,
                format!("symlink {why}; a case may not link outside itself"),
            ))
        };
        match std::fs::canonicalize(&dst).map(strip_verbatim) {
            Ok(real) if real.starts_with(to) => {}
            Ok(_) => return refuse("resolves outside before/".into()),
            Err(e) => return refuse(format!("can't be resolved ({e})")),
        }
    }
    Ok(())
}

/// Copies one folder; `depth` is how many folders below `before/` it is. Each link made
/// is pushed onto `links` as (source, copy).
fn copy_dir(
    from: &Path,
    to: &Path,
    depth: usize,
    links: &mut Vec<(PathBuf, PathBuf)>,
) -> Result<(), Error> {
    for entry in std::fs::read_dir(from).map_err(|e| Error::at(from, e))? {
        let entry = entry.map_err(|e| Error::at(from, e))?;
        let src = entry.path();
        let name = entry.file_name();
        if name == KEEP_MARKER {
            continue;
        }
        let dst = to.join(&name);
        let ft = entry.file_type().map_err(|e| Error::at(&src, e))?;
        if ft.is_symlink() {
            let target = std::fs::read_link(&src).map_err(|e| Error::at(&src, e))?;
            if !link_stays_inside(&target, depth) {
                return Err(Error::at(
                    &src,
                    format!(
                        "symlink to {} leaves before/; a case may not link outside itself",
                        target.display()
                    ),
                ));
            }
            symlink(&target, &dst, src.is_dir()).map_err(|e| Error::at(&dst, e))?;
            links.push((src, dst));
        } else if ft.is_dir() {
            std::fs::create_dir(&dst).map_err(|e| Error::at(&dst, e))?;
            copy_dir(&src, &dst, depth + 1, links)?;
        } else {
            std::fs::copy(&src, &dst).map_err(|e| Error::at(&src, e))?;
        }
    }
    Ok(())
}

/// Whether a link `depth` folders below `before/` with this target resolves inside
/// `before/`, judged on the text alone: relative, and never climbing above it.
fn link_stays_inside(target: &Path, depth: usize) -> bool {
    use std::path::Component;
    let mut level = depth as isize;
    for c in target.components() {
        match c {
            Component::Normal(_) => level += 1,
            Component::CurDir => {}
            Component::ParentDir => {
                level -= 1;
                if level < 0 {
                    return false;
                }
            }
            Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    true
}

#[cfg(unix)]
fn symlink(target: &Path, link: &Path, _dir: bool) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn symlink(target: &Path, link: &Path, dir: bool) -> std::io::Result<()> {
    if dir {
        std::os::windows::fs::symlink_dir(target, link)
    } else {
        std::os::windows::fs::symlink_file(target, link)
    }
}

/// Runs git in the workspace with the run's environment and returns its stdout.
pub(crate) fn git(
    root: &Path,
    env: &[(String, OsString)],
    args: &[&str],
) -> Result<String, String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("git {}: {e} (is git installed?)", args.join(" ")))?;
    if !out.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Initialises the workspace repo and commits `before/` as the base (D8).
fn git_base(root: &Path, env: &[(String, OsString)]) -> Result<String, Error> {
    let run = |args: &[&str]| git(root, env, args).map_err(|e| Error::at(root, e));
    run(&["init", "-q", "-b", "main"])?;
    run(&["add", "-A"])?;
    run(&[
        "-c",
        "commit.gpgsign=false",
        "commit",
        "-q",
        "--allow-empty",
        "-m",
        BASE_SUBJECT,
    ])?;
    Ok(run(&["rev-parse", "HEAD"])?.trim().to_string())
}

/// Runs the command with a null stdin, capturing output, killed after `timeout` (D12).
fn spawn(mut cmd: Command, timeout: Duration) -> Result<RawRun, String> {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("start {:?}: {e}", cmd.get_program()))?;
    let reader = |mut r: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = r.read_to_end(&mut buf);
            buf
        })
    };
    let out = reader(Box::new(child.stdout.take().expect("piped stdout")));
    let err = reader(Box::new(child.stderr.take().expect("piped stderr")));
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => break status,
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("timed out after {}s", timeout.as_secs_f32()));
            }
            None => std::thread::sleep(Duration::from_millis(5)),
        }
    };
    Ok(RawRun {
        stdout: out.join().unwrap_or_default(),
        stderr: err.join().unwrap_or_default(),
        exit: status.code().unwrap_or(-1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_verbatim_drive_prefix_only() {
        let s = |p: &str| {
            strip_verbatim(PathBuf::from(p))
                .to_string_lossy()
                .into_owned()
        };
        assert_eq!(s(r"\\?\C:\Users\runneradmin\x"), r"C:\Users\runneradmin\x");
        assert_eq!(s(r"\\?\d:\t"), r"d:\t");
        assert_eq!(s(r"\\?\UNC\server\share"), r"\\?\UNC\server\share");
        assert_eq!(s(r"\\?\C:"), r"\\?\C:");
        assert_eq!(s(r"C:\already\plain"), r"C:\already\plain");
        assert_eq!(s("/private/var/folders/x"), "/private/var/folders/x");
    }

    #[test]
    fn links_must_stay_inside_before() {
        let inside = |t: &str, depth| link_stays_inside(Path::new(t), depth);
        assert!(inside("a.txt", 0));
        assert!(inside("sub/../a.txt", 0));
        assert!(inside("./a", 0));
        assert!(inside("../a.txt", 1));
        assert!(!inside("../a.txt", 0));
        assert!(!inside("sub/../../a", 0));
        assert!(!inside("../../x", 1));
        assert!(!inside("/etc/passwd", 0));
        assert!(!inside("/etc/passwd", 3));
    }

    #[test]
    fn sandbox_paths_have_no_verbatim_prefix() {
        let sb = Sandbox::new().unwrap();
        for p in [&sb.root, &sb.home, &sb.config] {
            assert!(!p.to_string_lossy().starts_with(r"\\?\"), "{}", p.display());
            assert!(p.is_absolute() && p.is_dir(), "{}", p.display());
        }
    }
}
