//! The per-workspace file watcher (S-0019): one debounced worker per open
//! root. A worker watches the paths `daiku_core::Workspace::watch_paths`
//! returns, makes each event path relative to the root, classifies it with
//! `daiku_core::watch::classify` and emits a `workspace-changed` invalidation
//! payload through an [`EventSink`]. [`WatchRegistry`] holds one worker per
//! authorized open root and is reconciled by the `sync_watchers` command.
//! Nothing is written under a workspace.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use daiku_core::watch::classify;
use daiku_core::{Changed, WatchPath, Workspace};
use notify::PathOp;
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

/// Convert one debounced batch into a payload, or `None` when there is nothing
/// to emit (design D4).
///
/// * `error` — the debouncer reported a watcher error → reload all.
/// * Any event with `Flag::Rescan` → reload all (events were lost).
/// * Any event path equal to `root` → reload all (the root itself changed).
/// * Any event path that can't be made relative to `root` → reload all.
/// * Otherwise classify each relative path, drop ignored ones and deduplicate
///   the rest in first-seen order. An all-ignored batch is `None`.
fn classify_batch(root: &str, events: &[DebouncedEvent], error: bool) -> Option<WorkspaceChanged> {
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
            if let Some(c) = classify(rel)
                && seen.insert(c.clone())
            {
                changed.push(c);
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

/// The watch paths a worker still wants to register: their folders don't exist
/// yet. The worker re-checks them after each event batch and never creates
/// them (design D3).
#[derive(Debug, Default)]
struct WatchPlan {
    missing: Vec<WatchPath>,
}

/// Split the desired watch paths into those that exist now (to register in one
/// batch) and those that don't (retried later). `exists` is the caller's
/// predicate, keeping this bookkeeping free of filesystem creation.
fn split_watch_paths(
    watch: Vec<WatchPath>,
    exists: impl Fn(&Path) -> bool,
) -> (Vec<WatchPath>, WatchPlan) {
    let (register, missing) = watch.into_iter().partition(|w| exists(&w.path));
    (register, WatchPlan { missing })
}

impl WatchPlan {
    /// Re-check the still-missing paths: return those that now exist (ready to
    /// register) and keep the rest. Never creates a path.
    fn retry(&mut self, exists: impl Fn(&Path) -> bool) -> Vec<WatchPath> {
        let mut ready = Vec::new();
        self.missing.retain(|w| {
            if exists(&w.path) {
                ready.push(w.clone());
                false
            } else {
                true
            }
        });
        ready
    }
}

type Debouncer = notify_debouncer_full::Debouncer<
    notify::RecommendedWatcher,
    notify_debouncer_full::RecommendedCache,
>;

/// A running per-root watcher (design D2-D3). Dropping it — or calling
/// [`Worker::stop`] — signals the worker and joins its thread, so no further
/// event for this root is emitted afterwards.
pub struct Worker {
    handle: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
}

impl Worker {
    /// Signal the worker to stop and wait for its thread to finish.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Start a watcher worker for `root`, emitting classified payloads to `sink`
/// (design D2-D3).
pub fn spawn_worker(root: String, sink: Arc<dyn EventSink>) -> Worker {
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = stop.clone();
    let handle = thread::spawn(move || run_worker(root, sink, thread_stop));
    Worker {
        handle: Some(handle),
        stop,
    }
}

fn run_worker(root: String, sink: Arc<dyn EventSink>, stop: Arc<AtomicBool>) {
    let ws = Workspace::open(root.as_str(), app_env());
    let (existing, mut plan) = split_watch_paths(ws.watch_paths(), |p| p.exists());

    let (tx, rx) = mpsc::channel::<DebounceEventResult>();
    let mut debouncer = match new_debouncer(DEBOUNCE, None, tx) {
        Ok(debouncer) => debouncer,
        Err(_) => {
            // A debouncer that can't start can't be trusted: reload all.
            sink.emit(WorkspaceChanged::reload_all(&root));
            return;
        }
    };

    register_batch(&mut debouncer, &existing, &root, &*sink);

    while !stop.load(Ordering::SeqCst) {
        match rx.recv_timeout(IDLE) {
            Ok(result) => {
                // A batch arrived: retry any missing registrations first.
                let ready = plan.retry(|p| p.exists());
                register_batch(&mut debouncer, &ready, &root, &*sink);
                match result {
                    Ok(events) => {
                        if let Some(payload) = classify_batch(&root, &events, false) {
                            sink.emit(payload);
                        }
                    }
                    // A watcher error: reload every query for this root.
                    Err(_) => sink.emit(WorkspaceChanged::reload_all(&root)),
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    // Dropping `debouncer` stops its internal thread.
}

/// Register `paths` in one batch (design D2). A registration failure takes the
/// reload-all path; already-registered watches keep running.
fn register_batch(
    debouncer: &mut Debouncer,
    paths: &[WatchPath],
    root: &str,
    sink: &dyn EventSink,
) {
    if paths.is_empty() {
        return;
    }
    let ops: Vec<PathOp> = paths
        .iter()
        .map(|w| {
            if w.recursive {
                PathOp::watch_recursive(w.path.clone())
            } else {
                PathOp::watch_non_recursive(w.path.clone())
            }
        })
        .collect();
    if debouncer.update_paths(ops).is_err() {
        sink.emit(WorkspaceChanged::reload_all(root));
    }
}

/// One worker per open workspace root (design D1). [`sync`](Self::sync) is the
/// only way the set changes: it starts added roots, leaves running ones alone
/// and stops every root that left the set or lost authorization — joining its
/// thread before it returns, so a closed root cannot emit afterwards. Dropping
/// the registry stops every worker, which is the application-exit path.
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
        // Stop the removed and disallowed roots first: each join finishes before
        // this call returns (design D3).
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

    use notify::event::{Flag, ModifyKind};
    use notify::{Event, EventKind};

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
        let payload = classify_batch(&root.to_string_lossy(), &events, false).unwrap();
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
        let payload = classify_batch(&root.to_string_lossy(), &events, false).unwrap();
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
        let payload = classify_batch(&root.to_string_lossy(), &events, false).unwrap();
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
            classify_batch(&root.to_string_lossy(), &events, false),
            None
        );
    }

    #[test]
    fn rescan_reloads_all() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();
        let events = [modify(root, "STATE.json"), rescan()];
        let payload = classify_batch(&root.to_string_lossy(), &events, false).unwrap();
        assert_eq!(
            payload,
            WorkspaceChanged::reload_all(&root.to_string_lossy())
        );
    }

    #[test]
    fn callback_error_reloads_all() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();
        let payload = classify_batch(&root.to_string_lossy(), &[], true).unwrap();
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
        let payload = classify_batch(&root.to_string_lossy(), &events, false).unwrap();
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
        let payload = classify_batch(&root.to_string_lossy(), &events, false).unwrap();
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
    fn split_registers_existing_and_retries_missing_without_creating() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();
        let ws = Workspace::open(root, app_env());
        let (register, mut plan) = split_watch_paths(ws.watch_paths(), |p| p.exists());

        // Only the root exists; the five recursive rooms are missing.
        assert_eq!(
            register,
            vec![WatchPath {
                path: root.to_path_buf(),
                recursive: false,
            }]
        );
        assert_eq!(plan.missing.len(), 5);
        // The bookkeeping created nothing.
        for w in &plan.missing {
            assert!(
                !w.path.exists(),
                "{:?} should not have been created",
                w.path
            );
        }

        // A room created later becomes registered by retry, without creating
        // the still-missing rooms.
        std::fs::create_dir(root.join("plan")).unwrap();
        let ready = plan.retry(|p| p.exists());
        assert_eq!(
            ready,
            vec![WatchPath {
                path: root.join("plan"),
                recursive: true,
            }]
        );
        assert_eq!(plan.missing.len(), 4);
        for w in &plan.missing {
            assert!(
                !w.path.exists(),
                "{:?} should not have been created",
                w.path
            );
        }
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

    /// Write `rel` under `root` and return the payload it produced. The write
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
            if let Some(payload) = next_for(rx, root, wait_until) {
                return Some(payload);
            }
            if Instant::now() >= deadline {
                return None;
            }
        }
        None
    }

    #[test]
    fn a_state_json_write_emits_state() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = workspace(dir.path(), "ws");
        let cfg = tempfile::TempDir::new().unwrap();
        let (registry, rx, store) = harness(cfg.path());
        store.allow(&root);
        registry.sync(std::slice::from_ref(&root), |r| store.allows(r));

        let payload =
            put_until(&rx, &root, "STATE.json", Instant::now() + EVENTUAL).expect("no State event");
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

        // Confirm the worker is live, then let its queue settle.
        put_until(&rx, &root, "STATE.json", Instant::now() + EVENTUAL).expect("no State event");
        drain_for(&rx, &root, QUIET);

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
        put_until(&rx, &root, "STATE.json", Instant::now() + EVENTUAL).expect("no State event");

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

        // Both workers are live; then each queue settles.
        let payload =
            put_until(&rx, &a, "STATE.json", Instant::now() + EVENTUAL).expect("no event for a");
        assert_eq!(payload.changed, vec![Changed::State]);
        drain_for(&rx, &a, QUIET);
        let payload =
            put_until(&rx, &b, "STATE.json", Instant::now() + EVENTUAL).expect("no event for b");
        assert_eq!(payload.changed, vec![Changed::State]);
        drain_for(&rx, &b, QUIET);

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
}
