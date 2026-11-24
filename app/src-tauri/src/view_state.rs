//! `view-state.json`: the open workspaces, recents, selection, tab, focus and window
//! size, kept in the app config folder (design D1-D5). The frontend never touches the
//! file; it goes through the commands. Nothing here takes or writes under a workspace.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};

use crate::envelope::shown;

/// Recents hold at most this many roots.
const RECENTS_CAP: usize = 10;
const FILE: &str = "view-state.json";

#[derive(Serialize, Deserialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Tab {
    Plan,
    #[default]
    Build,
}

#[derive(Serialize, Deserialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Focus {
    #[default]
    Split,
    Viewer,
    Board,
}

#[derive(Serialize, Deserialize, Clone, Copy, Default, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Pane {
    #[default]
    Viewer,
    List,
}

/// Logical pixels.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default, rename_all = "camelCase")]
pub struct ViewState {
    pub schema: u32,
    /// Raw roots, chip order.
    pub open: Vec<String>,
    /// Raw roots, most recent first, at most 10.
    pub recents: Vec<String>,
    pub selected: Option<String>,
    pub tab: Tab,
    pub focus: Focus,
    pub pane: Pane,
    pub window: Option<Size>,
}

impl Default for ViewState {
    fn default() -> Self {
        ViewState {
            schema: 1,
            open: Vec::new(),
            recents: Vec::new(),
            selected: None,
            tab: Tab::default(),
            focus: Focus::default(),
            pane: Pane::default(),
            window: None,
        }
    }
}

/// A root with what to show for it. `root` is raw and only ever passed back to
/// commands; `name` and `shown` are safe to display (design D6, ADR-0008).
#[derive(Serialize, Clone, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRef {
    pub root: String,
    pub name: String,
    pub shown: String,
    pub is_workspace: bool,
}

/// Is `root` a daiku workspace? Reads only whether `.daiku/settings.json` is a file
/// (design D7).
pub fn probe(root: &str) -> WorkspaceRef {
    let path = Path::new(root);
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.to_string());
    WorkspaceRef {
        root: root.to_string(),
        name: shown(&name),
        shown: shown(root),
        is_workspace: path.join(".daiku").join("settings.json").is_file(),
    }
}

pub struct ViewStateStore {
    path: PathBuf,
    state: Mutex<ViewState>,
    /// Roots the person opened this session, beyond the saved open set: a root
    /// that `probe_workspace` found to be a workspace. Commands that read a
    /// workspace accept only these and the open set (S-0020 security finding 1).
    allowed: Mutex<Vec<String>>,
}

fn dedupe(roots: Vec<String>) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for r in roots {
        if !seen.contains(&r) {
            seen.push(r);
        }
    }
    seen
}

impl ViewStateStore {
    /// Reads `<dir>/view-state.json`. A missing, unreadable or unparseable file gives
    /// the defaults and is left as it is (design D4). Roots that are no longer
    /// workspaces are dropped in memory (design D5). Never writes.
    pub fn load(dir: &Path) -> ViewStateStore {
        let path = dir.join(FILE);
        let mut state = fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice::<ViewState>(&b).ok())
            .unwrap_or_default();
        state.open = dedupe(state.open)
            .into_iter()
            .filter(|r| probe(r).is_workspace)
            .collect();
        state.recents = dedupe(state.recents)
            .into_iter()
            .filter(|r| probe(r).is_workspace)
            .take(RECENTS_CAP)
            .collect();
        let keep = state.selected.take().filter(|s| state.open.contains(s));
        state.selected = keep.or_else(|| state.open.first().cloned());
        ViewStateStore {
            path,
            state: Mutex::new(state),
            allowed: Mutex::new(Vec::new()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, ViewState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Records a root the person opened this session (a probe that found a
    /// workspace), so commands accept it before the next save lands.
    pub fn allow(&self, root: &str) {
        let mut allowed = self.allowed.lock().unwrap_or_else(|e| e.into_inner());
        if !allowed.iter().any(|r| r == root) {
            allowed.push(root.to_string());
        }
    }

    /// Whether a command may read the workspace at `root`: it's in the open set, or
    /// was opened this session.
    pub fn allows(&self, root: &str) -> bool {
        self.lock().open.iter().any(|r| r == root)
            || self
                .allowed
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .iter()
                .any(|r| r == root)
    }

    /// A copy of the current state.
    pub fn current(&self) -> ViewState {
        self.lock().clone()
    }

    /// Replaces everything but the stored window size, then writes.
    pub fn save(&self, mut view: ViewState) {
        let mut state = self.lock();
        view.window = state.window;
        *state = view;
        self.write(&state);
    }

    /// Updates the window size in memory only.
    pub fn set_window(&self, size: Size) {
        self.lock().window = Some(size);
    }

    /// Writes the current state.
    pub fn flush(&self) {
        let state = self.lock();
        self.write(&state);
    }

    /// A ref for every root in `open` then `recents`, without repeats.
    pub fn refs(&self) -> Vec<WorkspaceRef> {
        let state = self.lock();
        let mut roots: Vec<&String> = Vec::new();
        for r in state.open.iter().chain(state.recents.iter()) {
            if !roots.contains(&r) {
                roots.push(r);
            }
        }
        roots.into_iter().map(|r| probe(r)).collect()
    }

    /// Atomic write (design D3): a temp file beside the target, then rename. A
    /// failure is logged and ignored: view state is a convenience.
    fn write(&self, state: &ViewState) {
        if let Err(e) = self.try_write(state) {
            eprintln!("daiku-app: couldn't write {}: {e}", self.path.display());
        }
    }

    fn try_write(&self, state: &ViewState) -> std::io::Result<()> {
        let dir = self.path.parent().unwrap_or(Path::new("."));
        fs::create_dir_all(dir)?;
        let tmp = dir.join(format!("{FILE}.tmp"));
        let json = serde_json::to_vec_pretty(state).map_err(std::io::Error::other)?;
        fs::write(&tmp, json)?;
        fs::rename(&tmp, &self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn workspace(parent: &Path, name: &str) -> String {
        let root = parent.join(name);
        fs::create_dir_all(root.join(".daiku")).unwrap();
        fs::write(root.join(".daiku/settings.json"), "{}").unwrap();
        root.to_string_lossy().into_owned()
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn missing_file_gives_defaults_and_writes_nothing() {
        let cfg = TempDir::new().unwrap();
        let store = ViewStateStore::load(cfg.path());
        assert_eq!(store.current(), ViewState::default());
        assert!(names(cfg.path()).is_empty());
    }

    #[test]
    fn corrupt_file_gives_defaults_and_is_untouched() {
        let cfg = TempDir::new().unwrap();
        let bytes: &[u8] = b"{ not json \xff\xfe";
        fs::write(cfg.path().join(FILE), bytes).unwrap();
        let store = ViewStateStore::load(cfg.path());
        assert_eq!(store.current(), ViewState::default());
        assert_eq!(fs::read(cfg.path().join(FILE)).unwrap(), bytes);
    }

    #[test]
    fn round_trip_keeps_everything() {
        let cfg = TempDir::new().unwrap();
        let ws = TempDir::new().unwrap();
        let a = workspace(ws.path(), "a");
        let b = workspace(ws.path(), "b");
        let store = ViewStateStore::load(cfg.path());
        store.set_window(Size {
            width: 1100.0,
            height: 700.0,
        });
        let view = ViewState {
            open: vec![a.clone(), b.clone()],
            recents: vec![b.clone(), a.clone()],
            selected: Some(b.clone()),
            tab: Tab::Plan,
            focus: Focus::Board,
            pane: Pane::List,
            ..ViewState::default()
        };
        store.save(view);
        let want = store.current();
        assert_eq!(
            want.window,
            Some(Size {
                width: 1100.0,
                height: 700.0
            })
        );
        let again = ViewStateStore::load(cfg.path());
        assert_eq!(again.current(), want);
        assert_eq!(names(cfg.path()), vec![FILE.to_string()]);
    }

    #[test]
    fn prune_drops_gone_and_non_workspaces_and_moves_selected() {
        let cfg = TempDir::new().unwrap();
        let ws = TempDir::new().unwrap();
        let gone = workspace(ws.path(), "gone");
        let real = workspace(ws.path(), "real");
        let plain = ws.path().join("plain");
        fs::create_dir(&plain).unwrap();
        let plain = plain.to_string_lossy().into_owned();
        fs::remove_dir_all(&gone).unwrap();
        let view = ViewState {
            open: vec![gone.clone(), plain.clone(), real.clone(), real.clone()],
            recents: vec![gone.clone(), real.clone(), plain],
            selected: Some(gone),
            ..ViewState::default()
        };
        fs::write(cfg.path().join(FILE), serde_json::to_vec(&view).unwrap()).unwrap();
        let store = ViewStateStore::load(cfg.path());
        let got = store.current();
        assert_eq!(got.open, vec![real.clone()]);
        assert_eq!(got.recents, vec![real.clone()]);
        assert_eq!(got.selected, Some(real));
    }

    #[test]
    fn prune_leaves_no_selection_when_nothing_is_open() {
        let cfg = TempDir::new().unwrap();
        let view = ViewState {
            open: vec!["/no/such/folder".into()],
            selected: Some("/no/such/folder".into()),
            ..ViewState::default()
        };
        fs::write(cfg.path().join(FILE), serde_json::to_vec(&view).unwrap()).unwrap();
        let got = ViewStateStore::load(cfg.path()).current();
        assert!(got.open.is_empty());
        assert_eq!(got.selected, None);
    }

    #[test]
    fn save_keeps_the_stored_window() {
        let cfg = TempDir::new().unwrap();
        let store = ViewStateStore::load(cfg.path());
        store.set_window(Size {
            width: 900.0,
            height: 600.0,
        });
        store.save(ViewState {
            window: Some(Size {
                width: 1.0,
                height: 2.0,
            }),
            ..ViewState::default()
        });
        let want = Some(Size {
            width: 900.0,
            height: 600.0,
        });
        assert_eq!(store.current().window, want);
        assert_eq!(ViewStateStore::load(cfg.path()).current().window, want);
    }

    #[test]
    fn probe_shows_a_visible_escape_and_keeps_the_root() {
        let ws = TempDir::new().unwrap();
        let root = ws.path().join("we\u{1b}ird");
        fs::create_dir(&root).unwrap();
        let raw = root.to_string_lossy().into_owned();
        let got = probe(&raw);
        assert_eq!(got.root, raw);
        assert_eq!(got.name, "we\\x1bird");
        assert!(got.shown.ends_with("we\\x1bird"));
        assert!(!got.shown.contains('\u{1b}'));
        assert!(!got.is_workspace);
    }

    #[test]
    fn probe_of_a_workspace_and_a_plain_folder() {
        let ws = TempDir::new().unwrap();
        let real = workspace(ws.path(), "real");
        assert!(probe(&real).is_workspace);
        assert_eq!(probe(&real).name, "real");
        assert!(!probe(&ws.path().to_string_lossy()).is_workspace);
    }

    #[test]
    fn save_writes_nothing_under_a_workspace() {
        let cfg = TempDir::new().unwrap();
        let ws = TempDir::new().unwrap();
        let a = workspace(ws.path(), "a");
        let before = names(Path::new(&a));
        let before_daiku = names(&Path::new(&a).join(".daiku"));
        let store = ViewStateStore::load(cfg.path());
        store.save(ViewState {
            open: vec![a.clone()],
            recents: vec![a.clone()],
            selected: Some(a.clone()),
            ..ViewState::default()
        });
        store.flush();
        assert_eq!(names(Path::new(&a)), before);
        assert_eq!(names(&Path::new(&a).join(".daiku")), before_daiku);
        assert_eq!(names(ws.path()), vec!["a".to_string()]);
    }

    #[test]
    fn allows_only_open_or_opened_roots() {
        let dir = tempfile::TempDir::new().unwrap();
        let ws = workspace(dir.path(), "ws");
        let store = ViewStateStore::load(&dir.path().join("cfg"));
        assert!(!store.allows(&ws));
        assert!(!store.allows("/"));
        store.allow(&ws);
        assert!(store.allows(&ws));
        assert!(!store.allows("/"));
        let other = workspace(dir.path(), "other");
        let mut view = store.current();
        view.open = vec![other.clone()];
        store.save(view);
        assert!(store.allows(&other));
    }
}
