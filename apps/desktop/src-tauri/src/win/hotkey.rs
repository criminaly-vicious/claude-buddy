//! Global show/hide hotkey (#29). [`parse`] turns the text from the settings
//! window ("Ctrl+Alt+B") into modifiers and a virtual-key code and is
//! tested everywhere. On Windows, [`HotkeyService`] owns a thread with a
//! message loop that registers the hotkey with `RegisterHotKey` and calls
//! back on `WM_HOTKEY`; a hotkey another app already holds is reported as
//! an error, never a crash.

use std::fmt;

/// Modifier bits, the same values as Win32's `MOD_*`.
pub const ALT: u32 = 0x1;
pub const CTRL: u32 = 0x2;
pub const SHIFT: u32 = 0x4;
pub const WIN: u32 = 0x8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hotkey {
    pub modifiers: u32,
    /// Win32 virtual-key code.
    pub key: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    UnknownPart(String),
    /// Needs Ctrl, Alt or Win (Shift alone would steal normal typing).
    NeedsModifier,
    /// Exactly one non-modifier key.
    KeyCount,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "the hotkey is empty"),
            Self::UnknownPart(part) => write!(f, "\"{part}\" is not a key Claude Buddy knows"),
            Self::NeedsModifier => write!(f, "a hotkey needs Ctrl, Alt or Win"),
            Self::KeyCount => write!(f, "a hotkey needs exactly one key besides the modifiers"),
        }
    }
}

/// Parses "Ctrl+Alt+B", "win + shift + f9", "Ctrl+Alt+0". Keys: A–Z, 0–9,
/// F1–F24, Space. Case and spaces do not matter.
pub fn parse(text: &str) -> Result<Hotkey, ParseError> {
    if text.trim().is_empty() {
        return Err(ParseError::Empty);
    }
    let mut modifiers = 0;
    let mut key = None;
    for part in text.split('+').map(str::trim) {
        let upper = part.to_ascii_uppercase();
        let modifier = match upper.as_str() {
            "CTRL" | "CONTROL" => Some(CTRL),
            "ALT" => Some(ALT),
            "SHIFT" => Some(SHIFT),
            "WIN" | "WINDOWS" | "SUPER" => Some(WIN),
            _ => None,
        };
        if let Some(m) = modifier {
            modifiers |= m;
            continue;
        }
        let code = key_code(&upper).ok_or_else(|| ParseError::UnknownPart(part.to_string()))?;
        if key.replace(code).is_some() {
            return Err(ParseError::KeyCount);
        }
    }
    let key = key.ok_or(ParseError::KeyCount)?;
    if modifiers & (CTRL | ALT | WIN) == 0 {
        return Err(ParseError::NeedsModifier);
    }
    Ok(Hotkey { modifiers, key })
}

/// The canonical spelling: "Ctrl+Alt+Shift+Win+B".
pub fn format(hotkey: Hotkey) -> String {
    let mut parts: Vec<String> = [(CTRL, "Ctrl"), (ALT, "Alt"), (SHIFT, "Shift"), (WIN, "Win")]
        .iter()
        .filter(|(bit, _)| hotkey.modifiers & bit != 0)
        .map(|(_, name)| name.to_string())
        .collect();
    parts.push(key_name(hotkey.key));
    parts.join("+")
}

fn key_code(upper: &str) -> Option<u32> {
    let bytes = upper.as_bytes();
    match bytes {
        // Virtual-key codes of letters and digits are their ASCII codes.
        [c] if c.is_ascii_uppercase() || c.is_ascii_digit() => Some(u32::from(*c)),
        _ if upper == "SPACE" => Some(0x20),
        [b'F', rest @ ..] => {
            let n: u32 = std::str::from_utf8(rest).ok()?.parse().ok()?;
            (1..=24).contains(&n).then(|| 0x70 + n - 1) // VK_F1 = 0x70
        }
        _ => None,
    }
}

fn key_name(code: u32) -> String {
    match code {
        0x20 => "Space".into(),
        0x70..=0x87 => format!("F{}", code - 0x70 + 1),
        c => char::from_u32(c).map(String::from).unwrap_or_default(),
    }
}

#[cfg(windows)]
pub use service::HotkeyService;

#[cfg(windows)]
mod service {
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::sync::Mutex;
    use std::time::Duration;

    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        RegisterHotKey, UnregisterHotKey, MOD_NOREPEAT,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetMessageW, PeekMessageW, PostThreadMessageW, MSG, PM_NOREMOVE, WM_APP, WM_HOTKEY,
    };

    use super::Hotkey;

    const HOTKEY_ID: i32 = 1;
    /// Thread message: apply the hotkey waiting in the request slot.
    const WM_SET: u32 = WM_APP + 1;

    type Request = Option<Hotkey>;
    /// Sends requests to the thread and receives its answers, one at a time.
    type Channel = (Sender<Request>, Receiver<Result<(), String>>);

    /// The hotkey thread. Requests go through a channel plus a thread
    /// message, because `RegisterHotKey` binds the hotkey to the thread
    /// that calls it.
    pub struct HotkeyService {
        thread_id: u32,
        requests: Mutex<Channel>,
    }

    impl HotkeyService {
        /// Starts the thread. `on_press` runs on it at every press.
        pub fn start(on_press: impl Fn() + Send + 'static) -> std::io::Result<Self> {
            let (request_tx, request_rx) = mpsc::channel::<Request>();
            let (result_tx, result_rx) = mpsc::channel();
            let (ready_tx, ready_rx) = mpsc::channel();
            std::thread::Builder::new()
                .name("global-hotkey".into())
                .spawn(move || {
                    let mut msg: MSG = unsafe { std::mem::zeroed() };
                    // SAFETY: creates this thread's message queue before anyone posts to it.
                    unsafe { PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_NOREMOVE) };
                    // SAFETY: plain FFI call.
                    let _ = ready_tx.send(unsafe { GetCurrentThreadId() });
                    let mut registered = false;
                    // SAFETY: standard message loop on a local MSG.
                    while unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } > 0 {
                        match msg.message {
                            WM_HOTKEY if msg.wParam == HOTKEY_ID as usize => on_press(),
                            WM_SET => {
                                while let Ok(request) = request_rx.try_recv() {
                                    let _ = result_tx.send(apply(request, &mut registered));
                                }
                            }
                            _ => {}
                        }
                    }
                })?;
            let thread_id = ready_rx
                .recv()
                .map_err(|_| std::io::Error::other("hotkey thread died"))?;
            Ok(Self {
                thread_id,
                requests: Mutex::new((request_tx, result_rx)),
            })
        }

        /// Replaces the hotkey (`None`: no hotkey). Err when Windows refuses
        /// it, typically because another app already uses it.
        pub fn set(&self, hotkey: Option<Hotkey>) -> Result<(), String> {
            let guard = self.requests.lock().unwrap_or_else(|p| p.into_inner());
            let (requests, results) = &*guard;
            requests
                .send(hotkey)
                .map_err(|_| "the hotkey thread stopped".to_string())?;
            // SAFETY: posting to a thread whose queue exists (created before `start` returned).
            if unsafe { PostThreadMessageW(self.thread_id, WM_SET, 0, 0) } == 0 {
                return Err("could not reach the hotkey thread".into());
            }
            results
                .recv_timeout(Duration::from_secs(2))
                .map_err(|_| "the hotkey thread did not answer".to_string())?
        }
    }

    /// On the hotkey thread: drop the old registration, add the new one.
    fn apply(request: Option<Hotkey>, registered: &mut bool) -> Result<(), String> {
        if *registered {
            // SAFETY: unregisters the id this thread registered.
            unsafe { UnregisterHotKey(std::ptr::null_mut(), HOTKEY_ID) };
            *registered = false;
        }
        let Some(hotkey) = request else {
            return Ok(());
        };
        // SAFETY: thread-bound registration (null window); no pointers involved.
        let ok = unsafe {
            RegisterHotKey(
                std::ptr::null_mut(),
                HOTKEY_ID,
                hotkey.modifiers | MOD_NOREPEAT,
                hotkey.key,
            )
        };
        if ok == 0 {
            return Err(format!(
                "{} is already used by another app or by Windows; choose another",
                super::format(hotkey)
            ));
        }
        *registered = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_default() {
        assert_eq!(
            parse("Ctrl+Alt+B"),
            Ok(Hotkey {
                modifiers: CTRL | ALT,
                key: 0x42
            })
        );
    }

    #[test]
    fn ignores_case_spacing_and_order() {
        assert_eq!(parse(" alt + control + b "), parse("Ctrl+Alt+B"));
        assert_eq!(
            parse("win+shift+f9"),
            Ok(Hotkey {
                modifiers: WIN | SHIFT,
                key: 0x78
            })
        );
        assert_eq!(parse("Ctrl+Alt+7").unwrap().key, u32::from(b'7'));
        assert_eq!(parse("Ctrl+Space").unwrap().key, 0x20);
        assert_eq!(parse("Alt+F24").unwrap().key, 0x87);
    }

    #[test]
    fn rejects_what_cannot_be_a_hotkey() {
        assert_eq!(parse(""), Err(ParseError::Empty));
        assert_eq!(parse("Shift+B"), Err(ParseError::NeedsModifier));
        assert_eq!(parse("B"), Err(ParseError::NeedsModifier));
        assert_eq!(parse("Ctrl+Alt"), Err(ParseError::KeyCount));
        assert_eq!(parse("Ctrl+A+B"), Err(ParseError::KeyCount));
        assert_eq!(
            parse("Ctrl+F25"),
            Err(ParseError::UnknownPart("F25".into()))
        );
        assert_eq!(parse("Ctrl+É"), Err(ParseError::UnknownPart("É".into())));
        assert_eq!(
            parse("Hyper+B"),
            Err(ParseError::UnknownPart("Hyper".into()))
        );
    }

    #[test]
    fn formats_canonically() {
        assert_eq!(format(parse("b+alt+ctrl").unwrap()), "Ctrl+Alt+B");
        assert_eq!(format(parse("shift+win+f1").unwrap()), "Shift+Win+F1");
        assert_eq!(format(parse("ctrl+space").unwrap()), "Ctrl+Space");
        let messages = [
            ParseError::Empty,
            ParseError::NeedsModifier,
            ParseError::KeyCount,
        ];
        assert!(messages.iter().all(|e| !e.to_string().is_empty()));
    }
}
