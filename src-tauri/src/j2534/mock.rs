use std::cell::RefCell;
use std::collections::VecDeque;

use crate::j2534::types::*;
use crate::j2534::Channel;

/// A single expected request → response pair
#[derive(Debug, Clone)]
struct Expectation {
    /// Expected TX CAN ID
    tx_id: u32,
    /// Expected UDS payload (without CAN header)
    expected_request: Vec<u8>,
    /// Response payloads to return (multiple for pending scenarios)
    responses: Vec<Vec<u8>>,
}

/// Mock J2534 channel for testing UDS client and services without hardware.
/// Supports multi-ECU routing by CAN ID (IMC, BCM, GWM etc).
pub struct MockChannel {
    expectations: RefCell<VecDeque<Expectation>>,
    /// Pending responses to deliver on next read() call
    pending_responses: RefCell<VecDeque<(u32, Vec<u8>)>>,
    /// Track sent messages for assertions
    sent_messages: RefCell<Vec<(u32, Vec<u8>)>>,
    /// Registered filters (tx_id, rx_id)
    filters: RefCell<Vec<(u32, u32)>>,
    /// If true, read() returns empty when no pending (simulates timeout)
    timeout_mode: RefCell<bool>,
}

impl MockChannel {
    pub fn new() -> Self {
        Self {
            expectations: RefCell::new(VecDeque::new()),
            pending_responses: RefCell::new(VecDeque::new()),
            sent_messages: RefCell::new(Vec::new()),
            filters: RefCell::new(Vec::new()),
            timeout_mode: RefCell::new(false),
        }
    }

    /// Expect a request with given payload on tx_id, respond with given payload from rx_id.
    /// The response CAN header is auto-derived from the filter for that tx_id.
    pub fn expect_request(&self, tx_id: u32, request: Vec<u8>, response: Vec<u8>) {
        self.expectations.borrow_mut().push_back(Expectation {
            tx_id,
            expected_request: request,
            responses: vec![response],
        });
    }

    /// Expect a request that returns multiple responses (e.g., pending then OK).
    pub fn expect_request_multi(&self, tx_id: u32, request: Vec<u8>, responses: Vec<Vec<u8>>) {
        self.expectations.borrow_mut().push_back(Expectation {
            tx_id,
            expected_request: request,
            responses,
        });
    }

    /// Set timeout mode — read() will return empty (simulating no ECU response)
    pub fn set_timeout_mode(&self, enabled: bool) {
        *self.timeout_mode.borrow_mut() = enabled;
    }

    /// Get all messages that were sent through this channel
    pub fn sent_messages(&self) -> Vec<(u32, Vec<u8>)> {
        self.sent_messages.borrow().clone()
    }

    /// Get registered filters
    pub fn filters(&self) -> Vec<(u32, u32)> {
        self.filters.borrow().clone()
    }

    /// Verify all expectations were consumed
    pub fn verify(&self) {
        let remaining = self.expectations.borrow().len();
        assert_eq!(
            remaining, 0,
            "MockChannel: {} expectations were not consumed",
            remaining
        );
    }

    /// Find the rx_id for a given tx_id from registered filters
    fn rx_id_for_tx(&self, tx_id: u32) -> u32 {
        for (filter_tx, filter_rx) in self.filters.borrow().iter() {
            if *filter_tx == tx_id {
                return *filter_rx;
            }
        }
        // Default: use common JLR pattern (tx + 8)
        tx_id + 8
    }
}

impl Channel for MockChannel {
    fn send(&self, msg: &PassThruMsg, _timeout_ms: u32) -> Result<(), String> {
        let can_id = msg.can_id();
        let payload = msg.payload().to_vec();

        self.sent_messages
            .borrow_mut()
            .push((can_id, payload.clone()));

        // Match against expectations
        let mut expectations = self.expectations.borrow_mut();
        if let Some(pos) = expectations
            .iter()
            .position(|e| e.tx_id == can_id && e.expected_request == payload)
        {
            let exp = expectations.remove(pos).unwrap();
            let rx_id = self.rx_id_for_tx(can_id);

            // Queue all responses
            for resp in exp.responses {
                self.pending_responses.borrow_mut().push_back((rx_id, resp));
            }
        }
        // If no expectation matches and not in timeout mode, that's OK —
        // the test might be checking that something was sent without caring about response

        Ok(())
    }

    fn read(&self, _timeout_ms: u32) -> Result<Vec<PassThruMsg>, String> {
        if *self.timeout_mode.borrow() {
            return Ok(vec![]);
        }

        let mut responses = self.pending_responses.borrow_mut();
        let mut msgs = Vec::new();

        while let Some((rx_id, payload)) = responses.pop_front() {
            let msg = PassThruMsg::new_iso15765(rx_id, &payload);
            msgs.push(msg);
        }

        Ok(msgs)
    }

    fn setup_iso15765_filter(&self, tx_id: u32, rx_id: u32) -> Result<u32, String> {
        let mut filters = self.filters.borrow_mut();
        let filter_id = filters.len() as u32;
        filters.push((tx_id, rx_id));
        Ok(filter_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_basic_send_recv() {
        let mock = MockChannel::new();
        mock.setup_iso15765_filter(0x7B3, 0x7BB).unwrap();
        mock.expect_request(
            0x7B3,
            vec![0x22, 0xF1, 0x90],
            vec![0x62, 0xF1, 0x90, 0x53, 0x41, 0x4A],
        );

        let tx_msg = PassThruMsg::new_iso15765(0x7B3, &[0x22, 0xF1, 0x90]);
        mock.send(&tx_msg, 2000).unwrap();

        let rx = mock.read(500).unwrap();
        assert_eq!(rx.len(), 1);
        assert_eq!(rx[0].payload(), &[0x62, 0xF1, 0x90, 0x53, 0x41, 0x4A]);
        assert_eq!(rx[0].can_id(), 0x7BB);
        mock.verify();
    }

    #[test]
    fn test_mock_multi_ecu() {
        let mock = MockChannel::new();
        mock.setup_iso15765_filter(0x7B3, 0x7BB).unwrap(); // IMC
        mock.setup_iso15765_filter(0x726, 0x72E).unwrap(); // BCM

        // IMC request
        mock.expect_request(0x7B3, vec![0x22, 0xF1, 0x90], vec![0x62, 0xF1, 0x90, 0x41]);
        // BCM request
        mock.expect_request(
            0x726,
            vec![0x22, 0x40, 0x2A],
            vec![0x62, 0x40, 0x2A, 0x00, 0x7C],
        );

        // Send IMC
        let tx1 = PassThruMsg::new_iso15765(0x7B3, &[0x22, 0xF1, 0x90]);
        mock.send(&tx1, 2000).unwrap();
        let rx1 = mock.read(500).unwrap();
        assert_eq!(rx1[0].can_id(), 0x7BB);
        assert_eq!(rx1[0].payload()[0], 0x62);

        // Send BCM
        let tx2 = PassThruMsg::new_iso15765(0x726, &[0x22, 0x40, 0x2A]);
        mock.send(&tx2, 2000).unwrap();
        let rx2 = mock.read(500).unwrap();
        assert_eq!(rx2[0].can_id(), 0x72E);
        assert_eq!(rx2[0].payload(), &[0x62, 0x40, 0x2A, 0x00, 0x7C]);

        mock.verify();
    }

    #[test]
    fn test_mock_pending_then_ok() {
        let mock = MockChannel::new();
        mock.setup_iso15765_filter(0x7B3, 0x7BB).unwrap();

        mock.expect_request_multi(
            0x7B3,
            vec![0x31, 0x01, 0x60, 0x3E, 0x01],
            vec![
                vec![0x7F, 0x31, 0x78],       // pending
                vec![0x71, 0x01, 0x60, 0x3E], // OK
            ],
        );

        let tx = PassThruMsg::new_iso15765(0x7B3, &[0x31, 0x01, 0x60, 0x3E, 0x01]);
        mock.send(&tx, 2000).unwrap();

        let rx = mock.read(500).unwrap();
        assert_eq!(rx.len(), 2);
        // First: pending
        assert_eq!(rx[0].payload()[0], 0x7F);
        assert_eq!(rx[0].payload()[2], 0x78);
        // Second: actual response
        assert_eq!(rx[1].payload()[0], 0x71);

        mock.verify();
    }

    #[test]
    fn test_mock_timeout() {
        let mock = MockChannel::new();
        mock.set_timeout_mode(true);

        let rx = mock.read(500).unwrap();
        assert!(rx.is_empty());
    }

    #[test]
    fn test_mock_tracks_sent() {
        let mock = MockChannel::new();
        let msg = PassThruMsg::new_iso15765(0x7B3, &[0x3E, 0x00]);
        mock.send(&msg, 2000).unwrap();

        let sent = mock.sent_messages();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].0, 0x7B3);
        assert_eq!(sent[0].1, vec![0x3E, 0x00]);
    }

    #[test]
    fn test_mock_filter_tracking() {
        let mock = MockChannel::new();
        mock.setup_iso15765_filter(0x7B3, 0x7BB).unwrap();
        mock.setup_iso15765_filter(0x726, 0x72E).unwrap();

        let filters = mock.filters();
        assert_eq!(filters.len(), 2);
        assert_eq!(filters[0], (0x7B3, 0x7BB));
        assert_eq!(filters[1], (0x726, 0x72E));
    }

    #[test]
    fn test_channel_send_builds_correct_msg() {
        let mock = MockChannel::new();
        let msg = PassThruMsg::new_iso15765(0x7B3, &[0x22, 0xF1, 0x90]);

        assert_eq!(msg.protocol_id, PROTOCOL_ISO15765);
        assert_eq!(msg.tx_flags, ISO15765_FRAME_PAD);
        assert_eq!(msg.data_size, 7); // 4 header + 3 payload
                                      // Header: 0x000007B3
        assert_eq!(&msg.data[0..4], &[0x00, 0x00, 0x07, 0xB3]);
        // Payload
        assert_eq!(&msg.data[4..7], &[0x22, 0xF1, 0x90]);

        mock.send(&msg, 2000).unwrap();
        let sent = mock.sent_messages();
        assert_eq!(sent[0].1, vec![0x22, 0xF1, 0x90]);
    }

    #[test]
    fn test_channel_recv_parses_msg() {
        let mock = MockChannel::new();
        mock.setup_iso15765_filter(0x7B3, 0x7BB).unwrap();
        mock.expect_request(
            0x7B3,
            vec![0x22, 0xF1, 0x90],
            vec![0x62, 0xF1, 0x90, 0x41, 0x42, 0x43],
        );

        let tx = PassThruMsg::new_iso15765(0x7B3, &[0x22, 0xF1, 0x90]);
        mock.send(&tx, 2000).unwrap();

        let rx = mock.read(500).unwrap();
        assert_eq!(rx.len(), 1);
        // Verify header is stripped correctly via payload()
        let payload = rx[0].payload();
        assert_eq!(payload, &[0x62, 0xF1, 0x90, 0x41, 0x42, 0x43]);
        // Verify CAN ID
        assert_eq!(rx[0].can_id(), 0x7BB);
    }

    #[test]
    fn test_channel_filter_setup() {
        let mock = MockChannel::new();
        let id = mock.setup_iso15765_filter(0x7B3, 0x7BB).unwrap();
        assert_eq!(id, 0);

        let id2 = mock.setup_iso15765_filter(0x726, 0x72E).unwrap();
        assert_eq!(id2, 1);
    }

    #[test]
    fn test_channel_timeout() {
        let mock = MockChannel::new();
        mock.set_timeout_mode(true);

        let rx = mock.read(5000).unwrap();
        assert!(rx.is_empty(), "Expected no messages in timeout mode");
    }
}
