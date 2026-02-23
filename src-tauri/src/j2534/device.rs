use std::ffi::c_void;
use std::sync::Arc;

use crate::j2534::dll::J2534Lib;
use crate::j2534::types::*;

/// Represents an opened J2534 device (PassThruOpen handle)
pub struct J2534Device {
    lib: Arc<J2534Lib>,
    device_id: u32,
}

impl J2534Device {
    /// Open a J2534 device using the loaded DLL
    pub fn open(lib: Arc<J2534Lib>) -> Result<Self, String> {
        let mut device_id: u32 = 0;
        let ret = unsafe { (lib.pass_thru_open)(std::ptr::null(), &mut device_id) };
        if ret != 0 {
            return Err(format!(
                "PassThruOpen failed: {}",
                J2534Error::from_code(ret)
            ));
        }
        Ok(Self { lib, device_id })
    }

    /// Read device version strings
    pub fn read_version(&self) -> Result<DeviceVersion, String> {
        let mut firmware = [0u8; 80];
        let mut dll = [0u8; 80];
        let mut api = [0u8; 80];
        let ret = unsafe {
            (self.lib.pass_thru_read_version)(
                self.device_id,
                firmware.as_mut_ptr(),
                dll.as_mut_ptr(),
                api.as_mut_ptr(),
            )
        };
        if ret != 0 {
            return Err(format!(
                "PassThruReadVersion failed: {}",
                J2534Error::from_code(ret)
            ));
        }
        Ok(DeviceVersion {
            firmware: String::from_utf8_lossy(
                &firmware[..firmware
                    .iter()
                    .position(|&b| b == 0)
                    .unwrap_or(firmware.len())],
            )
            .to_string(),
            dll: String::from_utf8_lossy(
                &dll[..dll.iter().position(|&b| b == 0).unwrap_or(dll.len())],
            )
            .to_string(),
            api: String::from_utf8_lossy(
                &api[..api.iter().position(|&b| b == 0).unwrap_or(api.len())],
            )
            .to_string(),
        })
    }

    /// Connect a channel with ISO15765 protocol
    pub fn connect_iso15765(&self, baudrate: u32) -> Result<J2534Channel, String> {
        let mut channel_id: u32 = 0;
        let ret = unsafe {
            (self.lib.pass_thru_connect)(
                self.device_id,
                PROTOCOL_ISO15765,
                0, // flags (FRAME_PAD is per-message, not connect flag)
                baudrate,
                &mut channel_id,
            )
        };
        if ret != 0 {
            return Err(format!(
                "PassThruConnect failed: {}",
                J2534Error::from_code(ret)
            ));
        }
        Ok(J2534Channel {
            lib: self.lib.clone(),
            channel_id,
        })
    }

    /// Connect a raw CAN channel (for broadcast messages)
    pub fn connect_can(&self, baudrate: u32) -> Result<J2534Channel, String> {
        let mut channel_id: u32 = 0;
        let ret = unsafe {
            (self.lib.pass_thru_connect)(
                self.device_id,
                PROTOCOL_CAN,
                0, // flags
                baudrate,
                &mut channel_id,
            )
        };
        if ret != 0 {
            return Err(format!(
                "PassThruConnect CAN failed: {}",
                J2534Error::from_code(ret)
            ));
        }
        Ok(J2534Channel {
            lib: self.lib.clone(),
            channel_id,
        })
    }

    pub fn device_id(&self) -> u32 {
        self.device_id
    }
}

impl Drop for J2534Device {
    fn drop(&mut self) {
        unsafe {
            (self.lib.pass_thru_close)(self.device_id);
        }
    }
}

/// Version info from a J2534 device
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeviceVersion {
    pub firmware: String,
    pub dll: String,
    pub api: String,
}

/// Represents a connected J2534 channel
pub struct J2534Channel {
    lib: Arc<J2534Lib>,
    channel_id: u32,
}

impl J2534Channel {
    /// Set up ISO15765 flow control filter for ECU communication
    pub fn setup_iso15765_filter(&self, tx_id: u32, rx_id: u32) -> Result<u32, String> {
        let mut mask = PassThruMsg::default();
        mask.protocol_id = PROTOCOL_ISO15765;
        mask.data_size = 4;
        mask.data[0..4].copy_from_slice(&0x000007FFu32.to_be_bytes());

        let mut pattern = PassThruMsg::default();
        pattern.protocol_id = PROTOCOL_ISO15765;
        pattern.data_size = 4;
        pattern.data[0..4].copy_from_slice(&rx_id.to_be_bytes());

        let mut flow_control = PassThruMsg::default();
        flow_control.protocol_id = PROTOCOL_ISO15765;
        flow_control.tx_flags = ISO15765_FRAME_PAD; // pad FC to 8 bytes — IMC requires it
        flow_control.data_size = 4;
        flow_control.data[0..4].copy_from_slice(&tx_id.to_be_bytes());

        let mut filter_id: u32 = 0;
        let ret = unsafe {
            (self.lib.pass_thru_start_msg_filter)(
                self.channel_id,
                FILTER_FLOW_CONTROL,
                &mask,
                &pattern,
                &flow_control,
                &mut filter_id,
            )
        };
        if ret != 0 {
            return Err(format!(
                "PassThruStartMsgFilter failed: {}",
                J2534Error::from_code(ret)
            ));
        }
        Ok(filter_id)
    }

    /// Send a raw CAN frame (8 bytes max, for broadcast on a CAN channel)
    pub fn send_raw_can(&self, can_id: u32, data: &[u8]) -> Result<(), String> {
        let mut msg = PassThruMsg::default();
        msg.protocol_id = PROTOCOL_CAN;
        msg.data[0..4].copy_from_slice(&can_id.to_be_bytes());
        let len = data.len().min(8);
        msg.data[4..4 + len].copy_from_slice(&data[..len]);
        msg.data_size = (4 + len) as u32;
        self.send(&msg, 100)
    }

    /// Send a message on the channel
    pub fn send(&self, msg: &PassThruMsg, timeout_ms: u32) -> Result<(), String> {
        let mut num_msgs: u32 = 1;
        let ret = unsafe {
            (self.lib.pass_thru_write_msgs)(self.channel_id, msg, &mut num_msgs, timeout_ms)
        };
        if ret != 0 {
            return Err(format!(
                "PassThruWriteMsgs failed: {}",
                J2534Error::from_code(ret)
            ));
        }
        Ok(())
    }

    /// Read messages from the channel
    pub fn read(&self, timeout_ms: u32) -> Result<Vec<PassThruMsg>, String> {
        let mut msgs = vec![PassThruMsg::default(); 10];
        let mut num_msgs: u32 = msgs.len() as u32;
        let ret = unsafe {
            (self.lib.pass_thru_read_msgs)(
                self.channel_id,
                msgs.as_mut_ptr(),
                &mut num_msgs,
                timeout_ms,
            )
        };
        // BufferEmpty (0x10) and Timeout (0x09) are not fatal — just means no messages yet
        if ret != 0 && ret != 0x10 && ret != 0x09 {
            return Err(format!(
                "PassThruReadMsgs failed: {}",
                J2534Error::from_code(ret)
            ));
        }
        msgs.truncate(num_msgs as usize);
        Ok(msgs)
    }

    /// Set ISO15765 flow control parameters via IOCTL SET_CONFIG
    pub fn set_iso15765_config(&self, bs: u32, stmin: u32, wft_max: u32) -> Result<(), String> {
        let mut configs = [
            SConfig {
                parameter: ISO15765_BS,
                value: bs,
            },
            SConfig {
                parameter: ISO15765_STMIN,
                value: stmin,
            },
            SConfig {
                parameter: ISO15765_WFT_MAX,
                value: wft_max,
            },
        ];
        let config_list = SConfigList {
            num_of_params: 3,
            config_ptr: configs.as_mut_ptr(),
        };
        let ret = unsafe {
            (self.lib.pass_thru_ioctl)(
                self.channel_id,
                SET_CONFIG,
                &config_list as *const SConfigList as *const c_void,
                std::ptr::null_mut(),
            )
        };
        if ret != 0 {
            return Err(format!(
                "PassThruIoctl SET_CONFIG failed: {}",
                J2534Error::from_code(ret)
            ));
        }
        Ok(())
    }

    /// Clear receive buffer
    pub fn clear_rx_buffer(&self) -> Result<(), String> {
        let ret = unsafe {
            (self.lib.pass_thru_ioctl)(
                self.channel_id,
                CLEAR_RX_BUFFER,
                std::ptr::null(),
                std::ptr::null_mut(),
            )
        };
        if ret != 0 {
            return Err(format!(
                "PassThruIoctl CLEAR_RX_BUFFER failed: {}",
                J2534Error::from_code(ret)
            ));
        }
        Ok(())
    }

    /// Clear transmit buffer
    pub fn clear_tx_buffer(&self) -> Result<(), String> {
        let ret = unsafe {
            (self.lib.pass_thru_ioctl)(
                self.channel_id,
                CLEAR_TX_BUFFER,
                std::ptr::null(),
                std::ptr::null_mut(),
            )
        };
        if ret != 0 {
            return Err(format!(
                "PassThruIoctl CLEAR_TX_BUFFER failed: {}",
                J2534Error::from_code(ret)
            ));
        }
        Ok(())
    }

    /// Read battery voltage from the device
    pub fn read_battery_voltage(&self) -> Result<f32, String> {
        let mut voltage: u32 = 0;
        let ret = unsafe {
            (self.lib.pass_thru_ioctl)(
                self.channel_id,
                READ_VBATT,
                std::ptr::null(),
                &mut voltage as *mut u32 as *mut c_void,
            )
        };
        if ret != 0 {
            return Err(format!(
                "PassThruIoctl READ_VBATT failed: {}",
                J2534Error::from_code(ret)
            ));
        }
        // Voltage is returned in millivolts
        Ok(voltage as f32 / 1000.0)
    }

    pub fn channel_id(&self) -> u32 {
        self.channel_id
    }
}

impl Drop for J2534Channel {
    fn drop(&mut self) {
        unsafe {
            (self.lib.pass_thru_disconnect)(self.channel_id);
        }
    }
}
