//! Native message boxes for the tray actions. Windows only; elsewhere (dev
//! on Linux) messages go to stderr and confirmations answer "no", so
//! nothing is changed without a person agreeing.

/// Asks OK / Cancel. Blocks the calling thread until answered.
pub fn confirm(title: &str, text: &str) -> bool {
    #[cfg(windows)]
    {
        win::message_box(title, text, win::Kind::Confirm)
    }
    #[cfg(not(windows))]
    {
        eprintln!("claude-buddy: {title}: {text}\n(no dialog on this platform: cancelled)");
        false
    }
}

/// Shows a message with an OK button. Blocks until closed.
pub fn inform(title: &str, text: &str, warning: bool) {
    #[cfg(windows)]
    {
        let kind = if warning {
            win::Kind::Warning
        } else {
            win::Kind::Info
        };
        win::message_box(title, text, kind);
    }
    #[cfg(not(windows))]
    {
        let _ = warning;
        eprintln!("claude-buddy: {title}: {text}");
    }
}

#[cfg(windows)]
mod win {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, IDOK, MB_ICONINFORMATION, MB_ICONWARNING, MB_OK, MB_OKCANCEL,
        MB_SETFOREGROUND, MB_TOPMOST,
    };

    pub enum Kind {
        Confirm,
        Info,
        Warning,
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    /// `MessageBoxW` without an owner, in front of everything (the app has
    /// no focused window of its own). Returns whether OK was pressed.
    pub fn message_box(title: &str, text: &str, kind: Kind) -> bool {
        let style = match kind {
            Kind::Confirm => MB_OKCANCEL | MB_ICONINFORMATION,
            Kind::Info => MB_OK | MB_ICONINFORMATION,
            Kind::Warning => MB_OK | MB_ICONWARNING,
        } | MB_SETFOREGROUND
            | MB_TOPMOST;
        let (title, text) = (wide(title), wide(text));
        // SAFETY: both strings are NUL-terminated UTF-16 that outlive the call.
        unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), style) == IDOK }
    }
}
