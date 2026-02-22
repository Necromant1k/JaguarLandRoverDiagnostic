use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::ecu_emulator::{EcuEmulatorManager, EcuId};
use crate::j2534::dll;
use crate::j2534::types::*;
use crate::state::{AppState, Connection};
use crate::uds::client::{LogDirection, LogEntry};
use crate::uds::services::{did, ecu_addr, routine};

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
        if entry.description.is_empty() { String::new() } else { format!(" {}", entry.description) }
    );
    let _ = app.emit("uds-log", entry);
}

fn emit_log_simple<R: tauri::Runtime>(app: &tauri::AppHandle<R>, direction: LogDirection, data: &[u8], description: &str) {
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
        emit_log_simple(&app, LogDirection::Tx, &[], &format!("Loading J2534 DLL: {}", path));
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
            emit_log_simple(&app, LogDirection::Tx, &[], &format!("Trying: {} ({})", name, p_str));

            match dll::J2534Lib::load(&p_str) {
                Ok(lib) => {
                    let lib = Arc::new(lib);
                    match crate::j2534::device::J2534Device::open(lib.clone()) {
                        Ok(device) => {
                            log::info!("Auto-detect: successfully opened {}", name);
                            emit_log_simple(&app, LogDirection::Rx, &[], &format!("Connected to {}", name));
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
            let default_path = dll::default_mongoose_dll_path().to_string_lossy().to_string();
            log::info!("Auto-detect: trying default Mongoose path: {}", default_path);
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

    emit_log_simple(&app, LogDirection::Tx, &[], "Disconnected from J2534 device");
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
        // so broadcast is optional — software routing still works without it.
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
                    &format!("Bench mode ON — emulating: {} (CAN broadcast active)", ecu_names.join(", ")),
                );
                mgr
            }
            Err(e) => {
                log::warn!("CAN broadcast channel unavailable: {} — using software routing only", e);
                emit_log_simple(
                    &app,
                    LogDirection::Rx,
                    &[],
                    &format!("Bench mode ON — emulating: {} (software routing only, no CAN broadcast)", ecu_names.join(", ")),
                );
                crate::ecu_emulator::EcuEmulatorManager::new(ecu_ids)
            }
        };
        conn.emulator_manager = Some(manager);
    } else {
        // Cleanup already done above
        emit_log_simple(&app, LogDirection::Rx, &[], "Bench mode OFF — emulation stopped");
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
            emulated_ecus: mgr.emulated_ecus().iter().map(|e| e.name().to_lowercase()).collect(),
        }),
        None => Ok(BenchModeStatus {
            enabled: false,
            emulated_ecus: vec![],
        }),
    }
}

/// Read ECU info — returns a list of DID entries for the given ECU
#[tauri::command]
pub fn read_ecu_info(app: AppHandle, state: State<'_, AppState>, ecu: String) -> Result<Vec<EcuInfoEntry>, String> {
    read_ecu_info_inner(&app, &state, &ecu).map_err(|e| log_err("read_ecu_info", e))
}

fn read_ecu_info_inner(app: &AppHandle, state: &State<'_, AppState>, ecu: &str) -> Result<Vec<EcuInfoEntry>, String> {
    let conn = state.connection.lock().map_err(|e| e.to_string())?;
    let conn = conn.as_ref().ok_or("Not connected")?;
    let channel: &dyn crate::j2534::Channel = conn.channel.as_ref().ok_or("No channel available")?;

    let emulator = conn.emulator_manager.as_ref();
    let entries = match ecu {
        "imc" => read_imc_info(app, channel, emulator),
        "bcm" => read_bcm_info(app, channel, emulator),
        _ => return Err(format!("Unknown ECU: {}", ecu)),
    };

    Ok(entries)
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
            emit_log_simple(app, LogDirection::Rx, &[], &format!("{} = {}", label, value));
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

fn read_imc_info<R: tauri::Runtime>(app: &tauri::AppHandle<R>, channel: &dyn crate::j2534::Channel, emulator: Option<&EcuEmulatorManager>) -> Vec<EcuInfoEntry> {
    let tx = ecu_addr::IMC_TX;
    let mut entries = Vec::new();
    let bench_mode = emulator.is_some();

    // Step 1: In bench mode, poll TesterPresent until IMC is ready
    if bench_mode {
        emit_log_simple(app, LogDirection::Tx, &[], "Bench mode: waiting for IMC to boot...");
        let mut imc_ready = false;
        for attempt in 1..=15 {
            emit_log_simple(app, LogDirection::Tx, &[0x3E, 0x00], &format!("TesterPresent poll {}/15", attempt));
            match send_uds_request(app, channel, tx, &[0x3E, 0x00], false, emulator) {
                Ok(_) => {
                    emit_log_simple(app, LogDirection::Rx, &[], &format!("IMC responded on attempt {}", attempt));
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
            emit_log_simple(app, LogDirection::Rx, &[], "IMC not responding after 15 attempts");
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

    // Step 2: Try Extended Session with retries in bench mode
    let mut extended_ok = false;
    if bench_mode {
        for attempt in 1..=5 {
            // Send TesterPresent before each attempt to keep session alive
            if attempt > 1 {
                let _ = send_uds_request(app, channel, tx, &[0x3E, 0x00], false, emulator);
            }
            emit_log_simple(app, LogDirection::Tx, &[0x10, 0x03], &format!("ExtendedSession attempt {}/5", attempt));
            match send_uds_request(app, channel, tx, &[0x10, 0x03], false, emulator) {
                Ok(_) => {
                    emit_log_simple(app, LogDirection::Rx, &[], "Extended Session OK");
                    extended_ok = true;
                    break;
                }
                Err(e) => {
                    emit_log_simple(app, LogDirection::Rx, &[], &format!("Extended Session failed: {} (attempt {}/5)", e, attempt));
                    if attempt < 5 {
                        std::thread::sleep(std::time::Duration::from_secs(2));
                    }
                }
            }
        }
    } else {
        emit_log_simple(app, LogDirection::Tx, &[0x10, 0x03], "ExtendedSession (IMC)");
        extended_ok = send_uds_request(app, channel, tx, &[0x10, 0x03], false, emulator).is_ok();
    }

    if !extended_ok {
        emit_log_simple(app, LogDirection::Rx, &[], "Extended Session failed — trying default-session DIDs");
    }

    // Step 3: Try Security Access (needed for VIN, software parts)
    if extended_ok {
        emit_log_simple(app, LogDirection::Tx, &[0x27, 0x11], "SecurityAccess RequestSeed (IMC)");
        match send_uds_request(app, channel, tx, &[0x27, 0x11], false, emulator) {
            Ok(seed_resp) if seed_resp.len() >= 5 => {
                let seed_int = ((seed_resp[2] as u32) << 16) | ((seed_resp[3] as u32) << 8) | (seed_resp[4] as u32);
                if seed_int != 0 {
                    let key_int = crate::uds::keygen::keygen_mki(seed_int, &crate::uds::keygen::DC0314_CONSTANTS);
                    let key_bytes = [((key_int >> 16) & 0xFF) as u8, ((key_int >> 8) & 0xFF) as u8, (key_int & 0xFF) as u8];
                    let mut key_req = vec![0x27, 0x12];
                    key_req.extend_from_slice(&key_bytes);
                    emit_log_simple(app, LogDirection::Tx, &key_req, "SecurityAccess SendKey (IMC)");
                    match send_uds_request(app, channel, tx, &key_req, false, emulator) {
                        Ok(_) => emit_log_simple(app, LogDirection::Rx, &[], "Security Access OK"),
                        Err(e) => emit_log_simple(app, LogDirection::Rx, &[], &format!("Security Access failed: {} — continuing without", e)),
                    }
                } else {
                    emit_log_simple(app, LogDirection::Rx, &[], "Already unlocked (zero seed)");
                }
            }
            Ok(_) => {}
            Err(e) => emit_log_simple(app, LogDirection::Rx, &[], &format!("Security seed failed: {} — continuing without", e)),
        }
    }

    // Step 4: Read DIDs
    entries.push(read_did_entry(app, channel, tx, did::ACTIVE_DIAG_SESSION, "Diag Session", format_diag_session, "status", emulator));

    if extended_ok {
        let _ = send_uds_request(app, channel, tx, &[0x3E, 0x00], false, emulator);
        entries.push(read_did_entry(app, channel, tx, did::IMC_STATUS, "IMC Status", format_imc_status, "status", emulator));

        let _ = send_uds_request(app, channel, tx, &[0x3E, 0x00], false, emulator);
        entries.push(read_did_entry(app, channel, tx, did::VIN, "VIN", format_string, "vehicle", emulator));

        let _ = send_uds_request(app, channel, tx, &[0x3E, 0x00], false, emulator);
        entries.push(read_did_entry(app, channel, tx, did::MASTER_RPM_PART, "Software Part", format_string, "software", emulator));

        let _ = send_uds_request(app, channel, tx, &[0x3E, 0x00], false, emulator);
        entries.push(read_did_entry(app, channel, tx, did::PBL_PART, "Bootloader", format_string, "software", emulator));

        let _ = send_uds_request(app, channel, tx, &[0x3E, 0x00], false, emulator);
        entries.push(read_did_entry(app, channel, tx, did::V850_PART, "V850 Part", format_string, "software", emulator));

        let _ = send_uds_request(app, channel, tx, &[0x3E, 0x00], false, emulator);
        entries.push(read_did_entry(app, channel, tx, did::TUNER_PART, "Tuner Part", format_string, "software", emulator));

        let _ = send_uds_request(app, channel, tx, &[0x3E, 0x00], false, emulator);
        entries.push(read_did_entry(app, channel, tx, did::POLAR_PART, "Polar Part", format_string, "software", emulator));

        let _ = send_uds_request(app, channel, tx, &[0x3E, 0x00], false, emulator);
        entries.push(read_did_entry(app, channel, tx, did::ECU_SERIAL, "ECU Serial", format_string, "hardware", emulator));

        let _ = send_uds_request(app, channel, tx, &[0x3E, 0x00], false, emulator);
        entries.push(read_did_entry(app, channel, tx, did::ECU_SERIAL2, "ECU Serial 2", format_string, "hardware", emulator));
    } else {
        // Default session fallback: try VIN and ECU Serial anyway
        emit_log_simple(app, LogDirection::Tx, &[], "Trying default-session DIDs (VIN, ECU Serial)...");

        // Try VIN in default session
        entries.push(read_did_entry(app, channel, tx, did::VIN, "VIN", format_string, "vehicle", emulator));
        // Try ECU Serial in default session
        entries.push(read_did_entry(app, channel, tx, did::ECU_SERIAL, "ECU Serial", format_string, "hardware", emulator));

        // Rest require Extended Session — show clear error
        let err_msg = if bench_mode {
            "Extended Session failed — IMC needs other ECUs on CAN bus"
        } else {
            "Requires Extended Session (enable bench mode)"
        };
        for (did_id, label, category) in [
            (did::IMC_STATUS, "IMC Status", "status"),
            (did::MASTER_RPM_PART, "Software Part", "software"),
            (did::PBL_PART, "Bootloader", "software"),
            (did::V850_PART, "V850 Part", "software"),
            (did::TUNER_PART, "Tuner Part", "software"),
            (did::POLAR_PART, "Polar Part", "software"),
            (did::ECU_SERIAL2, "ECU Serial 2", "hardware"),
        ] {
            entries.push(EcuInfoEntry {
                label: label.to_string(),
                did_hex: format!("{:04X}", did_id),
                value: None,
                error: Some(err_msg.to_string()),
                category: category.to_string(),
            });
        }
    }

    entries
}

fn read_bcm_info<R: tauri::Runtime>(app: &tauri::AppHandle<R>, channel: &dyn crate::j2534::Channel, emulator: Option<&EcuEmulatorManager>) -> Vec<EcuInfoEntry> {
    let tx = ecu_addr::BCM_TX;
    vec![
        read_did_entry(app, channel, tx, did::VIN, "VIN", format_string, "vehicle", emulator),
        read_did_entry(app, channel, tx, did::BATTERY_VOLTAGE, "Battery Voltage", format_voltage, "battery", emulator),
        read_did_entry(app, channel, tx, did::BATTERY_SOC, "Battery SOC", format_soc, "battery", emulator),
        read_did_entry(app, channel, tx, did::BATTERY_TEMP, "Battery Temp", format_temp, "battery", emulator),
    ]
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
    send_uds_request(app, channel, ecu_addr::IMC_TX, &[0x3E, 0x00], false, emulator)
        .map_err(|e| format!("TesterPresent failed: {}", e))?;

    // Step 2: Extended session
    emit_log_simple(app, LogDirection::Tx, &[0x10, 0x03], "DiagnosticSessionControl Extended");
    send_uds_request(app, channel, ecu_addr::IMC_TX, &[0x10, 0x03], false, emulator)
        .map_err(|e| format!("Extended session failed: {}", e))?;

    // Step 3: Security Access (if required)
    if needs_security {
        emit_log_simple(app, LogDirection::Tx, &[0x27, 0x11], "SecurityAccess RequestSeed");
        let seed_resp = send_uds_request(app, channel, ecu_addr::IMC_TX, &[0x27, 0x11], false, emulator)
            .map_err(|e| format!("Security seed request failed: {}", e))?;

        if seed_resp.len() >= 5 {
            let seed = &seed_resp[2..5];
            let seed_int = ((seed[0] as u32) << 16) | ((seed[1] as u32) << 8) | (seed[2] as u32);

            if seed_int != 0 {
                let key_int = crate::uds::keygen::keygen_mki(seed_int, &crate::uds::keygen::DC0314_CONSTANTS);
                let key_bytes = [
                    ((key_int >> 16) & 0xFF) as u8,
                    ((key_int >> 8) & 0xFF) as u8,
                    (key_int & 0xFF) as u8,
                ];

                let mut key_request = vec![0x27, 0x12];
                key_request.extend_from_slice(&key_bytes);
                emit_log_simple(app, LogDirection::Tx, &key_request, "SecurityAccess SendKey");
                send_uds_request(app, channel, ecu_addr::IMC_TX, &key_request, false, emulator)
                    .map_err(|e| format!("Security key send failed: {}", e))?;
            }
        }
    }

    Ok(())
}

/// Look up routine metadata from the known routines list
fn find_routine_meta(routine_id: u16) -> Option<RoutineInfo> {
    list_routines().into_iter().find(|r| r.routine_id == routine_id)
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
    let channel: &dyn crate::j2534::Channel = conn.channel.as_ref().ok_or("No channel available")?;

    // Look up routine metadata for SDD flow requirements
    let meta = find_routine_meta(routine_id);
    let needs_security = meta.as_ref().map_or(false, |m| m.needs_security);
    let needs_pending = meta.as_ref().map_or(false, |m| m.needs_pending);

    let emulator = conn.emulator_manager.as_ref();

    // Run SDD prerequisite flow (TesterPresent + Extended Session + optional Security)
    sdd_prerequisite_flow(app, channel, needs_security, emulator)?;

    // Send RoutineControl Start
    let mut request = vec![0x31, 0x01, (routine_id >> 8) as u8, (routine_id & 0xFF) as u8];
    request.extend_from_slice(data);

    let resp = send_uds_request(app, channel, ecu_addr::IMC_TX, &request, needs_pending, emulator)
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

/// Read CCF (Central Configuration File) from IMC via Report CCF routine (0x0E01)
#[tauri::command]
pub fn read_ccf(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<EcuInfoEntry>, String> {
    read_ccf_inner(&app, &state).map_err(|e| log_err("read_ccf", e))
}

fn read_ccf_inner(
    app: &AppHandle,
    state: &State<'_, AppState>,
) -> Result<Vec<EcuInfoEntry>, String> {
    let conn = state.connection.lock().map_err(|e| e.to_string())?;
    let conn = conn.as_ref().ok_or("Not connected")?;
    let channel: &dyn crate::j2534::Channel = conn.channel.as_ref().ok_or("No channel available")?;
    let emulator = conn.emulator_manager.as_ref();

    // SDD prerequisite flow (no security needed for CCF)
    sdd_prerequisite_flow(app, channel, false, emulator)?;

    // Run Report CCF routine (0x0E01) with wait_pending=true
    let request = vec![0x31, 0x01, 0x0E, 0x01];
    emit_log_simple(app, LogDirection::Tx, &request, "RoutineControl Report CCF (0x0E01)");
    let resp = send_uds_request(app, channel, ecu_addr::IMC_TX, &request, true, emulator)
        .map_err(|e| format!("Report CCF failed: {}", e))?;

    // Response: 71 01 0E 01 [CCF data...]
    let ccf_data = if resp.len() > 4 { &resp[4..] } else { &[] as &[u8] };

    emit_log_simple(app, LogDirection::Rx, &[], &format!(
        "CCF data: {} bytes",
        ccf_data.len()
    ));

    Ok(parse_ccf_entries(ccf_data))
}

/// Parse CCF response data into structured entries
fn parse_ccf_entries(data: &[u8]) -> Vec<EcuInfoEntry> {
    let mut entries = Vec::new();

    if data.is_empty() {
        entries.push(EcuInfoEntry {
            label: "CCF Status".to_string(),
            did_hex: "0E01".to_string(),
            value: Some("No configuration data".to_string()),
            error: None,
            category: "config".to_string(),
        });
        return entries;
    }

    // Raw CCF hex dump — always include for debugging
    let hex_str = data.iter().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join(" ");
    entries.push(EcuInfoEntry {
        label: "CCF Raw Data".to_string(),
        did_hex: "0E01".to_string(),
        value: Some(hex_str),
        error: None,
        category: "config".to_string(),
    });

    entries.push(EcuInfoEntry {
        label: "CCF Length".to_string(),
        did_hex: "0E01".to_string(),
        value: Some(format!("{} bytes", data.len())),
        error: None,
        category: "config".to_string(),
    });

    // Parse known CCF fields if enough data
    if !data.is_empty() {
        entries.push(EcuInfoEntry {
            label: "CCF Status Byte".to_string(),
            did_hex: "0E01".to_string(),
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
            did_hex: "0E01".to_string(),
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
            did_hex: "0E01".to_string(),
            value: Some(format!("{} (0x{:02X})", lang, data[2])),
            error: None,
            category: "config".to_string(),
        });
    }

    // Feature flags byte (if present)
    if data.len() >= 4 {
        let features = data[3];
        let mut feature_list = Vec::new();
        if features & 0x01 != 0 { feature_list.push("Navigation"); }
        if features & 0x02 != 0 { feature_list.push("DAB Radio"); }
        if features & 0x04 != 0 { feature_list.push("DVD"); }
        if features & 0x08 != 0 { feature_list.push("Bluetooth"); }
        if features & 0x10 != 0 { feature_list.push("WiFi"); }
        if features & 0x20 != 0 { feature_list.push("USB Media"); }
        if features & 0x40 != 0 { feature_list.push("Rear Camera"); }
        if features & 0x80 != 0 { feature_list.push("Surround Camera"); }
        let value = if feature_list.is_empty() {
            format!("None (0x{:02X})", features)
        } else {
            format!("{} (0x{:02X})", feature_list.join(", "), features)
        };
        entries.push(EcuInfoEntry {
            label: "Features".to_string(),
            did_hex: "0E01".to_string(),
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
    let channel: &dyn crate::j2534::Channel = conn.channel.as_ref().ok_or("No channel available")?;

    let data = send_read_did(&app, channel, ecu_tx, did_id, conn.emulator_manager.as_ref())
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
            emit_log_simple(app, LogDirection::Tx, &[], &format!("Busy retry {}/{}", busy_attempt, max_busy_retries));
        }

        match send_uds_request_once(app, channel, tx_id, request, wait_pending) {
            Ok(resp) => return Ok(resp),
            Err(e) if e.contains("0x21") && busy_attempt < max_busy_retries => {
                // EXML: MAX_RETRY_PERIOD=6000ms / MAX_BUSY_ATTEMPTS=6 = 1s per retry
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
        std::time::Duration::from_secs(30)
    } else {
        std::time::Duration::from_secs(3)
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
    info.push_str(&format!("OS: {} {}\n", std::env::consts::OS, std::env::consts::ARCH));
    info.push_str(&format!("Time: {}\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S")));
    info.push_str("---\n");
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::j2534::Channel;
    use crate::j2534::mock::MockChannel;
    use crate::uds::services::ecu_addr;

    /// Helper: create a Tauri app for testing (no window, just the event system)
    fn test_app() -> tauri::AppHandle<tauri::test::MockRuntime> {
        let app = tauri::test::mock_app();
        app.handle().clone()
    }

    /// Helper: create a MockChannel with IMC + BCM filters set up
    fn setup_mock_channel() -> MockChannel {
        let mock = MockChannel::new();
        mock.setup_iso15765_filter(ecu_addr::IMC_TX, ecu_addr::IMC_RX).unwrap();
        mock.setup_iso15765_filter(ecu_addr::BCM_TX, ecu_addr::BCM_RX).unwrap();
        mock.setup_iso15765_filter(ecu_addr::GWM_TX, ecu_addr::GWM_RX).unwrap();
        mock.setup_iso15765_filter(ecu_addr::IPC_TX, ecu_addr::IPC_RX).unwrap();
        mock
    }

    // ─── BCM bench mode tests ───────────────────────────────────────

    #[test]
    fn test_bcm_read_with_emulator() {
        // Bench mode ON: BCM is emulated, so all reads should return emulated values
        // No mock channel expectations needed — emulator handles everything
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
        assert!(entries[0].value.as_ref().unwrap().starts_with("SAJBA4BN"));
        // Battery Voltage
        assert_eq!(entries[1].label, "Battery Voltage");
        assert_eq!(entries[1].category, "battery");
        assert!(entries[1].value.is_some());
        assert!(entries[1].value.as_ref().unwrap().contains("V"));
        // Battery SOC
        assert_eq!(entries[2].label, "Battery SOC");
        assert_eq!(entries[2].category, "battery");
        assert!(entries[2].value.is_some());
        assert!(entries[2].value.as_ref().unwrap().contains("%"));
        // Battery Temp
        assert_eq!(entries[3].label, "Battery Temp");
        assert_eq!(entries[3].category, "battery");
        assert!(entries[3].value.is_some());
        assert!(entries[3].value.as_ref().unwrap().contains("°C"));
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
            assert!(entry.value.is_none(), "{} should not have value", entry.label);
        }
    }

    // ─── IMC bench mode tests ───────────────────────────────────────

    #[test]
    fn test_imc_read_extended_session_ok() {
        // IMC with Extended Session succeeding — should read all DIDs from EXML
        let app = test_app();
        let mock = setup_mock_channel();
        let tx = ecu_addr::IMC_TX;

        // TesterPresent → OK
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        // Extended Session → OK
        mock.expect_request(tx, vec![0x10, 0x03], vec![0x50, 0x03, 0x00, 0x32, 0x01, 0xF4]);
        // Security Access seed → seed response
        mock.expect_request(tx, vec![0x27, 0x11], vec![0x67, 0x11, 0x11, 0x22, 0x33]);
        let seed_int = 0x112233u32;
        let key_int = crate::uds::keygen::keygen_mki(seed_int, &crate::uds::keygen::DC0314_CONSTANTS);
        let key_bytes = [((key_int >> 16) & 0xFF) as u8, ((key_int >> 8) & 0xFF) as u8, (key_int & 0xFF) as u8];
        let mut key_req = vec![0x27, 0x12];
        key_req.extend_from_slice(&key_bytes);
        mock.expect_request(tx, key_req, vec![0x67, 0x12]);
        // D100 Diag Session
        mock.expect_request(tx, vec![0x22, 0xD1, 0x00], vec![0x62, 0xD1, 0x00, 0x03]);
        // TesterPresent + 0202 IMC Status
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(tx, vec![0x22, 0x02, 0x02], vec![0x62, 0x02, 0x02, 0x00]);
        // TesterPresent + F190 VIN
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(tx, vec![0x22, 0xF1, 0x90], vec![0x62, 0xF1, 0x90, 0x53, 0x41, 0x4A]);
        // TesterPresent + F188 Software Part
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(tx, vec![0x22, 0xF1, 0x88], vec![0x62, 0xF1, 0x88, 0x53, 0x57]);
        // TesterPresent + F180 Bootloader
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(tx, vec![0x22, 0xF1, 0x80], vec![0x62, 0xF1, 0x80, 0x42, 0x4C]);
        // TesterPresent + F120 V850
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(tx, vec![0x22, 0xF1, 0x20], vec![0x62, 0xF1, 0x20, 0x56, 0x38]);
        // TesterPresent + F121 Tuner
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(tx, vec![0x22, 0xF1, 0x21], vec![0x62, 0xF1, 0x21, 0x54, 0x55]);
        // TesterPresent + F1A5 Polar
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(tx, vec![0x22, 0xF1, 0xA5], vec![0x62, 0xF1, 0xA5, 0x50, 0x4C]);
        // TesterPresent + F18C ECU Serial
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(tx, vec![0x22, 0xF1, 0x8C], vec![0x62, 0xF1, 0x8C, 0x53, 0x4E]);
        // TesterPresent + F113 ECU Serial 2
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(tx, vec![0x22, 0xF1, 0x13], vec![0x62, 0xF1, 0x13, 0x48, 0x57]);

        let entries = read_imc_info(&app, &mock, None);

        assert_eq!(entries.len(), 10); // D100 + 9 DIDs from EXML
        assert_eq!(entries[0].label, "Diag Session");
        assert_eq!(entries[0].category, "status");
        assert_eq!(entries[1].label, "IMC Status");
        assert_eq!(entries[1].category, "status");
        assert_eq!(entries[2].label, "VIN");
        assert_eq!(entries[2].category, "vehicle");
        assert_eq!(entries[3].label, "Software Part");
        assert_eq!(entries[3].category, "software");
        assert_eq!(entries[4].label, "Bootloader");
        assert_eq!(entries[4].category, "software");
        assert_eq!(entries[5].label, "V850 Part");
        assert_eq!(entries[5].category, "software");
        assert_eq!(entries[6].label, "Tuner Part");
        assert_eq!(entries[6].category, "software");
        assert_eq!(entries[7].label, "Polar Part");
        assert_eq!(entries[7].category, "software");
        assert_eq!(entries[8].label, "ECU Serial");
        assert_eq!(entries[8].category, "hardware");
        assert_eq!(entries[9].label, "ECU Serial 2");
        assert_eq!(entries[9].category, "hardware");
        for entry in &entries {
            assert!(entry.value.is_some(), "{} should have value", entry.label);
            assert!(entry.error.is_none(), "{} should not have error", entry.label);
        }
    }

    #[test]
    fn test_imc_read_extended_session_fails_no_bench() {
        // Extended Session fails, bench mode OFF → fallback reads VIN + ECU Serial,
        // rest show "enable bench mode" message
        let app = test_app();
        let mock = setup_mock_channel();
        let tx = ecu_addr::IMC_TX;

        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        // Extended Session → NRC 0x12 (SubFunctionNotSupported)
        mock.expect_request(tx, vec![0x10, 0x03], vec![0x7F, 0x10, 0x12]);
        // D100 → Default
        mock.expect_request(tx, vec![0x22, 0xD1, 0x00], vec![0x62, 0xD1, 0x00, 0x01]);
        // Default session fallback: VIN + ECU Serial
        mock.expect_request(tx, vec![0x22, 0xF1, 0x90], vec![0x62, 0xF1, 0x90, 0x56, 0x49, 0x4E]);
        mock.expect_request(tx, vec![0x22, 0xF1, 0x8C], vec![0x62, 0xF1, 0x8C, 0x53, 0x4E]);

        let entries = read_imc_info(&app, &mock, None); // No emulator = no bench mode

        assert_eq!(entries.len(), 10); // D100 + VIN + ECU Serial + 7 error entries
        assert_eq!(entries[0].label, "Diag Session");
        assert!(entries[0].value.as_ref().unwrap().contains("Default"));
        // VIN and ECU Serial should have values from fallback
        assert_eq!(entries[1].label, "VIN");
        assert!(entries[1].value.is_some(), "VIN should read in default session");
        assert_eq!(entries[2].label, "ECU Serial");
        assert!(entries[2].value.is_some(), "ECU Serial should read in default session");
        // Remaining DIDs should have "enable bench mode" error
        for entry in &entries[3..] {
            assert!(entry.error.is_some(), "{} should have error", entry.label);
            assert!(entry.error.as_ref().unwrap().contains("enable bench mode"),
                "{}: wrong error message: {:?}", entry.label, entry.error);
        }
    }

    #[test]
    fn test_imc_read_extended_session_fails_with_bench() {
        // Extended Session fails after retries, bench mode IS ON → fallback reads VIN + ECU Serial,
        // rest should NOT say "enable bench mode"
        let app = test_app();
        let mock = setup_mock_channel();
        let tx = ecu_addr::IMC_TX;
        let emu = EcuEmulatorManager::new(vec![EcuId::Bcm]);

        // Bench mode: TesterPresent poll (succeeds on first try)
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        // Extended Session retries: 5 attempts, all fail
        // Attempt 1: Extended Session → NRC 0x12
        mock.expect_request(tx, vec![0x10, 0x03], vec![0x7F, 0x10, 0x12]);
        // Attempt 2: TesterPresent + Extended Session
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(tx, vec![0x10, 0x03], vec![0x7F, 0x10, 0x12]);
        // Attempt 3: TesterPresent + Extended Session
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(tx, vec![0x10, 0x03], vec![0x7F, 0x10, 0x12]);
        // Attempt 4: TesterPresent + Extended Session
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(tx, vec![0x10, 0x03], vec![0x7F, 0x10, 0x12]);
        // Attempt 5: TesterPresent + Extended Session
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(tx, vec![0x10, 0x03], vec![0x7F, 0x10, 0x12]);
        // D100 → Default
        mock.expect_request(tx, vec![0x22, 0xD1, 0x00], vec![0x62, 0xD1, 0x00, 0x01]);
        // Default session fallback: VIN + ECU Serial
        mock.expect_request(tx, vec![0x22, 0xF1, 0x90], vec![0x62, 0xF1, 0x90, 0x56, 0x49, 0x4E]);
        mock.expect_request(tx, vec![0x22, 0xF1, 0x8C], vec![0x62, 0xF1, 0x8C, 0x53, 0x4E]);

        let entries = read_imc_info(&app, &mock, Some(&emu));

        assert_eq!(entries.len(), 10); // D100 + VIN + ECU Serial + 7 error entries
        // VIN and ECU Serial should have values from fallback
        assert_eq!(entries[1].label, "VIN");
        assert!(entries[1].value.is_some(), "VIN should read in default session");
        assert_eq!(entries[2].label, "ECU Serial");
        assert!(entries[2].value.is_some(), "ECU Serial should read in default session");
        // Remaining DIDs should have the correct message (NOT "enable bench mode")
        for entry in &entries[3..] {
            assert!(entry.error.is_some(), "{} should have error", entry.label);
            let err = entry.error.as_ref().unwrap();
            assert!(!err.contains("enable bench mode"),
                "{}: should NOT say 'enable bench mode' when bench IS on. Got: {}", entry.label, err);
            assert!(err.contains("CAN bus"),
                "{}: should mention CAN bus. Got: {}", entry.label, err);
        }
    }

    #[test]
    fn test_imc_security_access_fails_gracefully() {
        // Security Access returns NRC 0x7F (serviceNotSupportedInActiveSession) — DIDs still read
        let app = test_app();
        let mock = setup_mock_channel();
        let tx = ecu_addr::IMC_TX;

        // TesterPresent → OK
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        // Extended Session → OK
        mock.expect_request(tx, vec![0x10, 0x03], vec![0x50, 0x03, 0x00, 0x32, 0x01, 0xF4]);
        // Security seed → NRC 0x7F (serviceNotSupportedInActiveSession)
        mock.expect_request(tx, vec![0x27, 0x11], vec![0x7F, 0x27, 0x7F]);
        // D100 Diag Session
        mock.expect_request(tx, vec![0x22, 0xD1, 0x00], vec![0x62, 0xD1, 0x00, 0x03]);
        // TesterPresent + IMC Status
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(tx, vec![0x22, 0x02, 0x02], vec![0x62, 0x02, 0x02, 0x00]);
        // TesterPresent + VIN
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(tx, vec![0x22, 0xF1, 0x90], vec![0x62, 0xF1, 0x90, 0x56, 0x49, 0x4E]);
        // TesterPresent + Software Part
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(tx, vec![0x22, 0xF1, 0x88], vec![0x62, 0xF1, 0x88, 0x53, 0x57]);
        // TesterPresent + Bootloader
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(tx, vec![0x22, 0xF1, 0x80], vec![0x62, 0xF1, 0x80, 0x42, 0x4C]);
        // TesterPresent + V850
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(tx, vec![0x22, 0xF1, 0x20], vec![0x62, 0xF1, 0x20, 0x56, 0x38]);
        // TesterPresent + Tuner
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(tx, vec![0x22, 0xF1, 0x21], vec![0x62, 0xF1, 0x21, 0x54, 0x55]);
        // TesterPresent + Polar
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(tx, vec![0x22, 0xF1, 0xA5], vec![0x62, 0xF1, 0xA5, 0x50, 0x4C]);
        // TesterPresent + ECU Serial
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(tx, vec![0x22, 0xF1, 0x8C], vec![0x62, 0xF1, 0x8C, 0x53, 0x4E]);
        // TesterPresent + ECU Serial 2
        mock.expect_request(tx, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(tx, vec![0x22, 0xF1, 0x13], vec![0x62, 0xF1, 0x13, 0x48, 0x57]);

        let entries = read_imc_info(&app, &mock, None);

        assert_eq!(entries.len(), 10);
        // All should still have values despite security failure
        for entry in &entries {
            assert!(entry.value.is_some(), "{} should have value even without security", entry.label);
            assert!(entry.error.is_none(), "{} should not have error", entry.label);
        }
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
        mock.expect_request(tx, vec![0x22, 0xF1, 0x90], vec![0x62, 0xF1, 0x90, 0x56, 0x49, 0x4E]);

        let result = send_uds_request(&app, &mock, tx, &[0x22, 0xF1, 0x90], false, None);
        assert!(result.is_ok(), "Should succeed after 3 retries: {:?}", result);
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
        assert!(result.unwrap_err().contains("0x21"), "Error should mention NRC 0x21");
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
            assert!(!r.category.is_empty(), "Routine 0x{:04X} missing category", r.routine_id);
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
