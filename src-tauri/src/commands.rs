use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::ecu_emulator::{EcuEmulatorManager, EcuId};
use crate::j2534::dll;
use crate::j2534::types::*;
use crate::state::{AppState, Connection};
use crate::uds::client::{LogDirection, LogEntry};
use crate::uds::services::{did, ecu_addr, routine};

/// CCF decode table — maps option_id → {name, values: {value_byte → label}}
static CCF_DECODE_JSON: &str = include_str!("../assets/ccf_decode.json");

/// Decode a CCF option value to human-readable string
fn decode_ccf_value(option_id: u16, raw_value: u8) -> String {
    if let Ok(table) = serde_json::from_str::<serde_json::Value>(CCF_DECODE_JSON) {
        let key = option_id.to_string();
        if let Some(entry) = table.get(&key) {
            if let Some(values) = entry.get("values") {
                let val_key = raw_value.to_string();
                if let Some(label) = values.get(&val_key) {
                    if let Some(s) = label.as_str() {
                        return format!("{} (0x{:02X})", s, raw_value);
                    }
                }
            }
            if let Some(name) = entry.get("name").and_then(|v| v.as_str()) {
                return format!("{}: 0x{:02X} (unknown)", name, raw_value);
            }
        }
    }
    format!("0x{:02X}", raw_value)
}

/// Get CCF option name from decode table
fn ccf_option_name(option_id: u16) -> String {
    if let Ok(table) = serde_json::from_str::<serde_json::Value>(CCF_DECODE_JSON) {
        let key = option_id.to_string();
        if let Some(entry) = table.get(&key) {
            if let Some(name) = entry.get("name").and_then(|v| v.as_str()) {
                return name.to_string();
            }
        }
    }
    format!("Option {}", option_id)
}

/// IMC CCF option IDs (from 0x0E02 List CCF response on SAJWA2G78G8V98048)
const IMC_CCF_OPTION_IDS: &[u16] = &[
    1, 2, 3, 4, 6, 7, 8, 9, 10, 11, 14, 15, 16, 17, 18, 19, 21, 22, 23, 25, 27, 29, 30, 31, 32,
    33, 34, 35, 36, 65, 67, 68, 69, 70, 71, 72, 73, 77, 79, 80, 81, 82, 83, 84, 86, 87, 88, 89,
    90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 105, 107, 108, 109, 110, 111, 112,
    113, 114, 116, 117, 119,
];

#[derive(Debug, Serialize)]
pub struct CcfCompareEntry {
    pub option_id: u16,
    pub name: String,
    pub gwm: Option<String>,
    pub bcm: Option<String>,
    pub imc: Option<String>,
    pub mismatch: bool,
}

/// Log an error and return it — ensures all command errors are visible in the log file
fn log_err(context: &str, msg: String) -> String {
    log::error!("[{}] {}", context, msg);
    msg
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub firmware_version: String,
    pub dll_version: String,
    pub api_version: String,
    pub dll_path: String,
}

#[derive(Debug, Serialize)]
pub struct EcuInfoEntry {
    pub label: String,
    pub did_hex: String,
    pub value: Option<String>,
    pub error: Option<String>,
    pub category: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RoutineInfo {
    pub routine_id: u16,
    pub name: String,
    pub description: String,
    pub category: String,
    pub needs_security: bool,
    pub needs_pending: bool,
}

#[derive(Debug, Serialize)]
pub struct RoutineResponse {
    pub success: bool,
    pub description: String,
    pub raw_data: Vec<u8>,
}

#[derive(Debug, Serialize)]
pub struct J2534DeviceEntry {
    pub name: String,
    pub dll_path: String,
}

fn emit_log<R: tauri::Runtime>(app: &tauri::AppHandle<R>, entry: LogEntry) {
    log::info!(
        "[UDS] {} [{}] {}{}",
        entry.timestamp,
        entry.direction,
        entry.data_hex,
        if entry.description.is_empty() {
            String::new()
        } else {
            format!(" {}", entry.description)
        }
    );
    let _ = app.emit("uds-log", entry);
}

fn emit_log_simple<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    direction: LogDirection,
    data: &[u8],
    description: &str,
) {
    emit_log(
        app,
        LogEntry {
            direction,
            data_hex: data
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join(" "),
            timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
            description: description.to_string(),
        },
    );
}

/// Discover available J2534 devices from the Windows registry
#[tauri::command]
pub fn discover_devices() -> Vec<J2534DeviceEntry> {
    dll::discover_j2534_dlls()
        .into_iter()
        .map(|(name, path)| J2534DeviceEntry {
            name,
            dll_path: path.to_string_lossy().to_string(),
        })
        .collect()
}

/// Connect to J2534 device
#[tauri::command]
pub fn connect(
    app: AppHandle,
    state: State<'_, AppState>,
    dll_path: Option<String>,
) -> Result<DeviceInfo, String> {
    let mut conn = state.connection.lock().map_err(|e| e.to_string())?;

    if conn.is_some() {
        return Err("Already connected. Disconnect first.".into());
    }

    let (lib, device, path) = if let Some(path) = dll_path {
        // Explicit path provided
        emit_log_simple(
            &app,
            LogDirection::Tx,
            &[],
            &format!("Loading J2534 DLL: {}", path),
        );
        let lib = Arc::new(dll::J2534Lib::load(&path)?);
        let device = crate::j2534::device::J2534Device::open(lib.clone())?;
        (lib, device, path)
    } else {
        // Auto-detect: try each discovered device until one opens successfully
        let devices = dll::discover_j2534_dlls();
        log::info!("Auto-detect: found {} J2534 devices", devices.len());

        let mut last_err = String::from("No J2534 devices found in registry");
        let mut found = None;

        for (name, p) in &devices {
            let p_str = p.to_string_lossy().to_string();
            log::info!("Auto-detect: trying {} at {}", name, p.display());
            emit_log_simple(
                &app,
                LogDirection::Tx,
                &[],
                &format!("Trying: {} ({})", name, p_str),
            );

            match dll::J2534Lib::load(&p_str) {
                Ok(lib) => {
                    let lib = Arc::new(lib);
                    match crate::j2534::device::J2534Device::open(lib.clone()) {
                        Ok(device) => {
                            log::info!("Auto-detect: successfully opened {}", name);
                            emit_log_simple(
                                &app,
                                LogDirection::Rx,
                                &[],
                                &format!("Connected to {}", name),
                            );
                            found = Some((lib, device, p_str));
                            break;
                        }
                        Err(e) => {
                            log::warn!("Auto-detect: {} failed to open device: {}", name, e);
                            last_err = format!("{}: {}", name, e);
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Auto-detect: {} failed to load DLL: {}", name, e);
                    last_err = format!("{}: {}", name, e);
                }
            }
        }

        // Fall back to default Mongoose path if no discovered device worked
        if found.is_none() {
            let default_path = dll::default_mongoose_dll_path()
                .to_string_lossy()
                .to_string();
            log::info!(
                "Auto-detect: trying default Mongoose path: {}",
                default_path
            );
            if let Ok(lib) = dll::J2534Lib::load(&default_path) {
                let lib = Arc::new(lib);
                if let Ok(device) = crate::j2534::device::J2534Device::open(lib.clone()) {
                    found = Some((lib, device, default_path));
                }
            }
        }

        found.ok_or_else(|| format!("No J2534 device responded. Last error: {}", last_err))?
    };

    let version = device.read_version()?;

    emit_log_simple(
        &app,
        LogDirection::Rx,
        &[],
        &format!(
            "Connected. FW: {}, DLL: {}, API: {}",
            version.firmware, version.dll, version.api
        ),
    );

    // Connect ISO15765 channel at 500kbps
    let channel = device.connect_iso15765(500000)?;

    // Configure ISO15765 flow control parameters for multi-frame support
    // BS=0 (send all frames without waiting), STMIN=0 (no delay between frames), WFT_MAX=0 (no wait frame limit)
    if let Err(e) = channel.set_iso15765_config(0, 0, 0) {
        emit_log_simple(
            &app,
            LogDirection::Rx,
            &[],
            &format!("Warning: SET_CONFIG ISO15765 failed: {}", e),
        );
    }

    // Setup flow control filter for IMC
    channel.setup_iso15765_filter(ecu_addr::IMC_TX, ecu_addr::IMC_RX)?;

    // Setup flow control filter for BCM (multi-ECU DID reading + bench mode emulation)
    channel.setup_iso15765_filter(ecu_addr::BCM_TX, ecu_addr::BCM_RX)?;

    // Setup flow control filters for GWM and IPC
    channel.setup_iso15765_filter(ecu_addr::GWM_TX, ecu_addr::GWM_RX)?;
    channel.setup_iso15765_filter(ecu_addr::IPC_TX, ecu_addr::IPC_RX)?;

    emit_log_simple(
        &app,
        LogDirection::Rx,
        &[],
        "ISO15765 channel connected, IMC + BCM + GWM + IPC filters set",
    );

    let info = DeviceInfo {
        firmware_version: version.firmware.clone(),
        dll_version: version.dll.clone(),
        api_version: version.api.clone(),
        dll_path: path.clone(),
    };

    *conn = Some(Connection {
        lib,
        device,
        channel: Some(channel),
        can_channel: None,
        dll_path: path,
        emulator_manager: None,
    });

    Ok(info)
}

/// Disconnect from J2534 device
#[tauri::command]
pub fn disconnect(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let mut conn = state.connection.lock().map_err(|e| e.to_string())?;

    if conn.is_none() {
        // Already disconnected — idempotent
        return Ok(());
    }

    // Drop connection (RAII will close channel and device)
    *conn = None;

    emit_log_simple(
        &app,
        LogDirection::Tx,
        &[],
        "Disconnected from J2534 device",
    );
    Ok(())
}

/// Bench mode status returned to frontend
#[derive(Debug, Serialize)]
pub struct BenchModeStatus {
    pub enabled: bool,
    pub emulated_ecus: Vec<String>,
}

/// Toggle bench mode (multi-ECU emulation)
#[tauri::command]
pub fn toggle_bench_mode(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
    ecus: Option<Vec<String>>,
) -> Result<(), String> {
    let mut conn = state.connection.lock().map_err(|e| e.to_string())?;
    let conn = conn.as_mut().ok_or("Not connected")?;

    // Always clean up any existing emulator/CAN channel first
    if let Some(mut mgr) = conn.emulator_manager.take() {
        mgr.stop();
    }
    conn.can_channel = None;

    if enabled {
        // Parse ECU list, default to BCM only
        let ecu_ids: Vec<EcuId> = ecus
            .unwrap_or_else(|| vec!["bcm".to_string()])
            .iter()
            .filter_map(|s| EcuId::from_str(s))
            .collect();

        if ecu_ids.is_empty() {
            return Err("No valid ECU IDs specified".into());
        }

        let ecu_names: Vec<String> = ecu_ids.iter().map(|e| e.name().to_string()).collect();

        // Try to open a raw CAN channel for broadcast (separate from ISO15765)
        // Some J2534 devices (e.g. MongoosePro) only support one channel at a time,
        // so broadcast must happen before ISO15765
        let manager = match conn.device.connect_can(500000) {
            Ok(can_channel) => {
                let can_channel_id = can_channel.channel_id();
                let mgr = crate::ecu_emulator::EcuEmulatorManager::new_with_broadcast(
                    &conn.lib,
                    can_channel_id,
                    ecu_ids,
                );
                conn.can_channel = Some(can_channel);
                emit_log_simple(
                    &app,
                    LogDirection::Rx,
                    &[],
                    &format!(
                        "Bench mode ON — emulating: {} (CAN broadcast active)",
                        ecu_names.join(", ")
                    ),
                );
                mgr
            }
            Err(e) => {
                log::warn!(
                    "CAN broadcast channel unavailable: {} — using software routing only",
                    e
                );
                emit_log_simple(
                    &app,
                    LogDirection::Rx,
                    &[],
                    &format!(
                        "Bench mode ON — emulating: {} (software routing only, no CAN broadcast)",
                        ecu_names.join(", ")
                    ),
                );
                crate::ecu_emulator::EcuEmulatorManager::new(ecu_ids)
            }
        };
        conn.emulator_manager = Some(manager);
    } else {
        // Cleanup already done above
        emit_log_simple(
            &app,
            LogDirection::Rx,
            &[],
            "Bench mode OFF — emulation stopped",
        );
    }

    Ok(())
}

/// Get bench mode status
#[tauri::command]
pub fn get_bench_mode_status(state: State<'_, AppState>) -> Result<BenchModeStatus, String> {
    let conn = state.connection.lock().map_err(|e| e.to_string())?;
    let conn = conn.as_ref().ok_or("Not connected")?;

    match &conn.emulator_manager {
        Some(mgr) => Ok(BenchModeStatus {
            enabled: true,
            emulated_ecus: mgr
                .emulated_ecus()
                .iter()
                .map(|e| e.name().to_lowercase())
                .collect(),
        }),
        None => Ok(BenchModeStatus {
            enabled: false,
            emulated_ecus: vec![],
        }),
    }
}

/// Read ECU info — returns a list of DID entries for the given ECU
#[tauri::command]
pub fn read_ecu_info(
    app: AppHandle,
    state: State<'_, AppState>,
    ecu: String,
) -> Result<Vec<EcuInfoEntry>, String> {
    read_ecu_info_inner(&app, &state, &ecu).map_err(|e| log_err("read_ecu_info", e))
}

fn read_ecu_info_inner(
    app: &AppHandle,
    state: &State<'_, AppState>,
    ecu: &str,
) -> Result<Vec<EcuInfoEntry>, String> {
    let mut conn = state.connection.lock().map_err(|e| e.to_string())?;
    let conn = conn.as_mut().ok_or("Not connected")?;

    // In bench mode, do CAN pre-broadcast before IMC reads to wake IMC
    // MongoosePro only supports one channel, so broadcast must happen before ISO15765
    if ecu == "imc" && conn.emulator_manager.is_some() {
        can_pre_broadcast(app, conn)?;
    }

    let channel: &dyn crate::j2534::Channel =
        conn.channel.as_ref().ok_or("No channel available")?;
    let emulator = conn.emulator_manager.as_ref();
    let entries = match ecu {
        "imc" => read_imc_info(app, channel, emulator),
        "bcm" => read_bcm_info(app, channel, emulator),
        "gwm" => read_gwm_info(app, channel, emulator),
        "ipc" => read_ipc_info(app, channel, emulator),
        _ => return Err(format!("Unknown ECU: {}", ecu)),
    };

    Ok(entries)
}

/// CAN pre-broadcast: temporarily swap ISO15765 channel for raw CAN,
/// broadcast NM messages to wake the IMC, then restore ISO15765.
/// MongoosePro only supports one J2534 channel at a time, so we must
/// close ISO15765 first, broadcast on CAN, then reopen ISO15765.
fn can_pre_broadcast(app: &AppHandle, conn: &mut Connection) -> Result<(), String> {
    emit_log_simple(
        app,
        LogDirection::Tx,
        &[],
        "Bench: CAN pre-broadcast to wake IMC...",
    );

    // Close ISO15765 channel to free the single J2534 channel slot
    conn.channel.take(); // Drop triggers PassThruDisconnect

    // Open raw CAN channel and broadcast NM messages for 5 seconds
    match conn.device.connect_can(500000) {
        Ok(can_ch) => {
            for cycle in 0..50 {
                // 50 × 100ms = 5 seconds
                for &(can_id, ref data) in crate::ecu_emulator::BROADCAST_MSGS {
                    let _ = can_ch.send_raw_can(can_id, data);
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
                if cycle % 10 == 9 {
                    emit_log_simple(
                        app,
                        LogDirection::Tx,
                        &[],
                        &format!("CAN broadcast: {}s / 5s", (cycle + 1) / 10),
                    );
                }
            }
            drop(can_ch); // Close CAN channel to free for ISO15765
        }
        Err(e) => {
            emit_log_simple(
                app,
                LogDirection::Rx,
                &[],
                &format!("Warning: CAN broadcast failed: {}", e),
            );
        }
    }

    // Re-open ISO15765 channel with all ECU filters
    let channel = conn
        .device
        .connect_iso15765(500000)
        .map_err(|e| format!("Failed to reopen ISO15765 after broadcast: {}", e))?;
    let _ = channel.set_iso15765_config(0, 0, 0);
    channel.setup_iso15765_filter(ecu_addr::IMC_TX, ecu_addr::IMC_RX)?;
    channel.setup_iso15765_filter(ecu_addr::BCM_TX, ecu_addr::BCM_RX)?;
    channel.setup_iso15765_filter(ecu_addr::GWM_TX, ecu_addr::GWM_RX)?;
    channel.setup_iso15765_filter(ecu_addr::IPC_TX, ecu_addr::IPC_RX)?;
    conn.channel = Some(channel);

    emit_log_simple(
        app,
        LogDirection::Rx,
        &[],
        "CAN broadcast done, ISO15765 restored",
    );
    Ok(())
}

fn read_did_entry<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    channel: &dyn crate::j2534::Channel,
    tx_id: u32,
    did_id: u16,
    label: &str,
    format_fn: fn(&[u8]) -> String,
    category: &str,
    emulator: Option<&EcuEmulatorManager>,
) -> EcuInfoEntry {
    let did_hex = format!("{:04X}", did_id);

    match send_read_did(app, channel, tx_id, did_id, emulator) {
        Ok(data) => {
            let value = format_fn(&data);
            emit_log_simple(
                app,
                LogDirection::Rx,
                &[],
                &format!("{} = {}", label, value),
            );
            EcuInfoEntry {
                label: label.to_string(),
                did_hex,
                value: Some(value),
                error: None,
                category: category.to_string(),
            }
        }
        Err(e) => EcuInfoEntry {
            label: label.to_string(),
            did_hex,
            value: None,
            error: Some(e),
            category: category.to_string(),
        },
    }
}

fn format_string(data: &[u8]) -> String {
    String::from_utf8_lossy(data).trim().to_string()
}

fn format_diag_session(data: &[u8]) -> String {
    if data.is_empty() {
        return "Unknown".to_string();
    }
    let session_str = match data[0] {
        0x01 => "Default",
        0x02 => "Programming",
        0x03 => "Extended",
        _ => "Unknown",
    };
    format!("{} (0x{:02X})", session_str, data[0])
}

fn format_imc_status(data: &[u8]) -> String {
    if data.is_empty() {
        return "Unknown".to_string();
    }
    match data[0] {
        0x00 => "Normal (0x00)".to_string(),
        0x01 => "Booting (0x01)".to_string(),
        0x02 => "Shutdown (0x02)".to_string(),
        0x03 => "Suspend (0x03)".to_string(),
        0x04 => "Standby (0x04)".to_string(),
        0x05 => "Error (0x05)".to_string(),
        _ => format!("0x{:02X}", data[0]),
    }
}

fn format_voltage(data: &[u8]) -> String {
    if data.len() >= 2 {
        let raw = ((data[0] as u16) << 8 | data[1] as u16) as f32;
        format!("{:.1} V", raw * 0.1)
    } else if !data.is_empty() {
        format!("{:.1} V", data[0] as f32 * 0.1)
    } else {
        "N/A".to_string()
    }
}

fn format_soc(data: &[u8]) -> String {
    if !data.is_empty() {
        format!("{}%", data[0])
    } else {
        "N/A".to_string()
    }
}

fn format_temp(data: &[u8]) -> String {
    if !data.is_empty() {
        // Typical: raw - 40 = degrees C
        let temp = data[0] as i16 - 40;
        format!("{} °C", temp)
    } else {
        "N/A".to_string()
    }
}

/// GWM battery voltage: offset=6V, resolution=0.05V (per MDX_GWM)
fn format_gwm_voltage(data: &[u8]) -> String {
    if !data.is_empty() {
        let v = 6.0 + (data[0] as f32 * 0.05);
        format!("{:.2} V", v)
    } else {
        "N/A".to_string()
    }
}

/// GWM battery temp: offset=-40°C (per MDX_GWM)
fn format_gwm_temp(data: &[u8]) -> String {
    if !data.is_empty() {
        let temp = data[0] as i16 - 40;
        format!("{} °C", temp)
    } else {
        "N/A".to_string()
    }
}

/// IPC odometer: 3 bytes, 1 km resolution
fn format_odometer(data: &[u8]) -> String {
    if data.len() >= 3 {
        let km = ((data[0] as u32) << 16) | ((data[1] as u32) << 8) | (data[2] as u32);
        format!("{} km", km)
    } else {
        "N/A".to_string()
    }
}

fn read_imc_info<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    channel: &dyn crate::j2534::Channel,
    emulator: Option<&EcuEmulatorManager>,
) -> Vec<EcuInfoEntry> {
    let tx = ecu_addr::IMC_TX;
    let mut entries = Vec::new();
    let bench_mode = emulator.is_some();

    // Step 1: TesterPresent — verify ECU is responsive
    // In bench mode, poll until IMC responds (CAN pre-broadcast should have woken it)
    if bench_mode {
        emit_log_simple(
            app,
            LogDirection::Tx,
            &[],
            "Bench mode: waiting for IMC to boot...",
        );
        let mut imc_ready = false;
        for attempt in 1..=15 {
            emit_log_simple(
                app,
                LogDirection::Tx,
                &[0x3E, 0x00],
                &format!("TesterPresent poll {}/15", attempt),
            );
            match send_uds_request(app, channel, tx, &[0x3E, 0x00], false, emulator) {
                Ok(_) => {
                    emit_log_simple(
                        app,
                        LogDirection::Rx,
                        &[],
                        &format!("IMC responded on attempt {}", attempt),
                    );
                    imc_ready = true;
                    break;
                }
                Err(_) => {
                    if attempt < 15 {
                        std::thread::sleep(std::time::Duration::from_secs(1));
                    }
                }
            }
        }
        if !imc_ready {
            emit_log_simple(
                app,
                LogDirection::Rx,
                &[],
                "IMC not responding after 15 attempts",
            );
            return vec![EcuInfoEntry {
                label: "IMC Status".to_string(),
                did_hex: "0202".to_string(),
                value: None,
                error: Some("IMC not responding — check power and CAN connection".to_string()),
                category: "status".to_string(),
            }];
        }
    } else {
        emit_log_simple(app, LogDirection::Tx, &[0x3E, 0x00], "TesterPresent (IMC)");
        let _ = send_uds_request(app, channel, tx, &[0x3E, 0x00], false, emulator);
    }

    // Step 2: Read D100 (works in Default + Extended Session per EXML)
    entries.push(read_did_entry(
        app,
        channel,
        tx,
        did::ACTIVE_DIAG_SESSION,
        "Diag Session",
        format_diag_session,
        "status",
        emulator,
    ));

    // Step 3: Read DIDs with no session restriction (per EXML — work in any session)
    // F111 is NOT defined for IMC in EXML — removed
    let dids: Vec<(u16, &str, fn(&[u8]) -> String, &str)> = vec![
        (did::VIN, "VIN", format_string, "vehicle"),
        (did::MASTER_RPM_PART, "SW Part", format_string, "software"),
        (did::V850_PART, "V850 Part", format_string, "software"),
        (did::POLAR_PART, "Polar Part", format_string, "software"),
        (did::ECU_SERIAL, "ECU Serial", format_string, "hardware"),
        (did::ECU_SERIAL2, "HW Part", format_string, "hardware"),
    ];

    for (did_id, label, formatter, category) in &dids {
        let _ = send_uds_request(app, channel, tx, &[0x3E, 0x00], false, emulator);
        entries.push(read_did_entry(
            app, channel, tx, *did_id, label, *formatter, category, emulator,
        ));
    }

    // Step 4: Extended Session for DID 0x0202 (requires Extended per EXML)
    emit_log_simple(
        app,
        LogDirection::Tx,
        &[0x10, 0x03],
        "ExtendedSession (IMC)",
    );
    if send_uds_request(app, channel, tx, &[0x10, 0x03], false, emulator).is_ok() {
        emit_log_simple(app, LogDirection::Rx, &[], "Extended Session OK");
        let _ = send_uds_request(app, channel, tx, &[0x3E, 0x00], false, emulator);
        entries.push(read_did_entry(
            app,
            channel,
            tx,
            did::IMC_STATUS,
            "IMC Status",
            format_imc_status,
            "status",
            emulator,
        ));
    } else {
        entries.push(EcuInfoEntry {
            label: "IMC Status".to_string(),
            did_hex: "0202".to_string(),
            value: None,
            error: Some("Extended Session required for 0x0202".to_string()),
            category: "status".to_string(),
        });
    }

    entries
}

fn read_bcm_info<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    channel: &dyn crate::j2534::Channel,
    emulator: Option<&EcuEmulatorManager>,
) -> Vec<EcuInfoEntry> {
    let tx = ecu_addr::BCM_TX;
    vec![
        read_did_entry(
            app,
            channel,
            tx,
            did::VIN,
            "VIN",
            format_string,
            "vehicle",
            emulator,
        ),
        read_did_entry(
            app,
            channel,
            tx,
            did::MASTER_RPM_PART,
            "SW Part",
            format_string,
            "software",
            emulator,
        ),
        read_did_entry(
            app,
            channel,
            tx,
            did::ECU_SERIAL,
            "ECU Serial",
            format_string,
            "hardware",
            emulator,
        ),
        read_did_entry(
            app,
            channel,
            tx,
            did::ECU_SERIAL2,
            "HW Part",
            format_string,
            "hardware",
            emulator,
        ),
    ]
}

fn read_gwm_info<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    channel: &dyn crate::j2534::Channel,
    emulator: Option<&EcuEmulatorManager>,
) -> Vec<EcuInfoEntry> {
    let tx = ecu_addr::GWM_TX;
    vec![
        read_did_entry(
            app,
            channel,
            tx,
            did::VIN,
            "VIN",
            format_string,
            "vehicle",
            emulator,
        ),
        read_did_entry(
            app,
            channel,
            tx,
            did::MASTER_RPM_PART,
            "SW Part",
            format_string,
            "software",
            emulator,
        ),
        read_did_entry(
            app,
            channel,
            tx,
            did::BATTERY_VOLTAGE,
            "Battery Voltage",
            format_gwm_voltage,
            "battery",
            emulator,
        ),
        read_did_entry(
            app,
            channel,
            tx,
            did::BATTERY_SOC,
            "Battery SOC",
            format_soc,
            "battery",
            emulator,
        ),
        read_did_entry(
            app,
            channel,
            tx,
            did::BATTERY_TEMP,
            "Battery Temp",
            format_gwm_temp,
            "battery",
            emulator,
        ),
    ]
}

fn read_ipc_info<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    channel: &dyn crate::j2534::Channel,
    emulator: Option<&EcuEmulatorManager>,
) -> Vec<EcuInfoEntry> {
    let tx = ecu_addr::IPC_TX;
    vec![
        read_did_entry(
            app,
            channel,
            tx,
            did::VIN,
            "VIN",
            format_string,
            "vehicle",
            emulator,
        ),
        read_did_entry(
            app,
            channel,
            tx,
            did::MASTER_RPM_PART,
            "SW Part",
            format_string,
            "software",
            emulator,
        ),
        read_did_entry(
            app,
            channel,
            tx,
            0x61BB,
            "Odometer",
            format_odometer,
            "vehicle",
            emulator,
        ),
    ]
}

/// Returns path for saving dump files — uses exe parent dir so it's always findable
fn dump_path(filename: &str) -> std::path::PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            return parent.join(filename);
        }
    }
    std::path::PathBuf::from(filename)
}

/// Full BCM DID scan — reads all known BCM DIDs in default + extended session,
/// saves raw response bytes to bcm_dump.json for offline emulator tuning.
#[tauri::command]
pub fn scan_bcm_full(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    scan_bcm_full_inner(&app, &state).map_err(|e| log_err("scan_bcm_full", e))
}

fn scan_bcm_full_inner(app: &AppHandle, state: &State<'_, AppState>) -> Result<String, String> {
    let conn = state.connection.lock().map_err(|e| e.to_string())?;
    let conn = conn.as_ref().ok_or("Not connected")?;
    let channel: &dyn crate::j2534::Channel = conn.channel.as_ref().ok_or("No channel")?;
    let emulator = conn.emulator_manager.as_ref();
    let tx = ecu_addr::BCM_TX;

    // All DIDs to scan: standard ISO 14229 DIDs
    let all_dids: &[u16] = &[
        // Standard ISO 14229 DIDs
        0xF190, 0xF188, 0xF18C, 0xF113, 0xF120, 0xF1A5, 0xF180, 0xF181, 0xF187, 0xF189, 0xF191,
        0xF1F1, // BCM-specific from MDX_BCM X260
        0x0528, 0x2A00, 0x2A01, 0x2A02, 0x2A03, 0x2A04, 0x3008, 0x3009, 0x300A, 0x300B, 0x401B,
        0x401C, 0x401D, 0x401E, 0x4020, 0x4021, 0x4025, 0x4026, 0x4027, 0x4028, 0x4029, 0x402A,
        0x402C, 0x402E, 0x4047, 0x4058, 0x4062, 0x4090, 0x40AB, 0x40DE, 0x41C3, 0x41DD, 0x5B17,
        0xA112, 0xC00B, 0xC124, 0xC18C, 0xC190, 0xC25F, 0xD00E, 0xD134, 0xDD01, 0xDD06, 0xDE00,
        0xDE01, 0xDE02, 0xDE03, 0xDE04, 0xDE06, 0xE103, 0xEE03, 0xEEB0, 0xEEB1, 0xEEB3, 0xEEBB,
    ];

    emit_log_simple(app, LogDirection::Tx, &[], "=== BCM FULL SCAN START ===");

    // Wake BCM with TesterPresent
    let _ = send_uds_request(app, channel, tx, &[0x3E, 0x00], false, emulator);

    let mut did_results: Vec<serde_json::Value> = Vec::new();
    let mut failed_in_default: Vec<u16> = Vec::new();

    // Pass 1: Default session
    emit_log_simple(app, LogDirection::Tx, &[], "Pass 1: Default session");
    for &did in all_dids {
        let req = [0x22, (did >> 8) as u8, (did & 0xFF) as u8];
        match send_uds_request(app, channel, tx, &req, false, emulator) {
            Ok(resp) => {
                let hex: String = resp
                    .iter()
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<_>>()
                    .join(" ");
                let ascii: String = resp
                    .iter()
                    .skip(3)
                    .map(|&b| {
                        if (0x20..0x7F).contains(&b) {
                            b as char
                        } else {
                            '.'
                        }
                    })
                    .collect();
                did_results.push(serde_json::json!({
                    "did": format!("{:04X}", did),
                    "session": "default",
                    "raw_hex": hex,
                    "bytes": resp,
                    "ascii": ascii.trim(),
                }));
            }
            Err(e) => {
                failed_in_default.push(did);
                did_results.push(serde_json::json!({
                    "did": format!("{:04X}", did),
                    "session": "default",
                    "error": e,
                }));
            }
        }
        // Brief TesterPresent to keep session alive
        let _ = send_uds_request(app, channel, tx, &[0x3E, 0x00], false, emulator);
    }

    // Pass 2: Extended session — retry failed DIDs
    if !failed_in_default.is_empty() {
        emit_log_simple(
            app,
            LogDirection::Tx,
            &[0x10, 0x03],
            "Pass 2: Extended session",
        );
        let _ = send_uds_request(app, channel, tx, &[0x10, 0x03], false, emulator);

        for &did in &failed_in_default {
            let req = [0x22, (did >> 8) as u8, (did & 0xFF) as u8];
            match send_uds_request(app, channel, tx, &req, false, emulator) {
                Ok(resp) => {
                    let hex: String = resp
                        .iter()
                        .map(|b| format!("{:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let ascii: String = resp
                        .iter()
                        .skip(3)
                        .map(|&b| {
                            if (0x20..0x7F).contains(&b) {
                                b as char
                            } else {
                                '.'
                            }
                        })
                        .collect();
                    // Replace the failed entry with success in extended session
                    if let Some(entry) = did_results
                        .iter_mut()
                        .find(|e| e["did"] == format!("{:04X}", did))
                    {
                        *entry = serde_json::json!({
                            "did": format!("{:04X}", did),
                            "session": "extended",
                            "raw_hex": hex,
                            "bytes": resp,
                            "ascii": ascii.trim(),
                        });
                    }
                }
                Err(_) => {} // Still failed — keep original error entry
            }
            let _ = send_uds_request(app, channel, tx, &[0x3E, 0x00], false, emulator);
        }
    }

    // Try CCF routines on BCM
    emit_log_simple(app, LogDirection::Tx, &[], "Trying BCM CCF routines...");
    let _ = send_uds_request(app, channel, tx, &[0x10, 0x03], false, emulator);
    let ccf_list_resp =
        send_uds_request(app, channel, tx, &[0x31, 0x01, 0x0E, 0x02], true, emulator);
    let ccf_retrieve_resp =
        send_uds_request(app, channel, tx, &[0x31, 0x01, 0x0E, 0x01], true, emulator);

    let ccf = serde_json::json!({
        "list_0E02": ccf_list_resp.as_ref().ok().map(|r| r.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" ")),
        "list_error": ccf_list_resp.as_ref().err(),
        "retrieve_0E01": ccf_retrieve_resp.as_ref().ok().map(|r| r.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" ")),
        "retrieve_error": ccf_retrieve_resp.as_ref().err(),
    });

    let ok_count = did_results
        .iter()
        .filter(|e| e.get("raw_hex").is_some())
        .count();
    let dump = serde_json::json!({
        "ecu": "BCM",
        "vehicle": "X260 MY16 Jaguar XF",
        "tx_id": format!("0x{:03X}", tx),
        "rx_id": format!("0x{:03X}", ecu_addr::BCM_RX),
        "total_dids": all_dids.len(),
        "ok_count": ok_count,
        "dids": did_results,
        "ccf": ccf,
    });

    let json_str = serde_json::to_string_pretty(&dump).map_err(|e| e.to_string())?;
    let path = dump_path("bcm_dump.json");
    std::fs::write(&path, &json_str).map_err(|e| format!("Write failed: {}", e))?;

    let msg = format!(
        "BCM scan done: {}/{} DIDs OK → {}",
        ok_count,
        all_dids.len(),
        path.display()
    );
    emit_log_simple(app, LogDirection::Rx, &[], &msg);
    Ok(msg)
}

/// Full GWM DID scan — reads all GWM DIDs (MDX_GWM X260 EXML), saves to gwm_dump.json
#[tauri::command]
pub fn scan_gwm_full(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    scan_gwm_full_inner(&app, &state).map_err(|e| log_err("scan_gwm_full", e))
}

fn scan_gwm_full_inner(app: &AppHandle, state: &State<'_, AppState>) -> Result<String, String> {
    let conn = state.connection.lock().map_err(|e| e.to_string())?;
    let conn = conn.as_ref().ok_or("Not connected")?;
    let channel: &dyn crate::j2534::Channel = conn.channel.as_ref().ok_or("No channel")?;
    let emulator = conn.emulator_manager.as_ref();
    let tx = ecu_addr::GWM_TX;

    // Standard ISO + GWM-specific DIDs from MDX_GWM X260
    let all_dids: &[u16] = &[
        0xF190, 0xF188, 0xF18C, 0xF113,
        // Battery management DIDs (GWM is the battery manager in X260)
        0x401B, 0x401C, 0x401E, 0x4020, 0x4021, 0x4025, 0x4026, 0x4027, 0x4028, 0x4029, 0x402A,
        0x402C, 0x402E, 0x4035, 0x4047, 0x4058, 0x4090, 0x0536, 0x41D0, 0x41E4, 0x41E5, 0x41E6,
        // Diagnostic session
        0xD100,
    ];

    emit_log_simple(app, LogDirection::Tx, &[], "=== GWM FULL SCAN START ===");
    scan_ecu_dids(
        app,
        channel,
        tx,
        "GWM",
        ecu_addr::GWM_RX,
        all_dids,
        emulator,
        "gwm_dump.json",
    )
}

/// Full IPC DID scan — reads all IPC DIDs (MDX_IPC X260 EXML), saves to ipc_dump.json
#[tauri::command]
pub fn scan_ipc_full(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    scan_ipc_full_inner(&app, &state).map_err(|e| log_err("scan_ipc_full", e))
}

fn scan_ipc_full_inner(app: &AppHandle, state: &State<'_, AppState>) -> Result<String, String> {
    let conn = state.connection.lock().map_err(|e| e.to_string())?;
    let conn = conn.as_ref().ok_or("Not connected")?;
    let channel: &dyn crate::j2534::Channel = conn.channel.as_ref().ok_or("No channel")?;
    let emulator = conn.emulator_manager.as_ref();
    let tx = ecu_addr::IPC_TX;

    // IPC DIDs from MDX_IPC X260 + standard ISO
    let all_dids: &[u16] = &[
        0xF190, 0xF188, 0xF111, 0xF18C, 0xF113, // IPC-specific
        0x61AB, 0x61AC, 0x61BB, 0xDD00, 0xC124, // Diagnostic session
        0xD100,
    ];

    emit_log_simple(app, LogDirection::Tx, &[], "=== IPC FULL SCAN START ===");
    scan_ecu_dids(
        app,
        channel,
        tx,
        "IPC",
        ecu_addr::IPC_RX,
        all_dids,
        emulator,
        "ipc_dump.json",
    )
}

/// Generic ECU DID scan: default session, then extended for failed DIDs. Saves to JSON.
fn scan_ecu_dids(
    app: &AppHandle,
    channel: &dyn crate::j2534::Channel,
    tx: u32,
    ecu_name: &str,
    rx_id: u32,
    all_dids: &[u16],
    emulator: Option<&EcuEmulatorManager>,
    filename: &str,
) -> Result<String, String> {
    // Wake ECU
    let _ = send_uds_request(app, channel, tx, &[0x3E, 0x00], false, emulator);

    let mut did_results: Vec<serde_json::Value> = Vec::new();
    let mut failed_in_default: Vec<u16> = Vec::new();

    emit_log_simple(app, LogDirection::Tx, &[], "Pass 1: Default session");
    for &did in all_dids {
        let req = [0x22, (did >> 8) as u8, (did & 0xFF) as u8];
        match send_uds_request(app, channel, tx, &req, false, emulator) {
            Ok(resp) => {
                let hex: String = resp
                    .iter()
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<_>>()
                    .join(" ");
                let ascii: String = resp
                    .iter()
                    .skip(3)
                    .map(|&b| {
                        if (0x20..0x7F).contains(&b) {
                            b as char
                        } else {
                            '.'
                        }
                    })
                    .collect();
                did_results.push(serde_json::json!({
                    "did": format!("{:04X}", did),
                    "session": "default",
                    "raw_hex": hex,
                    "bytes": resp,
                    "ascii": ascii.trim(),
                }));
            }
            Err(e) => {
                failed_in_default.push(did);
                did_results.push(serde_json::json!({
                    "did": format!("{:04X}", did),
                    "session": "default",
                    "error": e,
                }));
            }
        }
        let _ = send_uds_request(app, channel, tx, &[0x3E, 0x00], false, emulator);
    }

    if !failed_in_default.is_empty() {
        emit_log_simple(
            app,
            LogDirection::Tx,
            &[0x10, 0x03],
            "Pass 2: Extended session",
        );
        let _ = send_uds_request(app, channel, tx, &[0x10, 0x03], false, emulator);
        for &did in &failed_in_default {
            let req = [0x22, (did >> 8) as u8, (did & 0xFF) as u8];
            match send_uds_request(app, channel, tx, &req, false, emulator) {
                Ok(resp) => {
                    let hex: String = resp
                        .iter()
                        .map(|b| format!("{:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let ascii: String = resp
                        .iter()
                        .skip(3)
                        .map(|&b| {
                            if (0x20..0x7F).contains(&b) {
                                b as char
                            } else {
                                '.'
                            }
                        })
                        .collect();
                    // Replace the failed entry with success in extended session
                    if let Some(entry) = did_results
                        .iter_mut()
                        .find(|e| e["did"] == format!("{:04X}", did))
                    {
                        *entry = serde_json::json!({
                            "did": format!("{:04X}", did),
                            "session": "extended",
                            "raw_hex": hex,
                            "bytes": resp,
                            "ascii": ascii.trim(),
                        });
                    }
                }
                Err(_) => {}
            }
            let _ = send_uds_request(app, channel, tx, &[0x3E, 0x00], false, emulator);
        }
    }

    let ok_count = did_results
        .iter()
        .filter(|e| e.get("raw_hex").is_some())
        .count();
    let dump = serde_json::json!({
        "ecu": ecu_name,
        "vehicle": "X260 MY16 Jaguar XF",
        "tx_id": format!("0x{:03X}", tx),
        "rx_id": format!("0x{:03X}", rx_id),
        "total_dids": all_dids.len(),
        "ok_count": ok_count,
        "dids": did_results,
    });

    let json_str = serde_json::to_string_pretty(&dump).map_err(|e| e.to_string())?;
    let path = dump_path(filename);
    std::fs::write(&path, &json_str).map_err(|e| format!("Write failed: {}", e))?;

    let msg = format!(
        "{} scan done: {}/{} DIDs OK → {}",
        ecu_name,
        ok_count,
        all_dids.len(),
        path.display()
    );
    emit_log_simple(app, LogDirection::Rx, &[], &msg);
    Ok(msg)
}

/// SDD prerequisite flow: TesterPresent → Extended Session → Security Access (if needed)
/// This is the standard JLR SDD sequence required before executing secured routines.
fn sdd_prerequisite_flow<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    channel: &dyn crate::j2534::Channel,
    needs_security: bool,
    emulator: Option<&EcuEmulatorManager>,
) -> Result<(), String> {
    // Step 1: TesterPresent
    emit_log_simple(app, LogDirection::Tx, &[0x3E, 0x00], "TesterPresent");
    send_uds_request(
        app,
        channel,
        ecu_addr::IMC_TX,
        &[0x3E, 0x00],
        false,
        emulator,
    )
    .map_err(|e| format!("TesterPresent failed: {}", e))?;

    // Step 2: Extended session
    emit_log_simple(
        app,
        LogDirection::Tx,
        &[0x10, 0x03],
        "DiagnosticSessionControl Extended",
    );
    send_uds_request(
        app,
        channel,
        ecu_addr::IMC_TX,
        &[0x10, 0x03],
        false,
        emulator,
    )
    .map_err(|e| format!("Extended session failed: {}", e))?;

    // Step 3: Security Access (if required)
    if needs_security {
        emit_log_simple(
            app,
            LogDirection::Tx,
            &[0x27, 0x11],
            "SecurityAccess RequestSeed",
        );
        let seed_resp = send_uds_request(
            app,
            channel,
            ecu_addr::IMC_TX,
            &[0x27, 0x11],
            false,
            emulator,
        )
        .map_err(|e| format!("Security seed request failed: {}", e))?;

        if seed_resp.len() >= 5 {
            let seed = &seed_resp[2..5];
            let seed_int = ((seed[0] as u32) << 16) | ((seed[1] as u32) << 8) | (seed[2] as u32);

            if seed_int != 0 {
                let key_int =
                    crate::uds::keygen::keygen_mki(seed_int, &crate::uds::keygen::DC0314_CONSTANTS);
                let key_bytes = [
                    ((key_int >> 16) & 0xFF) as u8,
                    ((key_int >> 8) & 0xFF) as u8,
                    (key_int & 0xFF) as u8,
                ];

                let mut key_request = vec![0x27, 0x12];
                key_request.extend_from_slice(&key_bytes);
                emit_log_simple(
                    app,
                    LogDirection::Tx,
                    &key_request,
                    "SecurityAccess SendKey",
                );
                send_uds_request(
                    app,
                    channel,
                    ecu_addr::IMC_TX,
                    &key_request,
                    false,
                    emulator,
                )
                .map_err(|e| format!("Security key send failed: {}", e))?;
            }
        }
    }

    Ok(())
}

/// Look up routine metadata from the known routines list
fn find_routine_meta(routine_id: u16) -> Option<RoutineInfo> {
    list_routines()
        .into_iter()
        .find(|r| r.routine_id == routine_id)
}

/// Run a generic routine with SDD prerequisite flow
#[tauri::command]
pub fn run_routine(
    app: AppHandle,
    state: State<'_, AppState>,
    routine_id: u16,
    data: Vec<u8>,
) -> Result<RoutineResponse, String> {
    run_routine_inner(&app, &state, routine_id, &data).map_err(|e| log_err("run_routine", e))
}

fn run_routine_inner(
    app: &AppHandle,
    state: &State<'_, AppState>,
    routine_id: u16,
    data: &[u8],
) -> Result<RoutineResponse, String> {
    let conn = state.connection.lock().map_err(|e| e.to_string())?;
    let conn = conn.as_ref().ok_or("Not connected")?;
    let channel: &dyn crate::j2534::Channel =
        conn.channel.as_ref().ok_or("No channel available")?;

    // Look up routine metadata for SDD flow requirements
    let meta = find_routine_meta(routine_id);
    let needs_security = meta.as_ref().map_or(false, |m| m.needs_security);
    let needs_pending = meta.as_ref().map_or(false, |m| m.needs_pending);

    let emulator = conn.emulator_manager.as_ref();

    // Run SDD prerequisite flow (TesterPresent + Extended Session + optional Security)
    sdd_prerequisite_flow(app, channel, needs_security, emulator)?;

    // Send RoutineControl Start
    let mut request = vec![
        0x31,
        0x01,
        (routine_id >> 8) as u8,
        (routine_id & 0xFF) as u8,
    ];
    request.extend_from_slice(data);

    let resp = send_uds_request(
        app,
        channel,
        ecu_addr::IMC_TX,
        &request,
        needs_pending,
        emulator,
    )
    .map_err(|e| format!("Routine 0x{:04X} failed: {}", routine_id, e))?;

    let raw_data = if resp.len() > 4 {
        resp[4..].to_vec()
    } else {
        vec![]
    };

    let success = !resp.is_empty() && resp[0] == 0x71;
    let description = if success {
        format!(
            "Routine 0x{:04X} OK: {}",
            routine_id,
            raw_data
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join(" ")
        )
    } else {
        format!("Routine 0x{:04X} failed", routine_id)
    };

    Ok(RoutineResponse {
        success,
        description,
        raw_data,
    })
}

/// Read CCF (Central Configuration File) from IMC
/// Read the full CCF data block from one ECU via a DID read.
/// GWM uses 0xEE00, BCM uses 0xDE00.
fn read_ccf_block_did<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    channel: &dyn crate::j2534::Channel,
    tx: u32,
    ecu_name: &str,
    did: u16,
    emulator: Option<&EcuEmulatorManager>,
) -> Option<Vec<u8>> {
    let req = [0x22, (did >> 8) as u8, (did & 0xFF) as u8];
    emit_log_simple(
        app,
        LogDirection::Tx,
        &req,
        &format!("ReadDID 0x{:04X} ({} CCF block)", did, ecu_name),
    );
    match send_uds_request(app, channel, tx, &req, true, emulator) {
        Ok(resp) if resp.len() > 3 => {
            let data = resp[3..].to_vec();
            emit_log_simple(
                app,
                LogDirection::Rx,
                &[],
                &format!("{} CCF block: {} bytes", ecu_name, data.len()),
            );
            Some(data)
        }
        Ok(_) => {
            emit_log_simple(
                app,
                LogDirection::Rx,
                &[],
                &format!("{} CCF block: empty response", ecu_name),
            );
            None
        }
        Err(e) => {
            emit_log_simple(
                app,
                LogDirection::Rx,
                &[],
                &format!("{} CCF block failed: {}", ecu_name, e),
            );
            None
        }
    }
}

/// Read IMC's stored CCF via routine 0x0E01 (Report Central Configuration).
/// Returns the raw bytes if successful.
fn read_ccf_report_imc<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    channel: &dyn crate::j2534::Channel,
    emulator: Option<&EcuEmulatorManager>,
) -> Option<Vec<u8>> {
    let _ = send_uds_request(
        app,
        channel,
        ecu_addr::IMC_TX,
        &[0x3E, 0x00],
        false,
        emulator,
    );
    // Try 0x0E01 with no args, then with arg 0x01 (GWM_SYSTEM_A source)
    for args in [vec![0x31u8, 0x01, 0x0E, 0x01], vec![0x31, 0x01, 0x0E, 0x01, 0x01]] {
        let label = if args.len() == 4 {
            "RoutineControl Report CCF (0x0E01)"
        } else {
            "RoutineControl Report CCF (0x0E01 src=01)"
        };
        emit_log_simple(app, LogDirection::Tx, &args, label);
        match send_uds_request(app, channel, ecu_addr::IMC_TX, &args, true, emulator) {
            Ok(resp) if resp.len() > 4 => {
                let data = resp[4..].to_vec();
                emit_log_simple(
                    app,
                    LogDirection::Rx,
                    &[],
                    &format!("IMC Report CCF OK: {} bytes", data.len()),
                );
                return Some(data);
            }
            Ok(_) => {}
            Err(e) => {
                emit_log_simple(
                    app,
                    LogDirection::Rx,
                    &[],
                    &format!("{} failed: {}", label, e),
                );
            }
        }
    }

    // Try 0x0E00 retrieve results (31 03 0E 00)
    let results_req = [0x31u8, 0x03, 0x0E, 0x00];
    emit_log_simple(
        app,
        LogDirection::Tx,
        &results_req,
        "RoutineControl Request Results CCF (0x0E00)",
    );
    if let Ok(resp) = send_uds_request(app, channel, ecu_addr::IMC_TX, &results_req, true, emulator) {
        if resp.len() > 4 {
            let data = resp[4..].to_vec();
            emit_log_simple(
                app,
                LogDirection::Rx,
                &[],
                &format!("IMC CCF Results: {} bytes", data.len()),
            );
            return Some(data);
        }
    }

    // Fallback: try DID 0xEE00 on IMC itself
    read_ccf_block_did(app, channel, ecu_addr::IMC_TX, "IMC", 0xEE00, emulator)
}

/// Compare CCF across GWM, BCM, and IMC.
/// Reads the full CCF block from each ECU and decodes option values.
#[tauri::command]
pub fn compare_ccf(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<CcfCompareEntry>, String> {
    compare_ccf_inner(&app, &state).map_err(|e| log_err("compare_ccf", e))
}

fn compare_ccf_inner(
    app: &AppHandle,
    state: &State<'_, AppState>,
) -> Result<Vec<CcfCompareEntry>, String> {
    let conn = state.connection.lock().map_err(|e| e.to_string())?;
    let conn = conn.as_ref().ok_or("Not connected")?;
    let channel: &dyn crate::j2534::Channel =
        conn.channel.as_ref().ok_or("No channel available")?;
    let emulator = conn.emulator_manager.as_ref();

    // Enter Extended Session (needed for DID reads on some ECUs)
    sdd_prerequisite_flow(app, channel, false, emulator)?;

    // --- GWM CCF ---
    let _ = send_uds_request(
        app,
        channel,
        ecu_addr::GWM_TX,
        &[0x3E, 0x00],
        false,
        emulator,
    );
    let _ = send_uds_request(
        app,
        channel,
        ecu_addr::GWM_TX,
        &[0x10, 0x03],
        false,
        emulator,
    );
    let gwm_block = read_ccf_block_did(app, channel, ecu_addr::GWM_TX, "GWM", 0xEE00, emulator);

    // --- BCM CCF ---
    let _ = send_uds_request(
        app,
        channel,
        ecu_addr::BCM_TX,
        &[0x3E, 0x00],
        false,
        emulator,
    );
    let _ = send_uds_request(
        app,
        channel,
        ecu_addr::BCM_TX,
        &[0x10, 0x03],
        false,
        emulator,
    );
    let bcm_block = read_ccf_block_did(app, channel, ecu_addr::BCM_TX, "BCM", 0xDE00, emulator);

    // --- IMC CCF ---
    let _ = send_uds_request(
        app,
        channel,
        ecu_addr::IMC_TX,
        &[0x3E, 0x00],
        false,
        emulator,
    );
    let _ = send_uds_request(
        app,
        channel,
        ecu_addr::IMC_TX,
        &[0x10, 0x03],
        false,
        emulator,
    );
    let imc_block = read_ccf_report_imc(app, channel, emulator);

    // --- Decode and compare ---
    // CCF DID response has a 21-byte VDF header before the option data.
    // Option N's value is at byte (21 + N) in the raw DID response.
    const CCF_HDR: usize = 21;

    let mut entries: Vec<CcfCompareEntry> = Vec::new();

    for &opt_id in IMC_CCF_OPTION_IDS {
        let idx = CCF_HDR + opt_id as usize;

        let gwm_val = gwm_block.as_deref().and_then(|b| b.get(idx)).copied();
        let bcm_val = bcm_block.as_deref().and_then(|b| b.get(idx)).copied();
        let imc_val = imc_block.as_deref().and_then(|b| b.get(idx)).copied();

        let gwm_str = gwm_val.map(|v| decode_ccf_value(opt_id, v));
        let bcm_str = bcm_val.map(|v| decode_ccf_value(opt_id, v));
        let imc_str = imc_val.map(|v| decode_ccf_value(opt_id, v));

        // Mismatch if any two are present and differ
        let mismatch = match (&gwm_str, &bcm_str, &imc_str) {
            (Some(g), Some(b), _) if g != b => true,
            (Some(g), _, Some(i)) if g != i => true,
            (_, Some(b), Some(i)) if b != i => true,
            _ => false,
        };

        entries.push(CcfCompareEntry {
            option_id: opt_id,
            name: ccf_option_name(opt_id),
            gwm: gwm_str,
            bcm: bcm_str,
            imc: imc_str,
            mismatch,
        });
    }

    // Save each ECU's raw block separately
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    for (name, block_opt, filename) in [
        ("GWM", &gwm_block, "gwm_ccf_raw.json"),
        ("BCM", &bcm_block, "bcm_ccf_raw.json"),
        ("IMC", &imc_block, "imc_ccf_raw.json"),
    ] {
        if let Some(block) = block_opt {
            let hex: String = block
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join(" ");
            let dump = serde_json::json!({
                "ecu": name,
                "timestamp": ts,
                "bytes": block.len(),
                "raw_hex": hex,
                "raw_bytes": block,
            });
            if let Ok(json_str) = serde_json::to_string_pretty(&dump) {
                let path = dump_path(filename);
                let _ = std::fs::write(&path, &json_str);
                emit_log_simple(
                    app,
                    LogDirection::Rx,
                    &[],
                    &format!("{} CCF raw saved → {}", name, path.display()),
                );
            }
        }
    }

    // Save comparison to file
    let mismatches: Vec<_> = entries.iter().filter(|e| e.mismatch).collect();
    let dump = serde_json::json!({
        "timestamp": ts,
        "gwm_block_bytes": gwm_block.as_ref().map(|b| b.len()),
        "bcm_block_bytes": bcm_block.as_ref().map(|b| b.len()),
        "imc_block_bytes": imc_block.as_ref().map(|b| b.len()),
        "mismatches": mismatches.len(),
        "options": entries.iter().map(|e| serde_json::json!({
            "id": e.option_id,
            "name": e.name,
            "gwm": e.gwm,
            "bcm": e.bcm,
            "imc": e.imc,
            "mismatch": e.mismatch,
        })).collect::<Vec<_>>(),
    });
    if let Ok(json_str) = serde_json::to_string_pretty(&dump) {
        let path = dump_path("ccf_compare.json");
        let _ = std::fs::write(&path, &json_str);
        emit_log_simple(
            app,
            LogDirection::Rx,
            &[],
            &format!(
                "CCF compare saved → {} ({} mismatches)",
                path.display(),
                mismatches.len()
            ),
        );
    }

    Ok(entries)
}

/// Flow: Retrieve CCF (0x0E00) to download from network, then List CCF (0x0E02) to read stored data
#[tauri::command]
pub fn read_ccf(app: AppHandle, state: State<'_, AppState>) -> Result<Vec<EcuInfoEntry>, String> {
    read_ccf_inner(&app, &state).map_err(|e| log_err("read_ccf", e))
}

fn read_ccf_inner(
    app: &AppHandle,
    state: &State<'_, AppState>,
) -> Result<Vec<EcuInfoEntry>, String> {
    let conn = state.connection.lock().map_err(|e| e.to_string())?;
    let conn = conn.as_ref().ok_or("Not connected")?;
    let channel: &dyn crate::j2534::Channel =
        conn.channel.as_ref().ok_or("No channel available")?;
    let emulator = conn.emulator_manager.as_ref();

    let bench_mode = emulator.is_some();

    // SDD prerequisite flow (no security needed for CCF)
    sdd_prerequisite_flow(app, channel, false, emulator)?;

    // On bench, skip Retrieve CCF (0x0E00) — it downloads from GWM which doesn't exist on bench.
    // Go directly to List CCF (0x0E02) which reads what's stored in IMC flash.
    if !bench_mode {
        // Step 1: Retrieve CCF (0x0E00) — downloads CCF from GWM to IMC
        let retrieve_req = vec![0x31, 0x01, 0x0E, 0x00];
        emit_log_simple(
            app,
            LogDirection::Tx,
            &retrieve_req,
            "RoutineControl Retrieve CCF (0x0E00)",
        );
        match send_uds_request(
            app,
            channel,
            ecu_addr::IMC_TX,
            &retrieve_req,
            true,
            emulator,
        ) {
            Ok(resp) => {
                let data = if resp.len() > 4 {
                    &resp[4..]
                } else {
                    &[] as &[u8]
                };
                emit_log_simple(
                    app,
                    LogDirection::Rx,
                    &[],
                    &format!("Retrieve CCF OK: {} bytes", data.len()),
                );
                if !data.is_empty() {
                    return Ok(parse_ccf_entries(data));
                }
                // If Retrieve returned no data, try Request Results
                let _ = send_uds_request(
                    app,
                    channel,
                    ecu_addr::IMC_TX,
                    &[0x3E, 0x00],
                    false,
                    emulator,
                );
                let results_req = vec![0x31, 0x03, 0x0E, 0x00];
                emit_log_simple(
                    app,
                    LogDirection::Tx,
                    &results_req,
                    "RoutineControl Request Results CCF (0x0E00)",
                );
                if let Ok(results_resp) =
                    send_uds_request(app, channel, ecu_addr::IMC_TX, &results_req, true, emulator)
                {
                    let results_data = if results_resp.len() > 4 {
                        &results_resp[4..]
                    } else {
                        &[] as &[u8]
                    };
                    if !results_data.is_empty() {
                        emit_log_simple(
                            app,
                            LogDirection::Rx,
                            &[],
                            &format!("CCF Results: {} bytes", results_data.len()),
                        );
                        return Ok(parse_ccf_entries(results_data));
                    }
                }
            }
            Err(e) => {
                emit_log_simple(
                    app,
                    LogDirection::Rx,
                    &[],
                    &format!("Retrieve CCF failed: {} — trying List", e),
                );
            }
        }
    } else {
        emit_log_simple(
            app,
            LogDirection::Tx,
            &[],
            "Bench mode: skipping Retrieve CCF (no GWM on bus)",
        );
    }

    // List CCF (0x0E02) — reads what's stored in IMC flash
    let _ = send_uds_request(
        app,
        channel,
        ecu_addr::IMC_TX,
        &[0x3E, 0x00],
        false,
        emulator,
    );
    let list_req = vec![0x31, 0x01, 0x0E, 0x02];
    emit_log_simple(
        app,
        LogDirection::Tx,
        &list_req,
        "RoutineControl List CCF (0x0E02)",
    );
    match send_uds_request(app, channel, ecu_addr::IMC_TX, &list_req, true, emulator) {
        Ok(resp) => {
            let ccf_data = if resp.len() > 4 {
                &resp[4..]
            } else {
                &[] as &[u8]
            };
            emit_log_simple(
                app,
                LogDirection::Rx,
                &[],
                &format!("List CCF: {} bytes", ccf_data.len()),
            );
            // Save raw CCF bytes to file for later analysis
            if !ccf_data.is_empty() {
                let hex: String = ccf_data
                    .iter()
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<_>>()
                    .join(" ");
                let dump = serde_json::json!({
                    "ecu": "IMC",
                    "routine": "0x0E02 List CCF",
                    "timestamp": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                    "bytes": ccf_data.len(),
                    "raw_hex": hex,
                    "raw_bytes": ccf_data,
                });
                let path = dump_path("imc_ccf_dump.json");
                if let Ok(json_str) = serde_json::to_string_pretty(&dump) {
                    let _ = std::fs::write(&path, &json_str);
                    emit_log_simple(
                        app,
                        LogDirection::Rx,
                        &[],
                        &format!("CCF saved → {}", path.display()),
                    );
                }
            }
            Ok(parse_ccf_entries(ccf_data))
        }
        Err(e) => {
            emit_log_simple(
                app,
                LogDirection::Rx,
                &[],
                &format!("List CCF failed: {}", e),
            );
            Ok(vec![EcuInfoEntry {
                label: "CCF Status".to_string(),
                did_hex: "CCF".to_string(),
                value: None,
                error: Some(format!("CCF not available: {}", e)),
                category: "config".to_string(),
            }])
        }
    }
}

/// Parse CCF response data into structured entries
fn parse_ccf_entries(data: &[u8]) -> Vec<EcuInfoEntry> {
    let mut entries = Vec::new();

    if data.is_empty() {
        entries.push(EcuInfoEntry {
            label: "CCF Status".to_string(),
            did_hex: "CCF".to_string(),
            value: Some("No configuration data".to_string()),
            error: None,
            category: "config".to_string(),
        });
        return entries;
    }

    // Raw CCF hex dump — always include for debugging
    let hex_str = data
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(" ");
    entries.push(EcuInfoEntry {
        label: "CCF Raw Data".to_string(),
        did_hex: "CCF".to_string(),
        value: Some(hex_str),
        error: None,
        category: "config".to_string(),
    });

    entries.push(EcuInfoEntry {
        label: "CCF Length".to_string(),
        did_hex: "CCF".to_string(),
        value: Some(format!("{} bytes", data.len())),
        error: None,
        category: "config".to_string(),
    });

    // Parse known CCF fields if enough data
    if !data.is_empty() {
        entries.push(EcuInfoEntry {
            label: "CCF Status Byte".to_string(),
            did_hex: "CCF".to_string(),
            value: Some(format!("0x{:02X}", data[0])),
            error: None,
            category: "config".to_string(),
        });
    }

    if data.len() >= 2 {
        let market = match data[1] {
            0x01 => "Europe (EU)",
            0x02 => "North America (NA)",
            0x03 => "China (CN)",
            0x04 => "Middle East (ME)",
            0x05 => "Asia Pacific (AP)",
            0x06 => "Japan (JP)",
            0x07 => "Korea (KR)",
            _ => "Unknown",
        };
        entries.push(EcuInfoEntry {
            label: "Market/Region".to_string(),
            did_hex: "CCF".to_string(),
            value: Some(format!("{} (0x{:02X})", market, data[1])),
            error: None,
            category: "config".to_string(),
        });
    }

    if data.len() >= 3 {
        let lang = match data[2] {
            0x01 => "English",
            0x02 => "French",
            0x03 => "German",
            0x04 => "Spanish",
            0x05 => "Italian",
            0x06 => "Portuguese",
            0x07 => "Dutch",
            0x08 => "Russian",
            0x09 => "Chinese",
            0x0A => "Japanese",
            0x0B => "Korean",
            0x0C => "Arabic",
            _ => "Unknown",
        };
        entries.push(EcuInfoEntry {
            label: "Language".to_string(),
            did_hex: "CCF".to_string(),
            value: Some(format!("{} (0x{:02X})", lang, data[2])),
            error: None,
            category: "config".to_string(),
        });
    }

    // Feature flags byte (if present)
    if data.len() >= 4 {
        let features = data[3];
        let mut feature_list = Vec::new();
        if features & 0x01 != 0 {
            feature_list.push("Navigation");
        }
        if features & 0x02 != 0 {
            feature_list.push("DAB Radio");
        }
        if features & 0x04 != 0 {
            feature_list.push("DVD");
        }
        if features & 0x08 != 0 {
            feature_list.push("Bluetooth");
        }
        if features & 0x10 != 0 {
            feature_list.push("WiFi");
        }
        if features & 0x20 != 0 {
            feature_list.push("USB Media");
        }
        if features & 0x40 != 0 {
            feature_list.push("Rear Camera");
        }
        if features & 0x80 != 0 {
            feature_list.push("Surround Camera");
        }
        let value = if feature_list.is_empty() {
            format!("None (0x{:02X})", features)
        } else {
            format!("{} (0x{:02X})", feature_list.join(", "), features)
        };
        entries.push(EcuInfoEntry {
            label: "Features".to_string(),
            did_hex: "CCF".to_string(),
            value: Some(value),
            error: None,
            category: "config".to_string(),
        });
    }

    entries
}

/// Read a single DID
#[tauri::command]
pub fn read_did(
    app: AppHandle,
    state: State<'_, AppState>,
    ecu_tx: u32,
    did_id: u16,
) -> Result<Vec<u8>, String> {
    let conn = state.connection.lock().map_err(|e| e.to_string())?;
    let conn = conn.as_ref().ok_or("Not connected")?;
    let channel: &dyn crate::j2534::Channel =
        conn.channel.as_ref().ok_or("No channel available")?;

    let data = send_read_did(
        &app,
        channel,
        ecu_tx,
        did_id,
        conn.emulator_manager.as_ref(),
    )
    .map_err(|e| log_err("read_did", e))?;
    Ok(data)
}

/// List available routines
#[tauri::command]
pub fn list_routines() -> Vec<RoutineInfo> {
    vec![
        // Diagnostics
        RoutineInfo {
            routine_id: routine::SSH_ENABLE,
            name: "SSH Enable".into(),
            description: "Enable SSH access on IMC (0x603E)".into(),
            category: "Diagnostics".into(),
            needs_security: true,
            needs_pending: true,
        },
        RoutineInfo {
            routine_id: routine::ENG_SCREEN_LVL2,
            name: "Engineering Screen Level 2".into(),
            description: "Enable engineering screen level 2 (0x603D)".into(),
            category: "Diagnostics".into(),
            needs_security: true,
            needs_pending: false,
        },
        RoutineInfo {
            routine_id: routine::POWER_OVERRIDE,
            name: "IMC Power Override".into(),
            description: "Override IMC power state (0x6043)".into(),
            category: "Diagnostics".into(),
            needs_security: true,
            needs_pending: false,
        },
        // Configuration
        RoutineInfo {
            routine_id: routine::CONFIGURE_LINUX,
            name: "Configure Linux to Hardware".into(),
            description: "Reconfigure IMC Linux environment (0x6038)".into(),
            category: "Configuration".into(),
            needs_security: true,
            needs_pending: true,
        },
        RoutineInfo {
            routine_id: routine::VIN_LEARN,
            name: "VIN Learn".into(),
            description: "Learn VIN to ECU (0x0404)".into(),
            category: "Configuration".into(),
            needs_security: false,
            needs_pending: false,
        },
        RoutineInfo {
            routine_id: routine::RETRIEVE_CCF,
            name: "Retrieve CCF".into(),
            description: "Retrieve Central Configuration (0x0E00)".into(),
            category: "Configuration".into(),
            needs_security: false,
            needs_pending: true,
        },
        RoutineInfo {
            routine_id: routine::REPORT_CCF,
            name: "Report CCF".into(),
            description: "Report Central Configuration (0x0E01)".into(),
            category: "Configuration".into(),
            needs_security: false,
            needs_pending: true,
        },
        RoutineInfo {
            routine_id: routine::LIST_CCF,
            name: "List CCF".into(),
            description: "List Central Configuration (0x0E02)".into(),
            category: "Configuration".into(),
            needs_security: false,
            needs_pending: true,
        },
        // Recovery
        RoutineInfo {
            routine_id: routine::DVD_RECOVER,
            name: "Recover Locked DVD Region".into(),
            description: "Recover locked DVD region (0x603F)".into(),
            category: "Recovery".into(),
            needs_security: true,
            needs_pending: false,
        },
        RoutineInfo {
            routine_id: routine::RESET_PIN,
            name: "Reset Customer Pin".into(),
            description: "Reset customer PIN code (0x6042)".into(),
            category: "Recovery".into(),
            needs_security: true,
            needs_pending: false,
        },
        // Advanced
        RoutineInfo {
            routine_id: routine::FAN_CONTROL,
            name: "Control Auxiliary Fan".into(),
            description: "Control auxiliary fan (0x6041)".into(),
            category: "Advanced".into(),
            needs_security: true,
            needs_pending: false,
        },
        RoutineInfo {
            routine_id: routine::GEN_KEY,
            name: "Generate Key".into(),
            description: "Generate cryptographic key (0x6045)".into(),
            category: "Advanced".into(),
            needs_security: true,
            needs_pending: false,
        },
        RoutineInfo {
            routine_id: routine::SHARED_SECRET,
            name: "Shared Secret".into(),
            description: "Compute shared secret (0x6046)".into(),
            category: "Advanced".into(),
            needs_security: true,
            needs_pending: false,
        },
    ]
}

// ─── Internal helpers ───────────────────────────────────────────────

/// Human-readable DID name lookup
fn did_name(did_id: u16) -> &'static str {
    match did_id {
        0xF190 => "VIN",
        0xF111 => "Firmware Part",
        0xF188 => "Master RPM Part",
        0xD100 => "Diag Session",
        0x0202 => "IMC Status",
        0x402A => "Battery Voltage",
        0x4028 => "Battery SOC",
        0x4029 => "Battery Temp",
        0x4030 => "Door Status",
        0x4032 => "Fuel Level",
        _ => "",
    }
}

/// Send raw UDS request on a channel and get response.
/// If an emulator is provided and handles the tx_id, bypass J2534 entirely.
/// Handles NRC 0x21 (busyRepeatRequest) with retries per SDD EXML:
///   MAX_BUSY_ATTEMPTS=6, MAX_RETRY_PERIOD=6000ms
/// Handles NRC 0x78 (responsePending) by continuing to wait.
fn send_uds_request<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    channel: &dyn crate::j2534::Channel,
    tx_id: u32,
    request: &[u8],
    wait_pending: bool,
    emulator: Option<&EcuEmulatorManager>,
) -> Result<Vec<u8>, String> {
    // Software routing: if the target ECU is emulated, handle locally
    if let Some(emu) = emulator {
        if let Some(response) = emu.try_handle(tx_id, request) {
            emit_log_simple(app, LogDirection::Rx, &response, "EMU");
            if response[0] == 0x7F && response.len() >= 3 {
                let nrc = crate::uds::error::NegativeResponseCode::from_byte(response[2]);
                return Err(format!("NRC: {}", nrc));
            }
            return Ok(response);
        }
    }

    let max_busy_retries: u32 = 6;

    for busy_attempt in 0..=max_busy_retries {
        if busy_attempt > 0 {
            emit_log_simple(
                app,
                LogDirection::Tx,
                &[],
                &format!("Busy retry {}/{}", busy_attempt, max_busy_retries),
            );
        }

        match send_uds_request_once(app, channel, tx_id, request, wait_pending) {
            Ok(resp) => return Ok(resp),
            Err(e) if e.contains("0x21") && busy_attempt < max_busy_retries => {
                std::thread::sleep(std::time::Duration::from_secs(1));
                continue;
            }
            Err(e) => return Err(e),
        }
    }

    Err("Max busy retries exceeded".into())
}

/// Single attempt to send a UDS request and wait for response.
fn send_uds_request_once<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    channel: &dyn crate::j2534::Channel,
    tx_id: u32,
    request: &[u8],
    wait_pending: bool,
) -> Result<Vec<u8>, String> {
    let msg = PassThruMsg::new_iso15765(tx_id, request);
    channel.send(&msg, 2000)?;

    let timeout = if wait_pending {
        std::time::Duration::from_secs(60)
    } else {
        std::time::Duration::from_secs(5)
    };
    let start = std::time::Instant::now();

    loop {
        if start.elapsed() > timeout {
            return Err("Timeout waiting for response".into());
        }

        let msgs = channel.read(500)?;
        for m in msgs {
            if m.data_size <= 4 {
                continue;
            }
            let payload = m.payload();
            if payload.is_empty() {
                continue;
            }

            emit_log_simple(app, LogDirection::Rx, payload, "");

            // Negative response
            if payload[0] == 0x7F && payload.len() >= 3 {
                // Check service ID matches our request — stale NRCs from previous
                // requests (e.g. 7F 10 12 from a session change) must be ignored
                if payload[1] != request[0] {
                    emit_log_simple(
                        app,
                        LogDirection::Rx,
                        &[],
                        &format!(
                            "Ignoring stale NRC for service 0x{:02X} (expected 0x{:02X})",
                            payload[1], request[0]
                        ),
                    );
                    continue;
                }
                if payload[2] == 0x78 {
                    emit_log_simple(app, LogDirection::Pending, payload, "Response pending...");
                    continue;
                }
                let nrc = crate::uds::error::NegativeResponseCode::from_byte(payload[2]);
                return Err(format!("NRC: {}", nrc));
            }

            // Positive response
            let expected = request[0] + 0x40;
            if payload[0] == expected {
                return Ok(payload.to_vec());
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

/// Read a DID on a channel, with optional emulator bypass
fn send_read_did<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    channel: &dyn crate::j2534::Channel,
    tx_id: u32,
    did_id: u16,
    emulator: Option<&EcuEmulatorManager>,
) -> Result<Vec<u8>, String> {
    let request = vec![0x22, (did_id >> 8) as u8, (did_id & 0xFF) as u8];
    let name = did_name(did_id);
    let label = if name.is_empty() {
        format!("ReadDID 0x{:04X}", did_id)
    } else {
        format!("ReadDID {} ({:04X})", name, did_id)
    };
    emit_log_simple(app, LogDirection::Tx, &request, &label);
    let resp = send_uds_request(app, channel, tx_id, &request, false, emulator)?;
    // Return data after service ID + DID
    if resp.len() > 3 {
        Ok(resp[3..].to_vec())
    } else {
        Ok(vec![])
    }
}

/// Export UDS logs as text (called from frontend "Copy Logs" button)
#[tauri::command]
pub fn export_logs() -> Result<String, String> {
    // This returns system info — the actual UDS log entries are in the frontend state.
    // This gives the user Rust-side context to paste alongside the UI logs.
    let mut info = String::new();
    info.push_str(&format!("UDS App v{}\n", env!("CARGO_PKG_VERSION")));
    info.push_str(&format!(
        "OS: {} {}\n",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    info.push_str(&format!(
        "Time: {}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    info.push_str("---\n");
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::j2534::mock::MockChannel;
    use crate::j2534::Channel;
    use crate::uds::services::ecu_addr;

    /// Helper: create a Tauri app for testing (no window, just the event system)
    fn test_app() -> tauri::AppHandle<tauri::test::MockRuntime> {
        let app = tauri::test::mock_app();
        app.handle().clone()
    }

    /// Helper: create a MockChannel with IMC + BCM filters set up
    fn setup_mock_channel() -> MockChannel {
        let mock = MockChannel::new();
        mock.setup_iso15765_filter(ecu_addr::IMC_TX, ecu_addr::IMC_RX)
            .unwrap();
        mock.setup_iso15765_filter(ecu_addr::BCM_TX, ecu_addr::BCM_RX)
            .unwrap();
        mock.setup_iso15765_filter(ecu_addr::GWM_TX, ecu_addr::GWM_RX)
            .unwrap();
        mock.setup_iso15765_filter(ecu_addr::IPC_TX, ecu_addr::IPC_RX)
            .unwrap();
        mock
    }

    // ─── BCM bench mode tests ───────────────────────────────────────

    #[test]
    fn test_bcm_read_with_emulator() {
        // Bench mode ON: BCM is emulated, so all reads should return emulated values
        let app = test_app();
        let mock = setup_mock_channel();
        let emu = EcuEmulatorManager::new(vec![EcuId::Bcm]);
        let entries = read_bcm_info(&app, &mock, Some(&emu));

        assert_eq!(entries.len(), 4);
        // VIN
        assert_eq!(entries[0].label, "VIN");
        assert_eq!(entries[0].category, "vehicle");
        assert!(entries[0].value.is_some(), "VIN should have emulated value");
        assert!(entries[0].error.is_none());
        assert!(entries[0].value.as_ref().unwrap().starts_with("SAJBL4BVXG"));
        // SW Part
        assert_eq!(entries[1].label, "SW Part");
        assert_eq!(entries[1].category, "software");
        assert!(entries[1].value.is_some());
        assert!(entries[1].value.as_ref().unwrap().contains("GX73"));
        // ECU Serial
        assert_eq!(entries[2].label, "ECU Serial");
        assert_eq!(entries[2].category, "hardware");
        assert!(entries[2].value.is_some());
        // HW Part
        assert_eq!(entries[3].label, "HW Part");
        assert_eq!(entries[3].category, "hardware");
        assert!(entries[3].value.is_some());
    }

    #[test]
    fn test_bcm_read_without_emulator_timeout() {
        // No bench mode, no real ECU → all reads should timeout/fail
        let app = test_app();
        let mock = setup_mock_channel();
        mock.set_timeout_mode(true);
        let entries = read_bcm_info(&app, &mock, None);

        assert_eq!(entries.len(), 4);
        for entry in &entries {
            assert!(entry.error.is_some(), "{} should have error", entry.label);
            assert!(
                entry.value.is_none(),
                "{} should not have value",
                entry.label
            );
        }
    }

    // ─── IMC bench mode tests ───────────────────────────────────────

    #[test]
    fn test_imc_read_extended_session_ok() {
        // IMC with Extended Session succeeding — should read all DIDs
        // Flow: TesterPresent → D100 → (TesterPresent+DID)×6 → ExtendedSession → TesterPresent+0202
        let app = test_app();
        let mock = setup_mock_channel();
        let tx = ecu_addr::IMC_TX;

        // TesterPresent → OK
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        // D100 (Default Session, no TesterPresent before first DID)
        mock.expect_request(tx, vec![0x22, 0xD1, 0x00], vec![0x62, 0xD1, 0x00, 0x01]);
        // TesterPresent + VIN
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(
            tx,
            vec![0x22, 0xF1, 0x90],
            vec![0x62, 0xF1, 0x90, 0x53, 0x41, 0x4A],
        );
        // TesterPresent + Software Part
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(
            tx,
            vec![0x22, 0xF1, 0x88],
            vec![0x62, 0xF1, 0x88, 0x53, 0x57],
        );
        // TesterPresent + V850
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(
            tx,
            vec![0x22, 0xF1, 0x20],
            vec![0x62, 0xF1, 0x20, 0x56, 0x38],
        );
        // TesterPresent + Polar
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(
            tx,
            vec![0x22, 0xF1, 0xA5],
            vec![0x62, 0xF1, 0xA5, 0x50, 0x4C],
        );
        // TesterPresent + ECU Serial
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(
            tx,
            vec![0x22, 0xF1, 0x8C],
            vec![0x62, 0xF1, 0x8C, 0x53, 0x4E],
        );
        // TesterPresent + ECU Serial 2
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(
            tx,
            vec![0x22, 0xF1, 0x13],
            vec![0x62, 0xF1, 0x13, 0x48, 0x57],
        );
        // Extended Session → OK
        mock.expect_request(
            tx,
            vec![0x10, 0x03],
            vec![0x50, 0x03, 0x00, 0x32, 0x01, 0xF4],
        );
        // TesterPresent + 0202 IMC Status
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(tx, vec![0x22, 0x02, 0x02], vec![0x62, 0x02, 0x02, 0x00]);

        let entries = read_imc_info(&app, &mock, None);

        assert_eq!(entries.len(), 8);
        assert_eq!(entries[0].label, "Diag Session");
        assert!(entries[0].value.is_some());
        assert_eq!(entries[1].label, "VIN");
        assert!(
            entries[1].value.is_some(),
            "VIN works in Default Session per EXML"
        );
        // Last entry is 0202 with value (Extended Session succeeded)
        assert_eq!(entries[7].label, "IMC Status");
        assert!(entries[7].value.is_some());
        assert!(entries[7].error.is_none());
    }

    #[test]
    fn test_imc_read_extended_session_fails_no_bench() {
        // Extended Session fails → D100 + 6 DIDs read in Default Session + 0202 error = 8
        let app = test_app();
        let mock = setup_mock_channel();
        let tx = ecu_addr::IMC_TX;

        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        // D100
        mock.expect_request(tx, vec![0x22, 0xD1, 0x00], vec![0x62, 0xD1, 0x00, 0x01]);
        // TesterPresent + VIN
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(
            tx,
            vec![0x22, 0xF1, 0x90],
            vec![0x62, 0xF1, 0x90, 0x56, 0x49, 0x4E],
        );
        // TesterPresent + Software Part
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(
            tx,
            vec![0x22, 0xF1, 0x88],
            vec![0x62, 0xF1, 0x88, 0x53, 0x57],
        );
        // TesterPresent + V850
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(
            tx,
            vec![0x22, 0xF1, 0x20],
            vec![0x62, 0xF1, 0x20, 0x56, 0x38],
        );
        // TesterPresent + Polar
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(
            tx,
            vec![0x22, 0xF1, 0xA5],
            vec![0x62, 0xF1, 0xA5, 0x50, 0x4C],
        );
        // TesterPresent + ECU Serial
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(
            tx,
            vec![0x22, 0xF1, 0x8C],
            vec![0x62, 0xF1, 0x8C, 0x53, 0x4E],
        );
        // TesterPresent + ECU Serial 2
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(
            tx,
            vec![0x22, 0xF1, 0x13],
            vec![0x62, 0xF1, 0x13, 0x48, 0x57],
        );
        // Extended Session → NRC 0x12 (fails)
        mock.expect_request(tx, vec![0x10, 0x03], vec![0x7F, 0x10, 0x12]);

        let entries = read_imc_info(&app, &mock, None);

        assert_eq!(entries.len(), 8);
        assert_eq!(entries[0].label, "Diag Session");
        assert!(entries[0].value.is_some());
        assert_eq!(entries[1].label, "VIN");
        assert!(entries[1].value.is_some(), "VIN works in Default Session");
        // Last entry is 0202 with error (needs Extended Session)
        assert_eq!(entries[7].label, "IMC Status");
        assert!(entries[7].error.is_some());
        assert!(entries[7]
            .error
            .as_ref()
            .unwrap()
            .contains("Extended Session"));
    }

    #[test]
    fn test_imc_read_extended_session_fails_with_bench() {
        // Bench mode ON, Extended Session fails → all DIDs read in Default, 0202 gets error
        let app = test_app();
        let mock = setup_mock_channel();
        let tx = ecu_addr::IMC_TX;
        let emu = EcuEmulatorManager::new(vec![EcuId::Bcm]);

        // Bench mode: TesterPresent poll (succeeds on first try)
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        // D100
        mock.expect_request(tx, vec![0x22, 0xD1, 0x00], vec![0x62, 0xD1, 0x00, 0x01]);
        // TesterPresent + VIN
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(
            tx,
            vec![0x22, 0xF1, 0x90],
            vec![0x62, 0xF1, 0x90, 0x56, 0x49, 0x4E],
        );
        // TesterPresent + Software Part
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(
            tx,
            vec![0x22, 0xF1, 0x88],
            vec![0x62, 0xF1, 0x88, 0x53, 0x57],
        );
        // TesterPresent + V850
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(
            tx,
            vec![0x22, 0xF1, 0x20],
            vec![0x62, 0xF1, 0x20, 0x56, 0x38],
        );
        // TesterPresent + Polar
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(
            tx,
            vec![0x22, 0xF1, 0xA5],
            vec![0x62, 0xF1, 0xA5, 0x50, 0x4C],
        );
        // TesterPresent + ECU Serial
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(
            tx,
            vec![0x22, 0xF1, 0x8C],
            vec![0x62, 0xF1, 0x8C, 0x53, 0x4E],
        );
        // TesterPresent + ECU Serial 2
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(
            tx,
            vec![0x22, 0xF1, 0x13],
            vec![0x62, 0xF1, 0x13, 0x48, 0x57],
        );
        // Extended Session → fails
        mock.expect_request(tx, vec![0x10, 0x03], vec![0x7F, 0x10, 0x12]);

        let entries = read_imc_info(&app, &mock, Some(&emu));

        assert_eq!(entries.len(), 8);
        assert_eq!(entries[0].label, "Diag Session");
        assert!(entries[0].value.is_some());
        assert_eq!(entries[1].label, "VIN");
        assert!(entries[1].value.is_some(), "VIN works in Default Session");
        // Last entry is 0202 with error (needs Extended Session)
        assert_eq!(entries[7].label, "IMC Status");
        assert!(entries[7].error.is_some());
        assert!(entries[7]
            .error
            .as_ref()
            .unwrap()
            .contains("Extended Session"));
    }

    #[test]
    fn test_imc_did_failure_does_not_block_others() {
        // When a DID read fails, subsequent DIDs should still be attempted
        let app = test_app();
        let mock = setup_mock_channel();
        let tx = ecu_addr::IMC_TX;

        // TesterPresent → OK
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        // D100 → OK
        mock.expect_request(tx, vec![0x22, 0xD1, 0x00], vec![0x62, 0xD1, 0x00, 0x01]);
        // TesterPresent + VIN → NRC 0x31 (failure)
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(tx, vec![0x22, 0xF1, 0x90], vec![0x7F, 0x22, 0x31]);
        // TesterPresent + Software Part → OK
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(
            tx,
            vec![0x22, 0xF1, 0x88],
            vec![0x62, 0xF1, 0x88, 0x53, 0x57],
        );
        // TesterPresent + V850 → OK
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(
            tx,
            vec![0x22, 0xF1, 0x20],
            vec![0x62, 0xF1, 0x20, 0x56, 0x38],
        );
        // TesterPresent + Polar → OK
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(
            tx,
            vec![0x22, 0xF1, 0xA5],
            vec![0x62, 0xF1, 0xA5, 0x50, 0x4C],
        );
        // TesterPresent + ECU Serial → OK
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(
            tx,
            vec![0x22, 0xF1, 0x8C],
            vec![0x62, 0xF1, 0x8C, 0x53, 0x4E],
        );
        // TesterPresent + ECU Serial 2 → OK
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(
            tx,
            vec![0x22, 0xF1, 0x13],
            vec![0x62, 0xF1, 0x13, 0x48, 0x57],
        );
        // Extended Session → OK
        mock.expect_request(
            tx,
            vec![0x10, 0x03],
            vec![0x50, 0x03, 0x00, 0x32, 0x01, 0xF4],
        );
        // TesterPresent + 0202
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(tx, vec![0x22, 0x02, 0x02], vec![0x62, 0x02, 0x02, 0x00]);

        let entries = read_imc_info(&app, &mock, None);

        assert_eq!(entries.len(), 8);
        assert_eq!(entries[0].label, "Diag Session");
        assert!(entries[0].value.is_some(), "D100 should succeed");
        assert_eq!(entries[1].label, "VIN");
        assert!(entries[1].error.is_some(), "VIN should have error");
        assert_eq!(entries[2].label, "SW Part");
        assert!(
            entries[2].value.is_some(),
            "Software Part should succeed despite VIN failure"
        );
    }

    // ─── NRC 0x21 retry tests ─────────────────────────────────────

    #[test]
    fn test_nrc_0x21_retry_succeeds_on_4th_attempt() {
        // First 3 attempts return NRC 0x21 (busyRepeatRequest), 4th succeeds
        let app = test_app();
        let mock = setup_mock_channel();
        let tx = ecu_addr::IMC_TX;

        // 3x NRC 0x21, then success
        mock.expect_request(tx, vec![0x22, 0xF1, 0x90], vec![0x7F, 0x22, 0x21]);
        mock.expect_request(tx, vec![0x22, 0xF1, 0x90], vec![0x7F, 0x22, 0x21]);
        mock.expect_request(tx, vec![0x22, 0xF1, 0x90], vec![0x7F, 0x22, 0x21]);
        mock.expect_request(
            tx,
            vec![0x22, 0xF1, 0x90],
            vec![0x62, 0xF1, 0x90, 0x56, 0x49, 0x4E],
        );

        let result = send_uds_request(&app, &mock, tx, &[0x22, 0xF1, 0x90], false, None);
        assert!(
            result.is_ok(),
            "Should succeed after 3 retries: {:?}",
            result
        );
    }

    #[test]
    fn test_nrc_0x21_retry_exhausted() {
        // All 7 attempts (1 + 6 retries) return NRC 0x21 → error
        let app = test_app();
        let mock = setup_mock_channel();
        let tx = ecu_addr::IMC_TX;

        for _ in 0..7 {
            mock.expect_request(tx, vec![0x22, 0xF1, 0x90], vec![0x7F, 0x22, 0x21]);
        }

        let result = send_uds_request(&app, &mock, tx, &[0x22, 0xF1, 0x90], false, None);
        assert!(result.is_err(), "Should fail after exhausting retries");
        assert!(
            result.unwrap_err().contains("0x21"),
            "Error should mention NRC 0x21"
        );
    }

    // ─── DID name and format tests ──────────────────────────────────

    #[test]
    fn test_did_name_known() {
        assert_eq!(did_name(0xF190), "VIN");
        assert_eq!(did_name(0xF188), "Master RPM Part");
        assert_eq!(did_name(0xD100), "Diag Session");
        assert_eq!(did_name(0x0202), "IMC Status");
        assert_eq!(did_name(0x402A), "Battery Voltage");
    }

    #[test]
    fn test_did_name_unknown() {
        assert_eq!(did_name(0x9999), "");
    }

    #[test]
    fn test_format_diag_session_values() {
        assert!(format_diag_session(&[0x01]).contains("Default"));
        assert!(format_diag_session(&[0x02]).contains("Programming"));
        assert!(format_diag_session(&[0x03]).contains("Extended"));
        assert!(format_diag_session(&[0xFF]).contains("Unknown"));
        assert!(format_diag_session(&[]).contains("Unknown"));
    }

    #[test]
    fn test_format_imc_status_values() {
        assert!(format_imc_status(&[0x00]).contains("Normal"));
        assert!(format_imc_status(&[0x01]).contains("Booting"));
        assert!(format_imc_status(&[0x05]).contains("Error"));
        assert!(format_imc_status(&[0xAA]).contains("0xAA"));
    }

    #[test]
    fn test_format_voltage() {
        // 12.4V = raw 124 = 0x00, 0x7C
        let v = format_voltage(&[0x00, 0x7C]);
        assert!(v.contains("12.4"), "Expected 12.4V, got: {}", v);
    }

    #[test]
    fn test_format_soc() {
        assert_eq!(format_soc(&[85]), "85%");
        assert_eq!(format_soc(&[]), "N/A");
    }

    #[test]
    fn test_format_temp() {
        // raw 25 → 25 - 40 = -15°C
        assert!(format_temp(&[25]).contains("-15"));
        assert_eq!(format_temp(&[]), "N/A");
    }

    // ─── Existing tests ─────────────────────────────────────────────

    #[test]
    fn test_list_routines_not_empty() {
        let routines = list_routines();
        assert!(!routines.is_empty());
        assert!(routines.iter().any(|r| r.routine_id == 0x6038));
        assert!(routines.iter().any(|r| r.routine_id == 0x603E));
        assert!(routines.iter().any(|r| r.routine_id == 0x0404));
        // New routines
        assert!(routines.iter().any(|r| r.routine_id == 0x6045));
        assert!(routines.iter().any(|r| r.routine_id == 0x6046));
        assert!(routines.iter().any(|r| r.routine_id == 0x0E00));
        assert!(routines.iter().any(|r| r.routine_id == 0x0E01));
        assert!(routines.iter().any(|r| r.routine_id == 0x0E02));
    }

    #[test]
    fn test_list_routines_have_categories() {
        let routines = list_routines();
        for r in &routines {
            assert!(
                !r.category.is_empty(),
                "Routine 0x{:04X} missing category",
                r.routine_id
            );
        }
        assert!(routines.iter().any(|r| r.category == "Diagnostics"));
        assert!(routines.iter().any(|r| r.category == "Configuration"));
        assert!(routines.iter().any(|r| r.category == "Recovery"));
        assert!(routines.iter().any(|r| r.category == "Advanced"));
    }

    #[test]
    fn test_list_routines_security_flags() {
        let routines = list_routines();
        // SSH Enable needs security
        let ssh = routines.iter().find(|r| r.routine_id == 0x603E).unwrap();
        assert!(ssh.needs_security);
        assert!(ssh.needs_pending);
        // VIN Learn does NOT need security
        let vin = routines.iter().find(|r| r.routine_id == 0x0404).unwrap();
        assert!(!vin.needs_security);
        assert!(!vin.needs_pending);
        // Retrieve CCF: no security, but needs pending
        let ccf = routines.iter().find(|r| r.routine_id == 0x0E00).unwrap();
        assert!(!ccf.needs_security);
        assert!(ccf.needs_pending);
        // Engineering Screen: security, no pending
        let eng = routines.iter().find(|r| r.routine_id == 0x603D).unwrap();
        assert!(eng.needs_security);
        assert!(!eng.needs_pending);
    }

    #[test]
    fn test_discover_devices_non_windows() {
        // On non-Windows, returns empty
        let devices = discover_devices();
        // May or may not be empty depending on platform
        let _ = devices;
    }
}
