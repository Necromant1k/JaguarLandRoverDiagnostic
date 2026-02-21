use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::j2534::types::*;
use crate::uds::client::{LogDirection, LogEntry};
use crate::uds::services::ecu_addr;

// ─── ECU Identification ──────────────────────────────────────────────

/// Known ECU identifiers with their CAN addresses
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EcuId {
    Bcm,
    Gwm,
    Ipc,
}

impl EcuId {
    /// CAN ID used to send requests TO this ECU (from tester perspective)
    pub fn tx_id(self) -> u32 {
        match self {
            EcuId::Bcm => ecu_addr::BCM_TX,
            EcuId::Gwm => ecu_addr::GWM_TX,
            EcuId::Ipc => ecu_addr::IPC_TX,
        }
    }

    /// CAN ID used for responses FROM this ECU
    pub fn rx_id(self) -> u32 {
        match self {
            EcuId::Bcm => ecu_addr::BCM_RX,
            EcuId::Gwm => ecu_addr::GWM_RX,
            EcuId::Ipc => ecu_addr::IPC_RX,
        }
    }

    /// Human-readable name
    pub fn name(self) -> &'static str {
        match self {
            EcuId::Bcm => "BCM",
            EcuId::Gwm => "GWM",
            EcuId::Ipc => "IPC",
        }
    }

    /// Parse from string (case-insensitive)
    pub fn from_str(s: &str) -> Option<EcuId> {
        match s.to_lowercase().as_str() {
            "bcm" => Some(EcuId::Bcm),
            "gwm" => Some(EcuId::Gwm),
            "ipc" => Some(EcuId::Ipc),
            _ => None,
        }
    }

    /// All known ECU IDs
    pub fn all() -> &'static [EcuId] {
        &[EcuId::Bcm, EcuId::Gwm, EcuId::Ipc]
    }
}

// ─── ECU Handler Trait ───────────────────────────────────────────────

/// Trait for ECU-specific response logic
pub trait EcuHandler: Send {
    /// Build a UDS response for a given request payload.
    /// Returns None if the request should be ignored (not for this ECU).
    fn build_response(&self, request: &[u8]) -> Option<Vec<u8>>;

    /// ECU name for logging
    fn name(&self) -> &str;
}

// ─── BCM Handler ─────────────────────────────────────────────────────

pub struct BcmHandler;

impl EcuHandler for BcmHandler {
    fn name(&self) -> &str {
        "BCM"
    }

    fn build_response(&self, request: &[u8]) -> Option<Vec<u8>> {
        match request {
            // TesterPresent (3E 00)
            [0x3E, 0x00, ..] => Some(vec![0x7E, 0x00]),

            // DiagnosticSessionControl (10 XX)
            [0x10, session, ..] => Some(vec![0x50, *session, 0x00, 0x19, 0x01, 0xF4]),

            // ECUReset (11 XX)
            [0x11, reset_type, ..] => Some(vec![0x51, *reset_type]),

            // ReadDataByIdentifier (22 XX XX)
            [0x22, did_hi, did_lo, ..] => {
                let did = ((*did_hi as u16) << 8) | (*did_lo as u16);
                match did {
                    0x402A => Some(vec![0x62, 0x40, 0x2A, 0x00, 0x7C]), // Voltage 12.4V
                    0x4028 => Some(vec![0x62, 0x40, 0x28, 0x55]),       // SoC 85%
                    0x4029 => Some(vec![0x62, 0x40, 0x29, 0x19]),       // Temp 25°C
                    0x4030 => Some(vec![0x62, 0x40, 0x30, 0x00]),       // Door status: closed
                    0x4032 => Some(vec![0x62, 0x40, 0x32, 0x4B]),       // Fuel level 75%
                    0xF190 => {
                        let mut resp = vec![0x62, 0xF1, 0x90];
                        resp.extend_from_slice(b"SAJBA4BN0HA000000");
                        Some(resp)
                    }
                    _ => Some(vec![0x7F, 0x22, 0x31]), // requestOutOfRange
                }
            }

            // SecurityAccess (27 XX) → zero seed (already unlocked)
            [0x27, level, ..] => Some(vec![0x67, *level, 0x00, 0x00, 0x00]),

            // CommunicationControl (28 XX XX)
            [0x28, sub_function, ..] => Some(vec![0x68, *sub_function]),

            // WriteDataByIdentifier (2E XX XX ...)
            [0x2E, did_hi, did_lo, ..] => Some(vec![0x6E, *did_hi, *did_lo]),

            // RoutineControl (31 XX XX XX)
            [0x31, sub_fn, rid_hi, rid_lo, ..] => {
                Some(vec![0x71, *sub_fn, *rid_hi, *rid_lo])
            }

            // Unknown service → NRC serviceNotSupported
            [sid, ..] => Some(vec![0x7F, *sid, 0x11]),

            _ => None,
        }
    }
}

// ─── GWM Handler ─────────────────────────────────────────────────────

pub struct GwmHandler;

impl EcuHandler for GwmHandler {
    fn name(&self) -> &str {
        "GWM"
    }

    fn build_response(&self, request: &[u8]) -> Option<Vec<u8>> {
        match request {
            // TesterPresent
            [0x3E, 0x00, ..] => Some(vec![0x7E, 0x00]),

            // DiagnosticSessionControl
            [0x10, session, ..] => Some(vec![0x50, *session, 0x00, 0x19, 0x01, 0xF4]),

            // SecurityAccess → zero seed
            [0x27, level, ..] => Some(vec![0x67, *level, 0x00, 0x00, 0x00]),

            // Unknown → NRC serviceNotSupported
            [sid, ..] => Some(vec![0x7F, *sid, 0x11]),

            _ => None,
        }
    }
}

// ─── IPC Handler ─────────────────────────────────────────────────────

pub struct IpcHandler;

impl EcuHandler for IpcHandler {
    fn name(&self) -> &str {
        "IPC"
    }

    fn build_response(&self, request: &[u8]) -> Option<Vec<u8>> {
        match request {
            // TesterPresent
            [0x3E, 0x00, ..] => Some(vec![0x7E, 0x00]),

            // DiagnosticSessionControl
            [0x10, session, ..] => Some(vec![0x50, *session, 0x00, 0x19, 0x01, 0xF4]),

            // SecurityAccess → zero seed
            [0x27, level, ..] => Some(vec![0x67, *level, 0x00, 0x00, 0x00]),

            // Unknown → NRC serviceNotSupported
            [sid, ..] => Some(vec![0x7F, *sid, 0x11]),

            _ => None,
        }
    }
}

// ─── ECU Emulator Manager ────────────────────────────────────────────

/// Raw function pointers extracted from J2534Lib — these are Copy + Send.
struct RawJ2534Fns {
    read_msgs: unsafe extern "system" fn(u32, *mut PassThruMsg, *mut u32, u32) -> u32,
    write_msgs: unsafe extern "system" fn(u32, *const PassThruMsg, *mut u32, u32) -> u32,
}

// SAFETY: The function pointers are raw fn pointers (not closures), obtained
// from a loaded DLL that stays alive for the duration of the emulator.
// They reference static code, not mutable state.
unsafe impl Send for RawJ2534Fns {}

/// Multi-ECU emulator manager. One reader thread dispatches by CAN ID.
pub struct EcuEmulatorManager {
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    emulated_ecus: Vec<EcuId>,
}

impl EcuEmulatorManager {
    /// Start the emulator manager with the specified ECUs.
    pub fn start(
        lib: &Arc<crate::j2534::dll::J2534Lib>,
        channel_id: u32,
        app: AppHandle,
        ecus: Vec<EcuId>,
    ) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let fns = RawJ2534Fns {
            read_msgs: lib.pass_thru_read_msgs,
            write_msgs: lib.pass_thru_write_msgs,
        };

        // Build dispatch table: tx_can_id → (rx_can_id, handler)
        let mut handlers: HashMap<u32, (u32, Box<dyn EcuHandler>)> = HashMap::new();
        let ecu_names: Vec<String> = ecus.iter().map(|e| e.name().to_string()).collect();

        for ecu in &ecus {
            let handler: Box<dyn EcuHandler> = match ecu {
                EcuId::Bcm => Box::new(BcmHandler),
                EcuId::Gwm => Box::new(GwmHandler),
                EcuId::Ipc => Box::new(IpcHandler),
            };
            handlers.insert(ecu.tx_id(), (ecu.rx_id(), handler));
        }

        let emulated_ecus = ecus;

        let handle = thread::spawn(move || {
            Self::emulator_loop(fns, channel_id, handlers, &running_clone, &app, &ecu_names);
        });

        Self {
            running,
            handle: Some(handle),
            emulated_ecus,
        }
    }

    /// Stop the emulator manager thread.
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }

    /// Get the list of emulated ECUs.
    pub fn emulated_ecus(&self) -> &[EcuId] {
        &self.emulated_ecus
    }

    /// Try to handle a UDS request locally if it targets an emulated ECU.
    /// Returns Some(response) if handled, None if the ECU isn't emulated.
    pub fn try_handle(&self, tx_id: u32, request: &[u8]) -> Option<Vec<u8>> {
        for ecu in &self.emulated_ecus {
            if ecu.tx_id() == tx_id {
                return create_handler(*ecu).build_response(request);
            }
        }
        None
    }

    fn emulator_loop(
        fns: RawJ2534Fns,
        channel_id: u32,
        handlers: HashMap<u32, (u32, Box<dyn EcuHandler>)>,
        running: &AtomicBool,
        app: &AppHandle,
        ecu_names: &[String],
    ) {
        Self::emit_log(
            app,
            LogDirection::Rx,
            &[],
            &format!("ECU Emulator started: {}", ecu_names.join(", ")),
        );

        while running.load(Ordering::Relaxed) {
            let mut msgs = vec![PassThruMsg::default(); 4];
            let mut num_msgs: u32 = msgs.len() as u32;

            let ret = unsafe {
                (fns.read_msgs)(channel_id, msgs.as_mut_ptr(), &mut num_msgs, 100)
            };

            // BufferEmpty (0x10) or timeout (0x09) are normal
            if ret != 0 && ret != 0x10 && ret != 0x09 {
                thread::sleep(std::time::Duration::from_millis(50));
                continue;
            }

            msgs.truncate(num_msgs as usize);

            for msg in &msgs {
                let can_id = msg.can_id();
                let payload = msg.payload();
                if payload.is_empty() {
                    continue;
                }

                // Dispatch by CAN ID to the appropriate handler
                if let Some((rx_id, handler)) = handlers.get(&can_id) {
                    if let Some(response) = handler.build_response(payload) {
                        Self::emit_log(
                            app,
                            LogDirection::Rx,
                            payload,
                            &format!("{} EMU: intercepted 0x{:03X}", handler.name(), can_id),
                        );

                        let resp_msg = PassThruMsg::new_iso15765(*rx_id, &response);
                        let mut num_sent: u32 = 1;
                        let _ = unsafe {
                            (fns.write_msgs)(channel_id, &resp_msg, &mut num_sent, 100)
                        };

                        Self::emit_log(
                            app,
                            LogDirection::Tx,
                            &response,
                            &format!("{} EMU: response sent", handler.name()),
                        );
                    }
                }
            }
        }

        Self::emit_log(app, LogDirection::Rx, &[], "ECU Emulator stopped");
    }

    fn emit_log(app: &AppHandle, direction: LogDirection, data: &[u8], description: &str) {
        let _ = app.emit(
            "uds-log",
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
}

impl Drop for EcuEmulatorManager {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Create the default handler for an ECU ID
pub fn create_handler(ecu: EcuId) -> Box<dyn EcuHandler> {
    match ecu {
        EcuId::Bcm => Box::new(BcmHandler),
        EcuId::Gwm => Box::new(GwmHandler),
        EcuId::Ipc => Box::new(IpcHandler),
    }
}

// ─── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ─── ECU ID tests ───────────────────────────────────────────

    #[test]
    fn test_ecu_id_addresses() {
        assert_eq!(EcuId::Bcm.tx_id(), 0x726);
        assert_eq!(EcuId::Bcm.rx_id(), 0x72E);
        assert_eq!(EcuId::Gwm.tx_id(), 0x716);
        assert_eq!(EcuId::Gwm.rx_id(), 0x71E);
        assert_eq!(EcuId::Ipc.tx_id(), 0x720);
        assert_eq!(EcuId::Ipc.rx_id(), 0x728);
    }

    #[test]
    fn test_ecu_id_names() {
        assert_eq!(EcuId::Bcm.name(), "BCM");
        assert_eq!(EcuId::Gwm.name(), "GWM");
        assert_eq!(EcuId::Ipc.name(), "IPC");
    }

    #[test]
    fn test_ecu_id_from_str() {
        assert_eq!(EcuId::from_str("bcm"), Some(EcuId::Bcm));
        assert_eq!(EcuId::from_str("BCM"), Some(EcuId::Bcm));
        assert_eq!(EcuId::from_str("gwm"), Some(EcuId::Gwm));
        assert_eq!(EcuId::from_str("ipc"), Some(EcuId::Ipc));
        assert_eq!(EcuId::from_str("unknown"), None);
    }

    #[test]
    fn test_ecu_id_all() {
        let all = EcuId::all();
        assert_eq!(all.len(), 3);
        assert!(all.contains(&EcuId::Bcm));
        assert!(all.contains(&EcuId::Gwm));
        assert!(all.contains(&EcuId::Ipc));
    }

    // ─── BCM Handler tests (migrated from bcm_emulator) ────────

    #[test]
    fn test_bcm_handler_tester_present() {
        let handler = BcmHandler;
        let resp = handler.build_response(&[0x3E, 0x00]).unwrap();
        assert_eq!(resp, vec![0x7E, 0x00]);
    }

    #[test]
    fn test_bcm_handler_diag_session() {
        let handler = BcmHandler;
        let resp = handler.build_response(&[0x10, 0x01]).unwrap();
        assert_eq!(resp, vec![0x50, 0x01, 0x00, 0x19, 0x01, 0xF4]);
    }

    #[test]
    fn test_bcm_handler_diag_session_extended() {
        let handler = BcmHandler;
        let resp = handler.build_response(&[0x10, 0x03]).unwrap();
        assert_eq!(resp, vec![0x50, 0x03, 0x00, 0x19, 0x01, 0xF4]);
    }

    #[test]
    fn test_bcm_handler_security_access() {
        let handler = BcmHandler;
        let resp = handler.build_response(&[0x27, 0x11]).unwrap();
        assert_eq!(resp, vec![0x67, 0x11, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_bcm_handler_voltage() {
        let handler = BcmHandler;
        let resp = handler.build_response(&[0x22, 0x40, 0x2A]).unwrap();
        assert_eq!(resp, vec![0x62, 0x40, 0x2A, 0x00, 0x7C]);
    }

    #[test]
    fn test_bcm_handler_soc() {
        let handler = BcmHandler;
        let resp = handler.build_response(&[0x22, 0x40, 0x28]).unwrap();
        assert_eq!(resp, vec![0x62, 0x40, 0x28, 0x55]);
    }

    #[test]
    fn test_bcm_handler_temp() {
        let handler = BcmHandler;
        let resp = handler.build_response(&[0x22, 0x40, 0x29]).unwrap();
        assert_eq!(resp, vec![0x62, 0x40, 0x29, 0x19]);
    }

    #[test]
    fn test_bcm_handler_door_status() {
        let handler = BcmHandler;
        let resp = handler.build_response(&[0x22, 0x40, 0x30]).unwrap();
        assert_eq!(resp, vec![0x62, 0x40, 0x30, 0x00]);
    }

    #[test]
    fn test_bcm_handler_fuel_level() {
        let handler = BcmHandler;
        let resp = handler.build_response(&[0x22, 0x40, 0x32]).unwrap();
        assert_eq!(resp, vec![0x62, 0x40, 0x32, 0x4B]);
    }

    #[test]
    fn test_bcm_handler_vin() {
        let handler = BcmHandler;
        let resp = handler.build_response(&[0x22, 0xF1, 0x90]).unwrap();
        assert_eq!(resp[0], 0x62);
        let vin = String::from_utf8_lossy(&resp[3..]);
        assert_eq!(vin, "SAJBA4BN0HA000000");
    }

    #[test]
    fn test_bcm_handler_ecu_reset() {
        let handler = BcmHandler;
        let resp = handler.build_response(&[0x11, 0x01]).unwrap();
        assert_eq!(resp, vec![0x51, 0x01]);
    }

    #[test]
    fn test_bcm_handler_comm_control() {
        let handler = BcmHandler;
        let resp = handler.build_response(&[0x28, 0x01, 0x01]).unwrap();
        assert_eq!(resp, vec![0x68, 0x01]);
    }

    #[test]
    fn test_bcm_handler_write_did() {
        let handler = BcmHandler;
        let resp = handler.build_response(&[0x2E, 0x40, 0x30, 0x01]).unwrap();
        assert_eq!(resp, vec![0x6E, 0x40, 0x30]);
    }

    #[test]
    fn test_bcm_handler_routine_control() {
        let handler = BcmHandler;
        let resp = handler.build_response(&[0x31, 0x01, 0x60, 0x3E]).unwrap();
        assert_eq!(resp, vec![0x71, 0x01, 0x60, 0x3E]);
    }

    #[test]
    fn test_bcm_handler_unknown_did() {
        let handler = BcmHandler;
        let resp = handler.build_response(&[0x22, 0xFF, 0xFF]).unwrap();
        assert_eq!(resp, vec![0x7F, 0x22, 0x31]);
    }

    #[test]
    fn test_bcm_handler_unknown_service() {
        let handler = BcmHandler;
        let resp = handler.build_response(&[0x99]).unwrap();
        assert_eq!(resp, vec![0x7F, 0x99, 0x11]);
    }

    // ─── GWM Handler tests ──────────────────────────────────────

    #[test]
    fn test_gwm_handler_tester_present() {
        let handler = GwmHandler;
        let resp = handler.build_response(&[0x3E, 0x00]).unwrap();
        assert_eq!(resp, vec![0x7E, 0x00]);
    }

    #[test]
    fn test_gwm_handler_diag_session() {
        let handler = GwmHandler;
        let resp = handler.build_response(&[0x10, 0x03]).unwrap();
        assert_eq!(resp, vec![0x50, 0x03, 0x00, 0x19, 0x01, 0xF4]);
    }

    #[test]
    fn test_gwm_handler_unknown_service() {
        let handler = GwmHandler;
        let resp = handler.build_response(&[0x22, 0xF1, 0x90]).unwrap();
        assert_eq!(resp, vec![0x7F, 0x22, 0x11]);
    }

    // ─── IPC Handler tests ──────────────────────────────────────

    #[test]
    fn test_ipc_handler_tester_present() {
        let handler = IpcHandler;
        let resp = handler.build_response(&[0x3E, 0x00]).unwrap();
        assert_eq!(resp, vec![0x7E, 0x00]);
    }

    #[test]
    fn test_ipc_handler_diag_session() {
        let handler = IpcHandler;
        let resp = handler.build_response(&[0x10, 0x01]).unwrap();
        assert_eq!(resp, vec![0x50, 0x01, 0x00, 0x19, 0x01, 0xF4]);
    }

    #[test]
    fn test_ipc_handler_unknown_service() {
        let handler = IpcHandler;
        let resp = handler.build_response(&[0x31, 0x01, 0x04, 0x04]).unwrap();
        assert_eq!(resp, vec![0x7F, 0x31, 0x11]);
    }

    // ─── create_handler ─────────────────────────────────────────

    #[test]
    fn test_create_handler_bcm() {
        let handler = create_handler(EcuId::Bcm);
        assert_eq!(handler.name(), "BCM");
        let resp = handler.build_response(&[0x3E, 0x00]).unwrap();
        assert_eq!(resp, vec![0x7E, 0x00]);
    }

    #[test]
    fn test_create_handler_gwm() {
        let handler = create_handler(EcuId::Gwm);
        assert_eq!(handler.name(), "GWM");
    }

    #[test]
    fn test_create_handler_ipc() {
        let handler = create_handler(EcuId::Ipc);
        assert_eq!(handler.name(), "IPC");
    }

    // ─── try_handle tests ──────────────────────────────────────

    #[test]
    fn test_try_handle_bcm_vin() {
        let mgr = EcuEmulatorManager {
            running: Arc::new(AtomicBool::new(false)),
            handle: None,
            emulated_ecus: vec![EcuId::Bcm],
        };
        let resp = mgr.try_handle(ecu_addr::BCM_TX, &[0x22, 0xF1, 0x90]).unwrap();
        assert_eq!(resp[0], 0x62);
        let vin = String::from_utf8_lossy(&resp[3..]);
        assert_eq!(vin, "SAJBA4BN0HA000000");
    }

    #[test]
    fn test_try_handle_bcm_voltage() {
        let mgr = EcuEmulatorManager {
            running: Arc::new(AtomicBool::new(false)),
            handle: None,
            emulated_ecus: vec![EcuId::Bcm],
        };
        let resp = mgr.try_handle(ecu_addr::BCM_TX, &[0x22, 0x40, 0x2A]).unwrap();
        assert_eq!(resp, vec![0x62, 0x40, 0x2A, 0x00, 0x7C]);
    }

    #[test]
    fn test_try_handle_unknown_txid() {
        let mgr = EcuEmulatorManager {
            running: Arc::new(AtomicBool::new(false)),
            handle: None,
            emulated_ecus: vec![EcuId::Bcm],
        };
        // IMC TX is not emulated
        let resp = mgr.try_handle(ecu_addr::IMC_TX, &[0x22, 0xF1, 0x90]);
        assert!(resp.is_none());
    }

    #[test]
    fn test_try_handle_unknown_did() {
        let mgr = EcuEmulatorManager {
            running: Arc::new(AtomicBool::new(false)),
            handle: None,
            emulated_ecus: vec![EcuId::Bcm],
        };
        let resp = mgr.try_handle(ecu_addr::BCM_TX, &[0x22, 0xFF, 0xFF]).unwrap();
        assert_eq!(resp, vec![0x7F, 0x22, 0x31]); // NRC requestOutOfRange
    }
}
