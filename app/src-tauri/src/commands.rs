//! The commands the frontend invokes. Each runs core on the blocking pool through
//! [`with_workspace`] and ends in [`envelope::reply`].

use std::path::PathBuf;

use daiku_core::{Error, Outcome, Workspace};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager, State};

use crate::env::app_env;
use crate::envelope;
use crate::view_state::{self, ViewState, ViewStateStore, WorkspaceRef};

/// Opens the workspace at `root`, runs `f` on the blocking pool and returns its
/// envelope. A panic or cancelled task becomes a `Failure` of kind `Internal`.
pub async fn with_workspace<T, F>(root: String, f: F) -> Value
where
    T: Serialize + Send + 'static,
    F: FnOnce(&Workspace) -> Result<Outcome<T>, Error> + Send + 'static,
{
    let job = tauri::async_runtime::spawn_blocking(move || {
        let ws = Workspace::open(PathBuf::from(root), app_env());
        envelope::reply(f(&ws))
    })
    .await;
    match job {
        Ok(v) => v,
        Err(e) => envelope::internal(&e.to_string()),
    }
}

/// The most `read_text` returns: 1 MiB. A larger file is a `TooLarge` Failure.
pub const READ_CAP: usize = 1_048_576;

/// `None` when `root` is a workspace the person opened (in the open set, or probed
/// this session); otherwise the `NotOpen` Failure to return instead. Every command
/// that reads a workspace checks this first, so the page can't name any folder.
fn refuse(app: &AppHandle, root: &str) -> Option<Value> {
    let allowed = app
        .try_state::<ViewStateStore>()
        .is_some_and(|store| store.allows(root));
    (!allowed).then(|| envelope::not_open(root))
}

/// `daiku status` for the workspace at `root`.
#[tauri::command]
pub async fn status(app: AppHandle, root: String) -> Value {
    if let Some(no) = refuse(&app, &root) {
        return no;
    }
    with_workspace(root, |ws| ws.status()).await
}

/// Core's `tickets_list` for the workspace at `root`: every ticket with its fields.
#[tauri::command]
pub async fn tickets_list(app: AppHandle, root: String) -> Value {
    if let Some(no) = refuse(&app, &root) {
        return no;
    }
    with_workspace(root, |ws| ws.tickets_list()).await
}

/// A change's existing files by phase (core's `artifacts`).
#[tauri::command]
pub async fn artifacts(app: AppHandle, root: String, change: String) -> Value {
    if let Some(no) = refuse(&app, &root) {
        return no;
    }
    with_workspace(root, move |ws| ws.artifacts(&change)).await
}

/// The text of the workspace file `rel`, or a `TooLarge` Failure over [`READ_CAP`]
/// bytes. The size is checked on the bytes, before they're escaped.
#[tauri::command]
pub async fn read_text(app: AppHandle, root: String, rel: String) -> Value {
    if let Some(no) = refuse(&app, &root) {
        return no;
    }
    let job = tauri::async_runtime::spawn_blocking(move || {
        let ws = Workspace::open(PathBuf::from(root), app_env());
        read_capped(&ws, &rel)
    })
    .await;
    match job {
        Ok(v) => v,
        Err(e) => envelope::internal(&e.to_string()),
    }
}

fn read_capped(ws: &Workspace, rel: &str) -> Value {
    match ws.read_text(rel) {
        Err(e) => envelope::reply::<()>(Err(e)),
        Ok(text) => {
            let len = text.as_bytes().len();
            if len > READ_CAP {
                envelope::too_large(len)
            } else {
                envelope::reply(Ok(Outcome::new(text)))
            }
        }
    }
}

/// The saved view state with a [`WorkspaceRef`] for every root in it. Plain values,
/// not the envelope: these aren't core calls.
#[tauri::command]
pub fn view_state_load(store: State<ViewStateStore>) -> Value {
    serde_json::json!({ "state": store.current(), "refs": store.refs() })
}

/// Replaces the saved view state (the window size stays Rust's) and writes it.
#[tauri::command]
pub fn view_state_save(store: State<ViewStateStore>, state: ViewState) {
    store.save(state);
}

/// Is the folder at `root` a daiku workspace? Only checks for
/// `.daiku/settings.json`; runs no process and writes nothing. A workspace is
/// allowed for the other commands from then on (the person picked it).
#[tauri::command]
pub async fn probe_workspace(app: AppHandle, root: String) -> Result<WorkspaceRef, String> {
    let r = tauri::async_runtime::spawn_blocking(move || view_state::probe(&root))
        .await
        .map_err(|e| e.to_string())?;
    if r.is_workspace
        && let Some(store) = app.try_state::<ViewStateStore>()
    {
        store.allow(&r.root);
    }
    Ok(r)
}

/// The workspace to open at start. Only a `measure` build reads one; otherwise
/// `None`, and the person picks (design D7).
#[tauri::command]
pub fn measure_root() -> Option<String> {
    #[cfg(feature = "measure")]
    {
        std::env::var("DAIKU_APP_MEASURE_ROOT").ok()
    }
    #[cfg(not(feature = "measure"))]
    {
        None
    }
}

/// Called once the first `status` data is on screen: prints `ready <epoch-ms>` and
/// exits. Only in a `measure` build.
#[cfg(feature = "measure")]
#[tauri::command]
pub fn measure_ready() {
    use std::io::Write;
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let mut out = std::io::stdout();
    let _ = writeln!(out, "ready {ms}");
    let _ = out.flush();
    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use daiku_core::changes::ChangeNewOptions;
    use daiku_core::scaffold::InitOptions;
    use tempfile::TempDir;

    fn workspace() -> (TempDir, Workspace) {
        let dir = TempDir::new().unwrap();
        let ws = Workspace::open(dir.path(), app_env());
        ws.init(&InitOptions {
            no_git: true,
            force: false,
            vcs: "none".into(),
            version: "dev".into(),
        })
        .unwrap();
        (dir, ws)
    }

    fn put(dir: &TempDir, rel: &str, bytes: &[u8]) {
        let p = dir.path().join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
    }

    #[test]
    fn read_text_escapes_and_keeps_newlines_and_tabs() {
        let (dir, ws) = workspace();
        put(&dir, "a.md", b"x\x1b[31m\n\ty\n");
        let v = read_capped(&ws, "a.md");
        assert_eq!(v["data"], "x\\x1b[31m\n\ty\n");
    }

    #[test]
    fn read_text_cap_is_inclusive() {
        let (dir, ws) = workspace();
        put(&dir, "ok.md", &vec![b'a'; READ_CAP]);
        let v = read_capped(&ws, "ok.md");
        assert_eq!(v["data"].as_str().unwrap().len(), READ_CAP);
        put(&dir, "big.md", &vec![b'a'; READ_CAP + 1]);
        let v = read_capped(&ws, "big.md");
        assert_eq!(v["error"]["kind"], "TooLarge");
        assert!(v["error"]["message"].as_str().unwrap().contains("1.0 MiB"));
        assert!(v.get("data").is_none());
    }

    #[test]
    fn read_text_refuses_outside_paths() {
        let (_dir, ws) = workspace();
        let v = read_capped(&ws, "../outside");
        assert_eq!(v["error"]["kind"], "ReadOutside");
    }

    #[test]
    fn artifacts_lists_the_intake_and_refuses_unknown_changes() {
        let (_dir, ws) = workspace();
        let made = ws.change_new(&ChangeNewOptions {
            title: "Auth".into(),
            repos: vec![],
            rev: String::new(),
            body: "goal".into(),
            idea: 0,
            version: "dev".into(),
        });
        let change = made.result.unwrap().value.change.name;
        let v = envelope::reply(ws.artifacts(&change));
        let paths: Vec<_> = v["data"]["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["path"].as_str().unwrap().to_string())
            .collect();
        assert!(
            paths.iter().any(|p| p.starts_with("plan/00_intake/")),
            "{paths:?}"
        );
        let v = envelope::reply(ws.artifacts("nope"));
        assert_eq!(v["error"]["kind"], "UnknownChange");
    }

    #[test]
    fn tickets_list_returns_the_envelope_of_the_list() {
        let (_dir, ws) = workspace();
        let v = envelope::reply(ws.tickets_list());
        assert_eq!(v["data"]["tickets"].as_array().unwrap().len(), 0);
        ws.repo_add("api", "https://example.com/api.git", None, "main")
            .unwrap();
        ws.tickets_new(&daiku_core::tickets::TicketNewOptions {
            typ: "task".into(),
            title: "Fix typo".into(),
            parent: String::new(),
            repo: "api".into(),
            depends_on: vec![],
            requirements: vec![],
            inbox: true,
        })
        .unwrap();
        let v = envelope::reply(ws.tickets_list());
        let tickets = v["data"]["tickets"].as_array().unwrap();
        assert_eq!(tickets.len(), 1);
        assert_eq!(tickets[0]["title"], "Fix typo");
        assert_eq!(tickets[0]["inbox"], true);
    }

    /// `refuse` answers an unopened root with this Failure before core runs.
    #[test]
    fn an_unopened_root_is_the_not_open_failure() {
        let v = envelope::not_open("/somewhere/else");
        assert_eq!(v["error"]["kind"], "NotOpen");
        assert!(v.get("data").is_none());
    }
}
