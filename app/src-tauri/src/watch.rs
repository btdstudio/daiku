//! The per-workspace file watcher (S-0019): one debounced worker per open
//! root. A worker watches the paths `daiku_core::Workspace::watch_paths`
//! returns, makes each event path relative to the root, classifies it with
//! `daiku_core::watch::classify` and emits a `workspace-changed` invalidation
//! payload through an [`EventSink`]. [`WatchRegistry`] holds one worker per
//! authorized open root and is reconciled by the `sync_watchers` command.
//! Nothing is written under a workspace.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use daiku_core::settings::Settings;
use daiku_core::watch::classify;
use daiku_core::{Changed, WatchPath, Workspace, gofmt};
use notify::event::ModifyKind;
use notify::{EventKind, RecursiveMode};
use notify_debouncer_full::{DebounceEventResult, DebouncedEvent, new_debouncer};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::env::app_env;
use crate::view_state::ViewStateStore;

/// The debounce window for one root's watcher (design D2).
const DEBOUNCE: Duration = Duration::from_millis(150);

/// How long the worker waits for the next debounced batch before re-checking
/// whether it should stop.
const IDLE: Duration = Duration::from_millis(200);

/// The `workspace-changed` invalidation payload (design D4). It names the root
/// that changed and the deduplicated classified changes, or — when
/// `reload_all` is set — asks the frontend to refresh every query for the root.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceChanged {
    pub root: String,
    pub changed: Vec<Changed>,
    pub reload_all: bool,
}

impl WorkspaceChanged {
    /// A reload-everything payload for `root`: an empty `changed` list and
    /// `reload_all` set.
    fn reload_all(root: &str) -> WorkspaceChanged {
        WorkspaceChanged {
            root: root.to_string(),
            changed: Vec::new(),
            reload_all: true,
        }
    }
}

/// The Tauri event a worker's payload is sent on (design D4).
pub const EVENT: &str = "workspace-changed";

/// Where classified invalidation payloads go (design D4). Production sends
/// Tauri's [`EVENT`] (see [`AppEventSink`]); tests use a channel-backed sink so
/// they never need a Tauri window.
pub trait EventSink: Send + Sync {
    fn emit(&self, payload: WorkspaceChanged);
}

/// The production sink: the payload becomes Tauri's [`EVENT`].
pub struct AppEventSink {
    app: AppHandle,
}

impl AppEventSink {
    pub fn new(app: AppHandle) -> AppEventSink {
        AppEventSink { app }
    }
}

impl EventSink for AppEventSink {
    fn emit(&self, payload: WorkspaceChanged) {
        let _ = self.app.emit(EVENT, payload);
    }
}

/// Folders whose own creation, removal or rename can change what the views show
/// although `classify` names nothing for them.
const CONTAINERS: [&str; 4] = ["plan", "plan/specs", "build", "build/tickets"];

/// Whether an event creates, removes or renames its path, rather than changing
/// what's in it.
fn structural(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(_))
    )
}

/// The ADR folder the settings name, relative to the root and empty for the root
/// itself. `None` when the settings can't be read, as while they're half written.
fn adr_dir(ws: &Workspace) -> Option<PathBuf> {
    let dir = ws.settings().ok()?.value.effective.adr_dir;
    // An empty setting means the default folder.
    let dir = if dir.is_empty() {
        Settings::default().adr_dir
    } else {
        dir
    };
    let dir = gofmt::to_slash(&gofmt::clean(&dir));
    Some(if dir == "." {
        PathBuf::new()
    } else {
        dir.into()
    })
}

/// The watches an ADR folder needs beyond core's: none inside a room, which is
/// watched whole; otherwise the folder and each folder above it, so that creating
/// any of them is seen.
fn adr_watches(ws: &Workspace, dir: &Path) -> Vec<WatchPath> {
    let root = ws.root();
    let rooms = ws.watch_paths();
    let mut above: Vec<&Path> = dir
        .ancestors()
        .filter(|p| !p.as_os_str().is_empty())
        .collect();
    above.reverse();
    if above
        .first()
        .is_some_and(|top| rooms.iter().any(|w| w.path == root.join(top)))
    {
        return Vec::new();
    }
    above
        .into_iter()
        .map(|p| WatchPath {
            path: root.join(p),
            recursive: false,
        })
        .collect()
}

/// Whether an event on `rel` can change the ADRs listed from `dir`: a Markdown
/// file in it, the folder itself, or a folder above it that was created, removed
/// or renamed.
fn touches_adrs(dir: &Path, rel: &Path, kind: &EventKind) -> bool {
    rel == dir
        || (rel.parent() == Some(dir) && rel.extension().is_some_and(|e| e == "md"))
        || (structural(kind) && dir.starts_with(rel))
}

/// Convert one debounced batch into a payload, or `None` when there is nothing
/// to emit (design D4).
///
/// * `error` — the debouncer reported a watcher error → reload all.
/// * Any event with `Flag::Rescan` → reload all (events were lost).
/// * Any event path equal to `root` → reload all (the root itself changed).
/// * Any event path that can't be made relative to `root` → reload all.
/// * One of [`CONTAINERS`] created, removed or renamed → reload all (a whole
///   folder can move with only its own event).
/// * Otherwise classify each relative path, drop ignored ones and deduplicate
///   the rest in first-seen order. An all-ignored batch is `None`.
/// * A path that can change the ADRs listed from `adr`, the folder the settings
///   name, is also [`Changed::Adr`]: `classify` knows only the default folder.
fn classify_batch(
    root: &str,
    adr: &Path,
    events: &[DebouncedEvent],
    error: bool,
) -> Option<WorkspaceChanged> {
    if error {
        return Some(WorkspaceChanged::reload_all(root));
    }
    let base = Path::new(root);
    let mut changed: Vec<Changed> = Vec::new();
    let mut seen = HashSet::new();
    for event in events {
        if event.need_rescan() {
            return Some(WorkspaceChanged::reload_all(root));
        }
        for path in &event.paths {
            let rel = match path.strip_prefix(base) {
                Ok(rel) => rel,
                Err(_) => return Some(WorkspaceChanged::reload_all(root)),
            };
            if rel.as_os_str().is_empty() {
                // The root itself changed (renamed, removed, ...): everything
                // relative to it is now suspect.
                return Some(WorkspaceChanged::reload_all(root));
            }
            if structural(&event.kind) && CONTAINERS.iter().any(|c| rel == Path::new(c)) {
                return Some(WorkspaceChanged::reload_all(root));
            }
            let adr = touches_adrs(adr, rel, &event.kind).then_some(Changed::Adr);
            for c in classify(rel).into_iter().chain(adr) {
                if seen.insert(c.clone()) {
                    changed.push(c);
                }
            }
        }
    }
    if changed.is_empty() {
        None
    } else {
        Some(WorkspaceChanged {
            root: root.to_string(),
            changed,
            reload_all: false,
        })
    }
}

/// Which watch paths a worker has registered and which it still wants to: their
/// folders don't exist, vanished, or couldn't be watched. It never creates one
/// (design D3).
#[derive(Debug, Default)]
struct WatchPlan {
    missing: Vec<WatchPath>,
    registered: Vec<WatchPath>,
    /// The missing paths whose last registration failed, so a repeated failure
    /// isn't reported again.
    failed: HashSet<PathBuf>,
}

impl WatchPlan {
    fn new(watch: Vec<WatchPath>) -> WatchPlan {
        WatchPlan {
            missing: watch,
            ..WatchPlan::default()
        }
    }

    /// Try to register every missing path that exists, one at a time, so one
    /// failure doesn't hold back the rest. A failed path stays missing. Returns
    /// whether the views may now be stale: a path was registered (changes made
    /// before then went unseen) or failed for the first time.
    fn register(
        &mut self,
        exists: impl Fn(&Path) -> bool,
        mut watch: impl FnMut(&WatchPath) -> bool,
    ) -> bool {
        let mut stale = false;
        for w in std::mem::take(&mut self.missing) {
            if !exists(&w.path) {
                self.failed.remove(&w.path);
                self.missing.push(w);
            } else if watch(&w) {
                self.failed.remove(&w.path);
                self.registered.push(w);
                stale = true;
            } else {
                stale |= self.failed.insert(w.path.clone());
                self.missing.push(w);
            }
        }
        stale
    }

    /// Stop wanting `old` and want `new` instead. Returns the registered paths
    /// that are no longer wanted.
    fn replace(&mut self, old: &[WatchPath], new: Vec<WatchPath>) -> Vec<WatchPath> {
        let dropped = |w: &WatchPath| old.contains(w) && !new.contains(w);
        self.missing.retain(|w| !dropped(w));
        self.failed
            .retain(|p| !old.iter().any(|w| &w.path == p && !new.contains(w)));
        let (gone, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.registered)
            .into_iter()
            .partition(dropped);
        self.registered = kept;
        self.missing
            .extend(new.into_iter().filter(|w| !old.contains(w)));
        gone
    }

    /// Move the registered paths `lost` accepts back to missing and return them.
    fn take_lost(&mut self, lost: impl Fn(&Path) -> bool) -> Vec<WatchPath> {
        let (gone, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.registered)
            .into_iter()
            .partition(|w| lost(&w.path));
        self.registered = kept;
        self.missing.extend(gone.iter().cloned());
        gone
    }
}

type Debouncer = notify_debouncer_full::Debouncer<
    notify::RecommendedWatcher,
    notify_debouncer_full::RecommendedCache,
>;

/// The way out for one worker's payloads, shut when the worker is stopped.
struct Gate {
    sink: Arc<dyn EventSink>,
    shut: Mutex<bool>,
}

impl Gate {
    fn lock(&self) -> MutexGuard<'_, bool> {
        self.shut.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn is_shut(&self) -> bool {
        *self.lock()
    }

    fn shut(&self) {
        *self.lock() = true;
    }

    /// Emit unless shut. The lock is held while emitting, so nothing leaves after
    /// [`shut`](Self::shut) has returned.
    fn emit(&self, payload: WorkspaceChanged) {
        let shut = self.lock();
        if !*shut {
            self.sink.emit(payload);
        }
    }
}

/// A running per-root watcher (design D2-D3). Dropping it — or calling
/// [`Worker::stop`] — shuts its gate, so no further event for this root is
/// emitted afterwards, and its thread then ends by itself. Neither waits for the
/// thread, which may still be walking a large tree to register it.
pub struct Worker {
    gate: Arc<Gate>,
}

impl Worker {
    /// Stop the worker.
    pub fn stop(self) {}
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.gate.shut();
    }
}

/// Start a watcher worker for `root`, emitting classified payloads to `sink`
/// (design D2-D3).
pub fn spawn_worker(root: String, sink: Arc<dyn EventSink>) -> Worker {
    let gate = Arc::new(Gate {
        sink,
        shut: Mutex::new(false),
    });
    let thread_gate = gate.clone();
    thread::spawn(move || run_worker(root, &thread_gate));
    Worker { gate }
}

/// What one worker is watching.
struct Watched {
    ws: Workspace,
    plan: WatchPlan,
    /// The ADR folder as last read from the settings.
    adr: PathBuf,
}

impl Watched {
    fn new(root: &str) -> Watched {
        let ws = Workspace::open(root, app_env());
        let adr = adr_dir(&ws).unwrap_or_else(|| Settings::default().adr_dir.into());
        let mut watch = ws.watch_paths();
        watch.extend(adr_watches(&ws, &adr));
        Watched {
            plan: WatchPlan::new(watch),
            adr,
            ws,
        }
    }

    /// Read the ADR folder again and follow it if it moved. Returns the watches to
    /// drop.
    fn follow_adr_dir(&mut self) -> Vec<WatchPath> {
        let Some(dir) = adr_dir(&self.ws).filter(|dir| *dir != self.adr) else {
            return Vec::new();
        };
        let old = adr_watches(&self.ws, &self.adr);
        let new = adr_watches(&self.ws, &dir);
        self.adr = dir;
        self.plan.replace(&old, new)
    }
}

fn run_worker(root: String, gate: &Gate) {
    let mut watched = Watched::new(&root);

    let (tx, rx) = mpsc::channel::<DebounceEventResult>();
    let mut debouncer = match new_debouncer(DEBOUNCE, None, tx) {
        Ok(debouncer) => debouncer,
        Err(_) => {
            // A debouncer that can't start can't be trusted: reload all.
            gate.emit(WorkspaceChanged::reload_all(&root));
            return;
        }
    };

    watched
        .plan
        .register(Path::exists, |w| watch(&mut debouncer, w, gate));
    // Anything read before the watches were in place may have changed unseen.
    gate.emit(WorkspaceChanged::reload_all(&root));

    while !gate.is_shut() {
        match rx.recv_timeout(IDLE) {
            Ok(result) => {
                let payload = handle_batch(&root, &mut watched, &mut debouncer, gate, result);
                if let Some(payload) = payload {
                    gate.emit(payload);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    // Dropping `debouncer` stops its internal thread.
}

/// Bring the watches up to date with one batch, then say what to emit for it.
fn handle_batch(
    root: &str,
    watched: &mut Watched,
    debouncer: &mut Debouncer,
    gate: &Gate,
    result: DebounceEventResult,
) -> Option<WorkspaceChanged> {
    let events = result.as_deref().unwrap_or_default();
    for w in watched.follow_adr_dir() {
        let _ = debouncer.unwatch(&w.path);
    }
    let plan = &mut watched.plan;
    // A room that is gone, or was removed or renamed and is back, has lost its
    // watch on some platforms: drop it, to be registered afresh.
    let base = Path::new(root);
    let lost = plan.take_lost(|room| {
        room != base
            && (!room.exists()
                || events
                    .iter()
                    .any(|e| structural(&e.kind) && e.paths.iter().any(|p| p == room)))
    });
    for w in &lost {
        // The OS may have dropped it already.
        let _ = debouncer.unwatch(&w.path);
    }
    let registered = plan.register(Path::exists, |w| watch(debouncer, w, gate));
    if registered || !lost.is_empty() {
        return Some(WorkspaceChanged::reload_all(root));
    }
    // A watcher error reloads every query for this root.
    classify_batch(root, &watched.adr, events, result.is_err())
}

/// Register one path (design D2). A stopped worker registers nothing more.
fn watch(debouncer: &mut Debouncer, w: &WatchPath, gate: &Gate) -> bool {
    let mode = if w.recursive {
        RecursiveMode::Recursive
    } else {
        RecursiveMode::NonRecursive
    };
    !gate.is_shut() && debouncer.watch(&w.path, mode).is_ok()
}

/// One worker per open workspace root (design D1). [`sync`](Self::sync) is the
/// only way the set changes: it starts added roots, leaves running ones alone
/// and stops every root that left the set or lost authorization, so a closed root
/// cannot emit once it returns. It doesn't wait for worker threads, so it's safe
/// on the main thread. Dropping the registry stops every worker, which is the
/// application-exit path.
pub struct WatchRegistry {
    sink: Arc<dyn EventSink>,
    workers: Mutex<HashMap<String, Worker>>,
}

impl WatchRegistry {
    pub fn new(sink: Arc<dyn EventSink>) -> WatchRegistry {
        WatchRegistry {
            sink,
            workers: Mutex::new(HashMap::new()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, Worker>> {
        self.workers.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Reconcile the workers with `roots`, the complete desired open set. A root
    /// starts only when `allowed` accepts it — the same open/probed-root rule the
    /// read commands use — so no arbitrary folder is ever watched.
    pub fn sync(&self, roots: &[String], allowed: impl Fn(&str) -> bool) {
        let mut want: Vec<&String> = Vec::new();
        for root in roots {
            if !want.contains(&root) && allowed(root) {
                want.push(root);
            }
        }
        let mut workers = self.lock();
        // Stop the removed and disallowed roots first (design D3).
        let gone: Vec<String> = workers
            .keys()
            .filter(|root| !want.contains(root))
            .cloned()
            .collect();
        for root in gone {
            if let Some(worker) = workers.remove(&root) {
                worker.stop();
            }
        }
        for root in want {
            workers
                .entry(root.clone())
                .or_insert_with(|| spawn_worker(root.clone(), self.sink.clone()));
        }
    }
}

impl Drop for WatchRegistry {
    fn drop(&mut self) {
        self.lock().clear();
    }
}

/// Reconcile the watchers with `roots`, the complete set of open workspaces. A
/// plain lifecycle command like `view_state_save`, not a core envelope: the
/// frontend calls it after load, open and close. Only roots the read commands
/// accept are watched; watch failures reach the views as `reload_all` events.
#[tauri::command]
pub fn sync_watchers(app: AppHandle, registry: State<'_, WatchRegistry>, roots: Vec<String>) {
    registry.sync(&roots, |root| {
        app.try_state::<ViewStateStore>()
            .is_some_and(|store| store.allows(root))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    use notify::Event;
    use notify::event::{CreateKind, Flag, RemoveKind, RenameMode};

    fn default_adr() -> &'static Path {
        Path::new("plan/04_architecture/adr")
    }

    fn modify(root: &Path, rel: &str) -> DebouncedEvent {
        DebouncedEvent::new(
            Event::new(EventKind::Modify(ModifyKind::Any)).add_path(root.join(rel)),
            Instant::now(),
        )
    }

    fn rescan() -> DebouncedEvent {
        DebouncedEvent::new(
            Event::new(EventKind::Other).set_flag(Flag::Rescan),
            Instant::now(),
        )
    }

    #[test]
    fn classify_state_and_ticket() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();
        let events = [
            modify(root, "STATE.json"),
            modify(root, "plan/specs/4-x/tickets/S-1.md"),
        ];
        let payload =
            classify_batch(&root.to_string_lossy(), default_adr(), &events, false).unwrap();
        assert!(!payload.reload_all);
        assert_eq!(
            payload.changed,
            vec![
                Changed::State,
                Changed::Ticket {
                    id: "S-1".into(),
                    change: Some("4-x".into()),
                },
            ]
        );
    }

    #[test]
    fn duplicate_paths_produce_one_changed() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();
        let events = [modify(root, "STATE.json"), modify(root, "STATE.json")];
        let payload =
            classify_batch(&root.to_string_lossy(), default_adr(), &events, false).unwrap();
        assert_eq!(payload.changed, vec![Changed::State]);
    }

    #[test]
    fn dedup_preserves_first_seen_order() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();
        let events = [
            modify(root, "STATE.json"),
            modify(root, "plan/specs/4-x/tickets/S-1.md"),
            modify(root, "STATE.json"),
        ];
        let payload =
            classify_batch(&root.to_string_lossy(), default_adr(), &events, false).unwrap();
        assert_eq!(
            payload.changed,
            vec![
                Changed::State,
                Changed::Ticket {
                    id: "S-1".into(),
                    change: Some("4-x".into()),
                },
            ]
        );
    }

    #[test]
    fn ignored_only_batches_emit_nothing() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();
        let events = [
            modify(root, "CLAUDE.md"),
            modify(root, "README.md"),
            modify(root, ".DS_Store"),
        ];
        assert_eq!(
            classify_batch(&root.to_string_lossy(), default_adr(), &events, false),
            None
        );
    }

    #[test]
    fn rescan_reloads_all() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();
        let events = [modify(root, "STATE.json"), rescan()];
        let payload =
            classify_batch(&root.to_string_lossy(), default_adr(), &events, false).unwrap();
        assert_eq!(
            payload,
            WorkspaceChanged::reload_all(&root.to_string_lossy())
        );
    }

    #[test]
    fn callback_error_reloads_all() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();
        let payload = classify_batch(&root.to_string_lossy(), default_adr(), &[], true).unwrap();
        assert_eq!(
            payload,
            WorkspaceChanged::reload_all(&root.to_string_lossy())
        );
    }

    #[test]
    fn root_change_reloads_all() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();
        // A path equal to the root itself: the workspace root changed.
        let events = [modify(root, "")];
        let payload =
            classify_batch(&root.to_string_lossy(), default_adr(), &events, false).unwrap();
        assert_eq!(
            payload,
            WorkspaceChanged::reload_all(&root.to_string_lossy())
        );
    }

    #[test]
    fn outside_path_reloads_all() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();
        let other = tempfile::TempDir::new().unwrap();
        // A path from a different temp dir can't be made relative to `root`.
        let events = [modify(other.path(), "STATE.json")];
        let payload =
            classify_batch(&root.to_string_lossy(), default_adr(), &events, false).unwrap();
        assert_eq!(
            payload,
            WorkspaceChanged::reload_all(&root.to_string_lossy())
        );
    }

    #[test]
    fn workspace_changed_serializes_camel_case() {
        let payload = WorkspaceChanged {
            root: "/ws".into(),
            changed: vec![Changed::State],
            reload_all: false,
        };
        let json = serde_json::to_value(payload).unwrap();
        assert_eq!(json["root"], "/ws");
        assert_eq!(json["reloadAll"], false);
        assert_eq!(json["changed"][0]["kind"], "state");
        assert!(json.get("reload_all").is_none());
    }

    #[test]
    fn register_takes_what_exists_and_retries_the_rest_without_creating() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();
        let ws = Workspace::open(root, app_env());
        let mut plan = WatchPlan::new(ws.watch_paths());

        // Only the root exists; the five recursive rooms are missing.
        assert!(plan.register(Path::exists, |_| true));
        assert_eq!(
            plan.registered,
            vec![WatchPath {
                path: root.to_path_buf(),
                recursive: false,
            }]
        );
        assert_eq!(plan.missing.len(), 5);
        assert!(!plan.register(Path::exists, |_| true), "nothing new");

        // A room created later is registered, without creating the others.
        std::fs::create_dir(root.join("plan")).unwrap();
        assert!(plan.register(Path::exists, |_| true));
        assert_eq!(plan.registered.last().unwrap().path, root.join("plan"));
        assert_eq!(plan.missing.len(), 4);
        for w in &plan.missing {
            assert!(
                !w.path.exists(),
                "{:?} should not have been created",
                w.path
            );
        }

        // A lost room goes back to missing and can be registered again.
        let lost = plan.take_lost(|p| p == root.join("plan"));
        assert_eq!(lost.len(), 1);
        assert_eq!(plan.missing.len(), 5);
        assert!(plan.register(Path::exists, |_| true));
        assert_eq!(plan.missing.len(), 4);
    }

    #[test]
    fn a_failed_registration_holds_back_no_other_and_is_retried() {
        let watch: Vec<WatchPath> = ["a", "b", "c"]
            .map(|p| WatchPath {
                path: p.into(),
                recursive: true,
            })
            .into();
        let mut plan = WatchPlan::new(watch);

        assert!(plan.register(|_| true, |w| w.path != Path::new("a")));
        assert_eq!(plan.registered.len(), 2);
        assert_eq!(plan.missing.len(), 1);

        // The same failure again is not news.
        assert!(!plan.register(|_| true, |_| false));
        assert_eq!(plan.missing.len(), 1);

        assert!(plan.register(|_| true, |_| true));
        assert_eq!(plan.registered.len(), 3);
        assert!(plan.missing.is_empty());
        assert!(plan.failed.is_empty());
    }

    #[test]
    fn a_container_folder_created_removed_or_renamed_reloads_all() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();
        let at = root.to_string_lossy();
        for rel in CONTAINERS {
            for kind in [
                EventKind::Create(CreateKind::Folder),
                EventKind::Remove(RemoveKind::Folder),
                EventKind::Modify(ModifyKind::Name(RenameMode::Any)),
            ] {
                let event =
                    DebouncedEvent::new(Event::new(kind).add_path(root.join(rel)), Instant::now());
                assert_eq!(
                    classify_batch(&at, default_adr(), &[event], false),
                    Some(WorkspaceChanged::reload_all(&at)),
                    "{rel} {kind:?}"
                );
            }
        }
        // A change to what's in one is left to the paths inside it.
        assert_eq!(
            classify_batch(&at, default_adr(), &[modify(root, "build/tickets")], false),
            None
        );
    }

    #[test]
    fn a_custom_adr_folder_is_classified() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();
        let at = root.to_string_lossy();
        let adr = Path::new("docs/adr");
        let changed =
            |event: DebouncedEvent| classify_batch(&at, adr, &[event], false).map(|p| p.changed);
        assert_eq!(
            changed(modify(root, "docs/adr/0001-x.md")),
            Some(vec![Changed::Adr])
        );
        assert_eq!(changed(modify(root, "docs/adr")), Some(vec![Changed::Adr]));
        assert_eq!(changed(modify(root, "docs/adr/0001-x.md.swp")), None);
        assert_eq!(changed(modify(root, "docs/other.md")), None);
        assert_eq!(changed(modify(root, "docs")), None);
        let renamed = DebouncedEvent::new(
            Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Any)))
                .add_path(root.join("docs")),
            Instant::now(),
        );
        assert_eq!(changed(renamed), Some(vec![Changed::Adr]));

        // A folder inside a room keeps what `classify` says too.
        let payload = classify_batch(
            &at,
            Path::new("plan/decisions/log"),
            &[modify(root, "plan/decisions/log/0001-x.md")],
            false,
        )
        .unwrap();
        assert!(payload.changed.contains(&Changed::Adr), "{payload:?}");
        assert_eq!(payload.changed.len(), 2);
    }

    #[test]
    fn the_adr_folder_follows_the_settings() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = workspace(dir.path(), "ws");
        let at = Path::new(&root);
        let settings = |text: &str| put(&root, ".daiku/settings.json", text);
        let paths = |w: &Watched| -> Vec<PathBuf> {
            w.plan.missing.iter().map(|w| w.path.clone()).collect()
        };

        let mut watched = Watched::new(&root);
        assert_eq!(watched.adr, default_adr());
        assert_eq!(watched.plan.missing.len(), 6, "a room needs no more");

        settings(r#"{"adrDir": "./docs/adr/"}"#);
        assert!(watched.follow_adr_dir().is_empty());
        assert_eq!(watched.adr, Path::new("docs/adr"));
        assert_eq!(paths(&watched)[6..], [at.join("docs"), at.join("docs/adr")]);
        watched.plan.register(Path::exists, |_| true);
        std::fs::create_dir_all(at.join("docs/adr")).unwrap();
        watched.plan.register(Path::exists, |_| true);

        // Half-written settings change nothing.
        settings("{");
        assert!(watched.follow_adr_dir().is_empty());
        assert_eq!(watched.adr, Path::new("docs/adr"));

        // A shared parent stays; the folder no longer named is dropped.
        settings(r#"{"adrDir": "docs/decisions"}"#);
        let gone = watched.follow_adr_dir();
        assert_eq!(gone.len(), 1);
        assert_eq!(gone[0].path, at.join("docs/adr"));
        assert!(paths(&watched).contains(&at.join("docs/decisions")));

        settings("{}");
        let gone = watched.follow_adr_dir();
        assert_eq!(gone.len(), 1);
        assert_eq!(gone[0].path, at.join("docs"));
        assert!(!paths(&watched).contains(&at.join("docs/decisions")));
    }

    #[test]
    fn a_stopped_worker_emits_nothing() {
        let (tx, rx) = mpsc::channel();
        let gate = Arc::new(Gate {
            sink: Arc::new(ChannelSink(tx)),
            shut: Mutex::new(false),
        });
        let worker = Worker { gate: gate.clone() };
        gate.emit(WorkspaceChanged::reload_all("/a"));
        assert!(rx.try_recv().is_ok());
        worker.stop();
        gate.emit(WorkspaceChanged::reload_all("/a"));
        assert!(rx.try_recv().is_err());
    }

    /// How long a real-watcher test waits for an expected event.
    const EVENTUAL: Duration = Duration::from_secs(20);

    /// How long a test watches for an event that must not arrive.
    const QUIET: Duration = Duration::from_millis(800);

    /// The test event sink: payloads go down a channel instead of to Tauri.
    struct ChannelSink(mpsc::Sender<WorkspaceChanged>);

    impl EventSink for ChannelSink {
        fn emit(&self, payload: WorkspaceChanged) {
            let _ = self.0.send(payload);
        }
    }

    /// A workspace root holding only `.daiku/settings.json`: its five rooms stay
    /// missing until a test creates them. A fixture write by the test itself.
    fn workspace(dir: &Path, name: &str) -> String {
        let root = dir.join(name);
        std::fs::create_dir_all(root.join(".daiku")).unwrap();
        std::fs::write(root.join(".daiku").join("settings.json"), "{}").unwrap();
        root.to_string_lossy().into_owned()
    }

    /// A registry whose payloads arrive on the returned channel, plus a view-state
    /// store in its own config folder (never the workspace) for authorization.
    fn harness(
        cfg: &Path,
    ) -> (
        WatchRegistry,
        mpsc::Receiver<WorkspaceChanged>,
        ViewStateStore,
    ) {
        let (tx, rx) = mpsc::channel();
        let registry = WatchRegistry::new(Arc::new(ChannelSink(tx)));
        (registry, rx, ViewStateStore::load(cfg))
    }

    /// The next payload for `root` before `deadline`, ignoring other roots'.
    fn next_for(
        rx: &mpsc::Receiver<WorkspaceChanged>,
        root: &str,
        deadline: Instant,
    ) -> Option<WorkspaceChanged> {
        loop {
            let left = deadline.checked_duration_since(Instant::now())?;
            match rx.recv_timeout(left.min(Duration::from_millis(250))) {
                Ok(payload) if payload.root == root => return Some(payload),
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return None,
            }
        }
    }

    /// The next payload for `root` that carries `want`, before `deadline`.
    fn next_matching(
        rx: &mpsc::Receiver<WorkspaceChanged>,
        root: &str,
        want: &Changed,
        deadline: Instant,
    ) -> Option<WorkspaceChanged> {
        loop {
            let payload = next_for(rx, root, deadline)?;
            if payload.changed.contains(want) {
                return Some(payload);
            }
        }
    }

    /// Every payload for `root` that arrives within `wait`.
    fn drain_for(
        rx: &mpsc::Receiver<WorkspaceChanged>,
        root: &str,
        wait: Duration,
    ) -> Vec<WorkspaceChanged> {
        let deadline = Instant::now() + wait;
        let mut got = Vec::new();
        while let Some(payload) = next_for(rx, root, deadline) {
            got.push(payload);
        }
        got
    }

    /// A fixture write under `root`, as the test itself makes it.
    fn put(root: &str, rel: &str, text: &str) {
        std::fs::write(Path::new(root).join(rel), text).unwrap();
    }

    /// Write `rel` under `root` and return the classified payload it produced,
    /// passing over reload-all ones (a starting worker sends one). The write
    /// repeats — a change landing before the worker has registered its paths is
    /// missed by the OS — until a payload arrives or `deadline` passes.
    fn put_until(
        rx: &mpsc::Receiver<WorkspaceChanged>,
        root: &str,
        rel: &str,
        deadline: Instant,
    ) -> Option<WorkspaceChanged> {
        for attempt in 1.. {
            put(root, rel, &format!("attempt {attempt}"));
            let wait_until = (Instant::now() + Duration::from_secs(2)).min(deadline);
            while let Some(payload) = next_for(rx, root, wait_until) {
                if !payload.reload_all {
                    return Some(payload);
                }
            }
            if Instant::now() >= deadline {
                return None;
            }
        }
        None
    }

    /// Write `STATE.json` until the worker for `root` reports it, then let the
    /// queue settle. macOS can report the fixture's own `settings.json` write
    /// late, in the same batch as the first change, so exact payloads are only
    /// asserted for writes made after this returns.
    fn wait_live(rx: &mpsc::Receiver<WorkspaceChanged>, root: &str) {
        let deadline = Instant::now() + EVENTUAL;
        while !put_until(rx, root, "STATE.json", deadline)
            .expect("no State event")
            .changed
            .contains(&Changed::State)
        {}
        drain_for(rx, root, QUIET);
    }

    #[test]
    fn a_state_json_write_emits_state() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = workspace(dir.path(), "ws");
        let cfg = tempfile::TempDir::new().unwrap();
        let (registry, rx, store) = harness(cfg.path());
        store.allow(&root);
        registry.sync(std::slice::from_ref(&root), |r| store.allows(r));

        wait_live(&rx, &root);
        put(&root, "STATE.json", "{}");
        let payload = next_for(&rx, &root, Instant::now() + EVENTUAL).expect("no State event");
        assert_eq!(payload.changed, vec![Changed::State]);
        assert!(!payload.reload_all);
    }

    #[test]
    fn syncing_the_same_roots_does_not_duplicate_workers_or_events() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = workspace(dir.path(), "ws");
        let cfg = tempfile::TempDir::new().unwrap();
        let (registry, rx, store) = harness(cfg.path());
        store.allow(&root);
        let roots = [root.clone()];
        registry.sync(&roots, |r| store.allows(r));
        registry.sync(&roots, |r| store.allows(r));
        assert_eq!(registry.lock().len(), 1);

        wait_live(&rx, &root);

        // One write, one event.
        put(&root, "STATE.json", "{}");
        let payload = next_for(&rx, &root, Instant::now() + EVENTUAL).expect("no State event");
        assert_eq!(payload.changed, vec![Changed::State]);
        assert!(
            drain_for(&rx, &root, QUIET).is_empty(),
            "one write emitted more than once"
        );
    }

    #[test]
    fn a_closed_root_stops_its_events() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = workspace(dir.path(), "ws");
        let cfg = tempfile::TempDir::new().unwrap();
        let (registry, rx, store) = harness(cfg.path());
        store.allow(&root);
        registry.sync(std::slice::from_ref(&root), |r| store.allows(r));
        // Nothing emitted before the close may be left queued.
        wait_live(&rx, &root);

        registry.sync(&[], |r| store.allows(r));
        assert!(
            registry.lock().is_empty(),
            "the closed root kept its worker"
        );

        // Even a repeated write after the close emits nothing.
        let wait = put_until(
            &rx,
            &root,
            "STATE.json",
            Instant::now() + Duration::from_secs(3),
        );
        assert!(wait.is_none(), "a closed root still emitted {wait:?}");
    }

    #[test]
    fn an_unauthorized_root_is_not_started() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = workspace(dir.path(), "ws");
        let cfg = tempfile::TempDir::new().unwrap();
        let (registry, rx, store) = harness(cfg.path());
        // No allow() and no open set: not a workspace the person opened.
        registry.sync(std::slice::from_ref(&root), |r| store.allows(r));
        assert!(registry.lock().is_empty(), "an unopened root was watched");

        let wait = put_until(
            &rx,
            &root,
            "STATE.json",
            Instant::now() + Duration::from_secs(3),
        );
        assert!(wait.is_none(), "an unopened root emitted {wait:?}");
    }

    #[test]
    fn two_roots_stay_isolated() {
        let dir = tempfile::TempDir::new().unwrap();
        let a = workspace(dir.path(), "a");
        let b = workspace(dir.path(), "b");
        let cfg = tempfile::TempDir::new().unwrap();
        let (registry, rx, store) = harness(cfg.path());
        store.allow(&a);
        store.allow(&b);
        registry.sync(&[a.clone(), b.clone()], |r| store.allows(r));
        assert_eq!(registry.lock().len(), 2);

        wait_live(&rx, &a);
        wait_live(&rx, &b);

        put(&a, "STATE.json", "{}");
        let payload = next_for(&rx, &a, Instant::now() + EVENTUAL).expect("no event for a");
        assert_eq!(payload.changed, vec![Changed::State]);
        assert!(drain_for(&rx, &b, QUIET).is_empty(), "b saw a's change");

        put(&b, "STATE.json", "{}");
        let payload = next_for(&rx, &b, Instant::now() + EVENTUAL).expect("no event for b");
        assert_eq!(payload.changed, vec![Changed::State]);
        assert!(drain_for(&rx, &a, QUIET).is_empty(), "a saw b's change");
    }

    #[test]
    fn a_room_created_after_start_becomes_watched() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = workspace(dir.path(), "ws");
        let cfg = tempfile::TempDir::new().unwrap();
        let (registry, rx, store) = harness(cfg.path());
        store.allow(&root);
        registry.sync(std::slice::from_ref(&root), |r| store.allows(r));

        // The worker is running and registered what exists.
        put_until(&rx, &root, "STATE.json", Instant::now() + EVENTUAL).expect("no State event");
        // It created none of the rooms that were missing at start.
        for name in ["plan", "build", "ideas", "references"] {
            assert!(
                !Path::new(&root).join(name).exists(),
                "the watcher created {name}"
            );
        }

        // Create the missing room and a classified file in it, as the person's
        // CLI would. Each attempt writes at the root too, so the worker retries
        // the room it found missing; the room's own write is what must arrive.
        let file = Path::new(&root).join("plan/specs/1-x/functional-spec.md");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        let want = Changed::PlanArtifact {
            change: "1-x".into(),
        };
        let deadline = Instant::now() + EVENTUAL;
        for attempt in 1.. {
            put(&root, "STATE.json", &format!("tick {attempt}"));
            std::thread::sleep(Duration::from_millis(300));
            std::fs::write(&file, format!("attempt {attempt}")).unwrap();
            let wait_until = (Instant::now() + Duration::from_secs(2)).min(deadline);
            if let Some(payload) = next_matching(&rx, &root, &want, wait_until) {
                assert!(!payload.reload_all);
                break;
            }
            assert!(
                Instant::now() < deadline,
                "a room created after start never became watched"
            );
        }
    }
    /// The next reload-all payload for `root` before `deadline`.
    fn next_reload(rx: &mpsc::Receiver<WorkspaceChanged>, root: &str, deadline: Instant) -> bool {
        std::iter::from_fn(|| next_for(rx, root, deadline)).any(|p| p.reload_all)
    }

    /// A started registry for one new workspace, past its worker's ready signal.
    fn started(
        dir: &Path,
        cfg: &Path,
        rooms: &[&str],
    ) -> (String, WatchRegistry, mpsc::Receiver<WorkspaceChanged>) {
        let root = workspace(dir, "ws");
        for room in rooms {
            std::fs::create_dir(Path::new(&root).join(room)).unwrap();
        }
        let (registry, rx, store) = harness(cfg);
        store.allow(&root);
        registry.sync(std::slice::from_ref(&root), |r| store.allows(r));
        assert!(
            next_reload(&rx, &root, Instant::now() + EVENTUAL),
            "no reload once the watches were in place"
        );
        (root, registry, rx)
    }

    #[test]
    fn a_new_room_reloads_all_for_what_it_already_holds() {
        let dir = tempfile::TempDir::new().unwrap();
        let cfg = tempfile::TempDir::new().unwrap();
        let (root, _registry, rx) = started(dir.path(), cfg.path(), &[]);
        drain_for(&rx, &root, QUIET);

        // The room and its file appear at once, as `daiku idea new` makes them.
        std::fs::create_dir_all(Path::new(&root).join("ideas/1-x")).unwrap();
        put(&root, "ideas/1-x/summary.md", "x");
        assert!(next_reload(&rx, &root, Instant::now() + EVENTUAL));
    }

    #[test]
    fn a_recreated_room_is_watched_again() {
        let dir = tempfile::TempDir::new().unwrap();
        let cfg = tempfile::TempDir::new().unwrap();
        let (root, _registry, rx) = started(dir.path(), cfg.path(), &["ideas"]);
        drain_for(&rx, &root, QUIET);
        let room = Path::new(&root).join("ideas");

        std::fs::remove_dir(&room).unwrap();
        assert!(next_reload(&rx, &root, Instant::now() + EVENTUAL));
        std::fs::create_dir(&room).unwrap();
        assert!(next_reload(&rx, &root, Instant::now() + EVENTUAL));
        drain_for(&rx, &root, QUIET);
        assert!(
            drain_for(&rx, &root, QUIET).is_empty(),
            "registering again set off more reloads"
        );

        std::fs::create_dir(room.join("1-x")).unwrap();
        let want = Changed::Idea { id: 1 };
        assert!(next_matching(&rx, &root, &want, Instant::now() + EVENTUAL).is_some());
    }
    #[test]
    fn a_custom_adr_folder_is_watched_once_the_settings_name_it() {
        let dir = tempfile::TempDir::new().unwrap();
        let cfg = tempfile::TempDir::new().unwrap();
        let (root, _registry, rx) = started(dir.path(), cfg.path(), &[]);
        drain_for(&rx, &root, QUIET);

        put(&root, ".daiku/settings.json", r#"{"adrDir": "docs/adr"}"#);
        assert!(next_for(&rx, &root, Instant::now() + EVENTUAL).is_some());
        drain_for(&rx, &root, QUIET);

        // The folder and the one above it are made after the setting.
        std::fs::create_dir_all(Path::new(&root).join("docs/adr")).unwrap();
        assert!(next_reload(&rx, &root, Instant::now() + EVENTUAL));
        drain_for(&rx, &root, QUIET);

        put(&root, "docs/adr/0001-x.md", "x");
        let deadline = Instant::now() + EVENTUAL;
        assert!(next_matching(&rx, &root, &Changed::Adr, deadline).is_some());
    }
}
