pub mod device;
pub mod dll;
#[cfg(test)]
pub mod mock;
pub mod types;

use types::PassThruMsg;

/// Trait abstracting a J2534 channel for send/recv — enables mock testing
pub trait Channel: Send {
    fn send(&self, msg: &PassThruMsg, timeout_ms: u32) -> Result<(), String>;
    fn read(&self, timeout_ms: u32) -> Result<Vec<PassThruMsg>, String>;
    fn setup_iso15765_filter(&self, tx_id: u32, rx_id: u32) -> Result<u32, String>;
}

/// Implement Channel for the real J2534Channel
impl Channel for device::J2534Channel {
    fn send(&self, msg: &PassThruMsg, timeout_ms: u32) -> Result<(), String> {
        self.send(msg, timeout_ms)
    }

    fn read(&self, timeout_ms: u32) -> Result<Vec<PassThruMsg>, String> {
        self.read(timeout_ms)
    }

    fn setup_iso15765_filter(&self, tx_id: u32, rx_id: u32) -> Result<u32, String> {
        self.setup_iso15765_filter(tx_id, rx_id)
    }
}
