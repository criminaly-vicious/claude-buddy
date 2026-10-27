//! The Win32 pieces Tauri does not cover (ADR-0011). Every `unsafe` call is
//! wrapped in a small safe function:
//! - [`ensure_tool_window`]: keep the overlay out of Alt+Tab.
//! - [`start_mouse_hook`]: a low-level mouse hook that tells
//!   [`HitTester`] where the cursor is, so click-through can follow the
//!   buddies without polling.
//! - [`watch_display_changes`]: re-anchor when monitors or the taskbar change.

use std::io;
use std::sync::{Mutex, OnceLock};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, GetWindowLongPtrW, SetWindowLongPtrW, SetWindowsHookExW,
    GWL_EXSTYLE, HC_ACTION, MSG, MSLLHOOKSTRUCT, WH_MOUSE_LL, WM_DISPLAYCHANGE, WM_DPICHANGED,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_SETTINGCHANGE, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
};

use super::hit::HitTester;

/// Adds `WS_EX_TOOLWINDOW` (and drops `WS_EX_APPWINDOW`) so the overlay never
/// shows in Alt+Tab. Idempotent and cheap; tao rewrites the extended style
/// whenever it toggles a flag, so callers re-apply it after such changes.
/// Must run on the window's thread.
pub fn ensure_tool_window(hwnd: HWND) {
    // SAFETY: plain style read/write on a window handle we own.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let wanted = (style | WS_EX_TOOLWINDOW as isize) & !(WS_EX_APPWINDOW as isize);
        if wanted != style {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, wanted);
        }
    }
}

static HIT_TESTER: OnceLock<&'static Mutex<HitTester>> = OnceLock::new();

/// Installs a `WH_MOUSE_LL` hook on a dedicated thread with its own message
/// loop. The hook only updates `tester` (a few rectangle checks under a
/// lock) and returns at once, so it adds no noticeable latency to the
/// system's mouse handling. Call once.
pub fn start_mouse_hook(tester: &'static Mutex<HitTester>) -> io::Result<()> {
    if HIT_TESTER.set(tester).is_err() {
        return Err(io::Error::other("mouse hook already started"));
    }
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("overlay-mouse-hook".into())
        .spawn(move || {
            // SAFETY: `mouse_proc` matches HOOKPROC; a null module handle means this exe.
            let hook = unsafe {
                SetWindowsHookExW(
                    WH_MOUSE_LL,
                    Some(mouse_proc),
                    GetModuleHandleW(std::ptr::null()),
                    0,
                )
            };
            let installed = !hook.is_null();
            let _ = ready_tx.send(installed.then_some(()).ok_or_else(io::Error::last_os_error));
            if !installed {
                return;
            }
            // Low-level hooks are called through this thread's message loop.
            let mut msg: MSG = unsafe { std::mem::zeroed() };
            // SAFETY: standard message loop on a local MSG.
            while unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } > 0 {}
        })?;
    ready_rx
        .recv()
        .unwrap_or_else(|_| Err(io::Error::other("mouse hook thread died")))
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        if let Some(tester) = HIT_TESTER.get() {
            // SAFETY: for WH_MOUSE_LL with HC_ACTION, lparam points to an MSLLHOOKSTRUCT.
            let info = unsafe { &*(lparam as *const MSLLHOOKSTRUCT) };
            let (x, y) = (info.pt.x, info.pt.y);
            if let Ok(mut tester) = tester.try_lock() {
                match wparam as u32 {
                    WM_LBUTTONDOWN => tester.button_down(x, y),
                    WM_LBUTTONUP => tester.button_up(x, y),
                    _ => tester.moved(x, y),
                }
            }
        }
    }
    // SAFETY: always pass the event on; we never swallow mouse input.
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

type DisplayCallback = Box<dyn Fn() + Send + Sync>;
static DISPLAY_CALLBACK: OnceLock<DisplayCallback> = OnceLock::new();

/// Calls `on_change` when the display layout, a monitor's DPI or the work
/// area (taskbar size or position) changes. Must run on the window's thread.
pub fn watch_display_changes(hwnd: HWND, on_change: DisplayCallback) -> io::Result<()> {
    if DISPLAY_CALLBACK.set(on_change).is_err() {
        return Err(io::Error::other("display watch already started"));
    }
    // SAFETY: `subclass_proc` matches SUBCLASSPROC; called on the owning thread.
    if unsafe { SetWindowSubclass(hwnd, Some(subclass_proc), 1, 0) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    // SAFETY: forward first, so tao handles the message (DPI resize included) before we re-anchor.
    let result = unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
    if matches!(msg, WM_DISPLAYCHANGE | WM_DPICHANGED | WM_SETTINGCHANGE) {
        if let Some(callback) = DISPLAY_CALLBACK.get() {
            callback();
        }
    }
    result
}
