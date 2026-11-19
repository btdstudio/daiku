//! Repos: the read side of Go's `internal/core/repos` (D9). `REPOS.json` (committed)
//! names each logical repo; `.daiku/paths.json` (local) says where each is checked
//! out. Changing them arrives with the `repo` commands (S-0008).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::env::Dirs;
use crate::gojson::decode::{At, Kind, Reader, fold, mismatch, string, string_map};
use crate::state::store::{at, read_file, shown};
use crate::{Error, gofmt, gojson};

/// A logical repo name.
pub static NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z0-9][a-z0-9._-]*$").expect("valid pattern"));

/// One registered repo.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Repo {
    pub remote: String,
    pub default_branch: String,
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

impl Registry {
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
mod tests {
    use super::*;

    #[test]
    fn decoding_matches_go() {
        let mut n = 0;
        for line in include_str!("../tests/repos-go.txt").lines() {
            let mut parts = line.splitn(3, '\t');
            let (kind, hex, want) = (
                parts.next().unwrap(),
                parts.next().unwrap(),
                parts.next().unwrap(),
            );
            let input: Vec<u8> = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect();
            let s = gojson::marshal_string;
            let got = if kind == "repos" {
                decode_repos(&input).map(|m| {
                    let parts: Vec<_> = m
                        .iter()
                        .map(|(k, r)| {
                            format!(
                                "{}:{{\"remote\":{},\"defaultBranch\":{}}}",
                                s(k),
                                s(&r.remote),
                                s(&r.default_branch)
                            )
                        })
                        .collect();
                    format!("{{\"repos\":{{{}}}}}", parts.join(","))
                })
            } else {
                decode_paths(&input).map(|m| {
                    let parts: Vec<_> = m
                        .iter()
                        .map(|(k, v)| format!("{}:{}", s(k), s(v)))
                        .collect();
                    format!("{{{}}}", parts.join(","))
                })
            };
            let got = got.unwrap_or_else(|e| format!("error: {e}"));
            // Go keeps nil apart from empty; daiku treats them alike.
            let want = want
                .replace("{\"repos\":null}", "{\"repos\":{}}")
                .replace("null", "{}");
            assert_eq!(got, want, "{kind} {}", String::from_utf8_lossy(&input));
            n += 1;
        }
        assert!(n > 25);
    }

    #[test]
    fn load_and_resolve() {
        let t = crate::testutil::Temp::new();
        let dirs = Dirs {
            home: Some("/home/u".into()),
            xdg_config_home: None,
        };
        assert_eq!(load(t.path()).unwrap(), Registry::default());
        t.write(
            "REPOS.json",
            r#"{"repos": {"web": {"remote": "w"}, "api": {"remote": "a"}, "lib": {}}}"#,
        );
        t.write(
            ".daiku/paths.json",
            r#"{"api": "~/src/api", "web": "/abs/web"}"#,
        );
        let reg = load(t.path()).unwrap();
        assert_eq!(reg.names(), ["api", "lib", "web"]);
        assert_eq!(
            reg.resolve("api", &dirs).unwrap(),
            PathBuf::from("/home/u/src/api")
        );
        assert_eq!(
            reg.resolve("web", &dirs).unwrap(),
            PathBuf::from("/abs/web")
        );
        assert_eq!(
            reg.resolve("lib", &dirs).unwrap_err().to_string(),
            "daiku: repo \"lib\" has no local path — run `daiku repo path lib <path>`"
        );
        assert_eq!(
            reg.resolve("nope", &dirs).unwrap_err().to_string(),
            "daiku: unknown repo \"nope\" — run `daiku repo add`"
        );
        t.write("REPOS.json", "{\"repos\": 1}");
        assert_eq!(
            load(t.path()).unwrap_err().to_string(),
            format!(
                "daiku: cannot parse {}: json: cannot unmarshal number into Go struct field .repos of type map[string]repos.Repo",
                shown(t.path(), "REPOS.json")
            )
        );
    }
}
