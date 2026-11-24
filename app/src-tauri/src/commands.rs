//! The commands the frontend invokes. Each runs core on the blocking pool through
//! [`with_workspace`] and ends in [`envelope::reply`].

use std::path::PathBuf;

use daiku_core::{Error, Outcome, Workspace};
use serde::Serialize;
use serde_json::Value;
use tauri::State;

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

/// `daiku status` for the workspace at `root`.
#[tauri::command]
pub async fn status(root: String) -> Value {
    with_workspace(root, |ws| ws.status()).await
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
/// `.daiku/settings.json`; runs no process and writes nothing.
#[tauri::command]
pub async fn probe_workspace(root: String) -> Result<WorkspaceRef, String> {
    tauri::async_runtime::spawn_blocking(move || view_state::probe(&root))
        .await
        .map_err(|e| e.to_string())
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
