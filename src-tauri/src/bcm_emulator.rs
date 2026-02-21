use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use tauri::{AppHandle, Emitter};

use crate::j2534::types::*;
use crate::uds::client::{LogDirection, LogEntry};
use crate::uds::services::ecu_addr;

/// BCM CAN bus emulator for bench mode.
///
/// When the IMC module is on the bench (not in a vehicle), it expects a BCM
/// on the CAN bus. This emulator listens for BCM requests (CAN ID 0x726)
/// and responds with appropriate data (CAN ID 0x72E).
pub struct BcmEmulator {
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

/// Raw function pointers extracted from J2534Lib — these are Copy + Send.
struct RawJ2534Fns {
    read_msgs: unsafe extern "system" fn(u32, *mut PassThruMsg, *mut u32, u32) -> u32,
    write_msgs: unsafe extern "system" fn(u32, *const PassThruMsg, *mut u32, u32) -> u32,
}

// SAFETY: The function pointers are raw fn pointers (not closures), obtained
// from a loaded DLL that stays alive for the duration of the emulator.
// They reference static code, not mutable state.
unsafe impl Send for RawJ2534Fns {}

impl BcmEmulator {
    /// Start the BCM emulator background thread.
    ///
    /// The thread reads messages from the J2534 channel and auto-responds to
    /// BCM-addressed requests. Uses raw function pointers + channel_id to
    /// avoid Send/Sync issues with J2534Channel.
    pub fn start(
        lib: &Arc<crate::j2534::dll::J2534Lib>,
        channel_id: u32,
        app: AppHandle,
    ) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        let fns = RawJ2534Fns {
            read_msgs: lib.pass_thru_read_msgs,
            write_msgs: lib.pass_thru_write_msgs,
        };

        let handle = thread::spawn(move || {
            Self::emulator_loop(fns, channel_id, &running_clone, &app);
        });

        Self {
            running,
            handle: Some(handle),
        }
    }

    /// Stop the BCM emulator thread.
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }

    fn emulator_loop(
        fns: RawJ2534Fns,
        channel_id: u32,
        running: &AtomicBool,
        app: &AppHandle,
    ) {
        Self::emit_log(app, LogDirection::Rx, &[], "BCM Emulator started");

        while running.load(Ordering::Relaxed) {
            // Read messages with short timeout (100ms) so we can check `running`
            let mut msgs = vec![PassThruMsg::default(); 4];
            let mut num_msgs: u32 = msgs.len() as u32;

            let ret = unsafe {
                (fns.read_msgs)(channel_id, msgs.as_mut_ptr(), &mut num_msgs, 100)
            };

            // BufferEmpty (0x10) or timeout (0x09) are normal — no messages
            if ret != 0 && ret != 0x10 && ret != 0x09 {
                thread::sleep(std::time::Duration::from_millis(50));
                continue;
            }

            msgs.truncate(num_msgs as usize);

            for msg in &msgs {
                let can_id = msg.can_id();
                if can_id != ecu_addr::BCM_TX {
                    continue; // Not a BCM request — ignore
                }

                let payload = msg.payload();
                if payload.is_empty() {
                    continue;
                }

                if let Some(response) = Self::build_response(payload) {
                    Self::emit_log(
                        app,
                        LogDirection::Rx,
                        payload,
                        &format!("BCM EMU: intercepted 0x{:03X}", can_id),
                    );

                    let resp_msg = PassThruMsg::new_iso15765(ecu_addr::BCM_RX, &response);
                    let mut num_sent: u32 = 1;
                    let _ = unsafe {
                        (fns.write_msgs)(channel_id, &resp_msg, &mut num_sent, 100)
                    };

                    Self::emit_log(
                        app,
                        LogDirection::Tx,
                        &response,
                        "BCM EMU: response sent",
                    );
                }
            }
        }

        Self::emit_log(app, LogDirection::Rx, &[], "BCM Emulator stopped");
    }

    /// Build a BCM response for a given UDS request payload.
    fn build_response(request: &[u8]) -> Option<Vec<u8>> {
        match request {
            // TesterPresent (3E 00) → positive response (7E 00)
            [0x3E, 0x00, ..] => Some(vec![0x7E, 0x00]),

            // DiagnosticSessionControl (10 XX) → positive response (50 XX ...)
            [0x10, session, ..] => Some(vec![0x50, *session, 0x00, 0x19, 0x01, 0xF4]),

            // ReadDataByIdentifier (22 XX XX)
            [0x22, did_hi, did_lo, ..] => {
                let did = ((*did_hi as u16) << 8) | (*did_lo as u16);
                match did {
                    // Battery voltage (402A) → 12.4V = 0x007C
                    0x402A => Some(vec![0x62, 0x40, 0x2A, 0x00, 0x7C]),
                    // Battery SoC (4028) → 85%
                    0x4028 => Some(vec![0x62, 0x40, 0x28, 0x55]),
                    // Battery temp (4029) → 25°C
                    0x4029 => Some(vec![0x62, 0x40, 0x29, 0x19]),
                    // Unknown DID → NRC serviceNotSupported
                    _ => Some(vec![0x7F, 0x22, 0x31]),
                }
            }

            // SecurityAccess seed request (27 XX) → zero seed (already unlocked)
            [0x27, level, ..] => Some(vec![0x67, *level, 0x00, 0x00, 0x00]),

            // Unknown service → NRC serviceNotSupported
            [sid, ..] => Some(vec![0x7F, *sid, 0x11]),

            _ => None,
        }
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

impl Drop for BcmEmulator {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bcm_emulator_responds_voltage() {
        let request = vec![0x22, 0x40, 0x2A];
        let response = BcmEmulator::build_response(&request).unwrap();
        assert_eq!(response, vec![0x62, 0x40, 0x2A, 0x00, 0x7C]);
    }

    #[test]
    fn test_bcm_emulator_responds_soc() {
        let request = vec![0x22, 0x40, 0x28];
        let response = BcmEmulator::build_response(&request).unwrap();
        assert_eq!(response, vec![0x62, 0x40, 0x28, 0x55]);
    }

    #[test]
    fn test_bcm_emulator_responds_temp() {
        let request = vec![0x22, 0x40, 0x29];
        let response = BcmEmulator::build_response(&request).unwrap();
        assert_eq!(response, vec![0x62, 0x40, 0x29, 0x19]);
    }

    #[test]
    fn test_bcm_emulator_responds_tester_present() {
        let request = vec![0x3E, 0x00];
        let response = BcmEmulator::build_response(&request).unwrap();
        assert_eq!(response, vec![0x7E, 0x00]);
    }

    #[test]
    fn test_bcm_emulator_responds_diag_session() {
        let request = vec![0x10, 0x01];
        let response = BcmEmulator::build_response(&request).unwrap();
        assert_eq!(response, vec![0x50, 0x01, 0x00, 0x19, 0x01, 0xF4]);
    }

    #[test]
    fn test_bcm_emulator_responds_diag_session_extended() {
        let request = vec![0x10, 0x03];
        let response = BcmEmulator::build_response(&request).unwrap();
        assert_eq!(response, vec![0x50, 0x03, 0x00, 0x19, 0x01, 0xF4]);
    }

    #[test]
    fn test_bcm_emulator_responds_security_access() {
        let request = vec![0x27, 0x11];
        let response = BcmEmulator::build_response(&request).unwrap();
        assert_eq!(response, vec![0x67, 0x11, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_bcm_emulator_unknown_did_returns_nrc() {
        let request = vec![0x22, 0xFF, 0xFF];
        let response = BcmEmulator::build_response(&request).unwrap();
        assert_eq!(response, vec![0x7F, 0x22, 0x31]);
    }

    #[test]
    fn test_bcm_emulator_unknown_service_returns_nrc() {
        let request = vec![0x99];
        let response = BcmEmulator::build_response(&request).unwrap();
        assert_eq!(response, vec![0x7F, 0x99, 0x11]);
    }
}
