pub mod ecu_emulator;
pub mod commands;
pub mod j2534;
pub mod state;
pub mod uds;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize env_logger — writes to stderr which Tauri captures
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    log::info!("UDS App starting");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::discover_devices,
            commands::connect,
            commands::disconnect,
            commands::toggle_bench_mode,
            commands::get_bench_mode_status,
            commands::read_vehicle_info,
            commands::enable_ssh,
            commands::run_routine,
            commands::read_did,
            commands::list_routines,
            commands::export_logs,
        ])
        .setup(|_app| {
            // Open devtools in debug builds
            #[cfg(debug_assertions)]
            {
                use tauri::Manager;
                let window = _app.get_webview_window("main").unwrap();
                window.open_devtools();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
