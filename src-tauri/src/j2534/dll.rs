use std::ffi::c_void;
use std::path::PathBuf;

use crate::j2534::types::*;

/// Type aliases for J2534 DLL function pointers
type PassThruOpenFn = unsafe extern "system" fn(*const c_void, *mut u32) -> u32;
type PassThruCloseFn = unsafe extern "system" fn(u32) -> u32;
type PassThruConnectFn = unsafe extern "system" fn(u32, u32, u32, u32, *mut u32) -> u32;
type PassThruDisconnectFn = unsafe extern "system" fn(u32) -> u32;
type PassThruReadMsgsFn = unsafe extern "system" fn(u32, *mut PassThruMsg, *mut u32, u32) -> u32;
type PassThruWriteMsgsFn = unsafe extern "system" fn(u32, *const PassThruMsg, *mut u32, u32) -> u32;
type PassThruStartMsgFilterFn =
    unsafe extern "system" fn(u32, u32, *const PassThruMsg, *const PassThruMsg, *const PassThruMsg, *mut u32) -> u32;
type PassThruStopMsgFilterFn = unsafe extern "system" fn(u32, u32) -> u32;
type PassThruIoctlFn = unsafe extern "system" fn(u32, u32, *const c_void, *mut c_void) -> u32;
type PassThruReadVersionFn = unsafe extern "system" fn(u32, *mut u8, *mut u8, *mut u8) -> u32;

/// Holds a dynamically loaded J2534 DLL and its function pointers
pub struct J2534Lib {
    _lib: libloading::Library,
    pub pass_thru_open: PassThruOpenFn,
    pub pass_thru_close: PassThruCloseFn,
    pub pass_thru_connect: PassThruConnectFn,
    pub pass_thru_disconnect: PassThruDisconnectFn,
    pub pass_thru_read_msgs: PassThruReadMsgsFn,
    pub pass_thru_write_msgs: PassThruWriteMsgsFn,
    pub pass_thru_start_msg_filter: PassThruStartMsgFilterFn,
    pub pass_thru_stop_msg_filter: PassThruStopMsgFilterFn,
    pub pass_thru_ioctl: PassThruIoctlFn,
    pub pass_thru_read_version: PassThruReadVersionFn,
}

impl J2534Lib {
    /// Load J2534 DLL from the given path
    pub fn load(dll_path: &str) -> Result<Self, String> {
        unsafe {
            let lib = libloading::Library::new(dll_path)
                .map_err(|e| format!("Failed to load J2534 DLL '{}': {}", dll_path, e))?;

            let pass_thru_open = *lib
                .get::<PassThruOpenFn>(b"PassThruOpen\0")
                .map_err(|e| format!("PassThruOpen not found: {}", e))?;
            let pass_thru_close = *lib
                .get::<PassThruCloseFn>(b"PassThruClose\0")
                .map_err(|e| format!("PassThruClose not found: {}", e))?;
            let pass_thru_connect = *lib
                .get::<PassThruConnectFn>(b"PassThruConnect\0")
                .map_err(|e| format!("PassThruConnect not found: {}", e))?;
            let pass_thru_disconnect = *lib
                .get::<PassThruDisconnectFn>(b"PassThruDisconnect\0")
                .map_err(|e| format!("PassThruDisconnect not found: {}", e))?;
            let pass_thru_read_msgs = *lib
                .get::<PassThruReadMsgsFn>(b"PassThruReadMsgs\0")
                .map_err(|e| format!("PassThruReadMsgs not found: {}", e))?;
            let pass_thru_write_msgs = *lib
                .get::<PassThruWriteMsgsFn>(b"PassThruWriteMsgs\0")
                .map_err(|e| format!("PassThruWriteMsgs not found: {}", e))?;
            let pass_thru_start_msg_filter = *lib
                .get::<PassThruStartMsgFilterFn>(b"PassThruStartMsgFilter\0")
                .map_err(|e| format!("PassThruStartMsgFilter not found: {}", e))?;
            let pass_thru_stop_msg_filter = *lib
                .get::<PassThruStopMsgFilterFn>(b"PassThruStopMsgFilter\0")
                .map_err(|e| format!("PassThruStopMsgFilter not found: {}", e))?;
            let pass_thru_ioctl = *lib
                .get::<PassThruIoctlFn>(b"PassThruIoctl\0")
                .map_err(|e| format!("PassThruIoctl not found: {}", e))?;
            let pass_thru_read_version = *lib
                .get::<PassThruReadVersionFn>(b"PassThruReadVersion\0")
                .map_err(|e| format!("PassThruReadVersion not found: {}", e))?;

            Ok(Self {
                _lib: lib,
                pass_thru_open,
                pass_thru_close,
                pass_thru_connect,
                pass_thru_disconnect,
                pass_thru_read_msgs,
                pass_thru_write_msgs,
                pass_thru_start_msg_filter,
                pass_thru_stop_msg_filter,
                pass_thru_ioctl,
                pass_thru_read_version,
            })
        }
    }
}

/// Discover J2534 DLL paths from Windows registry.
/// Searches both native and WOW6432Node paths to catch all devices.
#[cfg(target_os = "windows")]
pub fn discover_j2534_dlls() -> Vec<(String, PathBuf)> {
    use winreg::enums::*;
    use winreg::RegKey;

    let mut results = Vec::new();
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    // Search both registry views — 32-bit and 64-bit
    let registry_paths = [
        (r"SOFTWARE\PassThruSupport.04.04", KEY_READ | KEY_WOW64_64KEY),
        (r"SOFTWARE\PassThruSupport.04.04", KEY_READ | KEY_WOW64_32KEY),
    ];

    let mut seen_dlls = std::collections::HashSet::new();

    for (path, flags) in &registry_paths {
        if let Ok(key) = hklm.open_subkey_with_flags(path, *flags) {
            for name in key.enum_keys().filter_map(|k| k.ok()) {
                if let Ok(subkey) = key.open_subkey_with_flags(&name, KEY_READ) {
                    if let Ok(dll_path) = subkey.get_value::<String, _>("FunctionLibrary") {
                        // Deduplicate by DLL path (case-insensitive)
                        let dll_lower = dll_path.to_lowercase();
                        if seen_dlls.contains(&dll_lower) {
                            continue;
                        }
                        seen_dlls.insert(dll_lower);

                        let device_name = subkey
                            .get_value::<String, _>("Name")
                            .unwrap_or_else(|_| name.clone());
                        results.push((device_name, PathBuf::from(dll_path)));
                    }
                }
            }
        }
    }

    results
}

#[cfg(not(target_os = "windows"))]
pub fn discover_j2534_dlls() -> Vec<(String, PathBuf)> {
    // On non-Windows, return empty — J2534 is Windows-only
    Vec::new()
}

/// Get the default Mongoose Pro JLR DLL path
pub fn default_mongoose_dll_path() -> PathBuf {
    PathBuf::from(r"C:\Program Files (x86)\Drew Technologies, Inc\J2534\MongoosePro JLR\monpj432.dll")
}
