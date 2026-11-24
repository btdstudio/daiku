//! What to watch in a workspace and what a changed path means (S-0014). The app's
//! watcher registers [`Workspace::watch_paths`] and calls [`classify`] on each event
//! path, made relative to the root. Both are pure: no I/O, nothing printed.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::Serialize;

use crate::Workspace;

/// A folder to watch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchPath {
    pub path: PathBuf,
    pub recursive: bool,
}

/// What a changed path means for the views.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[non_exhaustive]
pub enum Changed {
    /// `STATE.json`, `LOG.md` or `.daiku/active`.
    State,
    /// A ticket file: `plan/specs/<c>/tickets/<id>.md` (with its change) or
    /// `build/tickets/<id>.md` (without).
    Ticket { id: String, change: Option<String> },
    /// A file under `plan/specs/<c>/` or `plan/<room>/<c>/`, or `plan/00_intake/<n>.md`
    /// (the raw number as the change).
    PlanArtifact { change: String },
    /// A file under `build/<repo>/changes/<c>/`.
    BuildArtifact { repo: String, change: String },
    /// A file under `ideas/<n>-<slug>/`.
    Idea { id: i64 },
    /// The ADR folder `plan/04_architecture/adr/` or a file under it.
    Adr,
    /// `REPOS.json` or `.daiku/paths.json`.
    Repos,
    /// `.daiku/settings.json`.
    Settings,
    /// Anything under `references/`.
    Reference,
    /// Root `AGENTS.md`, and each room's `context.md`, `personas/**` and `templates/**`.
    Scaffold,
}

impl Workspace {
    /// The folders to watch: the root alone, then the five recursive ones. A folder
    /// that doesn't exist is still listed; this touches no filesystem.
    pub fn watch_paths(&self) -> Vec<WatchPath> {
        let root = self.root();
        let mut v = vec![WatchPath {
            path: root.to_path_buf(),
            recursive: false,
        }];
        v.extend(
            [".daiku", "plan", "build", "ideas", "references"].map(|n| WatchPath {
                path: root.join(n),
                recursive: true,
            }),
        );
        v
    }
}

/// Classify a path relative to the workspace root. `None` means nothing to refresh:
/// an ignored file, a path no rule covers, or one out of contract (absolute, with a
/// `.` or `..` component, or not UTF-8). `\` and `/` are the same separator.
pub fn classify(rel: &Path) -> Option<Changed> {
    let s = rel.to_str()?;
    let s: Cow<str> = if s.contains('\\') {
        s.replace('\\', "/").into()
    } else {
        s.into()
    };
    if s.starts_with('/') {
        return None;
    }
    let parts: Vec<&str> = s.split('/').filter(|p| !p.is_empty()).collect();
    let last = parts.last()?;
    if parts.iter().any(|p| *p == "." || *p == "..") {
        return None;
    }
    if parts.contains(&".git") || ignored(last) {
        return None;
    }
    match parts.as_slice() {
        ["STATE.json"] | ["LOG.md"] | [".daiku", "active"] => Some(Changed::State),
        ["REPOS.json"] | [".daiku", "paths.json"] => Some(Changed::Repos),
        [".daiku", "settings.json"] => Some(Changed::Settings),
        ["AGENTS.md"] => Some(Changed::Scaffold),
        ["references", ..] => Some(Changed::Reference),
        ["plan", rest @ ..] => plan(rest),
        ["build", rest @ ..] => build(rest),
        ["ideas", rest @ ..] => ideas(rest),
        _ => None,
    }
}

fn ignored(name: &str) -> bool {
    name == ".DS_Store"
        || name.starts_with("._")
        || name.starts_with(".#")
        || [".swp", ".swo", ".tmp", "~"]
            .iter()
            .any(|e| name.ends_with(e))
        || (name.len() >= 2 && name.starts_with('#') && name.ends_with('#'))
        || name == "4913"
}

fn md_stem(f: &str) -> Option<&str> {
    f.strip_suffix(".md").filter(|s| !s.is_empty())
}

fn digit_stem(f: &str) -> Option<&str> {
    md_stem(f).filter(|s| s.bytes().all(|b| b.is_ascii_digit()))
}

fn idea_id(folder: &str) -> Option<i64> {
    let (n, rest) = folder.split_once('-')?;
    if n.is_empty() || rest.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    n.parse().ok()
}

fn plan(rest: &[&str]) -> Option<Changed> {
    let artifact = |c: &str| Some(Changed::PlanArtifact { change: c.into() });
    match rest {
        ["specs", c, "tickets", f] if md_stem(f).is_some() => Some(Changed::Ticket {
            id: md_stem(f)?.into(),
            change: Some((*c).into()),
        }),
        ["specs", c, ..] => artifact(c),
        ["04_architecture", "adr", ..] => Some(Changed::Adr),
        ["00_intake", f] if digit_stem(f).is_some() => artifact(digit_stem(f)?),
        [_, "context.md"] => Some(Changed::Scaffold),
        [_, "personas" | "templates", ..] => Some(Changed::Scaffold),
        [_, c, _, ..] => artifact(c),
        _ => None,
    }
}

fn build(rest: &[&str]) -> Option<Changed> {
    match rest {
        ["tickets", f] if md_stem(f).is_some() => Some(Changed::Ticket {
            id: md_stem(f)?.into(),
            change: None,
        }),
        ["tickets", ..] => None,
        [repo, "changes", c, ..] => Some(Changed::BuildArtifact {
            repo: (*repo).into(),
            change: (*c).into(),
        }),
        [_, "context.md"] => Some(Changed::Scaffold),
        [_, "personas" | "templates", ..] => Some(Changed::Scaffold),
        _ => None,
    }
}

fn ideas(rest: &[&str]) -> Option<Changed> {
    match rest {
        ["context.md"] => Some(Changed::Scaffold),
        ["personas", ..] => Some(Changed::Scaffold),
        [folder, ..] => idea_id(folder).map(|id| Changed::Idea { id }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{self, RealProcess, Temp};

    fn ticket(id: &str, change: Option<&str>) -> Option<Changed> {
        Some(Changed::Ticket {
            id: id.into(),
            change: change.map(Into::into),
        })
    }
    fn plan_a(c: &str) -> Option<Changed> {
        Some(Changed::PlanArtifact { change: c.into() })
    }
    fn build_a(r: &str, c: &str) -> Option<Changed> {
        Some(Changed::BuildArtifact {
            repo: r.into(),
            change: c.into(),
        })
    }
    fn idea(id: i64) -> Option<Changed> {
        Some(Changed::Idea { id })
    }

    /// Every row of AC2 to AC7 (non-UTF-8 aside), with `/` separators.
    fn cases() -> Vec<(&'static str, Option<Changed>)> {
        use Changed::*;
        let state = Some(State);
        let repos = Some(Repos);
        let scaffold = Some(Scaffold);
        let mut v = vec![
            // AC2
            ("STATE.json", state.clone()),
            ("LOG.md", state.clone()),
            (".daiku/active", state),
            ("REPOS.json", repos.clone()),
            (".daiku/paths.json", repos),
            (".daiku/settings.json", Some(Settings)),
            (".daiku/manifest.json", None),
            ("plan/04_architecture/adr/0003-x.md", Some(Adr)),
            ("ideas/6-app-ui/summary.md", idea(6)),
            ("AGENTS.md", scaffold.clone()),
            ("plan/03_spec/personas/analyst.md", scaffold.clone()),
            (
                "build/01_design/templates/requirements.md",
                scaffold.clone(),
            ),
            ("ideas/context.md", scaffold.clone()),
            ("ideas/personas/facilitator.md", scaffold.clone()),
            ("CLAUDE.md", None),
            ("README.md", None),
            ("sync.md", None),
            ("sync-examples/x.md", None),
            ("target/foo", None),
            ("src/main.rs", None),
            ("plan/STATE.json", None),
            ("build/LOG.md", None),
            // AC3
            (
                "plan/specs/4-daiku-app-phase-one/tickets/S-0014.md",
                ticket("S-0014", Some("4-daiku-app-phase-one")),
            ),
            ("build/tickets/N-0012.md", ticket("N-0012", None)),
            ("plan/specs/4-x/tickets/notes.txt", plan_a("4-x")),
            ("plan/specs/4-x/tickets/sub/S-1.md", plan_a("4-x")),
            ("build/tickets/x.txt", None),
            // AC4
            ("plan/00_intake/3.md", plan_a("3")),
            (
                "plan/02_explore/1-rust-conversion/notes.md",
                plan_a("1-rust-conversion"),
            ),
            ("plan/02_explore/context.md", scaffold.clone()),
            // R16 / D7: bare folders
            ("plan/04_architecture/adr", Some(Adr)),
            ("plan/03_spec/personas", scaffold.clone()),
            ("plan/04_architecture/4-x/notes.md", plan_a("4-x")),
            ("plan/00_intake/context.md", scaffold.clone()),
            ("plan/00_intake/abc.md", None),
            ("plan/specs", None),
            ("plan/02_explore/notes.md", None),
            // AC5
            ("references/index.md", Some(Reference)),
            ("references/context.md", Some(Reference)),
            ("references/chats/x.md", Some(Reference)),
            // AC6
            (
                "build/daiku/changes/S-0014/requirements.md",
                build_a("daiku", "S-0014"),
            ),
            ("build/daiku/changes/S-0014", build_a("daiku", "S-0014")),
            ("build/daiku", None),
            ("build/daiku/changes", None),
            ("build/01_design/context.md", scaffold),
            ("ideas/7-x", idea(7)),
            ("ideas/7", None),
            ("ideas/notes/summary.md", None),
            ("ideas/99999999999999999999-x/a.md", None),
            ("plan/specs/4-x", plan_a("4-x")),
            // AC7
            ("/plan/x.md", None),
            ("./plan/x.md", None),
            ("plan/../x.md", None),
            ("..\\plan\\x.md", None),
            ("C:\\plan\\x.md", None),
            ("", None),
            ("plan//specs\\\\4-x/a.md", plan_a("4-x")),
        ];
        // AC2 ignored paths
        for p in [
            ".git/HEAD",
            "plan/.git/x",
            ".DS_Store",
            "plan/.DS_Store",
            "._x",
            "plan/x.md.swp",
            "x.swo",
            "plan/x.md~",
            ".#x.md",
            "#x.md#",
            "4913",
            "plan/x.tmp",
            "references/.DS_Store",
            "references/x.swp",
        ] {
            v.push((p, None));
        }
        v
    }

    #[test]
    fn classify_table() {
        for (p, want) in cases() {
            assert_eq!(classify(Path::new(p)), want, "path {p:?}");
        }
    }

    #[test]
    fn classify_table_with_backslashes() {
        for (p, want) in cases() {
            let q = p.replace('/', "\\");
            assert_eq!(classify(Path::new(&q)), want, "path {q:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn classify_non_utf8_is_none() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let p = Path::new(OsStr::from_bytes(b"plan/\xff.md"));
        assert_eq!(classify(p), None);
    }

    fn shape(root: &Path) -> Vec<WatchPath> {
        let mut v = vec![WatchPath {
            path: root.to_path_buf(),
            recursive: false,
        }];
        for n in [".daiku", "plan", "build", "ideas", "references"] {
            v.push(WatchPath {
                path: root.join(n),
                recursive: true,
            });
        }
        v
    }

    #[test]
    fn watch_paths_on_an_empty_root() {
        let t = Temp::new();
        let w = Workspace::open(t.path(), testutil::env(&t, Box::new(RealProcess)));
        let got = w.watch_paths();
        assert_eq!(got.len(), 6);
        assert_eq!(got, shape(t.path()));
    }

    #[test]
    fn watch_paths_on_a_nonexistent_root() {
        let t = Temp::new();
        let root = Path::new("/nonexistent/daiku-ws");
        let w = Workspace::open(root, testutil::env(&t, Box::new(RealProcess)));
        assert_eq!(w.watch_paths(), shape(root));
    }

    #[test]
    fn changed_serializes_as_documented() {
        use serde_json::json;
        let cases = [
            (Changed::State, json!({"kind":"state"})),
            (
                Changed::Ticket {
                    id: "S-1".into(),
                    change: None,
                },
                json!({"kind":"ticket","id":"S-1","change":null}),
            ),
            (
                Changed::Ticket {
                    id: "S-1".into(),
                    change: Some("4-x".into()),
                },
                json!({"kind":"ticket","id":"S-1","change":"4-x"}),
            ),
            (
                Changed::PlanArtifact {
                    change: "4-x".into(),
                },
                json!({"kind":"planArtifact","change":"4-x"}),
            ),
            (
                Changed::BuildArtifact {
                    repo: "daiku".into(),
                    change: "S-1".into(),
                },
                json!({"kind":"buildArtifact","repo":"daiku","change":"S-1"}),
            ),
            (Changed::Idea { id: 6 }, json!({"kind":"idea","id":6})),
            (Changed::Adr, json!({"kind":"adr"})),
            (Changed::Repos, json!({"kind":"repos"})),
            (Changed::Settings, json!({"kind":"settings"})),
            (Changed::Reference, json!({"kind":"reference"})),
            (Changed::Scaffold, json!({"kind":"scaffold"})),
        ];
        for (c, want) in cases {
            assert_eq!(serde_json::to_value(&c).unwrap(), want, "{c:?}");
        }
    }
}
