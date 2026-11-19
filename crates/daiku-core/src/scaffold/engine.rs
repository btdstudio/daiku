//! The scaffold engine: the embedded trees, the walk, the manifest, and writing,
//! updating and creating rooms (D2–D8, D11). A port of Go's
//! `internal/core/scaffold`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use include_dir::{Dir, DirEntry, include_dir};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::env::{Cmd, Env};
use crate::{Error, gofmt, gojson, json, write};

/// The scaffold as Go embedded it at `5aa7a78`: `root`, `plan`, `build`, `agents`.
static SCAFFOLD: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/scaffold");

/// The rooms, sorted, as Go's `roomNames` gives them, with their marker files.
pub(crate) const ROOMS: [(&str, Option<&str>); 2] = [("build", Some("01_design")), ("plan", None)];

/// One of the embedded trees.
pub(crate) fn tree(name: &str) -> &'static Dir<'static> {
    SCAFFOLD
        .get_dir(name)
        .unwrap_or_else(|| panic!("scaffold tree {name} is embedded"))
}

/// A file of the `agents` tree.
pub(crate) fn agents_file(name: &str) -> Option<&'static str> {
    tree("agents")
        .get_file(format!("agents/{name}"))
        .and_then(|f| f.contents_utf8())
}

/// A walk over embedded trees, as Go's `fs.WalkDir`: each folder's entries sorted
/// by name (byte order), depth first. `.DS_Store` is skipped, and in `vcs: none`
/// workspaces `.gitattributes` and `.gitkeep` too.
pub(crate) fn walk_tree(name: &str, prefix: &str, local: bool) -> Vec<(String, &'static [u8])> {
    let mut out = Vec::new();
    walk_dir(tree(name), name, prefix, local, &mut out);
    out
}

fn walk_dir(
    dir: &'static Dir<'static>,
    top: &str,
    prefix: &str,
    local: bool,
    out: &mut Vec<(String, &'static [u8])>,
) {
    let mut entries: Vec<&DirEntry> = dir.entries().iter().collect();
    entries.sort_by(|a, b| name(a.path()).cmp(name(b.path())));
    for entry in entries {
        match entry {
            DirEntry::Dir(d) => walk_dir(d, top, prefix, local, out),
            DirEntry::File(f) => {
                let base = name(f.path());
                if base == ".DS_Store" || (local && omit_when_local(base)) {
                    continue;
                }
                let inner = rel_in_tree(f.path(), top);
                out.push((join_rel(prefix, &inner), f.contents()));
            }
        }
    }
}

fn name(p: &Path) -> &str {
    p.file_name().and_then(|n| n.to_str()).unwrap_or("")
}

/// The path inside its tree (`root/ideas/x.md` -> `ideas/x.md`), slash-separated.
fn rel_in_tree(p: &Path, top: &str) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    s.strip_prefix(&format!("{top}/")).unwrap_or(&s).to_string()
}

fn join_rel(prefix: &str, rel: &str) -> String {
    if prefix.is_empty() {
        rel.to_string()
    } else {
        format!("{prefix}/{rel}")
    }
}

/// Go's `IsGitFile`.
pub(crate) fn is_git_file(base: &str) -> bool {
    matches!(base, ".gitignore" | ".gitattributes" | ".gitkeep")
}

/// Git files a `vcs: none` workspace leaves out (all but `.gitignore`).
fn omit_when_local(base: &str) -> bool {
    is_git_file(base) && base != ".gitignore"
}

/// `root` joined with a slash path, as Go's `filepath.Join` prints it.
pub(crate) fn shown(root: &Path, rel: &str) -> String {
    gofmt::join(&[gofmt::to_slash(&root.to_string_lossy()).as_str(), rel])
}

/// The manifest: each managed file's SHA-256 and the version that wrote it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub version: String,
    #[serde(default)]
    pub files: BTreeMap<String, String>,
}

pub(crate) fn hash(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// `update`'s (updated, skipped, unchanged), each sorted by path.
pub(crate) type UpdateLists = (Vec<String>, Vec<String>, Vec<String>);

/// The engine for one workspace: its root, its vcs (`git` or `none`) and the version
/// written into manifests.
pub(crate) struct Engine<'a> {
    pub root: &'a Path,
    pub vcs: String,
    pub version: String,
}

fn io(op: &'static str, path: String) -> impl FnOnce(std::io::Error) -> Error {
    move |source| Error::Io { op, path, source }
}

impl Engine<'_> {
    fn local(&self) -> bool {
        self.vcs == "none"
    }

    fn path(&self, rel: &str) -> PathBuf {
        let mut p = self.root.to_path_buf();
        for part in rel.split('/') {
            p.push(part);
        }
        p
    }

    /// Go's `HasRoom`: a real folder, plus its marker when the room has one.
    pub fn has_room(&self, room: &str) -> bool {
        if !room_exists(&self.path(room)) {
            return false;
        }
        match ROOMS.iter().find(|(r, _)| *r == room).and_then(|(_, m)| *m) {
            None => true,
            Some(marker) => {
                let rel = format!("{room}/{marker}");
                write::through_symlink(self.root, &rel).is_none()
                    && std::fs::symlink_metadata(self.path(&rel)).is_ok()
            }
        }
    }

    /// The trees a workspace holds: the root, then each present room.
    fn trees(&self) -> Vec<(&'static str, &'static str)> {
        let mut out = vec![("root", "")];
        out.extend(
            ROOMS
                .iter()
                .filter(|(r, _)| self.has_room(r))
                .map(|(r, _)| (*r, *r)),
        );
        out
    }

    fn walk(&self, trees: &[(&str, &str)]) -> Vec<(String, &'static [u8])> {
        trees
            .iter()
            .flat_map(|(t, prefix)| walk_tree(t, prefix, self.local()))
            .collect()
    }

    pub fn build_manifest(&self) -> Manifest {
        let files = self
            .walk(&self.trees())
            .into_iter()
            .map(|(rel, data)| (rel, hash(data)))
            .collect();
        Manifest {
            version: self.version.clone(),
            files,
        }
    }

    fn manifest_path(&self) -> PathBuf {
        self.root.join(".daiku").join("manifest.json")
    }

    /// Go's `ReadManifest`: `Ok(None)` when the file doesn't exist; otherwise Go's
    /// error text.
    pub fn read_manifest(&self) -> Result<Option<Manifest>, String> {
        let path = self.manifest_path();
        let data = match std::fs::read(&path) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(format!(
                    "open {}: {}",
                    shown(self.root, ".daiku/manifest.json"),
                    gofmt::io_error_text(&e)
                ));
            }
        };
        let value = gojson::parse(&data)?;
        serde_json::from_value(value)
            .map(Some)
            .map_err(|e| format!("json: {e}"))
    }

    pub fn write_manifest(&self, m: &Manifest) -> Result<(), Error> {
        let data = json::to_pretty(m)
            .expect("a manifest (strings and a map of strings) always serializes");
        write::write_file(self.root, &self.manifest_path(), &data)
    }

    /// Go's `Engine.write`: refuse a symlink on the path below the root, then
    /// create the folders and write.
    pub fn write(&self, rel: &str, data: &[u8]) -> Result<(), Error> {
        if let Some(link) = write::through_symlink(self.root, rel) {
            return Err(Error::WriteThroughSymlink {
                rel: rel.to_string(),
                link,
            });
        }
        let dest = self.path(rel);
        if let Some(parent) = dest.parent() {
            crate::write::mkdir_all(parent)?;
        }
        std::fs::write(&dest, data).map_err(io("open", shown(self.root, rel)))
    }

    /// Go's `keptDirs`: in `vcs: none`, the folders `.gitkeep` files would hold.
    fn kept_dirs(&self, trees: &[(&str, &str)]) -> Result<(), Error> {
        if !self.local() {
            return Ok(());
        }
        for (t, prefix) in trees {
            for (rel, _) in walk_tree(t, prefix, false) {
                if let Some(dir) = rel.strip_suffix("/.gitkeep") {
                    let p = self.path(dir);
                    crate::write::mkdir_all(&p)?;
                }
            }
        }
        Ok(())
    }

    /// Go's `Engine.Scaffold`, without `AGENTS.md`: the root tree, seeds, the vcs
    /// record and the manifest. Returns the files written and whether git init ran.
    pub fn scaffold(
        &self,
        force: bool,
        no_git: bool,
        env: &Env,
    ) -> Result<(Vec<String>, bool), Error> {
        let mut written = Vec::new();
        if self.root.as_os_str().is_empty() {
            // Go's os.MkdirAll("") fails where Rust's create_dir_all("") succeeds.
            // ENOENT, and Windows' ERROR_FILE_NOT_FOUND, are both 2.
            let source = std::io::Error::from_raw_os_error(2);
            return Err(Error::Io {
                op: "mkdir",
                path: String::new(),
                source,
            });
        }
        crate::write::mkdir_all(self.root)?;
        let mut git_init = false;
        if !no_git && !self.local() && std::fs::metadata(self.root.join(".git")).is_err() {
            run_git(self.root, &["init"], env)?;
            git_init = true;
        }
        for (rel, data) in self.walk(&[("root", "")]) {
            if !force && std::fs::metadata(self.path(&rel)).is_ok() {
                continue; // an existing, user-owned file
            }
            self.write(&rel, data)?;
            written.push(rel);
        }
        self.kept_dirs(&[("root", "")])?;
        written.extend(self.write_seeds()?);
        if self.local()
            && let Some(created) = self.record_local()?
        {
            written.push(created);
        }
        self.write_manifest(&self.build_manifest())?;
        Ok((written, git_init))
    }

    fn write_seeds(&self) -> Result<Vec<String>, Error> {
        let mut written = Vec::new();
        for (rel, data) in seeds() {
            if std::fs::metadata(self.path(rel)).is_ok() {
                continue;
            }
            self.write(rel, &data)?;
            written.push(rel.to_string());
        }
        Ok(written)
    }

    /// Go's `recordLocal`: set `vcs: none` in `.daiku/settings.json`; the path when
    /// this created the file.
    fn record_local(&self) -> Result<Option<String>, Error> {
        let rel = ".daiku/settings.json";
        let file = shown(self.root, rel);
        let (mut m, created) = match std::fs::read(self.path(rel)) {
            Ok(data) => match decode_first(&data) {
                Err(detail) => return Err(Error::InitSettingsParse { file, detail }),
                Ok(serde_json::Value::Object(obj)) => {
                    if obj.get("vcs").and_then(|v| v.as_str()) == Some("none") {
                        return Ok(None);
                    }
                    (obj, false)
                }
                Ok(_) => return Err(Error::InitSettingsNotObject { file }),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (serde_json::Map::new(), true),
            Err(source) => {
                return Err(Error::Io {
                    op: "open",
                    path: file,
                    source,
                });
            }
        };
        m.insert("vcs".into(), "none".into());
        let data = json::to_pretty(&m).expect("parsed JSON values always serialize");
        self.write(rel, &data)?;
        Ok(created.then(|| rel.to_string()))
    }

    /// Go's `Engine.Update`, without `AGENTS.md`: (updated, skipped, unchanged), each
    /// sorted by path.
    pub fn update(&self, dir_shown: &str) -> Result<UpdateLists, Error> {
        let cur = match self.read_manifest() {
            Ok(Some(m)) => m,
            Ok(None) => {
                if !std::fs::metadata(self.root.join(".daiku")).is_ok_and(|m| m.is_dir()) {
                    return Err(Error::NotAWorkspace {
                        prefix: "daiku update",
                        dir: dir_shown.to_string(),
                    });
                }
                Manifest {
                    version: String::new(),
                    files: BTreeMap::new(),
                }
            }
            Err(detail) => {
                return Err(Error::Manifest {
                    prefix: "daiku update",
                    detail,
                });
            }
        };
        let next = self.build_manifest();
        self.kept_dirs(&self.trees())?;
        let (mut updated, mut skipped, mut unchanged) = (Vec::new(), Vec::new(), Vec::new());
        for (rel, next_hash) in &next.files {
            let Ok(cur_data) = std::fs::read(self.path(rel)) else {
                self.write_managed(rel)?;
                updated.push(rel.clone());
                continue;
            };
            let h = hash(&cur_data);
            if &h == next_hash {
                unchanged.push(rel.clone());
            } else if cur.files.get(rel) == Some(&h) {
                self.write_managed(rel)?;
                updated.push(rel.clone());
            } else {
                skipped.push(rel.clone());
            }
        }
        self.write_manifest(&next)?;
        Ok((updated, skipped, unchanged))
    }

    fn write_managed(&self, rel: &str) -> Result<(), Error> {
        let (t, inner) = match ROOMS
            .iter()
            .find(|(r, _)| rel.starts_with(&format!("{r}/")))
        {
            Some((r, _)) => (*r, &rel[r.len() + 1..]),
            None => ("root", rel),
        };
        let file = tree(t)
            .get_file(format!("{t}/{inner}"))
            .ok_or_else(|| Error::Manifest {
                prefix: "daiku",
                detail: format!("open {inner}: file does not exist"),
            })?;
        self.write(rel, file.contents())
    }

    /// Go's `EnsureRoom`: `Ok(None)` when the room exists; otherwise the files
    /// written, sorted.
    pub fn ensure_room(&self, room: &str, dir_shown: &str) -> Result<Option<Vec<String>>, Error> {
        if !ROOMS.iter().any(|(r, _)| *r == room) {
            return Err(Error::UnknownRoom {
                room: room.to_string(),
            });
        }
        if std::fs::symlink_metadata(self.path(room)).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(Error::RoomSymlink {
                room: room.to_string(),
            });
        }
        if self.has_room(room) {
            return Ok(None);
        }
        let mut m = match self.read_manifest() {
            Ok(Some(m)) => m,
            Ok(None) => {
                if !std::fs::metadata(self.root.join(".daiku")).is_ok_and(|m| m.is_dir()) {
                    return Err(Error::NotAWorkspace {
                        prefix: "daiku",
                        dir: dir_shown.to_string(),
                    });
                }
                Manifest {
                    version: self.version.clone(),
                    files: BTreeMap::new(),
                }
            }
            Err(detail) => {
                return Err(Error::Manifest {
                    prefix: "daiku",
                    detail,
                });
            }
        };
        crate::write::mkdir_all(&self.path(room))?;
        let mut written = Vec::new();
        for (rel, data) in walk_tree(room, room, self.local()) {
            self.write(&rel, data)?;
            m.files.insert(rel.clone(), hash(data));
            written.push(rel);
        }
        self.kept_dirs(&[(room, room)])?;
        self.write_manifest(&m)?;
        written.sort();
        Ok(Some(written))
    }
}

/// Go's `os.Stat(dir/room)` as a real folder, not a link.
fn room_exists(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
}

/// daiku's seed files, as Go's `icm.seeds` writes them, in order.
fn seeds() -> Vec<(&'static str, Vec<u8>)> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct SettingsSeed<'a> {
        base_source: &'a str,
        gates: &'a str,
        source_dir: &'a str,
        adr_dir: &'a str,
        sync_file: &'a str,
        overview: &'a str,
    }
    let d = crate::settings::Settings::default();
    let settings = SettingsSeed {
        base_source: &d.base_source,
        gates: &d.gates,
        source_dir: &d.source_dir,
        adr_dir: &d.adr_dir,
        sync_file: &d.sync_file,
        overview: &d.overview,
    };
    let pretty = |v: &dyn erased::Ser| v.pretty();
    vec![
        (".daiku/settings.json", pretty(&settings)),
        (
            "STATE.json",
            pretty(&serde_json::json!({ "schema": 1, "changes": {} })),
        ),
        ("LOG.md", Vec::new()),
        ("REPOS.json", pretty(&serde_json::json!({ "repos": {} }))),
    ]
}

/// A small object-safe wrapper so `seeds` can hold different `Serialize` types.
mod erased {
    pub trait Ser {
        fn pretty(&self) -> Vec<u8>;
    }
    impl<T: serde::Serialize> Ser for T {
        fn pretty(&self) -> Vec<u8> {
            crate::json::to_pretty(self).unwrap_or_default()
        }
    }
}

/// Go's `json.Decoder.Decode` of the first value plus `dec.More()`: the value, or
/// Go's message (`EOF`, `unexpected EOF`, a syntax error, or trailing data).
fn decode_first(data: &[u8]) -> Result<serde_json::Value, String> {
    if data.iter().all(|b| b.is_ascii_whitespace()) {
        return Err("EOF".into());
    }
    match gojson::check(data) {
        Ok(()) => gojson::parse(data),
        Err(e) if e == "unexpected end of JSON input" => Err("unexpected EOF".into()),
        Err(e) if e.ends_with("after top-level value") => {
            // A first value, then more: Decode takes the first; More() reports the
            // rest unless the next byte closes something.
            let end = gojson::first_value_len(data).unwrap_or(data.len());
            let rest = data[end..].iter().find(|b| !b.is_ascii_whitespace());
            if matches!(rest, Some(b'}') | Some(b']')) {
                gojson::parse(&data[..end])
            } else {
                Err("trailing data after the object".into())
            }
        }
        Err(e) => Err(e),
    }
}

/// Runs git in `dir` through `Env`, as Go's `runGit`.
pub(crate) fn run_git(dir: &Path, args: &[&str], env: &Env) -> Result<(), Error> {
    let cmd = Cmd::new("git").args(args.iter().copied()).dir(dir);
    let joined = args.join(" ");
    match env.process.output(&cmd) {
        Ok(out) if out.success() => Ok(()),
        Ok(out) => {
            let mut output = String::from_utf8_lossy(&out.stdout).into_owned();
            output.push_str(&String::from_utf8_lossy(&out.stderr));
            let err = match out.code {
                Some(c) => format!("exit status {c}"),
                None => "signal: killed".into(),
            };
            Err(Error::Git {
                args: joined,
                err,
                output,
            })
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(Error::Git {
            args: joined,
            err: "exec: \"git\": executable file not found in $PATH".into(),
            output: String::new(),
        }),
        Err(e) => Err(Error::Git {
            args: joined,
            err: e.to_string(),
            output: String::new(),
        }),
    }
}
