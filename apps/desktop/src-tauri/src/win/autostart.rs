//! Start with Windows (#29): a value named [`VALUE_NAME`] under
//! `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, per user, no admin.
//!
//! Task Manager's "Startup apps" can switch an entry off without removing
//! it; it records that under `...\Explorer\StartupApproved\Run`. The real
//! state is therefore "our Run value exists and is not switched off there".
//! Turning autostart on from the settings removes our switch-off record, so
//! the checkbox always means what it shows.

/// Name of our value under `Run` (and `StartupApproved\Run`).
pub const VALUE_NAME: &str = "Claude Buddy";

/// The command stored under `Run`: the quoted executable path.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn run_command(exe: &std::path::Path) -> String {
    format!("\"{}\"", exe.display())
}

/// Whether a `StartupApproved\Run` value lets the entry run. The first byte
/// is 2 or 6 when enabled and odd (3, 7) when switched off.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn approved(data: &[u8]) -> bool {
    data.first().is_none_or(|b| b & 1 == 0)
}

/// Whether Claude Buddy starts with Windows. `None` where unsupported.
pub fn is_enabled() -> Option<bool> {
    #[cfg(windows)]
    {
        Some(win::is_enabled())
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Turns autostart on (for the running executable) or off.
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        win::set_enabled(enabled)
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
        Err("Start with Windows is only available on Windows".into())
    }
}

/// At startup: if autostart is on but points to another copy of the app
/// (moved or updated), point it to this one. Release builds only, so a
/// development build never takes over the installed app's entry.
pub fn repair() {
    #[cfg(windows)]
    if !cfg!(debug_assertions) {
        win::repair();
    }
}

#[cfg(windows)]
mod win {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
        RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE,
        REG_OPTION_NON_VOLATILE, REG_SZ,
    };

    use super::{approved, run_command, VALUE_NAME};

    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const APPROVED: &str =
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    /// An open registry key, closed on drop.
    struct Key(HKEY);

    impl Drop for Key {
        fn drop(&mut self) {
            // SAFETY: the key is owned and closed exactly once.
            unsafe { RegCloseKey(self.0) };
        }
    }

    impl Key {
        fn open(path: &str, access: u32) -> Option<Self> {
            let path = wide(path);
            let mut key: HKEY = std::ptr::null_mut();
            // SAFETY: NUL-terminated path; `key` receives the handle.
            let status =
                unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, access, &mut key) };
            (status == ERROR_SUCCESS).then_some(Self(key))
        }

        fn create(path: &str) -> Result<Self, String> {
            let path = wide(path);
            let mut key: HKEY = std::ptr::null_mut();
            // SAFETY: NUL-terminated path; null class and security; `key` receives the handle.
            let status = unsafe {
                RegCreateKeyExW(
                    HKEY_CURRENT_USER,
                    path.as_ptr(),
                    0,
                    std::ptr::null(),
                    REG_OPTION_NON_VOLATILE,
                    KEY_SET_VALUE | KEY_QUERY_VALUE,
                    std::ptr::null(),
                    &mut key,
                    std::ptr::null_mut(),
                )
            };
            if status == ERROR_SUCCESS {
                Ok(Self(key))
            } else {
                Err(format!("could not open the Run key (error {status})"))
            }
        }

        /// The raw bytes of value `name`, if present.
        fn read(&self, name: &str) -> Option<Vec<u8>> {
            let name = wide(name);
            let mut size = 0u32;
            // SAFETY: size query with a null buffer.
            let status = unsafe {
                RegQueryValueExW(
                    self.0,
                    name.as_ptr(),
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    &mut size,
                )
            };
            if status != ERROR_SUCCESS {
                return None;
            }
            let mut data = vec![0u8; size as usize];
            // SAFETY: `data` has the `size` bytes the query asked for.
            let status = unsafe {
                RegQueryValueExW(
                    self.0,
                    name.as_ptr(),
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    data.as_mut_ptr(),
                    &mut size,
                )
            };
            (status == ERROR_SUCCESS).then(|| {
                data.truncate(size as usize);
                data
            })
        }

        fn read_string(&self, name: &str) -> Option<String> {
            let bytes = self.read(name)?;
            let units: Vec<u16> = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes(*c))
                .take_while(|&u| u != 0)
                .collect();
            Some(String::from_utf16_lossy(&units))
        }

        fn write_string(&self, name: &str, value: &str) -> Result<(), String> {
            let name = wide(name);
            let data = wide(value);
            // SAFETY: REG_SZ data as NUL-terminated UTF-16, its byte length given.
            let status = unsafe {
                RegSetValueExW(
                    self.0,
                    name.as_ptr(),
                    0,
                    REG_SZ,
                    data.as_ptr().cast(),
                    (data.len() * 2) as u32,
                )
            };
            if status == ERROR_SUCCESS {
                Ok(())
            } else {
                Err(format!("could not write the Run value (error {status})"))
            }
        }

        fn delete(&self, name: &str) {
            let name = wide(name);
            // SAFETY: NUL-terminated name; a missing value is fine.
            unsafe { RegDeleteValueW(self.0, name.as_ptr()) };
        }
    }

    pub fn is_enabled() -> bool {
        let present = Key::open(RUN, KEY_QUERY_VALUE).is_some_and(|k| k.read(VALUE_NAME).is_some());
        let switched_off = Key::open(APPROVED, KEY_QUERY_VALUE)
            .and_then(|k| k.read(VALUE_NAME))
            .is_some_and(|data| !approved(&data));
        present && !switched_off
    }

    pub fn set_enabled(enabled: bool) -> Result<(), String> {
        let run = Key::create(RUN)?;
        if enabled {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            run.write_string(VALUE_NAME, &run_command(&exe))?;
            // Undo a switch-off from Task Manager, so "on" really is on.
            if let Some(approved) = Key::open(APPROVED, KEY_SET_VALUE) {
                approved.delete(VALUE_NAME);
            }
        } else {
            run.delete(VALUE_NAME);
        }
        Ok(())
    }

    pub fn repair() {
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let wanted = run_command(&exe);
        if let Some(run) = Key::open(RUN, KEY_QUERY_VALUE | KEY_SET_VALUE) {
            if run.read_string(VALUE_NAME).is_some_and(|v| v != wanted) {
                let _ = run.write_string(VALUE_NAME, &wanted);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_the_executable() {
        let exe = std::path::Path::new("/Program Files/Claude Buddy/claude-buddy.exe");
        assert_eq!(
            run_command(exe),
            "\"/Program Files/Claude Buddy/claude-buddy.exe\""
        );
    }

    #[test]
    fn reads_task_manager_switch_offs() {
        assert!(approved(&[2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]));
        assert!(approved(&[6, 0]));
        assert!(!approved(&[3, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8]));
        assert!(!approved(&[7]));
        assert!(approved(&[]));
    }
}
