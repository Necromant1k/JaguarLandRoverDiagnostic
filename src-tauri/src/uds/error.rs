use std::fmt;

/// UDS Negative Response Codes (ISO 14229)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NegativeResponseCode {
    GeneralReject,                             // 0x10
    ServiceNotSupported,                       // 0x11
    SubFunctionNotSupported,                   // 0x12
    IncorrectMessageLengthOrInvalidFormat,     // 0x13
    ResponseTooLong,                           // 0x14
    BusyRepeatRequest,                         // 0x21
    ConditionsNotCorrect,                      // 0x22
    RequestSequenceError,                      // 0x24
    NoResponseFromSubnetComponent,             // 0x25
    FailurePreventsExecutionOfRequestedAction, // 0x26
    RequestOutOfRange,                         // 0x31
    SecurityAccessDenied,                      // 0x33
    AuthenticationRequired,                    // 0x34
    InvalidKey,                                // 0x35
    ExceededNumberOfAttempts,                  // 0x36
    RequiredTimeDelayNotExpired,               // 0x37
    SecureDataTransmissionRequired,            // 0x38
    SecureDataTransmissionNotAllowed,          // 0x39
    SecureDataVerificationFailed,              // 0x3A
    UploadDownloadNotAccepted,                 // 0x70
    TransferDataSuspended,                     // 0x71
    GeneralProgrammingFailure,                 // 0x72
    WrongBlockSequenceCounter,                 // 0x73
    RequestCorrectlyReceivedResponsePending,   // 0x78
    SubFunctionNotSupportedInActiveSession,    // 0x7E
    ServiceNotSupportedInActiveSession,        // 0x7F
    Unknown(u8),
}

impl NegativeResponseCode {
    pub fn from_byte(byte: u8) -> Self {
        match byte {
            0x10 => Self::GeneralReject,
            0x11 => Self::ServiceNotSupported,
            0x12 => Self::SubFunctionNotSupported,
            0x13 => Self::IncorrectMessageLengthOrInvalidFormat,
            0x14 => Self::ResponseTooLong,
            0x21 => Self::BusyRepeatRequest,
            0x22 => Self::ConditionsNotCorrect,
            0x24 => Self::RequestSequenceError,
            0x25 => Self::NoResponseFromSubnetComponent,
            0x26 => Self::FailurePreventsExecutionOfRequestedAction,
            0x31 => Self::RequestOutOfRange,
            0x33 => Self::SecurityAccessDenied,
            0x34 => Self::AuthenticationRequired,
            0x35 => Self::InvalidKey,
            0x36 => Self::ExceededNumberOfAttempts,
            0x37 => Self::RequiredTimeDelayNotExpired,
            0x38 => Self::SecureDataTransmissionRequired,
            0x39 => Self::SecureDataTransmissionNotAllowed,
            0x3A => Self::SecureDataVerificationFailed,
            0x70 => Self::UploadDownloadNotAccepted,
            0x71 => Self::TransferDataSuspended,
            0x72 => Self::GeneralProgrammingFailure,
            0x73 => Self::WrongBlockSequenceCounter,
            0x78 => Self::RequestCorrectlyReceivedResponsePending,
            0x7E => Self::SubFunctionNotSupportedInActiveSession,
            0x7F => Self::ServiceNotSupportedInActiveSession,
            other => Self::Unknown(other),
        }
    }

    pub fn to_byte(&self) -> u8 {
        match self {
            Self::GeneralReject => 0x10,
            Self::ServiceNotSupported => 0x11,
            Self::SubFunctionNotSupported => 0x12,
            Self::IncorrectMessageLengthOrInvalidFormat => 0x13,
            Self::ResponseTooLong => 0x14,
            Self::BusyRepeatRequest => 0x21,
            Self::ConditionsNotCorrect => 0x22,
            Self::RequestSequenceError => 0x24,
            Self::NoResponseFromSubnetComponent => 0x25,
            Self::FailurePreventsExecutionOfRequestedAction => 0x26,
            Self::RequestOutOfRange => 0x31,
            Self::SecurityAccessDenied => 0x33,
            Self::AuthenticationRequired => 0x34,
            Self::InvalidKey => 0x35,
            Self::ExceededNumberOfAttempts => 0x36,
            Self::RequiredTimeDelayNotExpired => 0x37,
            Self::SecureDataTransmissionRequired => 0x38,
            Self::SecureDataTransmissionNotAllowed => 0x39,
            Self::SecureDataVerificationFailed => 0x3A,
            Self::UploadDownloadNotAccepted => 0x70,
            Self::TransferDataSuspended => 0x71,
            Self::GeneralProgrammingFailure => 0x72,
            Self::WrongBlockSequenceCounter => 0x73,
            Self::RequestCorrectlyReceivedResponsePending => 0x78,
            Self::SubFunctionNotSupportedInActiveSession => 0x7E,
            Self::ServiceNotSupportedInActiveSession => 0x7F,
            Self::Unknown(code) => *code,
        }
    }

    pub fn is_pending(&self) -> bool {
        matches!(self, Self::RequestCorrectlyReceivedResponsePending)
    }
}

impl fmt::Display for NegativeResponseCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GeneralReject => write!(f, "General reject (0x10)"),
            Self::ServiceNotSupported => write!(f, "Service not supported (0x11)"),
            Self::SubFunctionNotSupported => write!(f, "Sub-function not supported (0x12)"),
            Self::IncorrectMessageLengthOrInvalidFormat => {
                write!(f, "Incorrect message length or invalid format (0x13)")
            }
            Self::ResponseTooLong => write!(f, "Response too long (0x14)"),
            Self::BusyRepeatRequest => write!(f, "Busy - repeat request (0x21)"),
            Self::ConditionsNotCorrect => write!(f, "Conditions not correct (0x22)"),
            Self::RequestSequenceError => write!(f, "Request sequence error (0x24)"),
            Self::NoResponseFromSubnetComponent => {
                write!(f, "No response from subnet component (0x25)")
            }
            Self::FailurePreventsExecutionOfRequestedAction => {
                write!(f, "Failure prevents execution of requested action (0x26)")
            }
            Self::RequestOutOfRange => write!(f, "Request out of range (0x31)"),
            Self::SecurityAccessDenied => write!(f, "Security access denied (0x33)"),
            Self::AuthenticationRequired => write!(f, "Authentication required (0x34)"),
            Self::InvalidKey => write!(f, "Invalid key (0x35)"),
            Self::ExceededNumberOfAttempts => write!(f, "Exceeded number of attempts (0x36)"),
            Self::RequiredTimeDelayNotExpired => {
                write!(f, "Required time delay not expired (0x37)")
            }
            Self::SecureDataTransmissionRequired => {
                write!(f, "Secure data transmission required (0x38)")
            }
            Self::SecureDataTransmissionNotAllowed => {
                write!(f, "Secure data transmission not allowed (0x39)")
            }
            Self::SecureDataVerificationFailed => {
                write!(f, "Secure data verification failed (0x3A)")
            }
            Self::UploadDownloadNotAccepted => write!(f, "Upload/download not accepted (0x70)"),
            Self::TransferDataSuspended => write!(f, "Transfer data suspended (0x71)"),
            Self::GeneralProgrammingFailure => write!(f, "General programming failure (0x72)"),
            Self::WrongBlockSequenceCounter => write!(f, "Wrong block sequence counter (0x73)"),
            Self::RequestCorrectlyReceivedResponsePending => {
                write!(f, "Request correctly received - response pending (0x78)")
            }
            Self::SubFunctionNotSupportedInActiveSession => {
                write!(f, "Sub-function not supported in active session (0x7E)")
            }
            Self::ServiceNotSupportedInActiveSession => {
                write!(f, "Service not supported in active session (0x7F)")
            }
            Self::Unknown(code) => write!(f, "Unknown NRC (0x{:02X})", code),
        }
    }
}

/// UDS error type combining NRC and transport errors
#[derive(Debug, Clone)]
pub enum UdsError {
    NegativeResponse {
        service_id: u8,
        nrc: NegativeResponseCode,
    },
    Timeout,
    InvalidResponse(String),
    TransportError(String),
    NotConnected,
    SecurityError(String),
}

impl fmt::Display for UdsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NegativeResponse { service_id, nrc } => {
                write!(
                    f,
                    "Negative response for service 0x{:02X}: {}",
                    service_id, nrc
                )
            }
            Self::Timeout => write!(f, "Response timeout"),
            Self::InvalidResponse(msg) => write!(f, "Invalid response: {}", msg),
            Self::TransportError(msg) => write!(f, "Transport error: {}", msg),
            Self::NotConnected => write!(f, "Not connected to ECU"),
            Self::SecurityError(msg) => write!(f, "Security error: {}", msg),
        }
    }
}

impl std::error::Error for UdsError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nrc_from_byte_all_codes() {
        assert_eq!(
            NegativeResponseCode::from_byte(0x10),
            NegativeResponseCode::GeneralReject
        );
        assert_eq!(
            NegativeResponseCode::from_byte(0x11),
            NegativeResponseCode::ServiceNotSupported
        );
        assert_eq!(
            NegativeResponseCode::from_byte(0x12),
            NegativeResponseCode::SubFunctionNotSupported
        );
        assert_eq!(
            NegativeResponseCode::from_byte(0x13),
            NegativeResponseCode::IncorrectMessageLengthOrInvalidFormat
        );
        assert_eq!(
            NegativeResponseCode::from_byte(0x14),
            NegativeResponseCode::ResponseTooLong
        );
        assert_eq!(
            NegativeResponseCode::from_byte(0x21),
            NegativeResponseCode::BusyRepeatRequest
        );
        assert_eq!(
            NegativeResponseCode::from_byte(0x22),
            NegativeResponseCode::ConditionsNotCorrect
        );
        assert_eq!(
            NegativeResponseCode::from_byte(0x24),
            NegativeResponseCode::RequestSequenceError
        );
        assert_eq!(
            NegativeResponseCode::from_byte(0x31),
            NegativeResponseCode::RequestOutOfRange
        );
        assert_eq!(
            NegativeResponseCode::from_byte(0x33),
            NegativeResponseCode::SecurityAccessDenied
        );
        assert_eq!(
            NegativeResponseCode::from_byte(0x35),
            NegativeResponseCode::InvalidKey
        );
        assert_eq!(
            NegativeResponseCode::from_byte(0x36),
            NegativeResponseCode::ExceededNumberOfAttempts
        );
        assert_eq!(
            NegativeResponseCode::from_byte(0x37),
            NegativeResponseCode::RequiredTimeDelayNotExpired
        );
        assert_eq!(
            NegativeResponseCode::from_byte(0x78),
            NegativeResponseCode::RequestCorrectlyReceivedResponsePending
        );
        assert_eq!(
            NegativeResponseCode::from_byte(0x7E),
            NegativeResponseCode::SubFunctionNotSupportedInActiveSession
        );
        assert_eq!(
            NegativeResponseCode::from_byte(0x7F),
            NegativeResponseCode::ServiceNotSupportedInActiveSession
        );
    }

    #[test]
    fn test_nrc_display_readable() {
        assert_eq!(
            NegativeResponseCode::GeneralReject.to_string(),
            "General reject (0x10)"
        );
        assert_eq!(
            NegativeResponseCode::SecurityAccessDenied.to_string(),
            "Security access denied (0x33)"
        );
        assert_eq!(
            NegativeResponseCode::InvalidKey.to_string(),
            "Invalid key (0x35)"
        );
        assert_eq!(
            NegativeResponseCode::RequestCorrectlyReceivedResponsePending.to_string(),
            "Request correctly received - response pending (0x78)"
        );
    }

    #[test]
    fn test_nrc_unknown_code() {
        let nrc = NegativeResponseCode::from_byte(0xAA);
        assert_eq!(nrc, NegativeResponseCode::Unknown(0xAA));
        assert_eq!(nrc.to_string(), "Unknown NRC (0xAA)");
        assert_eq!(nrc.to_byte(), 0xAA);
    }

    #[test]
    fn test_nrc_roundtrip() {
        let codes: Vec<u8> = vec![
            0x10, 0x11, 0x12, 0x13, 0x14, 0x21, 0x22, 0x24, 0x25, 0x26, 0x31, 0x33, 0x34, 0x35,
            0x36, 0x37, 0x38, 0x39, 0x3A, 0x70, 0x71, 0x72, 0x73, 0x78, 0x7E, 0x7F,
        ];
        for code in codes {
            let nrc = NegativeResponseCode::from_byte(code);
            assert_eq!(nrc.to_byte(), code, "Roundtrip failed for 0x{:02X}", code);
        }
    }

    #[test]
    fn test_nrc_is_pending() {
        assert!(NegativeResponseCode::RequestCorrectlyReceivedResponsePending.is_pending());
        assert!(!NegativeResponseCode::GeneralReject.is_pending());
        assert!(!NegativeResponseCode::SecurityAccessDenied.is_pending());
    }

    #[test]
    fn test_uds_error_display() {
        let err = UdsError::NegativeResponse {
            service_id: 0x22,
            nrc: NegativeResponseCode::RequestOutOfRange,
        };
        assert_eq!(
            err.to_string(),
            "Negative response for service 0x22: Request out of range (0x31)"
        );

        assert_eq!(UdsError::Timeout.to_string(), "Response timeout");
        assert_eq!(UdsError::NotConnected.to_string(), "Not connected to ECU");
    }
}
