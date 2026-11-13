//! Spike #25 prototype (docs/session-detection.md#windows-terminal-tabs).
//!
//! `wt-tabs`         lists every Windows Terminal window and its tabs.
//! `wt-tabs <pid>`   reads the console title of process `pid` (e.g. the
//!                   `CLAUDE_PID` of a session), finds the tab with that
//!                   title, selects it and brings its window to the front.
//!
//! Run it from a terminal on Windows; it prints what it found and how long
//! each step took.

#[cfg(not(windows))]
fn main() {
    eprintln!("wt-tabs only runs on Windows");
}

#[cfg(windows)]
fn main() {
    let pid = std::env::args().nth(1).and_then(|a| a.parse::<u32>().ok());
    let mut out = String::new();
    let started = std::time::Instant::now();

    let title = pid.map(|pid| {
        let t = std::time::Instant::now();
        let title = win::console_title(pid);
        out += &format!(
            "console title of {pid}: {title:?} ({} ms)\n",
            t.elapsed().as_millis()
        );
        title
    });

    let t = std::time::Instant::now();
    let tabs = match win::terminal_tabs() {
        Ok(tabs) => tabs,
        Err(e) => {
            println!("{out}UI Automation failed: {e}");
            return;
        }
    };
    out += &format!(
        "{} tab(s) in Windows Terminal ({} ms)\n",
        tabs.len(),
        t.elapsed().as_millis()
    );
    for tab in &tabs {
        out += &format!(
            "  window {:#x} tab {} {}{:?}\n",
            tab.hwnd,
            tab.index,
            if tab.selected { "* " } else { "  " },
            tab.name
        );
    }

    if let Some(Some(title)) = title {
        let wanted = strip_status(&title);
        let matches: Vec<_> = tabs
            .iter()
            .filter(|tab| strip_status(&tab.name) == wanted)
            .collect();
        match matches.as_slice() {
            [] => out += "no tab has that title: fall back to focusing the window\n",
            [tab] => {
                let t = std::time::Instant::now();
                let ok = win::select(tab);
                out += &format!(
                    "selected window {:#x} tab {}: {ok} ({} ms)\n",
                    tab.hwnd,
                    tab.index,
                    t.elapsed().as_millis()
                );
            }
            many => {
                out += &format!(
                    "{} tabs share that title: ambiguous, fall back to the window\n",
                    many.len()
                )
            }
        }
    }
    out += &format!("total {} ms\n", started.elapsed().as_millis());
    print!("{out}");
}

/// Claude Code puts a status glyph and a space in front of the title in
/// terminals without OSC 7501 support; it changes with the session state,
/// so it is ignored when comparing.
#[cfg_attr(not(windows), allow(dead_code))]
fn strip_status(title: &str) -> &str {
    let title = title.trim();
    match title.split_once(' ') {
        Some((first, rest)) if !first.chars().any(char::is_alphanumeric) => rest.trim_start(),
        _ => title,
    }
}

#[cfg(windows)]
mod win {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::System::Console::{
        AttachConsole, FreeConsole, GetConsoleTitleW, ATTACH_PARENT_PROCESS,
    };
    use windows::Win32::System::Variant::VARIANT;
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationSelectionItemPattern, TreeScope_Descendants,
        UIA_ControlTypePropertyId, UIA_SelectionItemPatternId, UIA_TabItemControlTypeId,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, SetForegroundWindow, ShowWindow, SW_RESTORE,
    };

    /// Windows Terminal's top-level window class.
    const WT_CLASS: &str = "CASCADIA_HOSTING_WINDOW_CLASS";

    pub struct Tab {
        pub hwnd: isize,
        pub index: usize,
        pub name: String,
        pub selected: bool,
        pattern: Option<IUIAutomationSelectionItemPattern>,
    }

    /// The title of the console `pid` is attached to. Under Windows
    /// Terminal that is the pseudo console (OpenConsole.exe), which keeps
    /// the title the program set with OSC 0/2 or `SetConsoleTitle`.
    /// Detaches from our own console meanwhile, then reattaches to it.
    pub fn console_title(pid: u32) -> Option<String> {
        unsafe {
            let _ = FreeConsole();
            let attached = AttachConsole(pid).is_ok();
            let mut buf = [0u16; 1024];
            let len = if attached {
                GetConsoleTitleW(&mut buf)
            } else {
                0
            };
            let _ = FreeConsole();
            let _ = AttachConsole(ATTACH_PARENT_PROCESS);
            (len > 0).then(|| String::from_utf16_lossy(&buf[..len as usize]))
        }
    }

    pub fn terminal_tabs() -> windows::core::Result<Vec<Tab>> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let uia: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)?;
            let is_tab = uia.CreatePropertyCondition(
                UIA_ControlTypePropertyId,
                &VARIANT::from(UIA_TabItemControlTypeId.0),
            )?;
            let mut tabs = Vec::new();
            for hwnd in terminal_windows() {
                let root = uia.ElementFromHandle(HWND(hwnd as _))?;
                let found = root.FindAll(TreeScope_Descendants, &is_tab)?;
                for index in 0..found.Length()? {
                    let item = found.GetElement(index)?;
                    let pattern = item
                        .GetCurrentPatternAs::<IUIAutomationSelectionItemPattern>(
                            UIA_SelectionItemPatternId,
                        )
                        .ok();
                    let selected = pattern
                        .as_ref()
                        .and_then(|p| p.CurrentIsSelected().ok())
                        .is_some_and(|b| b.as_bool());
                    tabs.push(Tab {
                        hwnd,
                        index: index as usize,
                        name: item.CurrentName()?.to_string(),
                        selected,
                        pattern,
                    });
                }
            }
            Ok(tabs)
        }
    }

    /// Selects the tab, then brings its window to the front.
    pub fn select(tab: &Tab) -> bool {
        let Some(pattern) = &tab.pattern else {
            return false;
        };
        unsafe {
            let selected = pattern.Select().is_ok();
            let hwnd = HWND(tab.hwnd as _);
            let _ = ShowWindow(hwnd, SW_RESTORE);
            selected && SetForegroundWindow(hwnd).as_bool()
        }
    }

    fn terminal_windows() -> Vec<isize> {
        let mut found: Vec<isize> = Vec::new();
        unsafe {
            let _ = EnumWindows(
                Some(collect),
                LPARAM(&mut found as *mut Vec<isize> as isize),
            );
        }
        found
    }

    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let mut class = [0u16; 64];
        let len = GetClassNameW(hwnd, &mut class) as usize;
        if String::from_utf16_lossy(&class[..len]) == WT_CLASS {
            (*(lparam.0 as *mut Vec<isize>)).push(hwnd.0 as isize);
        }
        true.into()
    }
}

#[cfg(test)]
mod tests {
    use super::strip_status;

    #[test]
    fn ignores_the_status_glyph() {
        assert_eq!(strip_status("✳ Fix login bug"), "Fix login bug");
        assert_eq!(strip_status("⠂ Fix login bug "), "Fix login bug");
        assert_eq!(strip_status("Fix login bug"), "Fix login bug");
        assert_eq!(strip_status("Claude Code"), "Claude Code");
        assert_eq!(strip_status("PowerShell"), "PowerShell");
    }
}
