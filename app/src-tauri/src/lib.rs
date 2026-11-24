mod commands;
mod env;
mod envelope;
mod view_state;

use tauri::{LogicalSize, Manager, WindowEvent};

use view_state::{Size, ViewStateStore};

pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            let store = ViewStateStore::load(&dir);
            let saved = store.current().window;
            app.manage(store);
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
        commands::measure_root,
        commands::view_state_load,
        commands::view_state_save,
        commands::probe_workspace
    ]);
    #[cfg(feature = "measure")]
    let builder = builder.invoke_handler(tauri::generate_handler![
        commands::status,
        commands::measure_root,
        commands::measure_ready,
        commands::view_state_load,
        commands::view_state_save,
        commands::probe_workspace
    ]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running daiku-app");
}
