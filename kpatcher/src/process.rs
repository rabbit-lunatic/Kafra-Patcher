use anyhow::Result;

/// Starts an executable file in a cross-platform way.
///
/// This is the Windows version.
#[cfg(windows)]
pub fn start_executable<I, S>(exe_path: &str, exe_arguments: I) -> Result<bool>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    // Fold parameter list into a String
    let exe_parameter = exe_arguments
        .into_iter()
        .fold(String::new(), |a: String, b| a + " " + b.as_ref() + "");
    windows::win32_spawn_process_runas(exe_path, &exe_parameter)
}

/// Starts an executable file in a cross-platform way.
///
/// This is the non-Windows version.
#[cfg(not(windows))]
pub fn start_executable<I, S>(exe_path: &str, exe_arguments: I) -> Result<bool>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    use std::process::Command;

    let exe_arguments: Vec<String> = exe_arguments
        .into_iter()
        .map(|e| e.as_ref().into())
        .collect();
    Command::new(exe_path)
        .args(exe_arguments)
        .spawn()
        .map(|_| Ok(true))?
}

// Note: Taken from the rustup project
#[cfg(windows)]
mod windows {
    use anyhow::{anyhow, Result};
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    fn to_u16s<S: AsRef<OsStr>>(s: S) -> Result<Vec<u16>> {
        fn inner(s: &OsStr) -> Result<Vec<u16>> {
            let mut maybe_result: Vec<u16> = s.encode_wide().collect();
            if maybe_result.iter().any(|&u| u == 0) {
                return Err(anyhow!("strings passed to WinAPI cannot contain NULs"));
            }
            maybe_result.push(0);
            Ok(maybe_result)
        }
        inner(s.as_ref())
    }

    /// Spawns a process using ShellExecuteExW.
    /// First attempts to launch without elevation ("open"), then falls back
    /// to elevation ("runas") if the initial attempt fails.
    pub fn win32_spawn_process_runas<S>(path: S, parameter: S) -> Result<bool>
    where
        S: AsRef<OsStr>,
    {
        use std::ptr;
        use winapi::ctypes::c_int;
        use winapi::shared::minwindef::{BOOL, ULONG};
        use winapi::um::shellapi::SHELLEXECUTEINFOW;
        extern "system" {
            pub fn ShellExecuteExW(pExecInfo: *mut SHELLEXECUTEINFOW) -> BOOL;
        }
        const SEE_MASK_CLASSNAME: ULONG = 1;
        const SW_SHOW: c_int = 5;

        // Note: It seems `path` has to be absolute for the class overwrite to work
        let exe_path = std::env::current_dir()?.join(path.as_ref());
        
        // Get the directory of the executable to use as the working directory
        // This is critical for the game to find its DLLs and dependencies
        let exe_dir = exe_path.parent().map(|p| p.to_path_buf());
        let exe_dir_u16 = exe_dir
            .as_ref()
            .and_then(|p| p.to_str())
            .map(|s| to_u16s(s).ok())
            .flatten();
        
        let exe_path = to_u16s(exe_path.to_str().unwrap_or(""))?;
        let parameter = to_u16s(parameter)?;
        let class = to_u16s("exefile")?;

        // Helper to attempt launch with a given verb
        let try_launch = |verb: &str| -> Result<bool> {
            let verb_u16 = to_u16s(verb)?;
            let mut execute_info = SHELLEXECUTEINFOW {
                cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
                fMask: SEE_MASK_CLASSNAME,
                hwnd: ptr::null_mut(),
                lpVerb: verb_u16.as_ptr(),
                lpFile: exe_path.as_ptr(),
                lpParameters: parameter.as_ptr(),
                lpDirectory: exe_dir_u16
                    .as_ref()
                    .map(|v| v.as_ptr())
                    .unwrap_or(ptr::null()),
                nShow: SW_SHOW,
                hInstApp: ptr::null_mut(),
                lpIDList: ptr::null_mut(),
                lpClass: class.as_ptr(),
                hkeyClass: ptr::null_mut(),
                dwHotKey: 0,
                hMonitor: ptr::null_mut(),
                hProcess: ptr::null_mut(),
            };
            let result = unsafe { ShellExecuteExW(&mut execute_info) };
            Ok(result != 0)
        };

        // Try without elevation first
        match try_launch("open") {
            Ok(true) => {
                log::info!("Process launched without elevation");
                Ok(true)
            }
            _ => {
                // Fallback to elevation
                log::info!("Retrying with elevation (runas)");
                try_launch("runas")
            }
        }
    }
}
