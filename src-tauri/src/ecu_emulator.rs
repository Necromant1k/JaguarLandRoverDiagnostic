use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use serde::{Deserialize, Serialize};

use crate::j2534::types::*;
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
            // Real car data captured 2026-02-23 from SAJBL4BVXGCY16353 (X260 MY16 Jaguar XF)
            [0x22, did_hi, did_lo, ..] => {
                let did = ((*did_hi as u16) << 8) | (*did_lo as u16);
                match did {
                    0xF190 => {
                        let mut resp = vec![0x62, 0xF1, 0x90];
                        resp.extend_from_slice(b"SAJBL4BVXGCY16353");
                        Some(resp)
                    }
                    0xF188 => {
                        // SW part number (24 bytes, null-padded)
                        let mut resp = vec![0x62, 0xF1, 0x88];
                        let mut part = b"GX73-14C184-AK".to_vec();
                        part.resize(24, 0x00);
                        resp.extend_from_slice(&part);
                        Some(resp)
                    }
                    0xF18C => {
                        let mut resp = vec![0x62, 0xF1, 0x8C];
                        resp.extend_from_slice(b"1979149808");
                        Some(resp)
                    }
                    0xF113 => {
                        // HW part number (24 bytes, null-padded)
                        let mut resp = vec![0x62, 0xF1, 0x13];
                        let mut part = b"GX73-14F041-AK".to_vec();
                        part.resize(24, 0x00);
                        resp.extend_from_slice(&part);
                        Some(resp)
                    }
                    0x40AB => Some(vec![0x62, 0x40, 0xAB, 0x04]),
                    0x40DE => Some(vec![0x62, 0x40, 0xDE, 0x01]),
                    0x41DD => Some(vec![0x62, 0x41, 0xDD, 0x00]),
                    0xA112 => Some(vec![0x62, 0xA1, 0x12, 0x1F, 0xFF, 0xFF, 0x1F]),
                    0xC124 => Some(vec![0x62, 0xC1, 0x24, 0x6C, 0x04, 0x6C, 0x04]),
                    0xC190 => Some(vec![0x62, 0xC1, 0x90, 0x80, 0x00, 0x7D, 0x6C]),
                    0xD134 => Some(vec![0x62, 0xD1, 0x34, 0x00]),
                    0xDD01 => Some(vec![0x62, 0xDD, 0x01, 0x03, 0x5F, 0xB8]),
                    0xDD06 => Some(vec![0x62, 0xDD, 0x06, 0x67]),
                    0xDE02 => {
                        let mut resp = vec![0x62, 0xDE, 0x02];
                        resp.extend_from_slice(&[0xFF; 8]);
                        Some(resp)
                    }
                    0xDE03 => {
                        let mut resp = vec![0x62, 0xDE, 0x03];
                        resp.extend_from_slice(&[0xFF; 8]);
                        Some(resp)
                    }
                    // Battery DIDs (402A/4028/4029) are on GWM, not BCM — return 0x31
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

/// CAN broadcast messages captured from a real car that are NOT present on a bench
/// with only the IMC. These simulate BCM/GWM/IPC presence on the CAN bus.
/// Format: (CAN ID, 8-byte data)
pub(crate) const BROADCAST_MSGS: &[(u32, [u8; 8])] = &[
    // From car CAN dump — IDs absent in bench-only dump
    (0x070, [0xFF, 0x87, 0xD0, 0xFE, 0xFE, 0x3F, 0xFF, 0x03]),
    (0x0B0, [0x00, 0x04, 0x32, 0x03, 0xF8, 0x0D, 0x35, 0x00]),
    (0x0D0, [0xEC, 0x00, 0x42, 0x50, 0xE2, 0x69, 0xA8, 0x84]),
    (0x154, [0x27, 0xC7, 0x07, 0xED, 0x07, 0xD9, 0x07, 0xBD]),
    (0x1D0, [0x62, 0xFE, 0x00, 0x10, 0x80, 0x00, 0x80, 0x00]),
    (0x200, [0x01, 0x00, 0x00, 0x00, 0x03, 0x5E, 0x0E, 0x00]),
    (0x270, [0x00, 0xE8, 0x50, 0x00, 0x83, 0xFE, 0x03, 0x00]),
    (0x280, [0x00, 0x00, 0x03, 0xFE, 0x01, 0xFE, 0x13, 0xFE]),
    (0x2A0, [0x80, 0x81, 0x40, 0x00, 0x5D, 0x44, 0x66, 0x0D]),
    (0x2C0, [0x30, 0x00, 0x7D, 0xD0, 0x01, 0x40, 0x9A, 0x80]),
    (0x300, [0x01, 0x6B, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]),
    // Network Management heartbeats — IMC checks for these to know other ECUs are alive
    (0x400, [0x08, 0x01, 0x00, 0x00, 0x16, 0x04, 0x00, 0x01]), // BCM NM
    (0x407, [0x0A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]), // GWM NM
    (0x460, [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]), // IPC NM
    // Common IDs with car-specific data (bench has different values)
    (0x030, [0x04, 0x00, 0x00, 0x00, 0x00, 0x1F, 0xFE, 0x70]),
    (0x130, [0x02, 0x00, 0x50, 0x04, 0x04, 0x00, 0x00, 0x00]),
    (0x140, [0x00, 0x6D, 0x83, 0x00, 0x00, 0x7F, 0x80, 0x00]),
];

/// Raw write-only function pointer for the broadcast thread.
struct RawWriteFn {
    write_msgs: unsafe extern "system" fn(u32, *const PassThruMsg, *mut u32, u32) -> u32,
}

unsafe impl Send for RawWriteFn {}

/// Multi-ECU emulator manager.
/// - Software routing: try_handle() serves emulated ECU responses locally
/// - CAN broadcast: write-only thread sends periodic messages to simulate ECU presence
pub struct EcuEmulatorManager {
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
    emulated_ecus: Vec<EcuId>,
}

impl EcuEmulatorManager {
    /// Create emulator with software routing + CAN broadcast thread.
    /// `can_channel_id` is a raw CAN channel (not ISO15765) for broadcast.
    pub fn new_with_broadcast(
        lib: &Arc<crate::j2534::dll::J2534Lib>,
        can_channel_id: u32,
        ecus: Vec<EcuId>,
    ) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let write_fn = RawWriteFn {
            write_msgs: lib.pass_thru_write_msgs,
        };

        let handle = thread::spawn(move || {
            Self::broadcast_loop(write_fn, can_channel_id, &running_clone);
        });

        Self {
            running,
            handle: Some(handle),
            emulated_ecus: ecus,
        }
    }

    /// Create software-routing-only emulator (no CAN broadcast).
    pub fn new(ecus: Vec<EcuId>) -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            handle: None,
            emulated_ecus: ecus,
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

    /// Write-only broadcast loop: sends CAN messages to simulate ECU presence.
    /// Runs on a separate raw CAN channel — never reads, so it can't steal
    /// ISO15765 responses from the client thread.
    fn broadcast_loop(fns: RawWriteFn, can_channel_id: u32, running: &AtomicBool) {
        // Small delay to let the channel settle after connect
        thread::sleep(std::time::Duration::from_millis(100));

        while running.load(Ordering::Relaxed) {
            for &(can_id, ref data) in BROADCAST_MSGS {
                if !running.load(Ordering::Relaxed) {
                    break;
                }

                let mut msg = PassThruMsg::default();
                msg.protocol_id = 5; // PROTOCOL_CAN
                msg.data[0..4].copy_from_slice(&can_id.to_be_bytes());
                msg.data[4..12].copy_from_slice(data);
                msg.data_size = 12; // 4 bytes CAN ID + 8 bytes data

                let mut num_msgs: u32 = 1;
                let _ = unsafe {
                    (fns.write_msgs)(can_channel_id, &msg, &mut num_msgs, 50)
                };
            }

            // ~100ms cycle matches typical CAN bus timing
            thread::sleep(std::time::Duration::from_millis(100));
        }
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
    fn test_bcm_handler_voltage_returns_nrc() {
        // Real BCM returns 0x31 for 402A — battery data is on GWM
        let handler = BcmHandler;
        let resp = handler.build_response(&[0x22, 0x40, 0x2A]).unwrap();
        assert_eq!(resp, vec![0x7F, 0x22, 0x31]);
    }

    #[test]
    fn test_bcm_handler_soc_returns_nrc() {
        let handler = BcmHandler;
        let resp = handler.build_response(&[0x22, 0x40, 0x28]).unwrap();
        assert_eq!(resp, vec![0x7F, 0x22, 0x31]);
    }

    #[test]
    fn test_bcm_handler_temp_returns_nrc() {
        let handler = BcmHandler;
        let resp = handler.build_response(&[0x22, 0x40, 0x29]).unwrap();
        assert_eq!(resp, vec![0x7F, 0x22, 0x31]);
    }

    #[test]
    fn test_bcm_handler_door_status_returns_nrc() {
        let handler = BcmHandler;
        let resp = handler.build_response(&[0x22, 0x40, 0x30]).unwrap();
        assert_eq!(resp, vec![0x7F, 0x22, 0x31]);
    }

    #[test]
    fn test_bcm_handler_fuel_level_returns_nrc() {
        let handler = BcmHandler;
        let resp = handler.build_response(&[0x22, 0x40, 0x32]).unwrap();
        assert_eq!(resp, vec![0x7F, 0x22, 0x31]);
    }

    #[test]
    fn test_bcm_handler_vin() {
        let handler = BcmHandler;
        let resp = handler.build_response(&[0x22, 0xF1, 0x90]).unwrap();
        assert_eq!(resp[0], 0x62);
        let vin = String::from_utf8_lossy(&resp[3..]);
        assert_eq!(vin, "SAJBL4BVXGCY16353");
    }

    #[test]
    fn test_bcm_handler_sw_part() {
        let handler = BcmHandler;
        let resp = handler.build_response(&[0x22, 0xF1, 0x88]).unwrap();
        assert_eq!(resp[0], 0x62);
        let part = String::from_utf8_lossy(&resp[3..]).trim_matches('\0').to_string();
        assert_eq!(part.trim(), "GX73-14C184-AK");
    }

    #[test]
    fn test_bcm_handler_ecu_serial() {
        let handler = BcmHandler;
        let resp = handler.build_response(&[0x22, 0xF1, 0x8C]).unwrap();
        assert_eq!(resp[0], 0x62);
        let serial = String::from_utf8_lossy(&resp[3..]);
        assert_eq!(serial, "1979149808");
    }

    #[test]
    fn test_bcm_handler_hw_part() {
        let handler = BcmHandler;
        let resp = handler.build_response(&[0x22, 0xF1, 0x13]).unwrap();
        assert_eq!(resp[0], 0x62);
        let part = String::from_utf8_lossy(&resp[3..]).trim_matches('\0').to_string();
        assert_eq!(part.trim(), "GX73-14F041-AK");
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
        assert_eq!(vin, "SAJBL4BVXGCY16353");
    }

    #[test]
    fn test_try_handle_bcm_voltage_returns_nrc() {
        // Battery voltage is on GWM, not BCM — emulator returns 0x31
        let mgr = EcuEmulatorManager {
            running: Arc::new(AtomicBool::new(false)),
            handle: None,
            emulated_ecus: vec![EcuId::Bcm],
        };
        let resp = mgr.try_handle(ecu_addr::BCM_TX, &[0x22, 0x40, 0x2A]).unwrap();
        assert_eq!(resp, vec![0x7F, 0x22, 0x31]);
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
