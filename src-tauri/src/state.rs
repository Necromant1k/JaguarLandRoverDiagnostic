use std::sync::{Arc, Mutex};

use crate::bcm_emulator::BcmEmulator;
use crate::j2534::device::{J2534Channel, J2534Device};
use crate::j2534::dll::J2534Lib;

/// Active connection to a J2534 device with an ECU channel
pub struct Connection {
    pub lib: Arc<J2534Lib>,
    pub device: J2534Device,
    pub channel: Option<J2534Channel>,
    pub dll_path: String,
    pub bcm_emulator: Option<BcmEmulator>,
}

/// Global app state managed by Tauri
pub struct AppState {
    pub connection: Mutex<Option<Connection>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            connection: Mutex::new(None),
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connection.lock().unwrap().is_some()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
