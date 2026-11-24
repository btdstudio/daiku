mod commands;
mod env;
mod envelope;

pub fn run() {
    let builder = tauri::Builder::default().plugin(tauri_plugin_dialog::init());

    #[cfg(not(feature = "measure"))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        commands::status,
        commands::measure_root
    ]);
    #[cfg(feature = "measure")]
    let builder = builder.invoke_handler(tauri::generate_handler![
        commands::status,
        commands::measure_root,
        commands::measure_ready
    ]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running daiku-app");
}
