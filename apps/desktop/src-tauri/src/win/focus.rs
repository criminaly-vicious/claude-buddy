//! Click on a buddy: bring its session's terminal window to the front
//! (docs/session-detection.md#window-focus-click-on-a-buddy). Best effort:
//! when no window is found nothing happens and nothing is reported loudly.
//!
//! The choices are pure functions over snapshots, tested on every platform:
//! - [`ancestry`]: the session's process and its parents, stopping at the
//!   desktop shell and at parents that are not really parents (PID reuse).
//! - [`pick_window`]: the first process in that chain that owns an
//!   Alt+Tab-style window (Windows Terminal, VS Code, ...).
//!
//! The Win32 side (snapshots, the classic console fallback and the
//! foreground call) lives in `focus_win32.rs`.

use std::collections::{HashMap, HashSet};

/// One entry of a process snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessInfo {
    pub pid: u32,
    pub parent: u32,
    /// Executable file name, e.g. `WindowsTerminal.exe`.
    pub exe: String,
}

/// One top-level window, in z-order (front first) as `EnumWindows` lists them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowInfo {
    pub id: isize,
    pub pid: u32,
    pub visible: bool,
    /// Has an owner window (dialogs, pop-ups).
    pub owned: bool,
    /// `WS_EX_TOOLWINDOW`: palettes, the taskbar, our own overlay.
    pub tool: bool,
    pub titled: bool,
}

impl WindowInfo {
    /// Whether this is a window a person would switch to, like Alt+Tab shows.
    /// Minimised windows count: they are restored when focused.
    fn switchable(&self) -> bool {
        self.visible && !self.owned && !self.tool && self.titled
    }
}

/// Never walk past these: their windows are the desktop, the taskbar or
/// unrelated Explorer windows, not the session's terminal.
const STOP_AT: &[&str] = &[
    "explorer.exe",
    "svchost.exe",
    "services.exe",
    "wininit.exe",
    "winlogon.exe",
    "userinit.exe",
    "sihost.exe",
];

/// Enough for any real terminal → shell → tool chain; guards against loops.
const MAX_DEPTH: usize = 32;

/// `pid` and its ancestors, nearest first. `started_at(pid)` is the process
/// creation time when known: a "parent" started after its child is a new
/// process that reused a dead parent's PID, so the walk stops there.
pub fn ancestry(
    processes: &[ProcessInfo],
    pid: u32,
    started_at: impl Fn(u32) -> Option<u64>,
) -> Vec<u32> {
    let by_pid: HashMap<u32, &ProcessInfo> = processes.iter().map(|p| (p.pid, p)).collect();
    let mut chain = Vec::new();
    let mut seen = HashSet::new();
    let mut current = by_pid.get(&pid).copied();
    let mut child_started = None;
    while let Some(process) = current {
        if chain.len() >= MAX_DEPTH || !seen.insert(process.pid) {
            break;
        }
        let exe = process.exe.to_ascii_lowercase();
        if STOP_AT.contains(&exe.as_str()) {
            break;
        }
        let started = started_at(process.pid);
        if let (Some(parent), Some(child)) = (started, child_started) {
            if parent > child {
                break;
            }
        }
        chain.push(process.pid);
        child_started = started;
        // PID 0 is the idle process: the end of every chain.
        current = (process.parent != 0)
            .then(|| by_pid.get(&process.parent).copied())
            .flatten();
    }
    chain
}

/// The window to focus: the frontmost switchable window of the nearest
/// process in `chain` that has one.
pub fn pick_window(chain: &[u32], windows: &[WindowInfo]) -> Option<isize> {
    chain.iter().find_map(|pid| {
        windows
            .iter()
            .find(|w| w.pid == *pid && w.switchable())
            .map(|w| w.id)
    })
}

/// A console window as `GetConsoleWindow` returns it, with its owner. Under
/// a pseudo console (Windows Terminal as the default terminal, VS Code) the
/// console window is hidden and owned by the terminal's window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConsoleWindow {
    pub id: isize,
    pub visible: bool,
    /// The owner window, if it is visible.
    pub visible_owner: Option<isize>,
}

/// Fallback when the chain has no window, e.g. a classic console, whose
/// window belongs to a `conhost.exe` that is a child of the shell rather
/// than a parent of the session.
pub fn console_target(console: ConsoleWindow) -> Option<isize> {
    if console.visible {
        Some(console.id)
    } else {
        console.visible_owner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process(pid: u32, parent: u32, exe: &str) -> ProcessInfo {
        ProcessInfo {
            pid,
            parent,
            exe: exe.into(),
        }
    }

    fn window(id: isize, pid: u32) -> WindowInfo {
        WindowInfo {
            id,
            pid,
            visible: true,
            owned: false,
            tool: false,
            titled: true,
        }
    }

    /// explorer → WindowsTerminal → OpenConsole → pwsh → claude.
    fn windows_terminal() -> Vec<ProcessInfo> {
        vec![
            process(4, 0, "System"),
            process(100, 90, "explorer.exe"),
            process(200, 100, "WindowsTerminal.exe"),
            process(210, 200, "OpenConsole.exe"),
            process(220, 210, "pwsh.exe"),
            process(230, 220, "claude.exe"),
        ]
    }

    fn unknown_start(_: u32) -> Option<u64> {
        None
    }

    #[test]
    fn walks_up_to_the_desktop_shell_but_not_into_it() {
        assert_eq!(
            ancestry(&windows_terminal(), 230, unknown_start),
            [230, 220, 210, 200]
        );
    }

    #[test]
    fn stop_list_ignores_case() {
        let mut processes = windows_terminal();
        processes[1].exe = "EXPLORER.EXE".into();
        assert!(!ancestry(&processes, 230, unknown_start).contains(&100));
    }

    #[test]
    fn unknown_pid_gives_an_empty_chain() {
        assert!(ancestry(&windows_terminal(), 999, unknown_start).is_empty());
    }

    #[test]
    fn stops_at_a_missing_parent() {
        let processes = vec![process(230, 220, "claude.exe")];
        assert_eq!(ancestry(&processes, 230, unknown_start), [230]);
    }

    #[test]
    fn a_parent_started_after_its_child_is_a_reused_pid() {
        // pwsh (220) died; a newer process got PID 220.
        let start = |pid: u32| match pid {
            230 => Some(1_000),
            220 => Some(5_000),
            _ => None,
        };
        assert_eq!(ancestry(&windows_terminal(), 230, start), [230]);
    }

    #[test]
    fn survives_parent_loops() {
        let processes = vec![process(1, 2, "a.exe"), process(2, 1, "b.exe")];
        assert_eq!(ancestry(&processes, 1, unknown_start), [1, 2]);
        let looped = vec![process(7, 7, "self.exe")];
        assert_eq!(ancestry(&looped, 7, unknown_start), [7]);
    }

    #[test]
    fn depth_is_bounded() {
        let processes: Vec<_> = (1..=100)
            .map(|pid| process(pid, pid + 1, "x.exe"))
            .collect();
        assert_eq!(ancestry(&processes, 1, unknown_start).len(), MAX_DEPTH);
    }

    #[test]
    fn picks_the_nearest_process_with_a_switchable_window() {
        let chain = ancestry(&windows_terminal(), 230, unknown_start);
        // OpenConsole's pseudo console window is hidden; explorer's is out of the chain.
        let windows = vec![
            window(1, 100),
            WindowInfo {
                visible: false,
                ..window(2, 210)
            },
            window(3, 200),
        ];
        assert_eq!(pick_window(&chain, &windows), Some(3));
    }

    #[test]
    fn skips_owned_tool_and_untitled_windows() {
        let windows = vec![
            WindowInfo {
                owned: true,
                ..window(1, 5)
            },
            WindowInfo {
                tool: true,
                ..window(2, 5)
            },
            WindowInfo {
                titled: false,
                ..window(3, 5)
            },
        ];
        assert_eq!(pick_window(&[5], &windows), None);
        let mut with_main = windows.clone();
        with_main.push(window(4, 5));
        assert_eq!(pick_window(&[5], &with_main), Some(4));
    }

    #[test]
    fn frontmost_window_of_a_process_wins() {
        // VS Code with two windows: the one in front is listed first.
        let windows = vec![window(10, 5), window(11, 5)];
        assert_eq!(pick_window(&[9, 5], &windows), Some(10));
    }

    #[test]
    fn nearer_process_beats_frontmost_window() {
        let windows = vec![window(1, 6), window(2, 5)];
        assert_eq!(pick_window(&[5, 6], &windows), Some(2));
    }

    #[test]
    fn console_fallback_uses_the_window_or_its_terminal_owner() {
        let classic = ConsoleWindow {
            id: 1,
            visible: true,
            visible_owner: None,
        };
        assert_eq!(console_target(classic), Some(1));
        let pseudo = ConsoleWindow {
            id: 1,
            visible: false,
            visible_owner: Some(2),
        };
        assert_eq!(console_target(pseudo), Some(2));
        let headless = ConsoleWindow {
            id: 1,
            visible: false,
            visible_owner: None,
        };
        assert_eq!(console_target(headless), None);
    }
}
