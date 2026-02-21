use std::fmt;

// J2534 Protocol IDs
pub const PROTOCOL_CAN: u32 = 5;
pub const PROTOCOL_ISO15765: u32 = 6;

// J2534 Filter Types
pub const FILTER_PASS: u32 = 1;
pub const FILTER_BLOCK: u32 = 2;
pub const FILTER_FLOW_CONTROL: u32 = 3;

// J2534 Connect Flags
pub const CAN_29BIT_ID: u32 = 0x0100;

// J2534 TxFlags
pub const ISO15765_FRAME_PAD: u32 = 0x0040;

// J2534 IOCTL IDs
pub const GET_CONFIG: u32 = 0x01;
pub const SET_CONFIG: u32 = 0x02;
pub const READ_VBATT: u32 = 0x03;
pub const FIVE_BAUD_INIT: u32 = 0x04;
pub const FAST_INIT: u32 = 0x05;
pub const CLEAR_TX_BUFFER: u32 = 0x07;
pub const CLEAR_RX_BUFFER: u32 = 0x08;
pub const CLEAR_PERIODIC_MSGS: u32 = 0x09;
pub const CLEAR_MSG_FILTERS: u32 = 0x0A;

// Config Parameter IDs
pub const DATA_RATE: u32 = 0x01;
pub const LOOPBACK: u32 = 0x03;
pub const NODE_ADDRESS: u32 = 0x04;
pub const NETWORK_LINE: u32 = 0x05;
pub const P1_MIN: u32 = 0x06;
pub const P1_MAX: u32 = 0x07;
pub const P2_MIN: u32 = 0x08;
pub const P2_MAX: u32 = 0x09;
pub const P3_MIN: u32 = 0x0A;
pub const P3_MAX: u32 = 0x0B;
pub const P4_MIN: u32 = 0x0C;
pub const P4_MAX: u32 = 0x0D;
pub const ISO15765_BS: u32 = 0x1E;
pub const ISO15765_STMIN: u32 = 0x1F;
pub const ISO15765_WFT_MAX: u32 = 0x24;

pub const MAX_DATA_SIZE: usize = 4128;

/// PASSTHRU_MSG structure matching the J2534 API spec
#[repr(C)]
#[derive(Clone)]
pub struct PassThruMsg {
    pub protocol_id: u32,
    pub rx_status: u32,
    pub tx_flags: u32,
    pub timestamp: u32,
    pub data_size: u32,
    pub extra_data_index: u32,
    pub data: [u8; MAX_DATA_SIZE],
}

impl Default for PassThruMsg {
    fn default() -> Self {
        Self {
            protocol_id: 0,
            rx_status: 0,
            tx_flags: 0,
            timestamp: 0,
            data_size: 0,
            extra_data_index: 0,
            data: [0u8; MAX_DATA_SIZE],
        }
    }
}

impl PassThruMsg {
    pub fn new_iso15765(tx_id: u32, payload: &[u8]) -> Self {
        let mut msg = Self {
            protocol_id: PROTOCOL_ISO15765,
            tx_flags: ISO15765_FRAME_PAD,
            data_size: (4 + payload.len()) as u32,
            ..Default::default()
        };
        // Set the 4-byte CAN ID header (big-endian)
        msg.data[0] = ((tx_id >> 24) & 0xFF) as u8;
        msg.data[1] = ((tx_id >> 16) & 0xFF) as u8;
        msg.data[2] = ((tx_id >> 8) & 0xFF) as u8;
        msg.data[3] = (tx_id & 0xFF) as u8;
        // Copy payload after header
        msg.data[4..4 + payload.len()].copy_from_slice(payload);
        msg
    }

    pub fn payload(&self) -> &[u8] {
        if self.data_size > 4 {
            &self.data[4..self.data_size as usize]
        } else {
            &[]
        }
    }

    pub fn can_id(&self) -> u32 {
        ((self.data[0] as u32) << 24)
            | ((self.data[1] as u32) << 16)
            | ((self.data[2] as u32) << 8)
            | (self.data[3] as u32)
    }
}

impl fmt::Debug for PassThruMsg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PassThruMsg")
            .field("protocol_id", &self.protocol_id)
            .field("data_size", &self.data_size)
            .field(
                "data",
                &format_args!(
                    "[{}]",
                    self.data[..self.data_size as usize]
                        .iter()
                        .map(|b| format!("{:02X}", b))
                        .collect::<Vec<_>>()
                        .join(" ")
                ),
            )
            .finish()
    }
}

/// J2534 error codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum J2534Error {
    NoError = 0x00,
    NotSupported = 0x01,
    InvalidChannelId = 0x02,
    InvalidProtocolId = 0x03,
    NullParameter = 0x04,
    InvalidIoctlValue = 0x05,
    InvalidFlags = 0x06,
    Failed = 0x07,
    DeviceNotConnected = 0x08,
    Timeout = 0x09,
    InvalidMsg = 0x0A,
    InvalidTimeInterval = 0x0B,
    ExceededLimit = 0x0C,
    InvalidMsgId = 0x0D,
    DeviceInUse = 0x0E,
    InvalidIoctlId = 0x0F,
    BufferEmpty = 0x10,
    BufferFull = 0x11,
    BufferOverflow = 0x12,
    PinInvalid = 0x13,
    ChannelInUse = 0x14,
    MsgProtocolId = 0x15,
    InvalidFilterId = 0x16,
    NoFlowControl = 0x17,
    NotUnique = 0x18,
    InvalidBaudrate = 0x19,
    InvalidDeviceId = 0x1A,
}

impl J2534Error {
    pub fn from_code(code: u32) -> Self {
        match code {
            0x00 => Self::NoError,
            0x01 => Self::NotSupported,
            0x02 => Self::InvalidChannelId,
            0x03 => Self::InvalidProtocolId,
            0x04 => Self::NullParameter,
            0x05 => Self::InvalidIoctlValue,
            0x06 => Self::InvalidFlags,
            0x07 => Self::Failed,
            0x08 => Self::DeviceNotConnected,
            0x09 => Self::Timeout,
            0x0A => Self::InvalidMsg,
            0x0B => Self::InvalidTimeInterval,
            0x0C => Self::ExceededLimit,
            0x0D => Self::InvalidMsgId,
            0x0E => Self::DeviceInUse,
            0x0F => Self::InvalidIoctlId,
            0x10 => Self::BufferEmpty,
            0x11 => Self::BufferFull,
            0x12 => Self::BufferOverflow,
            0x13 => Self::PinInvalid,
            0x14 => Self::ChannelInUse,
            0x15 => Self::MsgProtocolId,
            0x16 => Self::InvalidFilterId,
            0x17 => Self::NoFlowControl,
            0x18 => Self::NotUnique,
            0x19 => Self::InvalidBaudrate,
            0x1A => Self::InvalidDeviceId,
            _ => Self::Failed,
        }
    }
}

impl fmt::Display for J2534Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoError => write!(f, "No error"),
            Self::NotSupported => write!(f, "Not supported"),
            Self::InvalidChannelId => write!(f, "Invalid channel ID"),
            Self::InvalidProtocolId => write!(f, "Invalid protocol ID"),
            Self::NullParameter => write!(f, "Null parameter"),
            Self::InvalidIoctlValue => write!(f, "Invalid IOCTL value"),
            Self::InvalidFlags => write!(f, "Invalid flags"),
            Self::Failed => write!(f, "Failed"),
            Self::DeviceNotConnected => write!(f, "Device not connected"),
            Self::Timeout => write!(f, "Timeout"),
            Self::InvalidMsg => write!(f, "Invalid message"),
            Self::InvalidTimeInterval => write!(f, "Invalid time interval"),
            Self::ExceededLimit => write!(f, "Exceeded limit"),
            Self::InvalidMsgId => write!(f, "Invalid message ID"),
            Self::DeviceInUse => write!(f, "Device in use"),
            Self::InvalidIoctlId => write!(f, "Invalid IOCTL ID"),
            Self::BufferEmpty => write!(f, "Buffer empty"),
            Self::BufferFull => write!(f, "Buffer full"),
            Self::BufferOverflow => write!(f, "Buffer overflow"),
            Self::PinInvalid => write!(f, "Pin invalid"),
            Self::ChannelInUse => write!(f, "Channel in use"),
            Self::MsgProtocolId => write!(f, "Message protocol ID mismatch"),
            Self::InvalidFilterId => write!(f, "Invalid filter ID"),
            Self::NoFlowControl => write!(f, "No flow control"),
            Self::NotUnique => write!(f, "Not unique"),
            Self::InvalidBaudrate => write!(f, "Invalid baudrate"),
            Self::InvalidDeviceId => write!(f, "Invalid device ID"),
        }
    }
}

impl std::error::Error for J2534Error {}

/// SCONFIG structure for IOCTL
#[repr(C)]
pub struct SConfig {
    pub parameter: u32,
    pub value: u32,
}

/// SCONFIG_LIST structure for IOCTL
#[repr(C)]
pub struct SConfigList {
    pub num_of_params: u32,
    pub config_ptr: *mut SConfig,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;

    #[test]
    fn test_passthru_msg_size() {
        // 6 u32 fields (24 bytes) + 4128 byte data array = 4152
        assert_eq!(mem::size_of::<PassThruMsg>(), 4152);
    }

    #[test]
    fn test_passthru_msg_data_offset() {
        // Data field should be at offset 24 (6 * 4 bytes)
        assert_eq!(mem::offset_of!(PassThruMsg, data), 24);
    }

    #[test]
    fn test_error_code_display() {
        assert_eq!(J2534Error::NoError.to_string(), "No error");
        assert_eq!(J2534Error::Timeout.to_string(), "Timeout");
        assert_eq!(
            J2534Error::DeviceNotConnected.to_string(),
            "Device not connected"
        );
        assert_eq!(J2534Error::BufferEmpty.to_string(), "Buffer empty");
        assert_eq!(J2534Error::InvalidBaudrate.to_string(), "Invalid baudrate");
    }

    #[test]
    fn test_error_code_from_code() {
        assert_eq!(J2534Error::from_code(0x00), J2534Error::NoError);
        assert_eq!(J2534Error::from_code(0x09), J2534Error::Timeout);
        assert_eq!(J2534Error::from_code(0x08), J2534Error::DeviceNotConnected);
        // Unknown code falls back to Failed
        assert_eq!(J2534Error::from_code(0xFF), J2534Error::Failed);
    }

    #[test]
    fn test_protocol_id_values() {
        assert_eq!(PROTOCOL_CAN, 5);
        assert_eq!(PROTOCOL_ISO15765, 6);
    }

    #[test]
    fn test_filter_type_values() {
        assert_eq!(FILTER_PASS, 1);
        assert_eq!(FILTER_BLOCK, 2);
        assert_eq!(FILTER_FLOW_CONTROL, 3);
    }

    #[test]
    fn test_ioctl_id_values() {
        assert_eq!(GET_CONFIG, 0x01);
        assert_eq!(SET_CONFIG, 0x02);
        assert_eq!(CLEAR_TX_BUFFER, 0x07);
        assert_eq!(CLEAR_RX_BUFFER, 0x08);
        assert_eq!(CLEAR_MSG_FILTERS, 0x0A);
    }

    #[test]
    fn test_passthru_msg_new_iso15765() {
        let msg = PassThruMsg::new_iso15765(0x7B3, &[0x22, 0xF1, 0x90]);
        assert_eq!(msg.protocol_id, PROTOCOL_ISO15765);
        assert_eq!(msg.tx_flags, ISO15765_FRAME_PAD);
        assert_eq!(msg.data_size, 7); // 4 header + 3 payload
        assert_eq!(msg.data[0], 0x00);
        assert_eq!(msg.data[1], 0x00);
        assert_eq!(msg.data[2], 0x07);
        assert_eq!(msg.data[3], 0xB3);
        assert_eq!(msg.data[4], 0x22);
        assert_eq!(msg.data[5], 0xF1);
        assert_eq!(msg.data[6], 0x90);
    }

    #[test]
    fn test_passthru_msg_payload() {
        let msg = PassThruMsg::new_iso15765(0x7B3, &[0x22, 0xF1, 0x90]);
        assert_eq!(msg.payload(), &[0x22, 0xF1, 0x90]);
    }

    #[test]
    fn test_passthru_msg_can_id() {
        let msg = PassThruMsg::new_iso15765(0x7B3, &[0x22, 0xF1, 0x90]);
        assert_eq!(msg.can_id(), 0x7B3);
    }

    #[test]
    fn test_passthru_msg_empty_payload() {
        let mut msg = PassThruMsg::default();
        msg.data_size = 4;
        assert_eq!(msg.payload(), &[] as &[u8]);
    }
}
