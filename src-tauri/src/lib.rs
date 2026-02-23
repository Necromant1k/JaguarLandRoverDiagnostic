pub mod ecu_emulator;
pub mod commands;
pub mod j2534;
pub mod state;
pub mod uds;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging — write to stderr + file (C:\udsapp\udsapp.log on Windows)
    let mut builder = env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("debug")
    );
    builder.format_timestamp_millis();

    // Also write logs to file
    #[cfg(target_os = "windows")]
    {
        use std::io::Write;
        let log_path = std::path::Path::new(r"C:\udsapp\udsapp.log");
        if let Ok(file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
        {
            let file = std::sync::Mutex::new(file);
            builder.format(move |_buf, record| {
                let msg = format!(
                    "{} [{}] {}: {}\n",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                    record.level(),
                    record.target(),
                    record.args()
                );
                // Write to both stderr and file
                let _ = std::io::stderr().write_all(msg.as_bytes());
                if let Ok(mut f) = file.lock() {
                    let _ = f.write_all(msg.as_bytes());
                    let _ = f.flush();
                }
                Ok(())
            });
        }
    }

    builder.init();

    log::info!("========================================");
    log::info!("UDS App starting");
    log::info!("Version: {}", env!("CARGO_PKG_VERSION"));
    log::info!("OS: {}", std::env::consts::OS);
    log::info!("Arch: {}", std::env::consts::ARCH);
    log::info!("CWD: {:?}", std::env::current_dir().unwrap_or_default());
    log::info!("EXE: {:?}", std::env::current_exe().unwrap_or_default());
    log::info!("========================================");

    log::info!("Creating Tauri builder...");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::discover_devices,
            commands::connect,
            commands::disconnect,
            commands::toggle_bench_mode,
            commands::get_bench_mode_status,
            commands::read_ecu_info,
            commands::run_routine,
            commands::read_ccf,
            commands::read_did,
            commands::list_routines,
            commands::export_logs,
            commands::scan_bcm_full,
            commands::scan_gwm_full,
            commands::scan_ipc_full,
        ])
        .setup(|_app| {
            log::info!("Tauri setup hook running");

            // Always open devtools for debugging
            {
                use tauri::Manager;
                log::info!("Opening devtools");
                if let Some(window) = _app.get_webview_window("main") {
                    window.open_devtools();
                    log::info!("Devtools opened");
                } else {
                    log::error!("Could not find main window!");
                }
            }

            log::info!("Setup complete, window should be visible");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
