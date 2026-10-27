//! Win32 side of [`super::focus`]. Every `unsafe` call is wrapped in a small
//! safe function; nothing here logs process names or window titles.

use std::sync::Mutex;

use windows_sys::core::BOOL;
use windows_sys::Win32::Foundation::{
    CloseHandle, FILETIME, HANDLE, HWND, INVALID_HANDLE_VALUE, LPARAM,
};
use windows_sys::Win32::System::Console::{AttachConsole, FreeConsole, GetConsoleWindow};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Threading::{
    AttachThreadInput, GetCurrentThreadId, GetProcessTimes, OpenProcess,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetForegroundWindow, GetWindow, GetWindowLongPtrW,
    GetWindowTextLengthW, GetWindowThreadProcessId, IsIconic, IsWindowVisible, SetForegroundWindow,
    ShowWindow, GWL_EXSTYLE, GW_OWNER, SW_RESTORE, WS_EX_TOOLWINDOW,
};

use super::focus::{ancestry, console_target, pick_window, ConsoleWindow, ProcessInfo, WindowInfo};

/// Brings the terminal window of process `pid` to the front. Returns
/// whether that window ended up in the foreground.
pub fn focus_process(pid: u32) -> bool {
    let Some(processes) = processes() else {
        return false;
    };
    let chain = ancestry(&processes, pid, creation_time);
    let target = pick_window(&chain, &top_level_windows()).or_else(|| console_of(pid));
    target.is_some_and(|id| bring_to_front(id as HWND))
}

/// Owned kernel handle, closed on drop.
struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle is owned and closed exactly once.
        unsafe { CloseHandle(self.0) };
    }
}

/// A Toolhelp snapshot of every process: PID, parent PID, executable name.
fn processes() -> Option<Vec<ProcessInfo>> {
    // SAFETY: plain FFI call; failure is INVALID_HANDLE_VALUE.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return None;
    }
    let snapshot = Handle(snapshot);
    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut list = Vec::new();
    // SAFETY: `entry` is a properly sized PROCESSENTRY32W on our stack.
    let mut more = unsafe { Process32FirstW(snapshot.0, &mut entry) } != 0;
    while more {
        let len = entry
            .szExeFile
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(entry.szExeFile.len());
        list.push(ProcessInfo {
            pid: entry.th32ProcessID,
            parent: entry.th32ParentProcessID,
            exe: String::from_utf16_lossy(&entry.szExeFile[..len]),
        });
        // SAFETY: as above.
        more = unsafe { Process32NextW(snapshot.0, &mut entry) } != 0;
    }
    Some(list)
}

/// Creation time of `pid` (100 ns ticks), when we may query it.
fn creation_time(pid: u32) -> Option<u64> {
    // SAFETY: plain FFI call; a null handle is handled below.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }
    let handle = Handle(handle);
    let zero = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
    // SAFETY: valid handle opened with PROCESS_QUERY_LIMITED_INFORMATION; out-pointers are locals.
    let ok =
        unsafe { GetProcessTimes(handle.0, &mut created, &mut exited, &mut kernel, &mut user) };
    (ok != 0).then(|| (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
}

/// Every top-level window, front first.
fn top_level_windows() -> Vec<WindowInfo> {
    let mut list: Vec<WindowInfo> = Vec::new();
    // SAFETY: `collect_window` matches WNDENUMPROC and only runs during this
    // call, while `list` is alive; lparam carries a pointer to it.
    unsafe { EnumWindows(Some(collect_window), &mut list as *mut _ as LPARAM) };
    list
}

unsafe extern "system" fn collect_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: lparam is the `&mut Vec<WindowInfo>` passed by `top_level_windows`.
    let list = unsafe { &mut *(lparam as *mut Vec<WindowInfo>) };
    list.push(describe(hwnd));
    1
}

fn describe(hwnd: HWND) -> WindowInfo {
    let mut pid = 0u32;
    // SAFETY: read-only queries on a window handle from EnumWindows; a window
    // that has gone meanwhile just answers 0 / null.
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut pid);
        WindowInfo {
            id: hwnd as isize,
            pid,
            visible: IsWindowVisible(hwnd) != 0,
            owned: !GetWindow(hwnd, GW_OWNER).is_null(),
            tool: GetWindowLongPtrW(hwnd, GWL_EXSTYLE) & WS_EX_TOOLWINDOW as isize != 0,
            titled: GetWindowTextLengthW(hwnd) > 0,
        }
    }
}

/// Attaching to another process's console changes process-wide state.
static CONSOLE: Mutex<()> = Mutex::new(());

/// The window of the console `pid` runs in, via `AttachConsole`, for classic
/// consoles that are not in the parent chain. Skipped when this process
/// already has a console (debug builds), so ours is never detached.
fn console_of(pid: u32) -> Option<isize> {
    let _guard = CONSOLE.lock().unwrap_or_else(|p| p.into_inner());
    // SAFETY: plain FFI calls; we attach only when we had no console, and
    // always detach again before returning.
    unsafe {
        if !GetConsoleWindow().is_null() || AttachConsole(pid) == 0 {
            return None;
        }
        let hwnd = GetConsoleWindow();
        FreeConsole();
        if hwnd.is_null() {
            return None;
        }
        let owner = GetWindow(hwnd, GW_OWNER);
        console_target(ConsoleWindow {
            id: hwnd as isize,
            visible: IsWindowVisible(hwnd) != 0,
            visible_owner: (!owner.is_null() && IsWindowVisible(owner) != 0)
                .then_some(owner as isize),
        })
    }
}

/// Restores `hwnd` if minimised and makes it the foreground window. The
/// click on the overlay normally grants this process the right to do so;
/// if Windows still refuses, attaching to the foreground thread's input
/// state is the documented way around the lock.
fn bring_to_front(hwnd: HWND) -> bool {
    // SAFETY: window calls on a handle that may have closed meanwhile; they
    // then fail harmlessly.
    unsafe {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        if SetForegroundWindow(hwnd) != 0 && GetForegroundWindow() == hwnd {
            return true;
        }
        let foreground = GetForegroundWindow();
        let theirs = GetWindowThreadProcessId(foreground, std::ptr::null_mut());
        let ours = GetCurrentThreadId();
        if theirs != 0 && theirs != ours && AttachThreadInput(ours, theirs, 1) != 0 {
            BringWindowToTop(hwnd);
            SetForegroundWindow(hwnd);
            AttachThreadInput(ours, theirs, 0);
        }
        GetForegroundWindow() == hwnd
    }
}
