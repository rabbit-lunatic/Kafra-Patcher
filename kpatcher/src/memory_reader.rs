//! Memory reader for Ragnarok Online client process.
//!
//! Reads game state (player name, map, levels) from the running game process
//! using Windows API (OpenProcess + ReadProcessMemory).
//! Ported from HorizonRichPresence C++ MemoryReader.

#![cfg(windows)]

use std::ffi::OsString;
use std::mem;
use std::os::windows::ffi::OsStringExt;
use std::path::Path;
use std::ptr;

use winapi::shared::minwindef::{DWORD, FALSE, HMODULE, MAX_PATH};
use winapi::um::handleapi::CloseHandle;
use winapi::um::memoryapi::ReadProcessMemory;
use winapi::um::processthreadsapi::OpenProcess;
use winapi::um::psapi::{EnumProcessModulesEx, GetModuleBaseNameW, LIST_MODULES_ALL};
use winapi::um::winnt::{HANDLE, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};

// Memory offsets from HorizonRichPresence (RichPresence.h)
const OFFSET_BASE_LV: usize = 0x11D4750;
const OFFSET_JOB_LV: usize = 0x11D4758;
const OFFSET_NAME: usize = 0x11DB2B8;
const OFFSET_CITY_NAME: usize = 0x11D470C;
const OFFSET_LOGIN_CHECK: usize = 0xDEDEC0;

/// Game data extracted from the running client's memory.
#[derive(Debug, Clone)]
pub struct GameData {
    pub player_name: String,
    pub map_name: String,
    pub base_level: i32,
    pub job_level: i32,
    pub is_in_login: bool,
}

/// RAII wrapper around a Windows process HANDLE.
pub struct ProcessHandle {
    handle: HANDLE,
}

impl ProcessHandle {
    pub fn raw(&self) -> HANDLE {
        self.handle
    }
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }
}

/// Find the PID of a running process by executable name.
///
/// Uses `sysinfo` to enumerate processes. The `exe_name` is matched
/// case-insensitively against the process file name (basename only).
pub fn find_game_process(exe_name: &str) -> Option<u32> {
    use sysinfo::System;

    let target = Path::new(exe_name)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();

    let mut sys = System::new();
    sys.refresh_processes();

    for (pid, process) in sys.processes() {
        let proc_name = process
            .name()
            .to_lowercase();
        if proc_name == target {
            return Some(pid.as_u32());
        }
    }
    None
}

/// Open a process for reading its memory.
pub fn open_process(pid: u32) -> Option<ProcessHandle> {
    let handle = unsafe {
        OpenProcess(
            PROCESS_VM_READ | PROCESS_QUERY_INFORMATION,
            FALSE,
            pid as DWORD,
        )
    };
    if handle.is_null() {
        None
    } else {
        Some(ProcessHandle { handle })
    }
}

/// Get the base address of the main module of the target process.
///
/// Enumerates loaded modules via `EnumProcessModulesEx` and matches the
/// module whose basename equals `module_name` (case-insensitive).
pub fn get_module_base(proc_handle: &ProcessHandle, module_name: &str) -> Option<usize> {
    let handle = proc_handle.raw();
    let target = Path::new(module_name)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();

    let mut modules: Vec<HMODULE> = vec![ptr::null_mut(); 1024];
    let mut cb_needed: DWORD = 0;

    let success = unsafe {
        EnumProcessModulesEx(
            handle,
            modules.as_mut_ptr(),
            (modules.len() * mem::size_of::<HMODULE>()) as DWORD,
            &mut cb_needed,
            LIST_MODULES_ALL,
        )
    };

    if success == 0 {
        return None;
    }

    let count = cb_needed as usize / mem::size_of::<HMODULE>();
    for i in 0..count {
        let module = modules[i];
        let mut name_buf = [0u16; MAX_PATH];
        let len = unsafe {
            GetModuleBaseNameW(handle, module, name_buf.as_mut_ptr(), MAX_PATH as DWORD)
        };
        if len == 0 {
            continue;
        }
        let mod_name = OsString::from_wide(&name_buf[..len as usize])
            .to_string_lossy()
            .to_lowercase();
        if mod_name == target {
            return Some(module as usize);
        }
    }
    None
}

/// Read a 32-bit integer from the target process memory.
fn read_i32(handle: HANDLE, address: usize) -> Option<i32> {
    let mut value: i32 = 0;
    let mut bytes_read: usize = 0;
    let success = unsafe {
        ReadProcessMemory(
            handle,
            address as *const _,
            &mut value as *mut i32 as *mut _,
            mem::size_of::<i32>(),
            &mut bytes_read,
        )
    };
    if success != 0 && bytes_read == mem::size_of::<i32>() {
        Some(value)
    } else {
        None
    }
}

/// Read a null-terminated string from the target process memory.
fn read_string(handle: HANDLE, address: usize, max_len: usize) -> Option<String> {
    let mut buffer = vec![0u8; max_len + 1];
    let mut bytes_read: usize = 0;
    let success = unsafe {
        ReadProcessMemory(
            handle,
            address as *const _,
            buffer.as_mut_ptr() as *mut _,
            max_len,
            &mut bytes_read,
        )
    };
    if success == 0 || bytes_read == 0 {
        return None;
    }
    // Truncate at first null byte
    let end = buffer.iter().position(|&b| b == 0).unwrap_or(bytes_read);
    String::from_utf8_lossy(&buffer[..end])
        .to_string()
        .into()
}

/// Read all game data from the target process in one call.
///
/// Returns `None` if any critical read fails (process likely closed).
pub fn read_game_data(proc_handle: &ProcessHandle, base_address: usize) -> Option<GameData> {
    let handle = proc_handle.raw();

    let base_level = read_i32(handle, base_address + OFFSET_BASE_LV)?;
    let job_level = read_i32(handle, base_address + OFFSET_JOB_LV)?;
    let player_name = read_string(handle, base_address + OFFSET_NAME, 24)?;
    let map_name = read_string(handle, base_address + OFFSET_CITY_NAME, 64)?;
    let login_check = read_string(handle, base_address + OFFSET_LOGIN_CHECK, 32)
        .unwrap_or_default();

    let is_in_login = login_check.to_lowercase().contains("login.r") || map_name.is_empty();

    Some(GameData {
        player_name,
        map_name,
        base_level,
        job_level,
        is_in_login,
    })
}
