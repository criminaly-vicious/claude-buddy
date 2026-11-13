//! One Claude Buddy per Windows session (#27). The first instance owns a
//! named mutex and waits on a named event; a second launch finds the
//! mutex, signals the event (the first one then shows its buddies) and
//! exits. Off Windows every launch is a first instance (development only).

/// Name prefix for the kernel objects, in the session's `Local\` namespace.
/// Shared by debug and release builds: two instances would also fight over
/// `bridge.json`.
#[cfg_attr(not(windows), allow(dead_code))]
const NAME: &str = "Local\\io.github.criminaly-vicious.claude-buddy";

pub enum Instance {
    /// Keep running. `on_launch` is called whenever another launch is attempted.
    First,
    /// Another instance is running and was told to show itself: exit now.
    #[cfg_attr(not(windows), allow(dead_code))]
    Second,
}

/// Claims the single instance. `on_launch` runs on a background thread
/// each time a later launch hands over to this one.
pub fn claim(on_launch: impl Fn() + Send + 'static) -> Instance {
    #[cfg(windows)]
    {
        win::claim(on_launch)
    }
    #[cfg(not(windows))]
    {
        let _ = on_launch;
        Instance::First
    }
}

#[cfg(windows)]
mod win {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{
        CreateEventW, CreateMutexW, OpenEventW, SetEvent, WaitForSingleObject, EVENT_MODIFY_STATE,
        INFINITE,
    };

    use super::{Instance, NAME};

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    pub fn claim(on_launch: impl Fn() + Send + 'static) -> Instance {
        let mutex_name = wide(&format!("{NAME}.instance"));
        let event_name = wide(&format!("{NAME}.show"));
        // SAFETY: plain FFI calls with NUL-terminated names. The handles are
        // deliberately kept open for the life of the process: they *are* the
        // single-instance claim.
        unsafe {
            let mutex = CreateMutexW(std::ptr::null(), 0, mutex_name.as_ptr());
            if mutex.is_null() {
                // Cannot tell; run rather than refuse to start.
                return Instance::First;
            }
            if GetLastError() == ERROR_ALREADY_EXISTS {
                let event = OpenEventW(EVENT_MODIFY_STATE, 0, event_name.as_ptr());
                if !event.is_null() {
                    SetEvent(event);
                }
                return Instance::Second;
            }
            // Auto-reset: one wake per later launch.
            let event = CreateEventW(std::ptr::null(), 0, 0, event_name.as_ptr());
            if event.is_null() {
                return Instance::First;
            }
            let event = event as usize; // HANDLE is a pointer; pass it to the thread as a number.
            let _ = std::thread::Builder::new()
                .name("single-instance".into())
                .spawn(move || {
                    while WaitForSingleObject(event as _, INFINITE) == WAIT_OBJECT_0 {
                        on_launch();
                    }
                });
        }
        Instance::First
    }
}
