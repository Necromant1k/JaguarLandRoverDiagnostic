use crate::j2534::Channel;
use crate::uds::client::UdsClient;
use crate::uds::error::UdsError;
use crate::uds::keygen;

/// ECU CAN addresses for JLR X260
pub mod ecu_addr {
    pub const IMC_TX: u32 = 0x7B3;
    pub const IMC_RX: u32 = 0x7BB;
    pub const GWM_TX: u32 = 0x716;
    pub const GWM_RX: u32 = 0x71E;
    pub const BCM_TX: u32 = 0x726;
    pub const BCM_RX: u32 = 0x72E;
    pub const IPC_TX: u32 = 0x720;
    pub const IPC_RX: u32 = 0x728;
}

/// Diagnostic session types
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum DiagSession {
    Default = 0x01,
    Programming = 0x02,
    Extended = 0x03,
}

/// Known DID identifiers
pub mod did {
    pub const VIN: u16 = 0xF190;
    pub const ASSEMBLY_PART: u16 = 0xF187;   // Assembly/Spare Part Number
    pub const HARDWARE_PART: u16 = 0xF191;   // ECU Hardware Number
    pub const MASTER_RPM_PART: u16 = 0xF188;
    pub const V850_PART: u16 = 0xF120;
    pub const TUNER_PART: u16 = 0xF121;
    pub const POLAR_PART: u16 = 0xF1A5;
    pub const PBL_PART: u16 = 0xF180;
    pub const ECU_SERIAL: u16 = 0xF18C;
    pub const ECU_SERIAL2: u16 = 0xF113;
    pub const ACTIVE_DIAG_SESSION: u16 = 0xD100;
    pub const IMC_STATUS: u16 = 0x0202;
    pub const BATTERY_VOLTAGE: u16 = 0x402A;
    pub const BATTERY_SOC: u16 = 0x4028;
    pub const BATTERY_TEMP: u16 = 0x4029;
}

/// Known routine IDs
pub mod routine {
    pub const CONFIGURE_LINUX: u16 = 0x6038;
    pub const ENG_SCREEN_LVL2: u16 = 0x603D;
    pub const SSH_ENABLE: u16 = 0x603E;
    pub const DVD_RECOVER: u16 = 0x603F;
    pub const FAN_CONTROL: u16 = 0x6041;
    pub const RESET_PIN: u16 = 0x6042;
    pub const POWER_OVERRIDE: u16 = 0x6043;
    pub const GEN_KEY: u16 = 0x6045;
    pub const SHARED_SECRET: u16 = 0x6046;
    pub const VIN_LEARN: u16 = 0x0404;
    pub const RETRIEVE_CCF: u16 = 0x0E00;
    pub const REPORT_CCF: u16 = 0x0E01;
    pub const LIST_CCF: u16 = 0x0E02;
}

/// Routine control sub-functions
pub const ROUTINE_START: u8 = 0x01;
pub const ROUTINE_STOP: u8 = 0x02;
pub const ROUTINE_RESULTS: u8 = 0x03;

// ─── Diagnostic Session Control (0x10) ──────────────────────────────

pub fn diagnostic_session<C: Channel>(client: &UdsClient<C>, session: DiagSession) -> Result<Vec<u8>, UdsError> {
    let request = vec![0x10, session as u8];
    client.send_recv(&request, 2000, false)
}

// ─── TesterPresent (0x3E) ───────────────────────────────────────────

pub fn tester_present<C: Channel>(client: &UdsClient<C>) -> Result<Vec<u8>, UdsError> {
    let request = vec![0x3E, 0x00];
    client.send_recv(&request, 2000, false)
}

pub fn tester_present_no_response<C: Channel>(client: &UdsClient<C>) -> Result<(), UdsError> {
    let request = vec![0x3E, 0x80];
    client.send_no_response(&request)
}

// ─── ReadDataByIdentifier (0x22) ────────────────────────────────────

pub fn read_did<C: Channel>(client: &UdsClient<C>, did_id: u16) -> Result<Vec<u8>, UdsError> {
    let request = vec![0x22, (did_id >> 8) as u8, (did_id & 0xFF) as u8];
    let response = client.send_recv(&request, 2000, false)?;
    // Response: 0x62 DID_HI DID_LO DATA...
    if response.len() < 3 {
        return Err(UdsError::InvalidResponse("ReadDID response too short".into()));
    }
    Ok(response)
}

/// Read a DID and return just the data portion (after service ID + DID bytes)
pub fn read_did_data<C: Channel>(client: &UdsClient<C>, did_id: u16) -> Result<Vec<u8>, UdsError> {
    let response = read_did(client, did_id)?;
    Ok(response[3..].to_vec())
}

/// Read VIN from IMC
pub fn read_vin<C: Channel>(client: &UdsClient<C>) -> Result<String, UdsError> {
    let data = read_did_data(client, did::VIN)?;
    Ok(String::from_utf8_lossy(&data).trim().to_string())
}

/// Read a part number DID and return as string
pub fn read_part_number<C: Channel>(client: &UdsClient<C>, did_id: u16) -> Result<String, UdsError> {
    let data = read_did_data(client, did_id)?;
    Ok(String::from_utf8_lossy(&data).trim().to_string())
}

/// Read battery voltage from BCM (DID 402A)
/// Returns voltage as f32 — raw bytes * 0.1
pub fn read_battery_voltage<C: Channel>(client: &UdsClient<C>) -> Result<f32, UdsError> {
    let data = read_did_data(client, did::BATTERY_VOLTAGE)?;
    if data.is_empty() {
        return Err(UdsError::InvalidResponse("Empty voltage data".into()));
    }
    // Voltage is typically 2 bytes, value * 0.1 = volts
    let raw = if data.len() >= 2 {
        ((data[0] as u16) << 8 | data[1] as u16) as f32
    } else {
        data[0] as f32
    };
    Ok(raw * 0.1)
}

// ─── SecurityAccess (0x27) ──────────────────────────────────────────

/// Request security seed from ECU
pub fn security_request_seed<C: Channel>(client: &UdsClient<C>, level: u8) -> Result<Vec<u8>, UdsError> {
    let request = vec![0x27, level];
    let response = client.send_recv(&request, 2000, false)?;
    // Response: 0x67 LEVEL SEED[0..N]
    if response.len() < 2 {
        return Err(UdsError::InvalidResponse(
            "SecurityAccess seed response too short".into(),
        ));
    }
    Ok(response)
}

/// Send security key to ECU
pub fn security_send_key<C: Channel>(client: &UdsClient<C>, level: u8, key: &[u8]) -> Result<Vec<u8>, UdsError> {
    let mut request = vec![0x27, level];
    request.extend_from_slice(key);
    client.send_recv(&request, 2000, false)
}

/// Full security access flow: request seed → compute key → send key
/// Returns Ok(true) if unlocked, Ok(false) if already unlocked (zero seed)
pub fn security_access<C: Channel>(
    client: &UdsClient<C>,
    seed_level: u8,
    key_level: u8,
    constants: &[u8; 5],
) -> Result<bool, UdsError> {
    // Request seed
    let seed_response = security_request_seed(client, seed_level)?;
    // Response: 0x67 LEVEL SEED[0] SEED[1] SEED[2]
    if seed_response.len() < 5 {
        return Err(UdsError::InvalidResponse(
            "Seed response too short, expected at least 5 bytes".into(),
        ));
    }

    let seed = &seed_response[2..5];
    let seed_int = ((seed[0] as u32) << 16) | ((seed[1] as u32) << 8) | (seed[2] as u32);

    // Zero seed means already unlocked
    if seed_int == 0 {
        return Ok(false);
    }

    // Compute key using KeyGenMkI
    let key_int = keygen::keygen_mki(seed_int, constants);
    let key_bytes = [
        ((key_int >> 16) & 0xFF) as u8,
        ((key_int >> 8) & 0xFF) as u8,
        (key_int & 0xFF) as u8,
    ];

    // Send key
    security_send_key(client, key_level, &key_bytes)?;
    Ok(true)
}

// ─── RoutineControl (0x31) ──────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct RoutineResult {
    pub routine_id: u16,
    pub status: Option<u8>,
    pub result: Option<u8>,
    pub error: Option<u8>,
    pub raw_data: Vec<u8>,
    pub description: String,
}

pub fn routine_control<C: Channel>(
    client: &UdsClient<C>,
    sub_function: u8,
    routine_id: u16,
    data: &[u8],
    wait_pending: bool,
) -> Result<RoutineResult, UdsError> {
    let mut request = vec![
        0x31,
        sub_function,
        (routine_id >> 8) as u8,
        (routine_id & 0xFF) as u8,
    ];
    request.extend_from_slice(data);

    let response = client.send_recv(&request, 5000, wait_pending)?;
    // Response: 0x71 SUB RID_HI RID_LO [STATUS] [RESULT] [ERROR]
    let raw_data = if response.len() > 4 {
        response[4..].to_vec()
    } else {
        vec![]
    };

    let status = raw_data.first().copied();
    let result = raw_data.get(1).copied();
    let error = raw_data.get(2).copied();

    let description = describe_routine_result(routine_id, status, result, error);

    Ok(RoutineResult {
        routine_id,
        status,
        result,
        error,
        raw_data,
        description,
    })
}

/// Start a routine
pub fn routine_start<C: Channel>(
    client: &UdsClient<C>,
    routine_id: u16,
    data: &[u8],
    wait_pending: bool,
) -> Result<RoutineResult, UdsError> {
    routine_control(client, ROUTINE_START, routine_id, data, wait_pending)
}

// ─── ECUReset (0x11) ────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub enum ResetType {
    HardReset = 0x01,
    KeyOffOnReset = 0x02,
    SoftReset = 0x03,
}

pub fn ecu_reset<C: Channel>(client: &UdsClient<C>, reset_type: ResetType) -> Result<Vec<u8>, UdsError> {
    let request = vec![0x11, reset_type as u8];
    client.send_recv(&request, 2000, false)
}

// ─── Routine 0x6038 decode ──────────────────────────────────────────

fn describe_routine_result(routine_id: u16, status: Option<u8>, result: Option<u8>, error: Option<u8>) -> String {
    match routine_id {
        0x6038 => describe_6038(status, result, error),
        0x0404 => describe_0404(status),
        _ => {
            let mut parts = vec![];
            if let Some(s) = status {
                parts.push(format!("status=0x{:02X}", s));
            }
            if let Some(r) = result {
                parts.push(format!("result=0x{:02X}", r));
            }
            if let Some(e) = error {
                parts.push(format!("error=0x{:02X}", e));
            }
            if parts.is_empty() {
                "OK".to_string()
            } else {
                parts.join(", ")
            }
        }
    }
}

fn describe_6038(status: Option<u8>, result: Option<u8>, error: Option<u8>) -> String {
    let mut parts = vec![];

    if let Some(s) = status {
        let status_str = match s {
            0x10 => "Routine finished",
            0x20 => "Routine completed",
            0x21 => "Routine aborted",
            0x22 => "Routine active",
            _ => "Unknown status",
        };
        parts.push(format!("Status: {} (0x{:02X})", status_str, s));
    }

    if let Some(r) = result {
        let result_str = match r {
            0x01 => "Completed",
            0x02 => "In progress",
            0x03 => "Failed to configure",
            _ => "Unknown result",
        };
        parts.push(format!("Result: {} (0x{:02X})", result_str, r));
    }

    if let Some(e) = error {
        let mut errors = vec![];
        if e & 0x01 != 0 { errors.push("Boot parameter"); }
        if e & 0x02 != 0 { errors.push("Symlinks"); }
        if e & 0x04 != 0 { errors.push("Start-up configuration XML"); }
        if e & 0x08 != 0 { errors.push("Manifest symlinks"); }
        if e & 0x10 != 0 { errors.push("DVD region"); }
        if e & 0x20 != 0 { errors.push("Polar switch"); }
        if e & 0x40 != 0 { errors.push("Gracenotes"); }
        if e & 0x80 != 0 { errors.push("Application manager LCF"); }
        if !errors.is_empty() {
            parts.push(format!("Errors: {}", errors.join(", ")));
        } else if e == 0 {
            parts.push("No errors".to_string());
        }
    }

    if parts.is_empty() {
        "OK".to_string()
    } else {
        parts.join(" | ")
    }
}

fn describe_0404(status: Option<u8>) -> String {
    if let Some(s) = status {
        let status_str = match s {
            0x20 => "Routine completed",
            0x21 => "Routine aborted",
            0x22 => "Routine active",
            _ => "Unknown status",
        };
        format!("{} (0x{:02X})", status_str, s)
    } else {
        "OK".to_string()
    }
}

// ─── SDD Prerequisite Flow ───────────────────────────────────────────

/// Execute SDD prerequisite flow: TesterPresent → Extended Session → Security Access (if needed)
/// This is the standard JLR SDD sequence required before executing secured routines.
pub fn sdd_prerequisite_flow<C: Channel>(
    client: &UdsClient<C>,
    needs_security: bool,
) -> Result<(), UdsError> {
    // 1. Wake up with TesterPresent
    tester_present(client)?;

    // 2. Switch to extended diagnostic session
    diagnostic_session(client, DiagSession::Extended)?;

    // 3. Security access (if required)
    if needs_security {
        security_access(client, 0x11, 0x12, &keygen::DC0314_CONSTANTS)?;
    }

    Ok(())
}

/// Execute a full routine with SDD prerequisite flow.
/// TesterPresent → Extended Session → Security Access (if needed) → RoutineControl Start
pub fn execute_routine_sdd<C: Channel>(
    client: &UdsClient<C>,
    routine_id: u16,
    data: &[u8],
    needs_security: bool,
    needs_pending: bool,
) -> Result<RoutineResult, UdsError> {
    sdd_prerequisite_flow(client, needs_security)?;
    routine_start(client, routine_id, data, needs_pending)
}

// ─── SSH Enable flow ────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct SshResult {
    pub success: bool,
    pub ip_address: String,
    pub message: String,
}

/// Full SSH enable flow: TesterPresent → ExtendedSession → SecurityAccess → Routine 0x603E
pub fn enable_ssh<C: Channel>(client: &UdsClient<C>) -> Result<SshResult, UdsError> {
    let result = execute_routine_sdd(client, routine::SSH_ENABLE, &[0x01], true, true)?;

    Ok(SshResult {
        success: true,
        ip_address: "192.168.103.11".to_string(),
        message: format!("SSH ENABLED — Connect: root@192.168.103.11 | {}", result.description),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::j2534::mock::MockChannel;
    use crate::uds::error::NegativeResponseCode;

    fn make_imc_client(mock: MockChannel) -> UdsClient<MockChannel> {
        mock.setup_iso15765_filter(ecu_addr::IMC_TX, ecu_addr::IMC_RX).unwrap();
        UdsClient::new(mock, ecu_addr::IMC_TX, ecu_addr::IMC_RX)
    }

    fn make_bcm_client(mock: MockChannel) -> UdsClient<MockChannel> {
        mock.setup_iso15765_filter(ecu_addr::BCM_TX, ecu_addr::BCM_RX).unwrap();
        UdsClient::new(mock, ecu_addr::BCM_TX, ecu_addr::BCM_RX)
    }

    // ─── DiagnosticSessionControl (0x10) ─────────────────────────

    #[test]
    fn test_session_default() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::IMC_TX, vec![0x10, 0x01], vec![0x50, 0x01, 0x00, 0x19, 0x01, 0xF4]);
        let client = make_imc_client(mock);
        let resp = diagnostic_session(&client, DiagSession::Default).unwrap();
        assert_eq!(resp[0], 0x50);
        assert_eq!(resp[1], 0x01);
    }

    #[test]
    fn test_session_extended() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::IMC_TX, vec![0x10, 0x03], vec![0x50, 0x03, 0x00, 0x19, 0x01, 0xF4]);
        let client = make_imc_client(mock);
        let resp = diagnostic_session(&client, DiagSession::Extended).unwrap();
        assert_eq!(resp[0], 0x50);
        assert_eq!(resp[1], 0x03);
    }

    #[test]
    fn test_session_programming() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::IMC_TX, vec![0x10, 0x02], vec![0x50, 0x02, 0x00, 0x19, 0x01, 0xF4]);
        let client = make_imc_client(mock);
        let resp = diagnostic_session(&client, DiagSession::Programming).unwrap();
        assert_eq!(resp[0], 0x50);
        assert_eq!(resp[1], 0x02);
    }

    #[test]
    fn test_session_rejected() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::IMC_TX, vec![0x10, 0x03], vec![0x7F, 0x10, 0x22]);
        let client = make_imc_client(mock);
        let err = diagnostic_session(&client, DiagSession::Extended).unwrap_err();
        match err {
            UdsError::NegativeResponse { service_id, nrc } => {
                assert_eq!(service_id, 0x10);
                assert_eq!(nrc, NegativeResponseCode::ConditionsNotCorrect);
            }
            other => panic!("Expected NRC, got {:?}", other),
        }
    }

    // ─── TesterPresent (0x3E) ────────────────────────────────────

    #[test]
    fn test_tester_present() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::IMC_TX, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        let client = make_imc_client(mock);
        let resp = tester_present(&client).unwrap();
        assert_eq!(resp, vec![0x7E, 0x00]);
    }

    #[test]
    fn test_tester_present_no_response() {
        let mock = MockChannel::new();
        let client = make_imc_client(mock);
        // suppressResponse — just sends, no response expected
        tester_present_no_response(&client).unwrap();
        let sent = client.channel().sent_messages();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].1, vec![0x3E, 0x80]);
    }

    // ─── ReadDataByIdentifier (0x22) ─────────────────────────────

    #[test]
    fn test_read_did_f190_vin() {
        let mock = MockChannel::new();
        let vin_bytes = b"SAJBA4BN0HA123456";
        let mut resp = vec![0x62, 0xF1, 0x90];
        resp.extend_from_slice(vin_bytes);
        mock.expect_request(ecu_addr::IMC_TX, vec![0x22, 0xF1, 0x90], resp);
        let client = make_imc_client(mock);
        let vin = read_vin(&client).unwrap();
        assert_eq!(vin, "SAJBA4BN0HA123456");
    }

    #[test]
    fn test_read_did_f188_part_number() {
        let mock = MockChannel::new();
        let part = b"GX63-14F012-AC";
        let mut resp = vec![0x62, 0xF1, 0x88];
        resp.extend_from_slice(part);
        mock.expect_request(ecu_addr::IMC_TX, vec![0x22, 0xF1, 0x88], resp);
        let client = make_imc_client(mock);
        let pn = read_part_number(&client, did::MASTER_RPM_PART).unwrap();
        assert_eq!(pn, "GX63-14F012-AC");
    }

    #[test]
    fn test_read_did_f120_v850() {
        let mock = MockChannel::new();
        let mut resp = vec![0x62, 0xF1, 0x20];
        resp.extend_from_slice(b"GX63-14F045-AB");
        mock.expect_request(ecu_addr::IMC_TX, vec![0x22, 0xF1, 0x20], resp);
        let client = make_imc_client(mock);
        let pn = read_part_number(&client, did::V850_PART).unwrap();
        assert_eq!(pn, "GX63-14F045-AB");
    }

    #[test]
    fn test_read_did_f121_tuner() {
        let mock = MockChannel::new();
        let mut resp = vec![0x62, 0xF1, 0x21];
        resp.extend_from_slice(b"GX63-18K875-AA");
        mock.expect_request(ecu_addr::IMC_TX, vec![0x22, 0xF1, 0x21], resp);
        let client = make_imc_client(mock);
        let pn = read_part_number(&client, did::TUNER_PART).unwrap();
        assert_eq!(pn, "GX63-18K875-AA");
    }

    #[test]
    fn test_read_did_f1a5_polar() {
        let mock = MockChannel::new();
        let mut resp = vec![0x62, 0xF1, 0xA5];
        resp.extend_from_slice(b"GX63-POLAR-01");
        mock.expect_request(ecu_addr::IMC_TX, vec![0x22, 0xF1, 0xA5], resp);
        let client = make_imc_client(mock);
        let pn = read_part_number(&client, did::POLAR_PART).unwrap();
        assert_eq!(pn, "GX63-POLAR-01");
    }

    #[test]
    fn test_read_did_f180_pbl() {
        let mock = MockChannel::new();
        let mut resp = vec![0x62, 0xF1, 0x80];
        resp.extend_from_slice(b"GX63-PBL-001");
        mock.expect_request(ecu_addr::IMC_TX, vec![0x22, 0xF1, 0x80], resp);
        let client = make_imc_client(mock);
        let pn = read_part_number(&client, did::PBL_PART).unwrap();
        assert_eq!(pn, "GX63-PBL-001");
    }

    #[test]
    fn test_read_did_f18c_serial() {
        let mock = MockChannel::new();
        let mut resp = vec![0x62, 0xF1, 0x8C];
        resp.extend_from_slice(b"SN123456789");
        mock.expect_request(ecu_addr::IMC_TX, vec![0x22, 0xF1, 0x8C], resp);
        let client = make_imc_client(mock);
        let pn = read_part_number(&client, did::ECU_SERIAL).unwrap();
        assert_eq!(pn, "SN123456789");
    }

    #[test]
    fn test_read_did_f113_serial2() {
        let mock = MockChannel::new();
        let mut resp = vec![0x62, 0xF1, 0x13];
        resp.extend_from_slice(b"HW_SN_002_IMCBOARD");
        mock.expect_request(ecu_addr::IMC_TX, vec![0x22, 0xF1, 0x13], resp);
        let client = make_imc_client(mock);
        let data = read_did_data(&client, did::ECU_SERIAL2).unwrap();
        assert_eq!(String::from_utf8_lossy(&data).trim(), "HW_SN_002_IMCBOARD");
    }

    #[test]
    fn test_read_did_d100_session() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::IMC_TX, vec![0x22, 0xD1, 0x00], vec![0x62, 0xD1, 0x00, 0x01]);
        let client = make_imc_client(mock);
        let data = read_did_data(&client, did::ACTIVE_DIAG_SESSION).unwrap();
        assert_eq!(data, vec![0x01]); // default session
    }

    #[test]
    fn test_read_did_0202_status() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::IMC_TX, vec![0x22, 0x02, 0x02], vec![0x62, 0x02, 0x02, 0xAA]);
        let client = make_imc_client(mock);
        let data = read_did_data(&client, did::IMC_STATUS).unwrap();
        assert_eq!(data, vec![0xAA]);
    }

    #[test]
    fn test_read_did_402a_voltage() {
        let mock = MockChannel::new();
        // 0x007C = 124 → 12.4V
        mock.expect_request(ecu_addr::BCM_TX, vec![0x22, 0x40, 0x2A], vec![0x62, 0x40, 0x2A, 0x00, 0x7C]);
        let client = make_bcm_client(mock);
        let voltage = read_battery_voltage(&client).unwrap();
        assert!((voltage - 12.4).abs() < 0.01);
    }

    #[test]
    fn test_read_did_4028_soc() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::BCM_TX, vec![0x22, 0x40, 0x28], vec![0x62, 0x40, 0x28, 0x55]);
        let client = make_bcm_client(mock);
        let data = read_did_data(&client, did::BATTERY_SOC).unwrap();
        assert_eq!(data, vec![0x55]); // 85% SoC
    }

    #[test]
    fn test_read_did_4029_temp() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::BCM_TX, vec![0x22, 0x40, 0x29], vec![0x62, 0x40, 0x29, 0x19]);
        let client = make_bcm_client(mock);
        let data = read_did_data(&client, did::BATTERY_TEMP).unwrap();
        assert_eq!(data, vec![0x19]); // 25°C
    }

    #[test]
    fn test_read_did_not_supported() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::IMC_TX, vec![0x22, 0xFF, 0xFF], vec![0x7F, 0x22, 0x31]);
        let client = make_imc_client(mock);
        let err = read_did(&client, 0xFFFF).unwrap_err();
        match err {
            UdsError::NegativeResponse { service_id, nrc } => {
                assert_eq!(service_id, 0x22);
                assert_eq!(nrc, NegativeResponseCode::RequestOutOfRange);
            }
            other => panic!("Expected NRC, got {:?}", other),
        }
    }

    // ─── SecurityAccess (0x27) ───────────────────────────────────

    #[test]
    fn test_security_seed_request() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::IMC_TX, vec![0x27, 0x11], vec![0x67, 0x11, 0x11, 0x22, 0x33]);
        let client = make_imc_client(mock);
        let resp = security_request_seed(&client, 0x11).unwrap();
        assert_eq!(resp[0], 0x67);
        assert_eq!(resp[1], 0x11);
        assert_eq!(&resp[2..5], &[0x11, 0x22, 0x33]);
    }

    #[test]
    fn test_security_key_send_ok() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::IMC_TX, vec![0x27, 0x12, 0xAA, 0xBB, 0xCC], vec![0x67, 0x12]);
        let client = make_imc_client(mock);
        let resp = security_send_key(&client, 0x12, &[0xAA, 0xBB, 0xCC]).unwrap();
        assert_eq!(resp, vec![0x67, 0x12]);
    }

    #[test]
    fn test_security_key_invalid() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::IMC_TX, vec![0x27, 0x12, 0x00, 0x00, 0x00], vec![0x7F, 0x27, 0x35]);
        let client = make_imc_client(mock);
        let err = security_send_key(&client, 0x12, &[0x00, 0x00, 0x00]).unwrap_err();
        match err {
            UdsError::NegativeResponse { nrc, .. } => {
                assert_eq!(nrc, NegativeResponseCode::InvalidKey);
            }
            other => panic!("Expected InvalidKey NRC, got {:?}", other),
        }
    }

    #[test]
    fn test_security_exceeded_attempts() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::IMC_TX, vec![0x27, 0x11], vec![0x7F, 0x27, 0x36]);
        let client = make_imc_client(mock);
        let err = security_request_seed(&client, 0x11).unwrap_err();
        match err {
            UdsError::NegativeResponse { nrc, .. } => {
                assert_eq!(nrc, NegativeResponseCode::ExceededNumberOfAttempts);
            }
            other => panic!("Expected ExceededNumberOfAttempts, got {:?}", other),
        }
    }

    #[test]
    fn test_security_required_delay() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::IMC_TX, vec![0x27, 0x11], vec![0x7F, 0x27, 0x37]);
        let client = make_imc_client(mock);
        let err = security_request_seed(&client, 0x11).unwrap_err();
        match err {
            UdsError::NegativeResponse { nrc, .. } => {
                assert_eq!(nrc, NegativeResponseCode::RequiredTimeDelayNotExpired);
            }
            other => panic!("Expected RequiredTimeDelayNotExpired, got {:?}", other),
        }
    }

    #[test]
    fn test_security_full_flow() {
        let mock = MockChannel::new();
        // Seed request → seed response
        mock.expect_request(ecu_addr::IMC_TX, vec![0x27, 0x11], vec![0x67, 0x11, 0x11, 0x22, 0x33]);
        // Compute key for seed 0x112233 with DC0314 constants
        let seed_int = 0x112233u32;
        let key_int = keygen::keygen_mki(seed_int, &keygen::DC0314_CONSTANTS);
        let key_bytes = [
            ((key_int >> 16) & 0xFF) as u8,
            ((key_int >> 8) & 0xFF) as u8,
            (key_int & 0xFF) as u8,
        ];
        let mut key_req = vec![0x27, 0x12];
        key_req.extend_from_slice(&key_bytes);
        mock.expect_request(ecu_addr::IMC_TX, key_req, vec![0x67, 0x12]);
        let client = make_imc_client(mock);
        let unlocked = security_access(&client, 0x11, 0x12, &keygen::DC0314_CONSTANTS).unwrap();
        assert!(unlocked);
    }

    #[test]
    fn test_security_zero_seed_already_unlocked() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::IMC_TX, vec![0x27, 0x11], vec![0x67, 0x11, 0x00, 0x00, 0x00]);
        let client = make_imc_client(mock);
        let unlocked = security_access(&client, 0x11, 0x12, &keygen::DC0314_CONSTANTS).unwrap();
        assert!(!unlocked); // false = was already unlocked
    }

    // ─── RoutineControl (0x31) ───────────────────────────────────

    #[test]
    fn test_routine_6038_start() {
        let mock = MockChannel::new();
        mock.expect_request(
            ecu_addr::IMC_TX,
            vec![0x31, 0x01, 0x60, 0x38],
            vec![0x71, 0x01, 0x60, 0x38, 0x10, 0x01, 0x00],
        );
        let client = make_imc_client(mock);
        let result = routine_start(&client, routine::CONFIGURE_LINUX, &[], false).unwrap();
        assert_eq!(result.routine_id, 0x6038);
        assert_eq!(result.status, Some(0x10));
        assert_eq!(result.result, Some(0x01));
        assert_eq!(result.error, Some(0x00));
    }

    #[test]
    fn test_routine_603d_eng_screen_2() {
        let mock = MockChannel::new();
        mock.expect_request(
            ecu_addr::IMC_TX,
            vec![0x31, 0x01, 0x60, 0x3D],
            vec![0x71, 0x01, 0x60, 0x3D],
        );
        let client = make_imc_client(mock);
        let result = routine_start(&client, routine::ENG_SCREEN_LVL2, &[], false).unwrap();
        assert_eq!(result.routine_id, 0x603D);
    }

    #[test]
    fn test_routine_603e_ssh_enable() {
        let mock = MockChannel::new();
        mock.expect_request(
            ecu_addr::IMC_TX,
            vec![0x31, 0x01, 0x60, 0x3E, 0x01],
            vec![0x71, 0x01, 0x60, 0x3E],
        );
        let client = make_imc_client(mock);
        let result = routine_start(&client, routine::SSH_ENABLE, &[0x01], false).unwrap();
        assert_eq!(result.routine_id, 0x603E);
    }

    #[test]
    fn test_routine_603e_ssh_with_pending() {
        let mock = MockChannel::new();
        mock.expect_request_multi(
            ecu_addr::IMC_TX,
            vec![0x31, 0x01, 0x60, 0x3E, 0x01],
            vec![
                vec![0x7F, 0x31, 0x78], // pending
                vec![0x71, 0x01, 0x60, 0x3E], // OK
            ],
        );
        let client = make_imc_client(mock);
        let result = routine_start(&client, routine::SSH_ENABLE, &[0x01], true).unwrap();
        assert_eq!(result.routine_id, 0x603E);
    }

    #[test]
    fn test_routine_603f_dvd_recover() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::IMC_TX, vec![0x31, 0x01, 0x60, 0x3F], vec![0x71, 0x01, 0x60, 0x3F]);
        let client = make_imc_client(mock);
        let result = routine_start(&client, routine::DVD_RECOVER, &[], false).unwrap();
        assert_eq!(result.routine_id, 0x603F);
    }

    #[test]
    fn test_routine_6041_fan_control() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::IMC_TX, vec![0x31, 0x01, 0x60, 0x41], vec![0x71, 0x01, 0x60, 0x41]);
        let client = make_imc_client(mock);
        let result = routine_start(&client, routine::FAN_CONTROL, &[], false).unwrap();
        assert_eq!(result.routine_id, 0x6041);
    }

    #[test]
    fn test_routine_6042_reset_pin() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::IMC_TX, vec![0x31, 0x01, 0x60, 0x42], vec![0x71, 0x01, 0x60, 0x42]);
        let client = make_imc_client(mock);
        let result = routine_start(&client, routine::RESET_PIN, &[], false).unwrap();
        assert_eq!(result.routine_id, 0x6042);
    }

    #[test]
    fn test_routine_6043_power_override() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::IMC_TX, vec![0x31, 0x01, 0x60, 0x43], vec![0x71, 0x01, 0x60, 0x43]);
        let client = make_imc_client(mock);
        let result = routine_start(&client, routine::POWER_OVERRIDE, &[], false).unwrap();
        assert_eq!(result.routine_id, 0x6043);
    }

    #[test]
    fn test_routine_6045_gen_key() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::IMC_TX, vec![0x31, 0x01, 0x60, 0x45], vec![0x71, 0x01, 0x60, 0x45]);
        let client = make_imc_client(mock);
        let result = routine_start(&client, routine::GEN_KEY, &[], false).unwrap();
        assert_eq!(result.routine_id, 0x6045);
    }

    #[test]
    fn test_routine_6046_shared_secret() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::IMC_TX, vec![0x31, 0x01, 0x60, 0x46], vec![0x71, 0x01, 0x60, 0x46]);
        let client = make_imc_client(mock);
        let result = routine_start(&client, routine::SHARED_SECRET, &[], false).unwrap();
        assert_eq!(result.routine_id, 0x6046);
    }

    #[test]
    fn test_routine_0404_vin_learn() {
        let mock = MockChannel::new();
        mock.expect_request(
            ecu_addr::IMC_TX,
            vec![0x31, 0x01, 0x04, 0x04],
            vec![0x71, 0x01, 0x04, 0x04, 0x20],
        );
        let client = make_imc_client(mock);
        let result = routine_start(&client, routine::VIN_LEARN, &[], false).unwrap();
        assert_eq!(result.routine_id, 0x0404);
        assert_eq!(result.status, Some(0x20));
    }

    #[test]
    fn test_routine_0e00_retrieve_ccf() {
        let mock = MockChannel::new();
        mock.expect_request(
            ecu_addr::IMC_TX,
            vec![0x31, 0x01, 0x0E, 0x00],
            vec![0x71, 0x01, 0x0E, 0x00, 0xAA, 0xBB, 0xCC],
        );
        let client = make_imc_client(mock);
        let result = routine_start(&client, routine::RETRIEVE_CCF, &[], false).unwrap();
        assert_eq!(result.routine_id, 0x0E00);
        assert_eq!(result.raw_data, vec![0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn test_routine_0e01_report_ccf() {
        let mock = MockChannel::new();
        mock.expect_request(
            ecu_addr::IMC_TX,
            vec![0x31, 0x01, 0x0E, 0x01],
            vec![0x71, 0x01, 0x0E, 0x01, 0x01, 0x02],
        );
        let client = make_imc_client(mock);
        let result = routine_start(&client, routine::REPORT_CCF, &[], false).unwrap();
        assert_eq!(result.routine_id, 0x0E01);
    }

    #[test]
    fn test_routine_0e02_list_ccf() {
        let mock = MockChannel::new();
        mock.expect_request(
            ecu_addr::IMC_TX,
            vec![0x31, 0x01, 0x0E, 0x02],
            vec![0x71, 0x01, 0x0E, 0x02, 0x05],
        );
        let client = make_imc_client(mock);
        let result = routine_start(&client, routine::LIST_CCF, &[], false).unwrap();
        assert_eq!(result.routine_id, 0x0E02);
    }

    #[test]
    fn test_routine_rejected_security() {
        let mock = MockChannel::new();
        mock.expect_request(
            ecu_addr::IMC_TX,
            vec![0x31, 0x01, 0x60, 0x3E, 0x01],
            vec![0x7F, 0x31, 0x33], // securityAccessDenied
        );
        let client = make_imc_client(mock);
        let err = routine_start(&client, routine::SSH_ENABLE, &[0x01], false).unwrap_err();
        match err {
            UdsError::NegativeResponse { nrc, .. } => {
                assert_eq!(nrc, NegativeResponseCode::SecurityAccessDenied);
            }
            other => panic!("Expected SecurityAccessDenied, got {:?}", other),
        }
    }

    #[test]
    fn test_routine_rejected_session() {
        let mock = MockChannel::new();
        mock.expect_request(
            ecu_addr::IMC_TX,
            vec![0x31, 0x01, 0x60, 0x3E, 0x01],
            vec![0x7F, 0x31, 0x7E], // subFunctionNotSupportedInActiveSession
        );
        let client = make_imc_client(mock);
        let err = routine_start(&client, routine::SSH_ENABLE, &[0x01], false).unwrap_err();
        match err {
            UdsError::NegativeResponse { nrc, .. } => {
                assert_eq!(nrc, NegativeResponseCode::SubFunctionNotSupportedInActiveSession);
            }
            other => panic!("Expected SubFunctionNotSupportedInActiveSession, got {:?}", other),
        }
    }

    // ─── ECUReset (0x11) ─────────────────────────────────────────

    #[test]
    fn test_ecu_reset_hard() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::IMC_TX, vec![0x11, 0x01], vec![0x51, 0x01]);
        let client = make_imc_client(mock);
        let resp = ecu_reset(&client, ResetType::HardReset).unwrap();
        assert_eq!(resp, vec![0x51, 0x01]);
    }

    #[test]
    fn test_ecu_reset_soft() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::IMC_TX, vec![0x11, 0x03], vec![0x51, 0x03]);
        let client = make_imc_client(mock);
        let resp = ecu_reset(&client, ResetType::SoftReset).unwrap();
        assert_eq!(resp, vec![0x51, 0x03]);
    }

    #[test]
    fn test_ecu_reset_key_off_on() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::IMC_TX, vec![0x11, 0x02], vec![0x51, 0x02]);
        let client = make_imc_client(mock);
        let resp = ecu_reset(&client, ResetType::KeyOffOnReset).unwrap();
        assert_eq!(resp, vec![0x51, 0x02]);
    }

    // ─── SSH Enable Flow (integration) ──────────────────────────

    #[test]
    fn test_ssh_enable_full_flow() {
        let mock = MockChannel::new();
        // TesterPresent
        mock.expect_request(ecu_addr::IMC_TX, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        // Extended session
        mock.expect_request(ecu_addr::IMC_TX, vec![0x10, 0x03], vec![0x50, 0x03, 0x00, 0x19, 0x01, 0xF4]);
        // Security seed
        mock.expect_request(ecu_addr::IMC_TX, vec![0x27, 0x11], vec![0x67, 0x11, 0x11, 0x22, 0x33]);
        // Security key (computed)
        let seed_int = 0x112233u32;
        let key_int = keygen::keygen_mki(seed_int, &keygen::DC0314_CONSTANTS);
        let key_bytes = [
            ((key_int >> 16) & 0xFF) as u8,
            ((key_int >> 8) & 0xFF) as u8,
            (key_int & 0xFF) as u8,
        ];
        let mut key_req = vec![0x27, 0x12];
        key_req.extend_from_slice(&key_bytes);
        mock.expect_request(ecu_addr::IMC_TX, key_req, vec![0x67, 0x12]);
        // SSH routine
        mock.expect_request(
            ecu_addr::IMC_TX,
            vec![0x31, 0x01, 0x60, 0x3E, 0x01],
            vec![0x71, 0x01, 0x60, 0x3E],
        );

        let client = make_imc_client(mock);
        let result = enable_ssh(&client).unwrap();
        assert!(result.success);
        assert_eq!(result.ip_address, "192.168.103.11");
        assert!(result.message.contains("SSH ENABLED"));
    }

    #[test]
    fn test_ssh_enable_security_fails() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::IMC_TX, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(ecu_addr::IMC_TX, vec![0x10, 0x03], vec![0x50, 0x03, 0x00, 0x19, 0x01, 0xF4]);
        // Security seed fails
        mock.expect_request(ecu_addr::IMC_TX, vec![0x27, 0x11], vec![0x7F, 0x27, 0x36]);

        let client = make_imc_client(mock);
        let err = enable_ssh(&client).unwrap_err();
        match err {
            UdsError::NegativeResponse { nrc, .. } => {
                assert_eq!(nrc, NegativeResponseCode::ExceededNumberOfAttempts);
            }
            other => panic!("Expected NRC, got {:?}", other),
        }
    }

    #[test]
    fn test_ssh_enable_pending_handling() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::IMC_TX, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(ecu_addr::IMC_TX, vec![0x10, 0x03], vec![0x50, 0x03, 0x00, 0x19, 0x01, 0xF4]);
        mock.expect_request(ecu_addr::IMC_TX, vec![0x27, 0x11], vec![0x67, 0x11, 0x00, 0x00, 0x00]); // already unlocked
        // SSH routine with pending
        mock.expect_request_multi(
            ecu_addr::IMC_TX,
            vec![0x31, 0x01, 0x60, 0x3E, 0x01],
            vec![
                vec![0x7F, 0x31, 0x78], // pending
                vec![0x71, 0x01, 0x60, 0x3E], // OK
            ],
        );

        let client = make_imc_client(mock);
        let result = enable_ssh(&client).unwrap();
        assert!(result.success);
    }

    #[test]
    fn test_ssh_enable_routine_fails() {
        let mock = MockChannel::new();
        mock.expect_request(ecu_addr::IMC_TX, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        mock.expect_request(ecu_addr::IMC_TX, vec![0x10, 0x03], vec![0x50, 0x03, 0x00, 0x19, 0x01, 0xF4]);
        mock.expect_request(ecu_addr::IMC_TX, vec![0x27, 0x11], vec![0x67, 0x11, 0x00, 0x00, 0x00]); // already unlocked
        // SSH routine rejected
        mock.expect_request(
            ecu_addr::IMC_TX,
            vec![0x31, 0x01, 0x60, 0x3E, 0x01],
            vec![0x7F, 0x31, 0x22], // conditionsNotCorrect
        );

        let client = make_imc_client(mock);
        let err = enable_ssh(&client).unwrap_err();
        match err {
            UdsError::NegativeResponse { nrc, .. } => {
                assert_eq!(nrc, NegativeResponseCode::ConditionsNotCorrect);
            }
            other => panic!("Expected NRC, got {:?}", other),
        }
    }

    // ─── execute_routine_sdd tests ─────────────────────────────

    #[test]
    fn test_execute_routine_sdd_with_security() {
        let mock = MockChannel::new();
        // TesterPresent
        mock.expect_request(ecu_addr::IMC_TX, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        // Extended session
        mock.expect_request(ecu_addr::IMC_TX, vec![0x10, 0x03], vec![0x50, 0x03, 0x00, 0x19, 0x01, 0xF4]);
        // Security seed
        mock.expect_request(ecu_addr::IMC_TX, vec![0x27, 0x11], vec![0x67, 0x11, 0x11, 0x22, 0x33]);
        // Security key (computed)
        let seed_int = 0x112233u32;
        let key_int = keygen::keygen_mki(seed_int, &keygen::DC0314_CONSTANTS);
        let key_bytes = [
            ((key_int >> 16) & 0xFF) as u8,
            ((key_int >> 8) & 0xFF) as u8,
            (key_int & 0xFF) as u8,
        ];
        let mut key_req = vec![0x27, 0x12];
        key_req.extend_from_slice(&key_bytes);
        mock.expect_request(ecu_addr::IMC_TX, key_req, vec![0x67, 0x12]);
        // Routine 0x603D
        mock.expect_request(
            ecu_addr::IMC_TX,
            vec![0x31, 0x01, 0x60, 0x3D],
            vec![0x71, 0x01, 0x60, 0x3D],
        );

        let client = make_imc_client(mock);
        let result = execute_routine_sdd(&client, routine::ENG_SCREEN_LVL2, &[], true, false).unwrap();
        assert_eq!(result.routine_id, 0x603D);
    }

    #[test]
    fn test_execute_routine_sdd_without_security() {
        let mock = MockChannel::new();
        // TesterPresent
        mock.expect_request(ecu_addr::IMC_TX, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        // Extended session
        mock.expect_request(ecu_addr::IMC_TX, vec![0x10, 0x03], vec![0x50, 0x03, 0x00, 0x19, 0x01, 0xF4]);
        // NO security step — goes straight to routine
        mock.expect_request(
            ecu_addr::IMC_TX,
            vec![0x31, 0x01, 0x04, 0x04],
            vec![0x71, 0x01, 0x04, 0x04, 0x20],
        );

        let client = make_imc_client(mock);
        let result = execute_routine_sdd(&client, routine::VIN_LEARN, &[], false, false).unwrap();
        assert_eq!(result.routine_id, 0x0404);
        assert_eq!(result.status, Some(0x20));
    }

    #[test]
    fn test_execute_routine_sdd_security_fails() {
        let mock = MockChannel::new();
        // TesterPresent
        mock.expect_request(ecu_addr::IMC_TX, vec![0x3E, 0x00], vec![0x7E, 0x00]);
        // Extended session
        mock.expect_request(ecu_addr::IMC_TX, vec![0x10, 0x03], vec![0x50, 0x03, 0x00, 0x19, 0x01, 0xF4]);
        // Security seed fails — exceeded attempts
        mock.expect_request(ecu_addr::IMC_TX, vec![0x27, 0x11], vec![0x7F, 0x27, 0x36]);

        let client = make_imc_client(mock);
        let err = execute_routine_sdd(&client, routine::ENG_SCREEN_LVL2, &[], true, false).unwrap_err();
        match err {
            UdsError::NegativeResponse { nrc, .. } => {
                assert_eq!(nrc, NegativeResponseCode::ExceededNumberOfAttempts);
            }
            other => panic!("Expected ExceededNumberOfAttempts, got {:?}", other),
        }
    }

    // ─── Existing decode tests ───────────────────────────────────

    #[test]
    fn test_describe_6038_status_finished() {
        let desc = describe_6038(Some(0x10), None, None);
        assert!(desc.contains("Routine finished"));
    }

    #[test]
    fn test_describe_6038_result_completed() {
        let desc = describe_6038(None, Some(0x01), None);
        assert!(desc.contains("Completed"));
    }

    #[test]
    fn test_describe_6038_result_failed() {
        let desc = describe_6038(None, Some(0x03), None);
        assert!(desc.contains("Failed to configure"));
    }

    #[test]
    fn test_describe_6038_error_boot_param() {
        let desc = describe_6038(None, None, Some(0x01));
        assert!(desc.contains("Boot parameter"));
    }

    #[test]
    fn test_describe_6038_error_symlinks() {
        let desc = describe_6038(None, None, Some(0x02));
        assert!(desc.contains("Symlinks"));
    }

    #[test]
    fn test_describe_6038_error_startup_xml() {
        let desc = describe_6038(None, None, Some(0x04));
        assert!(desc.contains("Start-up configuration XML"));
    }

    #[test]
    fn test_describe_6038_error_manifest() {
        let desc = describe_6038(None, None, Some(0x08));
        assert!(desc.contains("Manifest symlinks"));
    }

    #[test]
    fn test_describe_6038_error_dvd() {
        let desc = describe_6038(None, None, Some(0x10));
        assert!(desc.contains("DVD region"));
    }

    #[test]
    fn test_describe_6038_error_polar() {
        let desc = describe_6038(None, None, Some(0x20));
        assert!(desc.contains("Polar switch"));
    }

    #[test]
    fn test_describe_6038_error_gracenotes() {
        let desc = describe_6038(None, None, Some(0x40));
        assert!(desc.contains("Gracenotes"));
    }

    #[test]
    fn test_describe_6038_error_app_mgr() {
        let desc = describe_6038(None, None, Some(0x80));
        assert!(desc.contains("Application manager LCF"));
    }

    #[test]
    fn test_describe_6038_multiple_errors() {
        let desc = describe_6038(None, None, Some(0x03)); // boot param + symlinks
        assert!(desc.contains("Boot parameter"));
        assert!(desc.contains("Symlinks"));
    }

    #[test]
    fn test_describe_6038_no_errors() {
        let desc = describe_6038(None, None, Some(0x00));
        assert!(desc.contains("No errors"));
    }

    #[test]
    fn test_describe_0404_completed() {
        let desc = describe_0404(Some(0x20));
        assert!(desc.contains("Routine completed"));
    }

    #[test]
    fn test_describe_0404_aborted() {
        let desc = describe_0404(Some(0x21));
        assert!(desc.contains("Routine aborted"));
    }

    #[test]
    fn test_describe_0404_active() {
        let desc = describe_0404(Some(0x22));
        assert!(desc.contains("Routine active"));
    }

    #[test]
    fn test_diag_session_values() {
        assert_eq!(DiagSession::Default as u8, 0x01);
        assert_eq!(DiagSession::Programming as u8, 0x02);
        assert_eq!(DiagSession::Extended as u8, 0x03);
    }

    #[test]
    fn test_ecu_addresses() {
        assert_eq!(ecu_addr::IMC_TX, 0x7B3);
        assert_eq!(ecu_addr::IMC_RX, 0x7BB);
        assert_eq!(ecu_addr::GWM_TX, 0x716);
        assert_eq!(ecu_addr::GWM_RX, 0x71E);
        assert_eq!(ecu_addr::BCM_TX, 0x726);
        assert_eq!(ecu_addr::BCM_RX, 0x72E);
        assert_eq!(ecu_addr::IPC_TX, 0x720);
        assert_eq!(ecu_addr::IPC_RX, 0x728);
    }

    #[test]
    fn test_did_values() {
        assert_eq!(did::VIN, 0xF190);
        assert_eq!(did::BATTERY_VOLTAGE, 0x402A);
        assert_eq!(did::ECU_SERIAL, 0xF18C);
    }

    #[test]
    fn test_routine_ids() {
        assert_eq!(routine::CONFIGURE_LINUX, 0x6038);
        assert_eq!(routine::SSH_ENABLE, 0x603E);
        assert_eq!(routine::VIN_LEARN, 0x0404);
    }
}
