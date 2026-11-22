//! Repos: Go's `internal/core/repos` (D9). `REPOS.json` (committed) names each
//! logical repo; `.daiku/paths.json` (local) says where each is checked out. S-0005
//! ported reading them; S-0008 adds changing them and the checks on top.

mod url;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::env::{Cmd, Dirs, Process};
use crate::gojson::decode::{At, Kind, Reader, boolean, fold, mismatch, string, string_map};
use crate::json;
use crate::state::store::{at, read_file, shown};
use crate::write;
use crate::{Error, gofmt, gojson};

/// A logical repo name.
pub static NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z0-9][a-z0-9._-]*$").expect("valid pattern"));

/// One registered repo.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Repo {
    pub remote: String,
    pub default_branch: String,
    /// Retired: kept for its builds' history, but not cloned and not given new
    /// work (N-0008). Written only when set, so a live entry is unchanged.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub archived: bool,
    /// Why, when `repo archive --reason` gave one.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub archived_reason: String,
}

/// The registry: every repo and its local path, if set.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Registry {
    pub repos: BTreeMap<String, Repo>,
    /// Name → absolute or `~/` path.
    pub paths: BTreeMap<String, String>,
}

/// Go's `Load`. Missing files are empty; unreadable or invalid ones are errors
/// with Go's messages.
pub fn load(root: &Path) -> Result<Registry, Error> {
    let mut reg = Registry::default();
    if let Some(data) = read(root, "REPOS.json")? {
        reg.repos = decode_repos(&data).map_err(|detail| Error::StateParse {
            path: shown(root, "REPOS.json"),
            detail,
        })?;
        // The file is shared, so a name can come from a hand edit: check it as
        // `add` does before anything uses it as a folder name (N-0004).
        if let Some(name) = reg.repos.keys().find(|n| !NAME_RE.is_match(n)) {
            return Err(Error::InvalidRepoName {
                name: name.clone(),
                path: Some(shown(root, "REPOS.json")),
            });
        }
    }
    if let Some(data) = read(root, ".daiku/paths.json")? {
        reg.paths = decode_paths(&data).map_err(|detail| Error::StateParse {
            path: shown(root, ".daiku/paths.json"),
            detail,
        })?;
    }
    Ok(reg)
}

fn read(root: &Path, rel: &str) -> Result<Option<Vec<u8>>, Error> {
    read_file(&at(root, rel)).map_err(|detail| Error::CannotRead {
        path: shown(root, rel),
        detail,
    })
}

/// `REPOS.json` as Go's `json.Unmarshal` into `struct { Repos map[string]Repo }`
/// reads it.
pub fn decode_repos(data: &[u8]) -> Result<BTreeMap<String, Repo>, String> {
    gojson::check(data)?;
    let mut r = Reader::new(data);
    let mut repos: Option<BTreeMap<String, Repo>> = None;
    match r.kind() {
        Kind::Null => {
            r.literal();
        }
        Kind::Object => {
            r.open();
            while let Some(key) = r.next_key() {
                if fold(&key) != "REPOS" {
                    r.skip();
                    continue;
                }
                let at = At::Field {
                    strukt: "",
                    field: "repos",
                };
                match r.kind() {
                    Kind::Null => {
                        r.literal();
                        repos = None;
                    }
                    Kind::Object => {
                        r.open();
                        let map = repos.get_or_insert_with(BTreeMap::new);
                        while let Some(name) = r.next_key() {
                            map.insert(name, repo(&mut r)?);
                        }
                    }
                    k => return Err(mismatch(&mut r, k, at, "map[string]repos.Repo")),
                }
            }
        }
        k => {
            return Err(mismatch(
                &mut r,
                k,
                At::Value,
                "struct { Repos map[string]repos.Repo \"json:\\\"repos\\\"\" }",
            ));
        }
    }
    Ok(repos.unwrap_or_default())
}

fn repo(r: &mut Reader) -> Result<Repo, String> {
    let mut repo = Repo::default();
    match r.kind() {
        Kind::Null => {
            r.literal();
        }
        Kind::Object => {
            r.open();
            let f = |field| At::Field {
                strukt: "Repo",
                field,
            };
            while let Some(key) = r.next_key() {
                match fold(&key).as_str() {
                    "REMOTE" => string(r, &mut repo.remote, f("repos.remote"))?,
                    "DEFAULTBRANCH" => {
                        string(r, &mut repo.default_branch, f("repos.defaultBranch"))?
                    }
                    "ARCHIVED" => boolean(r, &mut repo.archived, f("repos.archived"))?,
                    "ARCHIVEDREASON" => {
                        string(r, &mut repo.archived_reason, f("repos.archivedReason"))?
                    }
                    _ => r.skip(),
                }
            }
        }
        k => {
            let at = At::Field {
                strukt: "",
                field: "repos",
            };
            return Err(mismatch(r, k, at, "repos.Repo"));
        }
    }
    Ok(repo)
}

/// `.daiku/paths.json` as Go's `json.Unmarshal` into `map[string]string` reads it.
pub fn decode_paths(data: &[u8]) -> Result<BTreeMap<String, String>, String> {
    gojson::check(data)?;
    let mut r = Reader::new(data);
    let mut paths = Some(BTreeMap::new());
    string_map(&mut r, &mut paths, At::Value)?;
    Ok(paths.unwrap_or_default())
}

/// The repo's branch when `add` gets none.
pub const DEFAULT_BRANCH: &str = "main";

static HELPER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9+.-]+::").expect("valid pattern"));

/// Go's `checkRemote`: refuses remotes that shouldn't be committed or handed to
/// git, since the registry comes from a shared repo and a remote can reach
/// `git clone` on another person's machine.
pub fn check_remote(remote: &str) -> Result<(), Error> {
    if remote.is_empty() {
        return Err(Error::RemoteRequired);
    }
    if remote.starts_with('-') {
        return Err(Error::InvalidRemote {
            remote: remote.into(),
        });
    }
    if HELPER_RE.is_match(remote) {
        return Err(Error::RemoteHelper {
            remote: remote.into(),
        });
    }
    if let Some(u) = url::parse(remote)
        && u.has_user
        && (u.scheme == "http" || u.scheme == "https")
    {
        return Err(Error::RemoteCredentials);
    }
    Ok(())
}

/// Go's `filepath.Abs` for a path: absolute paths are cleaned, and relative ones
/// are joined to the client's current folder (`Dirs::cwd`).
pub fn abs(path: &str, dirs: &Dirs) -> Result<String, Error> {
    let joined = if Path::new(path).is_absolute() {
        path.to_string()
    } else {
        let cwd = dirs.cwd.as_ref().ok_or(Error::NoCwd)?;
        format!("{}/{}", cwd.to_string_lossy(), path)
    };
    let clean = gofmt::clean(&gofmt::to_slash(&joined));
    Ok(if cfg!(windows) {
        clean.replace('/', "\\")
    } else {
        clean
    })
}

/// Go's `storedPath`: `~/` paths as written, anything else absolute.
fn stored_path(path: &str, dirs: &Dirs) -> Result<String, Error> {
    if path == "~" || path.starts_with("~/") {
        return Ok(path.to_string());
    }
    abs(path, dirs)
}

/// One thing `doctor` found wrong with a repo.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RepoProblem {
    pub repo: String,
    pub detail: String,
}

static SCP_REMOTE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:[^@/]+@)?([^:/]+):([^/].*)$").expect("valid pattern"));

/// `remote` for display: a `scheme://` URL's userinfo becomes `***` when the
/// scheme is http(s), or when the userinfo holds a password (`:`). Everything
/// else, including scp-style remotes, is returned as it is (N-0005).
///
/// A raw scan rather than `url::parse`, which gives up on exactly the inputs that
/// must still be masked. Only `/` ends the authority, so a `?` or `#` inside the
/// userinfo can't hide its `@`, and the userinfo runs to the authority's last `@`.
/// Leading whitespace is kept but doesn't hide the scheme.
pub fn redact_remote(remote: &str) -> String {
    let body = remote.trim_start();
    let lead = &remote[..remote.len() - body.len()];
    let Some(sep) = body.find("://") else {
        return remote.to_string();
    };
    let scheme = &body[..sep];
    let valid_scheme = scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"+.-".contains(&b));
    if !valid_scheme {
        return remote.to_string();
    }
    let after = &body[sep + 3..];
    let authority = &after[..after.find('/').unwrap_or(after.len())];
    let Some(at) = authority.rfind('@') else {
        return remote.to_string();
    };
    let userinfo = &authority[..at];
    let http = scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https");
    if !http && !userinfo.contains(':') {
        return remote.to_string();
    }
    format!("{lead}{scheme}://***{}", &after[at..])
}

/// Go's `normalizeRemote`: the SSH and HTTPS spellings of one repo compare equal.
pub fn normalize_remote(remote: &str) -> String {
    let trimmed = remote.trim();
    let mut s = match url::parse(trimmed) {
        Some(u) if !u.scheme.is_empty() && !u.host.is_empty() => {
            format!("{}{}", u.hostname().to_lowercase(), u.path)
        }
        _ => match SCP_REMOTE_RE.captures(trimmed) {
            Some(m) => format!("{}/{}", m[1].to_lowercase(), &m[2]),
            None => trimmed.to_string(),
        },
    };
    s = s.trim_end_matches('/').to_string();
    match s.strip_suffix(".git") {
        Some(rest) => rest.to_string(),
        None => s,
    }
}

static SHELL_SAFE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9_@%+=:,./-]+$").expect("valid pattern"));

/// Go's `shellQuote`: the string as one shell word.
pub fn shell_quote(s: &str) -> String {
    if SHELL_SAFE_RE.is_match(s) {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn is_dir(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_dir())
}

/// What `REPOS.json` holds.
#[derive(serde::Serialize)]
struct RegistryFile<'a> {
    repos: &'a BTreeMap<String, Repo>,
}

impl Registry {
    /// Go's `Add`: registers a repo. An empty branch means [`DEFAULT_BRANCH`].
    pub fn add(&mut self, name: &str, remote: &str, branch: &str) -> Result<(), Error> {
        if !NAME_RE.is_match(name) {
            return Err(Error::InvalidRepoName {
                name: name.into(),
                path: None,
            });
        }
        if self.repos.contains_key(name) {
            return Err(Error::RepoExists { name: name.into() });
        }
        check_remote(remote)?;
        let branch = if branch.is_empty() {
            DEFAULT_BRANCH
        } else {
            branch
        };
        self.repos.insert(
            name.to_string(),
            Repo {
                remote: remote.into(),
                default_branch: branch.into(),
                ..Repo::default()
            },
        );
        Ok(())
    }

    /// Go's `SetPath`: records where a registered repo sits on this machine. A
    /// `~/` path is kept as written; anything else is made absolute.
    pub fn set_path(&mut self, name: &str, path: &str, dirs: &Dirs) -> Result<(), Error> {
        if !self.repos.contains_key(name) {
            return Err(Error::UnknownRepo { name: name.into() });
        }
        if path.is_empty() {
            return Err(Error::PathRequired);
        }
        let stored = stored_path(path, dirs)?;
        self.paths.insert(name.into(), stored);
        Ok(())
    }

    /// Marks `name` archived, with an optional reason folded to one line (N-0008).
    pub fn archive(&mut self, name: &str, reason: &str) -> Result<(), Error> {
        let Some(repo) = self.repos.get_mut(name) else {
            return Err(Error::UnknownRepo { name: name.into() });
        };
        if repo.archived {
            return Err(Error::RepoAlreadyArchived { name: name.into() });
        }
        repo.archived = true;
        repo.archived_reason = crate::text::fold_reason(reason).trim().to_string();
        Ok(())
    }

    /// Makes an archived repo live again, dropping its reason (N-0008).
    pub fn unarchive(&mut self, name: &str) -> Result<(), Error> {
        let Some(repo) = self.repos.get_mut(name) else {
            return Err(Error::UnknownRepo { name: name.into() });
        };
        if !repo.archived {
            return Err(Error::RepoNotArchived { name: name.into() });
        }
        repo.archived = false;
        repo.archived_reason.clear();
        Ok(())
    }

    /// How many repos aren't archived: what `repo doctor` checks.
    pub fn live_count(&self) -> usize {
        self.repos.values().filter(|r| !r.archived).count()
    }

    /// Whether `name` is registered and archived.
    pub fn is_archived(&self, name: &str) -> bool {
        self.repos.get(name).is_some_and(|r| r.archived)
    }

    /// Go's `Save`: writes `REPOS.json` and `.daiku/paths.json` through the write
    /// guard, checking both first.
    pub fn save(&self, root: &Path) -> Result<(), Error> {
        // Both files pass the write guard before either is written, so a refused
        // `.daiku/paths.json` can't leave `REPOS.json` half-updated (N-0008).
        write::check_path(root, &at(root, "REPOS.json"))?;
        write::check_path(root, &at(root, ".daiku/paths.json"))?;
        let repos = json::to_pretty(&RegistryFile { repos: &self.repos })
            .expect("the registry always serializes");
        write::write_file(root, &at(root, "REPOS.json"), &repos)?;
        let paths = json::to_pretty(&self.paths).expect("the paths always serialize");
        write::write_file(root, &at(root, ".daiku/paths.json"), &paths)
    }

    /// Go's `Doctor`: repos with no local path, a path missing on disk, or a
    /// checkout whose origin doesn't match the registered remote.
    pub fn doctor(&self, dirs: &Dirs, process: &dyn Process) -> Vec<RepoProblem> {
        let mut problems = Vec::new();
        let mut add = |repo: &str, detail: String| {
            problems.push(RepoProblem {
                repo: repo.to_string(),
                detail,
            });
        };
        for name in self.names() {
            // An archived repo isn't expected on disk, so only a checkout that's
            // still there is checked, for a swapped remote (N-0008).
            let archived = self.is_archived(&name);
            let Ok(path) = self.resolve(&name, dirs) else {
                if !archived {
                    add(&name, "no local path".into());
                }
                continue;
            };
            let shown = path.to_string_lossy().into_owned();
            if !is_dir(&path) {
                if !archived {
                    add(&name, format!("missing on disk: {shown}"));
                }
                continue;
            }
            let cmd = Cmd::new("git")
                .args(["remote", "get-url", "origin"])
                .dir(&path);
            let origin = match process.output(&cmd) {
                Ok(out) if out.code == Some(0) => {
                    String::from_utf8_lossy(&out.stdout).trim().to_string()
                }
                _ => {
                    if !archived {
                        add(&name, format!("not a git checkout with an origin: {shown}"));
                    }
                    continue;
                }
            };
            let registered = &self.repos[&name].remote;
            if normalize_remote(&origin) != normalize_remote(registered) {
                add(
                    &name,
                    format!(
                        "remote mismatch: {shown} has origin {}, registered as {}",
                        redact_remote(&origin),
                        redact_remote(registered),
                    ),
                );
            }
        }
        problems
    }

    /// Go's `Clone`: clones a repo from its registered remote and, once that
    /// succeeds, records its path. It refuses an existing destination.
    ///
    /// This is the one core write outside the workspace and the write guard on
    /// purpose: a checkout lives wherever the user keeps code, and Go creates its
    /// parent folders the same way (S-0008 D3).
    pub fn clone_repo(
        &mut self,
        name: &str,
        dest: &str,
        dirs: &Dirs,
        process: &dyn Process,
    ) -> Result<(), Error> {
        let Some(repo) = self.repos.get(name) else {
            return Err(Error::UnknownRepo { name: name.into() });
        };
        check_remote(&repo.remote)?;
        let remote = repo.remote.clone();
        let stored = stored_path(dest, dirs)?;
        let path = expand(&stored, dirs)?;
        if std::fs::metadata(&path).is_ok() {
            return Err(Error::CloneExists {
                path: path.to_string_lossy().into_owned(),
            });
        }
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            write::mkdir_all(parent)?;
        }
        let cmd = Cmd::new("git").args([
            "clone".to_string(),
            "--quiet".to_string(),
            "--".to_string(),
            remote,
            path.to_string_lossy().into_owned(),
        ]);
        let result = process.output(&cmd);
        if let Some(cause) = crate::env::run_failure("git", &result) {
            let output = result.map_or_else(
                |_| String::new(),
                |o| {
                    let mut text = String::from_utf8_lossy(&o.stdout).into_owned();
                    text.push_str(&String::from_utf8_lossy(&o.stderr));
                    text.trim().to_string()
                },
            );
            return Err(Error::CloneFailed {
                name: name.into(),
                cause,
                output,
            });
        }
        self.paths.insert(name.into(), stored);
        Ok(())
    }

    /// Go's `EnvFlags`: harness flags that give an agent access to the named
    /// repos, one `--add-dir <path>` each, shell-quoted. Every repo must exist on
    /// this machine: quietly leaving one out would leave an agent working without
    /// it.
    pub fn env_flags(&self, names: &[String], dirs: &Dirs) -> Result<String, Error> {
        let mut flags: Vec<String> = Vec::new();
        for name in names {
            let path = self.resolve(name, dirs)?;
            if !is_dir(&path) {
                return Err(Error::RepoMissingOnDisk {
                    name: name.clone(),
                    path: path.to_string_lossy().into_owned(),
                });
            }
            flags.push("--add-dir".into());
            flags.push(shell_quote(&path.to_string_lossy()));
        }
        Ok(flags.join(" "))
    }

    /// Go's `Names`: the registered names, sorted.
    pub fn names(&self) -> Vec<String> {
        self.repos.keys().cloned().collect()
    }

    /// Go's `Resolve`: the repo's local path, `~/` expanded from `dirs.home`.
    pub fn resolve(&self, name: &str, dirs: &Dirs) -> Result<PathBuf, Error> {
        if !self.repos.contains_key(name) {
            return Err(Error::UnknownRepo {
                name: name.to_string(),
            });
        }
        let Some(p) = self.paths.get(name) else {
            return Err(Error::RepoNoPath {
                name: name.to_string(),
            });
        };
        expand(p, dirs)
    }
}

/// Go's `expand`: `~` and `~/…` from the home folder.
fn expand(path: &str, dirs: &Dirs) -> Result<PathBuf, Error> {
    if path != "~" && !path.starts_with("~/") {
        return Ok(PathBuf::from(path));
    }
    let Some(home) = dirs.home.as_ref().filter(|h| !h.as_os_str().is_empty()) else {
        return Err(Error::NoHome);
    };
    let rest = &path[1..];
    Ok(PathBuf::from(gofmt::join(&[
        home.to_string_lossy().as_ref(),
        rest,
    ])))
}

#[cfg(test)]
mod tests;
