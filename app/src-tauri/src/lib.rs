mod commands;
mod env;
mod envelope;
mod view_state;
mod watch;

use std::sync::Arc;

use tauri::{LogicalSize, Manager, WindowEvent};

use view_state::{Size, ViewStateStore};
use watch::{AppEventSink, WatchRegistry};

pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            let store = ViewStateStore::load(&dir);
            let saved = store.current().window;
            app.manage(store);
            app.manage(WatchRegistry::new(Arc::new(AppEventSink::new(
                app.handle().clone(),
            ))));
            if let Some(window) = app.get_webview_window("main") {
                if let Some(size) = saved {
                    let _ = window.set_size(LogicalSize::new(size.width, size.height));
                }
                window.show()?;
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            let Some(store) = window.try_state::<ViewStateStore>() else {
                return;
            };
            match event {
                WindowEvent::Resized(physical) => {
                    let logical = physical.to_logical::<f64>(window.scale_factor().unwrap_or(1.0));
                    store.set_window(Size {
                        width: logical.width,
                        height: logical.height,
                    });
                }
                WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed => store.flush(),
                _ => {}
            }
        });

    #[cfg(not(feature = "measure"))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        commands::status,
        commands::tickets_list,
        commands::next,
        commands::artifacts,
        commands::read_text,
        commands::idea_list,
        commands::adr_list,
        commands::read_artifact,
        commands::measure_root,
        commands::view_state_load,
        commands::view_state_save,
        commands::probe_workspace,
        watch::sync_watchers
    ]);
    #[cfg(feature = "measure")]
    let builder = builder.invoke_handler(tauri::generate_handler![
        commands::status,
        commands::tickets_list,
        commands::next,
        commands::artifacts,
        commands::read_text,
        commands::idea_list,
        commands::adr_list,
        commands::read_artifact,
        commands::measure_root,
        commands::measure_ready,
        commands::view_state_load,
        commands::view_state_save,
        commands::probe_workspace,
        watch::sync_watchers
    ]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running daiku-app");
}

#[cfg(test)]
mod tests {
    /// The opener may open `https://*` URLs and nothing else (AC11, N1).
    #[test]
    fn capability_scopes_the_opener_to_https() {
        let text = include_str!("../capabilities/default.json");
        let cap: serde_json::Value = serde_json::from_str(text).unwrap();
        let perms = cap["permissions"].as_array().unwrap();
        let opener: Vec<_> = perms
            .iter()
            .filter(|p| {
                p.as_str()
                    .unwrap_or_else(|| p["identifier"].as_str().unwrap())
                    .starts_with("opener:")
            })
            .collect();
        assert_eq!(
            opener,
            vec![&serde_json::json!({
                "identifier": "opener:allow-open-url",
                "allow": [{ "url": "https://*" }]
            })]
        );
    }
}
