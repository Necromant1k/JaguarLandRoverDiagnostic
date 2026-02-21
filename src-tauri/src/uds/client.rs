use std::time::{Duration, Instant};

use crate::j2534::Channel;
use crate::j2534::types::PassThruMsg;
use crate::uds::error::{NegativeResponseCode, UdsError};

/// Log entry direction
#[derive(Debug, Clone, serde::Serialize)]
pub enum LogDirection {
    Tx,
    Rx,
    Error,
    Pending,
}

impl std::fmt::Display for LogDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogDirection::Tx => write!(f, "TX"),
            LogDirection::Rx => write!(f, "RX"),
            LogDirection::Error => write!(f, "ERR"),
            LogDirection::Pending => write!(f, "..."),
        }
    }
}

/// Log entry for UDS communication
#[derive(Debug, Clone, serde::Serialize)]
pub struct LogEntry {
    pub direction: LogDirection,
    pub data_hex: String,
    pub timestamp: String,
    pub description: String,
}

/// Callback type for logging UDS messages
pub type LogCallback = Box<dyn Fn(LogEntry) + Send + Sync>;

/// UDS Client wrapping any Channel implementation for ECU communication.
/// Works with real J2534Channel or MockChannel for testing.
pub struct UdsClient<C: Channel> {
    channel: C,
    tx_id: u32,
    rx_id: u32,
    log_callback: Option<LogCallback>,
}

impl<C: Channel> UdsClient<C> {
    pub fn new(channel: C, tx_id: u32, rx_id: u32) -> Self {
        Self {
            channel,
            tx_id,
            rx_id,
            log_callback: None,
        }
    }

    pub fn set_log_callback(&mut self, callback: LogCallback) {
        self.log_callback = Some(callback);
    }

    fn log(&self, direction: LogDirection, data: &[u8], description: &str) {
        if let Some(ref cb) = self.log_callback {
            cb(LogEntry {
                direction,
                data_hex: data
                    .iter()
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<_>>()
                    .join(" "),
                timestamp: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                description: description.to_string(),
            });
        }
    }

    /// Send a UDS request and receive the response, handling NRC 0x78 (ResponsePending)
    pub fn send_recv(
        &self,
        request: &[u8],
        timeout_ms: u32,
        wait_pending: bool,
    ) -> Result<Vec<u8>, UdsError> {
        let service_id = request[0];
        self.log(LogDirection::Tx, request, &describe_service(service_id));

        // Build and send the ISO15765 message
        let msg = PassThruMsg::new_iso15765(self.tx_id, request);
        self.channel
            .send(&msg, 2000)
            .map_err(UdsError::TransportError)?;

        let total_timeout = if wait_pending {
            Duration::from_millis(30000)
        } else {
            Duration::from_millis(timeout_ms as u64)
        };
        let start = Instant::now();

        loop {
            if start.elapsed() > total_timeout {
                self.log(LogDirection::Error, &[], "Timeout waiting for response");
                return Err(UdsError::Timeout);
            }

            let read_timeout = 500; // poll every 500ms
            let msgs = self
                .channel
                .read(read_timeout)
                .map_err(UdsError::TransportError)?;

            for msg in msgs {
                if msg.data_size <= 4 {
                    continue;
                }
                let payload = msg.payload();
                if payload.is_empty() {
                    continue;
                }

                // Check for negative response
                if payload[0] == 0x7F && payload.len() >= 3 {
                    let resp_service = payload[1];
                    let nrc = NegativeResponseCode::from_byte(payload[2]);

                    if nrc.is_pending() {
                        self.log(LogDirection::Pending, payload, "Response pending...");
                        // Continue waiting for actual response
                        continue;
                    }

                    self.log(
                        LogDirection::Error,
                        payload,
                        &format!("NRC: {}", nrc),
                    );
                    return Err(UdsError::NegativeResponse {
                        service_id: resp_service,
                        nrc,
                    });
                }

                // Check for positive response (service_id + 0x40)
                let expected_response_id = service_id + 0x40;
                if payload[0] == expected_response_id {
                    self.log(LogDirection::Rx, payload, &describe_service(service_id));
                    return Ok(payload.to_vec());
                }

                // Unexpected response — log but continue
                self.log(
                    LogDirection::Error,
                    payload,
                    &format!(
                        "Unexpected response: expected 0x{:02X}, got 0x{:02X}",
                        expected_response_id, payload[0]
                    ),
                );
            }

            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Send without expecting a response (e.g., TesterPresent with suppressResponse)
    pub fn send_no_response(&self, request: &[u8]) -> Result<(), UdsError> {
        self.log(
            LogDirection::Tx,
            request,
            &describe_service(request[0]),
        );
        let msg = PassThruMsg::new_iso15765(self.tx_id, request);
        self.channel
            .send(&msg, 2000)
            .map_err(UdsError::TransportError)
    }

    pub fn tx_id(&self) -> u32 {
        self.tx_id
    }

    pub fn rx_id(&self) -> u32 {
        self.rx_id
    }

    pub fn channel(&self) -> &C {
        &self.channel
    }
}

fn describe_service(service_id: u8) -> String {
    match service_id {
        0x10 => "DiagnosticSessionControl".to_string(),
        0x11 => "ECUReset".to_string(),
        0x22 => "ReadDataByIdentifier".to_string(),
        0x27 => "SecurityAccess".to_string(),
        0x2E => "WriteDataByIdentifier".to_string(),
        0x31 => "RoutineControl".to_string(),
        0x34 => "RequestDownload".to_string(),
        0x36 => "TransferData".to_string(),
        0x37 => "RequestTransferExit".to_string(),
        0x3E => "TesterPresent".to_string(),
        _ => format!("Service 0x{:02X}", service_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::j2534::mock::MockChannel;
    use crate::uds::error::NegativeResponseCode;

    fn make_client(mock: MockChannel) -> UdsClient<MockChannel> {
        mock.setup_iso15765_filter(0x7B3, 0x7BB).unwrap();
        UdsClient::new(mock, 0x7B3, 0x7BB)
    }

    #[test]
    fn test_positive_response() {
        let mock = MockChannel::new();
        mock.expect_request(
            0x7B3,
            vec![0x22, 0xF1, 0x90],
            vec![0x62, 0xF1, 0x90, 0x53, 0x41, 0x4A],
        );
        let client = make_client(mock);

        let resp = client.send_recv(&[0x22, 0xF1, 0x90], 2000, false).unwrap();
        assert_eq!(resp, vec![0x62, 0xF1, 0x90, 0x53, 0x41, 0x4A]);
    }

    #[test]
    fn test_negative_response() {
        let mock = MockChannel::new();
        mock.expect_request(
            0x7B3,
            vec![0x22, 0xFF, 0xFF],
            vec![0x7F, 0x22, 0x31], // requestOutOfRange
        );
        let client = make_client(mock);

        let err = client.send_recv(&[0x22, 0xFF, 0xFF], 2000, false).unwrap_err();
        match err {
            UdsError::NegativeResponse { service_id, nrc } => {
                assert_eq!(service_id, 0x22);
                assert_eq!(nrc, NegativeResponseCode::RequestOutOfRange);
            }
            other => panic!("Expected NegativeResponse, got {:?}", other),
        }
    }

    #[test]
    fn test_response_pending_then_ok() {
        let mock = MockChannel::new();
        mock.expect_request_multi(
            0x7B3,
            vec![0x31, 0x01, 0x60, 0x3E, 0x01],
            vec![
                vec![0x7F, 0x31, 0x78], // pending
                vec![0x71, 0x01, 0x60, 0x3E], // OK
            ],
        );
        let client = make_client(mock);

        let resp = client
            .send_recv(&[0x31, 0x01, 0x60, 0x3E, 0x01], 5000, true)
            .unwrap();
        assert_eq!(resp[0], 0x71);
    }

    #[test]
    fn test_response_pending_timeout() {
        let mock = MockChannel::new();
        // Send pending, then nothing — should timeout
        mock.expect_request(
            0x7B3,
            vec![0x31, 0x01, 0x60, 0x3E, 0x01],
            vec![0x7F, 0x31, 0x78], // pending only
        );
        let client = make_client(mock);

        // Use a very short timeout to not block tests
        let err = client
            .send_recv(&[0x31, 0x01, 0x60, 0x3E, 0x01], 200, false)
            .unwrap_err();
        assert!(matches!(err, UdsError::Timeout));
    }

    #[test]
    fn test_timeout_no_response() {
        let mock = MockChannel::new();
        mock.set_timeout_mode(true);
        let client = make_client(mock);

        let err = client.send_recv(&[0x22, 0xF1, 0x90], 200, false).unwrap_err();
        assert!(matches!(err, UdsError::Timeout));
    }

    #[test]
    fn test_wrong_service_id() {
        let mock = MockChannel::new();
        // Return response for a different service
        mock.expect_request(
            0x7B3,
            vec![0x22, 0xF1, 0x90],
            vec![0x50, 0x03, 0x00, 0x19], // DiagSession response instead of ReadDID
        );
        let client = make_client(mock);

        // The client should see 0x50 but expect 0x62 — it will log but timeout
        // since there's no matching positive response
        let err = client.send_recv(&[0x22, 0xF1, 0x90], 200, false).unwrap_err();
        assert!(matches!(err, UdsError::Timeout));
    }
}
