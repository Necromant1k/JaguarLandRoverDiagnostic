pub mod bcm_emulator;
pub mod commands;
pub mod j2534;
pub mod state;
pub mod uds;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::discover_devices,
            commands::connect,
            commands::disconnect,
            commands::toggle_bench_mode,
            commands::read_vehicle_info,
            commands::enable_ssh,
            commands::run_routine,
            commands::read_did,
            commands::list_routines,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
