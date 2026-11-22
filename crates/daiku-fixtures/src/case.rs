//! Fixture cases: a folder holding `case.toml` (inputs and the `[expect]` table), and
//! optionally `before/`, `config/`, `stage/` and `expect/` (the files a run writes).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::Error;

/// An operating system a case can be skipped on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Os {
    Macos,
    Linux,
    Windows,
}

impl Os {
    /// The OS this binary was built for, if it's one cases can name.
    pub fn current() -> Option<Os> {
        match std::env::consts::OS {
            "macos" => Some(Os::Macos),
            "linux" => Some(Os::Linux),
            "windows" => Some(Os::Windows),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Os::Macos => "macos",
            Os::Linux => "linux",
            Os::Windows => "windows",
        }
    }
}

/// The contents of `case.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseSpec {
    /// Arguments passed to the binary. `{ROOT}`, `{HOME}` and `{CONFIG}` expand.
    pub args: Vec<String>,
    /// Extra environment variables. Values expand like `args`.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Compare stdout lines as a sorted list.
    #[serde(default)]
    pub unordered_stdout: bool,
    /// Run in a git repo and record `git-log`.
    #[serde(default)]
    pub git: bool,
    /// Git cases only: the run's repo gets this `origin` remote after the base
    /// commit (`git remote add origin`). The one remote setting a case can make.
    #[serde(default)]
    pub origin: Option<String>,
    /// Operating systems the case doesn't run on.
    #[serde(default)]
    pub skip_on: Vec<Os>,
    /// Recorded differences from the Go CLI, as free text.
    #[serde(default)]
    pub deviations: String,
    /// A shared starting workspace under `<cases>/../workspaces/`, copied in before
    /// the case's own `before/` and `config/`.
    #[serde(default)]
    pub workspace: Option<String>,
    /// When set, `PATH` holds only these tools, resolved from the caller's `PATH`.
    #[serde(default)]
    pub tools: Vec<String>,
    /// The expected result, written by `capture`; `None` until then.
    #[serde(default)]
    pub expect: Option<Expect>,
}

/// A case's expected result: the `[expect]` table at the end of `case.toml`, which
/// `capture` writes. Empty parts are left out.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expect {
    pub exit: i32,
    #[serde(default)]
    pub stdout: String,
    #[serde(default)]
    pub stderr: String,
    /// stdout when it isn't valid UTF-8, in the escaped form (`\\`, `\xNN`);
    /// then `stdout` is empty.
    #[serde(default)]
    pub stdout_bytes: String,
    /// stderr when it isn't valid UTF-8, as `stdout_bytes`.
    #[serde(default)]
    pub stderr_bytes: String,
    /// What the run changed in the workspace; see `Observation::changes`.
    #[serde(default)]
    pub changes: Vec<String>,
    /// For git cases: the log of the commits the run made.
    #[serde(default)]
    pub git_log: String,
}

impl Expect {
    /// The expected stdout's bytes, from whichever form is stored.
    pub fn stdout_bytes(&self) -> Result<Vec<u8>, String> {
        either(&self.stdout, &self.stdout_bytes, "stdout")
    }

    /// The expected stderr's bytes, from whichever form is stored.
    pub fn stderr_bytes(&self) -> Result<Vec<u8>, String> {
        either(&self.stderr, &self.stderr_bytes, "stderr")
    }
}

/// `text`, or the bytes `escaped` stands for; never both.
fn either(text: &str, escaped: &str, name: &str) -> Result<Vec<u8>, String> {
    match (text.is_empty(), escaped.is_empty()) {
        (_, true) => Ok(text.as_bytes().to_vec()),
        (true, false) => crate::unescape(escaped).map_err(|e| format!("{name}_bytes: {e}")),
        (false, false) => Err(format!("set {name} or {name}_bytes, not both")),
    }
}

/// The line that starts a case's expectation in `case.toml`.
pub const EXPECT_HEADER: &str = "[expect]";

/// A loaded case.
#[derive(Debug, Clone)]
pub struct Case {
    pub name: String,
    pub dir: PathBuf,
    pub spec: CaseSpec,
    /// The named workspace's folder, which holds `tree/` and maybe `config/`.
    pub workspace_dir: Option<PathBuf>,
    /// `case.toml` up to its `[expect]` table: the inputs, kept as written.
    pub inputs: String,
}

impl Case {
    /// The starting workspace. A missing folder means an empty workspace, since git
    /// can't store an empty one.
    pub fn before_dir(&self) -> PathBuf {
        self.dir.join("before")
    }

    /// The files the run added or changed, at their paths. A case whose run writes
    /// no file has no `expect/`.
    pub fn expect_dir(&self) -> PathBuf {
        self.dir.join("expect")
    }

    /// Files for the run's global config folder (`XDG_CONFIG_HOME`).
    pub fn config_dir(&self) -> PathBuf {
        self.dir.join("config")
    }

    /// Files added to the run's repo and staged (`git add`) after the base commit,
    /// for git cases.
    pub fn stage_dir(&self) -> PathBuf {
        self.dir.join("stage")
    }

    /// The starting folders in copy order, as (source, goes to config?, label):
    /// the workspace's `tree/` and `config/`, then the case's `before/` and `config/`.
    pub fn sources(&self) -> Vec<(PathBuf, bool, String)> {
        let mut out = Vec::new();
        if let (Some(w), Some(name)) = (&self.workspace_dir, &self.spec.workspace) {
            out.push((w.join("tree"), false, format!("workspaces/{name}/tree")));
            out.push((w.join("config"), true, format!("workspaces/{name}/config")));
        }
        out.push((self.before_dir(), false, "before".into()));
        out.push((self.config_dir(), true, "config".into()));
        out
    }

    /// The OS the case is skipped on here, if any.
    pub fn skipped_here(&self) -> Option<Os> {
        let os = Os::current()?;
        self.spec.skip_on.contains(&os).then_some(os)
    }

    /// Loads one case folder.
    ///
    /// The case folder, `case.toml`, `before` and `expect` may not be symlinks: a
    /// committed case must not reach outside itself.
    pub fn load(dir: &Path) -> Result<Case, Error> {
        if no_symlink(dir)?.is_none() {
            return Err(Error::at(dir, "no such case"));
        }
        let file = dir.join("case.toml");
        no_symlink(&file)?;
        let text = std::fs::read_to_string(&file).map_err(|e| Error::at(&file, e))?;
        let spec: CaseSpec = toml::from_str(&text).map_err(|e| Error::at(&file, e))?;
        let inputs = split_inputs(&text).map_err(|e| Error::at(&file, e))?;
        if let Some(e) = &spec.expect {
            e.stdout_bytes().map_err(|m| Error::at(&file, m))?;
            e.stderr_bytes().map_err(|m| Error::at(&file, m))?;
        }
        for sub in ["before", "expect", "config", "stage"] {
            folder_or_none(&dir.join(sub))?;
        }
        if !spec.git && dir.join("stage").exists() {
            return Err(Error::at(
                &dir.join("stage"),
                "stage/ needs git = true: it's staged after the base commit",
            ));
        }
        for key in spec.env.keys() {
            reserved_env(key).map_err(|e| Error::at(&file, e))?;
        }
        if let Some(url) = &spec.origin {
            if !spec.git {
                return Err(Error::at(&file, "origin needs git = true"));
            }
            origin_url(url).map_err(|e| Error::at(&file, format!("origin: {e}")))?;
        }
        no_git_config(&dir.join("config"))?;
        for tool in &spec.tools {
            plain_name(tool).map_err(|e| Error::at(&file, format!("tools: {e}")))?;
        }
        let workspace_dir = match &spec.workspace {
            None => None,
            Some(name) => {
                plain_name(name).map_err(|e| Error::at(&file, format!("workspace: {e}")))?;
                let cases = dir.parent().unwrap_or(Path::new("."));
                let wdir = cases.join("..").join("workspaces").join(name);
                let tree = wdir.join("tree");
                if no_symlink(&wdir)?.is_none() || folder_or_none(&tree)?.is_none() {
                    return Err(Error::at(
                        &file,
                        format!("workspace {name:?} has no {}", tree.display()),
                    ));
                }
                folder_or_none(&wdir.join("config"))?;
                no_git_config(&wdir.join("config"))?;
                Some(wdir)
            }
        };
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(Case {
            name,
            dir: dir.to_path_buf(),
            spec,
            workspace_dir,
            inputs,
        })
    }
}

/// Loads every case under `dir` (folders holding `case.toml`), sorted by name, or
/// only the named ones when `names` isn't empty.
pub fn load_cases(dir: &Path, names: &[String]) -> Result<Vec<Case>, Error> {
    if !names.is_empty() {
        return names
            .iter()
            .map(|n| {
                let d = dir.join(n);
                if d.join("case.toml").is_file() {
                    Case::load(&d)
                } else {
                    Err(Error::at(&d, "no such case"))
                }
            })
            .collect();
    }
    let entries = std::fs::read_dir(dir).map_err(|e| Error::at(dir, e))?;
    let mut dirs = Vec::new();
    for entry in entries {
        let path = entry.map_err(|e| Error::at(dir, e))?.path();
        if path.join("case.toml").is_file() {
            dirs.push(path);
        }
    }
    dirs.sort();
    dirs.iter().map(|d| Case::load(d)).collect()
}

/// The metadata of `path` without following a symlink, `None` if it doesn't exist, or
/// an error if it is a symlink.
pub(crate) fn no_symlink(path: &Path) -> Result<Option<std::fs::Metadata>, Error> {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => Err(Error::at(
            path,
            "is a symlink; a case may not link outside itself",
        )),
        Ok(m) => Ok(Some(m)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::at(path, e)),
    }
}

/// The metadata of a folder that may be missing; an error if it's a symlink or a file.
pub(crate) fn folder_or_none(path: &Path) -> Result<Option<std::fs::Metadata>, Error> {
    match no_symlink(path)? {
        Some(m) if !m.is_dir() => Err(Error::at(path, "not a folder")),
        m => Ok(m),
    }
}

/// A workspace or tool name: one plain path component.
pub(crate) fn plain_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\\', ':'])
        || name.contains('\0')
    {
        return Err(format!("{name:?} must be a plain name"));
    }
    Ok(())
}

/// Refuses an `origin` that could reach git as anything but a URL: empty, an
/// option (`-…`), control characters, or a remote helper (`ext::…`), which a
/// fetch would run as a command.
pub(crate) fn origin_url(url: &str) -> Result<(), String> {
    if url.is_empty() {
        return Err("must not be empty".into());
    }
    if url.starts_with('-') {
        return Err(format!("{url:?} must not start with '-'"));
    }
    if url.chars().any(char::is_control) {
        return Err(format!("{url:?} must not hold control characters"));
    }
    if let Some((scheme, _)) = url.split_once("::")
        && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"+.-".contains(&b))
    {
        return Err(format!("{url:?} is a remote helper"));
    }
    Ok(())
}

/// Refuses a case `env` key that could redirect a process: the variables the runner
/// sets, and git's, the dynamic loader's and macOS's loader variables.
pub(crate) fn reserved_env(key: &str) -> Result<(), String> {
    const FIXED: [&str; 8] = [
        "PATH",
        "HOME",
        "XDG_CONFIG_HOME",
        "USERPROFILE",
        "SYSTEMROOT",
        "TZ",
        "LANG",
        "LC_ALL",
    ];
    let upper = key.to_ascii_uppercase();
    let prefixed = ["GIT_", "LD_", "DYLD_", "XDG_"]
        .iter()
        .any(|p| upper.starts_with(p));
    if FIXED.contains(&upper.as_str()) || prefixed || key.is_empty() || key.contains('=') {
        return Err(format!(
            "env: {key:?} can't be set by a case (the runner sets it, or it could redirect git or the loader)"
        ));
    }
    Ok(())
}

/// Refuses a `git` folder (any case) in a config folder: it would be git's global
/// config, which can name commands for git to run. `config/` is for daiku's own
/// settings.
pub(crate) fn no_git_config(config: &Path) -> Result<(), Error> {
    let Ok(entries) = std::fs::read_dir(config) else {
        return Ok(());
    };
    for entry in entries {
        let entry = entry.map_err(|e| Error::at(config, e))?;
        if entry
            .file_name()
            .to_string_lossy()
            .eq_ignore_ascii_case("git")
        {
            return Err(Error::at(
                &entry.path(),
                "config/ may not hold git's config; it's for daiku's settings",
            ));
        }
    }
    Ok(())
}

/// The part of `case.toml` before its `[expect]` header line. `capture` replaces
/// everything from that line on, and checks the inputs read back unchanged.
pub(crate) fn split_inputs(text: &str) -> Result<String, String> {
    let mut inputs = String::new();
    for line in text.split_inclusive('\n') {
        if line.trim() == EXPECT_HEADER {
            return Ok(inputs);
        }
        inputs.push_str(line);
    }
    Ok(inputs)
}
