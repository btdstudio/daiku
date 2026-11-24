//! A change's files by phase, and confined reads (S-0016).
//!
//! The phase-to-file mapping is daiku's own knowledge of the rooms' naming, so it
//! lives here and not in a client. [`Workspace::artifacts`] lists the files a
//! change has, one phase at a time; [`Workspace::read_text`] reads one workspace
//! file and refuses anything outside the root or through a symlink. Neither
//! writes, and the mapping never reads a file's contents.

use std::path::Path;

use schemars::JsonSchema;
use serde::Serialize;

use crate::state::store::at;
use crate::state::{BUILD_LOOP, Change, Store};
use crate::text::Text;
use crate::{Error, Outcome, Workspace, gofmt, write};

/// One existing file a change produced, and the phase it belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub phase: String,
    /// Workspace-relative, `/`-separated.
    pub path: String,
}

/// A change's files in phase order, with the phases of its machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactList {
    pub change: Change,
    /// The machine's phase names in order (plan: eight, build: nine).
    pub phases: Vec<String>,
    /// Within a phase, paths are sorted bytewise.
    pub artifacts: Vec<Artifact>,
}

/// Where a phase's files are.
enum Pattern {
    /// One file.
    Exact(String),
    /// The files directly in `dir` ending in `ext`, but not those named in
    /// `except`. It doesn't recurse.
    Dir {
        dir: String,
        ext: &'static str,
        except: &'static [&'static str],
    },
    /// The change's `sources`.
    Sources,
}

type PhaseMap = Vec<(&'static str, Vec<Pattern>)>;

fn exact(rel: String) -> Vec<Pattern> {
    vec![Pattern::Exact(rel)]
}

fn dir(dir: String, except: &'static [&'static str]) -> Vec<Pattern> {
    vec![Pattern::Dir {
        dir,
        ext: ".md",
        except,
    }]
}

/// The plan mapping for the change `name` (`<c>`).
fn plan_map(name: &str) -> PhaseMap {
    vec![
        ("intake", vec![Pattern::Sources]),
        ("research", dir(format!("plan/01_research/{name}"), &[])),
        (
            "explore",
            dir(format!("plan/02_explore/{name}"), &["synthesis.md"]),
        ),
        (
            "synthesis",
            exact(format!("plan/02_explore/{name}/synthesis.md")),
        ),
        (
            "spec",
            exact(format!("plan/specs/{name}/functional-spec.md")),
        ),
        (
            "architecture",
            exact(format!("plan/specs/{name}/architecture.md")),
        ),
        ("breakdown", dir(format!("plan/specs/{name}/tickets"), &[])),
        ("publish", vec![]),
    ]
}

/// The build mapping for the change folder `d` (`build/<repo>/changes/<id>`).
fn build_map(d: &str) -> PhaseMap {
    vec![
        ("ticket", vec![Pattern::Sources]),
        ("requirements", exact(format!("{d}/requirements.md"))),
        ("design", exact(format!("{d}/design.md"))),
        ("tasks", exact(format!("{d}/tasks.json"))),
        ("implement", vec![]),
        ("test", exact(format!("{d}/test-results.md"))),
        ("review", exact(format!("{d}/code-review.md"))),
        ("security", exact(format!("{d}/security-review.md"))),
        ("ship", vec![]),
    ]
}

/// `rel` cleaned, if it is a regular file under `root` that is local and has no
/// symlink below the root; otherwise `None`, as for a missing file.
fn confine(root: &Path, rel: &str) -> Option<String> {
    let rel = gofmt::to_slash(&gofmt::clean(rel));
    if !gofmt::is_local(&rel) || write::through_symlink(root, &rel).is_some() {
        return None;
    }
    let meta = std::fs::symlink_metadata(at(root, &rel)).ok()?;
    meta.file_type().is_file().then_some(rel)
}

/// The confined files one pattern names, unsorted.
fn gather(root: &Path, c: &Change, p: &Pattern) -> Vec<String> {
    match p {
        Pattern::Exact(rel) => confine(root, rel).into_iter().collect(),
        Pattern::Sources => c.sources.iter().filter_map(|s| confine(root, s)).collect(),
        Pattern::Dir { dir, ext, except } => {
            if write::through_symlink(root, dir).is_some() {
                return Vec::new();
            }
            let Ok(entries) = std::fs::read_dir(at(root, dir)) else {
                return Vec::new();
            };
            entries
                .flatten()
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|n| n.ends_with(ext) && !except.contains(&n.as_str()))
                .filter_map(|n| confine(root, &format!("{dir}/{n}")))
                .collect()
        }
    }
}

impl Workspace {
    /// A change's existing files by phase. An unknown change is `UnknownChange`;
    /// a phase with no files contributes nothing. It reads no file's contents and
    /// writes nothing, whatever the change's gates say.
    pub fn artifacts(&self, change: &str) -> Result<Outcome<ArtifactList>, Error> {
        let store = Store::reader(self.root(), self.env());
        let c = store.find(change)?;
        let map = if c.loop_ == BUILD_LOOP {
            build_map(&store.rel_dir(&c)?)
        } else {
            plan_map(&c.name)
        };
        let mut artifacts = Vec::new();
        for phase in c.machine().names() {
            let patterns = map
                .iter()
                .find(|(name, _)| *name == phase)
                .map_or(&[][..], |(_, p)| p.as_slice());
            let mut paths: Vec<String> = patterns
                .iter()
                .flat_map(|p| gather(self.root(), &c, p))
                .collect();
            paths.sort();
            paths.dedup();
            artifacts.extend(paths.into_iter().map(|path| Artifact {
                phase: phase.to_string(),
                path,
            }));
        }
        let phases = c.machine().names().map(str::to_string).collect();
        Ok(Outcome {
            value: ArtifactList {
                change: c,
                phases,
                artifacts,
            },
            warnings: store.take_warnings(),
        })
    }

    /// The bytes of the workspace file `rel`, which keeps non-UTF-8 bytes. `\` is
    /// read as `/`. A path that is absolute, empty, climbs out with `..` or has a
    /// `.` or `..` segment is `ReadOutside`; a symlink anywhere below the root is
    /// `ReadThroughSymlink`; a missing or unreadable file is `CannotRead`. A root
    /// that is itself reached through a symlink is fine.
    pub fn read_text(&self, rel: &str) -> Result<Text, Error> {
        let rel = rel.replace('\\', "/");
        if !gofmt::is_local(&rel) || gofmt::clean(&rel) != rel || rel == "." {
            return Err(Error::ReadOutside { path: rel });
        }
        if let Some(link) = write::through_symlink(self.root(), &rel) {
            return Err(Error::ReadThroughSymlink { rel, link });
        }
        match std::fs::read(at(self.root(), &rel)) {
            Ok(bytes) => Ok(Text::from(bytes)),
            Err(e) => Err(Error::CannotRead {
                path: rel,
                detail: e.to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::machine::{BUILD, PLAN};
    use crate::state::store::SeedOptions;
    use crate::state::{PLAN_LOOP, Store};
    use crate::testutil::{self, RealProcess, Temp};

    fn ws(t: &Temp) -> Workspace {
        t.write(".daiku/settings.json", "{\"vcs\": \"none\"}\n");
        Workspace::open(t.path(), testutil::env(t, Box::new(RealProcess)))
    }

    fn seed(t: &Temp, name: &str, loop_: &str, sources: &[&str], repos: &[&str]) {
        let w = ws(t);
        let s = Store::at(w.root(), w.env()).unwrap();
        let o = SeedOptions {
            sources: sources.iter().map(|s| s.to_string()).collect(),
            repos: repos.iter().map(|s| s.to_string()).collect(),
            ..SeedOptions::default()
        };
        s.seed(name, loop_, o).unwrap();
    }

    fn listed(w: &Workspace, name: &str) -> Vec<(String, String)> {
        w.artifacts(name)
            .unwrap()
            .value
            .artifacts
            .into_iter()
            .map(|a| (a.phase, a.path))
            .collect()
    }

    fn pairs(v: &[(&str, &str)]) -> Vec<(String, String)> {
        v.iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }

    #[test]
    fn tables_follow_the_machines() {
        let plan: Vec<_> = plan_map("1-x").iter().map(|(n, _)| *n).collect();
        assert_eq!(plan, PLAN.names().collect::<Vec<_>>());
        let build: Vec<_> = build_map("build/r/changes/S-0001")
            .iter()
            .map(|(n, _)| *n)
            .collect();
        assert_eq!(build, BUILD.names().collect::<Vec<_>>());
    }

    #[test]
    fn plan_lists_by_phase_in_order() {
        let t = Temp::new();
        seed(&t, "1-x", PLAN_LOOP, &["plan/00_intake/1.md"], &[]);
        t.write("plan/00_intake/1.md", "i");
        t.write("plan/01_research/1-x/b.md", "r");
        t.write("plan/01_research/1-x/a.md", "r");
        t.write("plan/01_research/1-x/notes.txt", "not markdown");
        t.write("plan/02_explore/1-x/core.md", "e");
        t.write("plan/02_explore/1-x/synthesis.md", "s");
        t.write("plan/specs/1-x/functional-spec.md", "f");
        t.write("plan/specs/1-x/architecture.md", "a");
        t.write("plan/specs/1-x/tickets/S-0003.md", "t");
        t.write("plan/specs/1-x/tickets/S-0001.md", "t");
        t.write("plan/specs/1-x/tickets/S-0002.md", "t");
        t.write("plan/specs/1-x/tickets/nested/S-0009.md", "t");
        let w = ws(&t);
        let list = w.artifacts("1-x").unwrap().value;
        assert_eq!(
            list.phases,
            [
                "intake",
                "research",
                "explore",
                "synthesis",
                "spec",
                "architecture",
                "breakdown",
                "publish"
            ]
        );
        assert_eq!(list.change.name, "1-x");
        assert_eq!(
            listed(&w, "1-x"),
            pairs(&[
                ("intake", "plan/00_intake/1.md"),
                ("research", "plan/01_research/1-x/a.md"),
                ("research", "plan/01_research/1-x/b.md"),
                ("explore", "plan/02_explore/1-x/core.md"),
                ("synthesis", "plan/02_explore/1-x/synthesis.md"),
                ("spec", "plan/specs/1-x/functional-spec.md"),
                ("architecture", "plan/specs/1-x/architecture.md"),
                ("breakdown", "plan/specs/1-x/tickets/S-0001.md"),
                ("breakdown", "plan/specs/1-x/tickets/S-0002.md"),
                ("breakdown", "plan/specs/1-x/tickets/S-0003.md"),
            ])
        );
    }

    #[test]
    fn two_intake_sources_list_in_bytewise_order() {
        let t = Temp::new();
        seed(
            &t,
            "1-x",
            PLAN_LOOP,
            &["plan/00_intake/2.md", "plan/00_intake/10.md"],
            &[],
        );
        t.write("plan/00_intake/2.md", "i");
        t.write("plan/00_intake/10.md", "i");
        assert_eq!(
            listed(&ws(&t), "1-x"),
            pairs(&[
                ("intake", "plan/00_intake/10.md"),
                ("intake", "plan/00_intake/2.md"),
            ])
        );
    }

    #[test]
    fn empty_phases_and_missing_files_are_no_error() {
        let t = Temp::new();
        seed(&t, "1-x", PLAN_LOOP, &["plan/00_intake/1.md"], &[]);
        // No research folder, the intake file is gone, and nothing else exists.
        assert_eq!(listed(&ws(&t), "1-x"), pairs(&[]));
        seed(
            &t,
            "S-0001",
            BUILD_LOOP,
            &["plan/specs/1-x/tickets/S-0001.md"],
            &["app"],
        );
        let w = ws(&t);
        let list = w.artifacts("S-0001").unwrap().value;
        assert!(list.artifacts.is_empty());
        assert_eq!(list.phases.len(), 9);
        // A plan folder found on disk and not recorded has no sources.
        std::fs::create_dir_all(t.p("plan/specs/7-found")).unwrap();
        assert_eq!(listed(&w, "7-found"), pairs(&[]));
    }

    #[test]
    fn build_lists_its_ticket_and_files() {
        for ticket in [
            "plan/specs/1-x/tickets/S-0001.md",
            "build/tickets/S-0001.md",
        ] {
            let t = Temp::new();
            seed(&t, "S-0001", BUILD_LOOP, &[ticket], &["app"]);
            let d = "build/app/changes/S-0001";
            t.write(ticket, "t");
            for f in ["requirements.md", "design.md", "tasks.json"] {
                t.write(&format!("{d}/{f}"), "x");
            }
            let w = ws(&t);
            assert_eq!(
                listed(&w, "S-0001"),
                pairs(&[
                    ("ticket", ticket),
                    ("requirements", &format!("{d}/requirements.md")),
                    ("design", &format!("{d}/design.md")),
                    ("tasks", &format!("{d}/tasks.json")),
                ])
            );
            for f in ["test-results.md", "code-review.md", "security-review.md"] {
                t.write(&format!("{d}/{f}"), "x");
            }
            // Files implement and ship might own are not listed.
            t.write(&format!("{d}/notes.md"), "x");
            let list = w.artifacts("S-0001").unwrap().value;
            let phases: Vec<_> = list.artifacts.iter().map(|a| a.phase.as_str()).collect();
            assert_eq!(
                phases,
                [
                    "ticket",
                    "requirements",
                    "design",
                    "tasks",
                    "test",
                    "review",
                    "security"
                ]
            );
            assert_eq!(
                list.phases,
                BUILD.names().map(str::to_string).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn unconfined_candidates_are_skipped() {
        let t = Temp::new();
        seed(
            &t,
            "1-x",
            PLAN_LOOP,
            &["../x", "/etc/hosts", "plan/00_intake/1.md"],
            &[],
        );
        t.write("plan/00_intake/1.md", "i");
        let w = ws(&t);
        assert_eq!(
            listed(&w, "1-x"),
            pairs(&[("intake", "plan/00_intake/1.md")])
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_skipped() {
        use std::os::unix::fs::symlink;
        let t = Temp::new();
        seed(&t, "1-x", PLAN_LOOP, &[], &[]);
        let elsewhere = Temp::new();
        elsewhere.write("S-0001.md", "t");
        std::fs::create_dir_all(t.p("plan/specs/1-x")).unwrap();
        symlink(elsewhere.path(), t.p("plan/specs/1-x/tickets")).unwrap();
        t.write("real.md", "x");
        symlink(t.p("real.md"), t.p("plan/specs/1-x/functional-spec.md")).unwrap();
        t.write("plan/specs/1-x/architecture.md", "a");
        assert_eq!(
            listed(&ws(&t), "1-x"),
            pairs(&[("architecture", "plan/specs/1-x/architecture.md")])
        );
    }

    #[test]
    fn unknown_change_is_refused() {
        let t = Temp::new();
        let w = ws(&t);
        for name in ["x", "", "1-nofolder"] {
            let e = w.artifacts(name).unwrap_err();
            assert!(matches!(e, Error::UnknownChange { .. }), "{name}: {e}");
        }
    }

    #[test]
    fn read_text_refuses_paths_outside() {
        let t = Temp::new();
        let w = ws(&t);
        for rel in [
            "../outside",
            "/abs/path",
            "",
            "a/../../b",
            "a\\..\\..\\b",
            "a/./b",
            "a/../b",
            "a//b",
            "a/",
            ".",
        ] {
            let e = w.read_text(rel).unwrap_err();
            assert!(matches!(e, Error::ReadOutside { .. }), "{rel:?}: {e}");
        }
        assert_eq!(
            w.read_text("../x").unwrap_err().to_string(),
            "daiku: won't read ../x: it's outside the workspace"
        );
        // Backslashes are shown as the slashes they were read as.
        assert_eq!(
            w.read_text("a\\..\\..\\b").unwrap_err().to_string(),
            "daiku: won't read a/../../b: it's outside the workspace"
        );
        #[cfg(windows)]
        assert!(matches!(
            w.read_text("C:\\x").unwrap_err(),
            Error::ReadOutside { .. }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn read_text_refuses_symlinks() {
        use std::os::unix::fs::symlink;
        let t = Temp::new();
        let elsewhere = Temp::new();
        elsewhere.write("f.md", "secret");
        symlink(elsewhere.path(), t.p("link")).unwrap();
        t.write("real/f.md", "x");
        symlink(t.p("real/f.md"), t.p("real/alias.md")).unwrap();
        let w = ws(&t);
        for rel in ["link/f.md", "real/alias.md"] {
            let e = w.read_text(rel).unwrap_err();
            assert!(matches!(e, Error::ReadThroughSymlink { .. }), "{rel}: {e}");
        }
        let msg = w.read_text("real/alias.md").unwrap_err().to_string();
        assert!(
            msg.starts_with("daiku: won't read real/alias.md: ")
                && msg.ends_with(
                    "real/alias.md is a symlink, which could lead outside the workspace"
                ),
            "{msg}"
        );
    }

    #[test]
    fn read_text_reads_bytes() {
        let t = Temp::new();
        let w = ws(&t);
        let bytes = b"ok \xff\xfe bad".to_vec();
        std::fs::create_dir_all(t.p("docs")).unwrap();
        std::fs::write(t.p("docs/x.md"), &bytes).unwrap();
        assert_eq!(w.read_text("docs/x.md").unwrap().as_bytes(), bytes);
        assert_eq!(w.read_text("docs\\x.md").unwrap().as_bytes(), bytes);
        let e = w.read_text("docs/missing.md").unwrap_err();
        assert!(matches!(e, Error::CannotRead { .. }), "{e}");
        let e = w.read_text("docs").unwrap_err();
        assert!(matches!(e, Error::CannotRead { .. }), "{e}");
    }

    #[cfg(unix)]
    #[test]
    fn read_text_allows_a_root_behind_a_symlink() {
        let t = Temp::new();
        let real = Temp::new();
        real.write("docs/x.md", "hi");
        let link = t.p("root-link");
        std::os::unix::fs::symlink(real.path(), &link).unwrap();
        let w = Workspace::open(&link, testutil::env(&t, Box::new(RealProcess)));
        assert_eq!(w.read_text("docs/x.md").unwrap().as_bytes(), b"hi");
    }
}
