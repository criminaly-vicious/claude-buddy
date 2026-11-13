//! Windows: holds a handle to the process and blocks until it is signalled.
//! The handle keeps the process object alive, so PID reuse cannot fool us,
//! and nothing polls.

use std::io;

use windows_sys::Win32::Foundation::{CloseHandle, ERROR_INVALID_PARAMETER, HANDLE};
use windows_sys::Win32::System::Threading::{
    OpenProcess, WaitForSingleObject, INFINITE, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_SYNCHRONIZE,
};

pub fn watch(pid: u32, on_exit: impl FnOnce() + Send + 'static) -> io::Result<()> {
    let process = ProcessHandle::open(pid)?;
    std::thread::Builder::new()
        .name(format!("liveness-{pid}"))
        .spawn(move || {
            if let Some(process) = process {
                process.wait_for_exit();
            }
            on_exit();
        })?;
    Ok(())
}

/// Owned process handle, closed on drop.
struct ProcessHandle(HANDLE);

// SAFETY: a process handle can be used from any thread.
unsafe impl Send for ProcessHandle {}

impl ProcessHandle {
    /// Opens `pid` for waiting. `Ok(None)` when no such process exists.
    fn open(pid: u32) -> io::Result<Option<Self>> {
        // SAFETY: plain FFI call; a null return is handled below.
        let handle = unsafe {
            OpenProcess(
                PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
                0,
                pid,
            )
        };
        if !handle.is_null() {
            return Ok(Some(Self(handle)));
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) {
            Ok(None)
        } else {
            Err(error)
        }
    }

    /// Blocks until the process exits.
    fn wait_for_exit(&self) {
        // SAFETY: `self.0` is a valid handle opened with SYNCHRONIZE.
        unsafe { WaitForSingleObject(self.0, INFINITE) };
    }
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        // SAFETY: the handle is owned and closed exactly once.
        unsafe { CloseHandle(self.0) };
    }
}
