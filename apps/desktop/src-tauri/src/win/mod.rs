//! The overlay window: transparent, always on top, out of the taskbar and
//! Alt+Tab, never focused, anchored to a corner of a monitor's work area, and
//! click-through except over the regions the frontend registers (buddies,
//! hover card). Decisions: ADR-0011. Geometry and hit-testing are pure and
//! tested everywhere; the Win32 parts live in `win32.rs`.

pub mod focus;
#[cfg(windows)]
mod focus_win32;
mod geometry;
mod hit;
#[cfg(windows)]
mod win32;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewWindow};

pub use geometry::{Corner, LogicalRect, Rect};
use hit::HitTester;

/// Label of the overlay window in `tauri.conf.json`.
pub const OVERLAY_LABEL: &str = "overlay";
/// Overlay size in logical pixels until the frontend reports the group's size.
const DEFAULT_SIZE: (f64, f64) = (480.0, 160.0);
/// Event carrying the new [`Anchor`] whenever it changes.
pub const EVENT_ANCHOR: &str = "overlay:anchor";
/// Event carrying `true` when the pointer enters a registered region and
/// `false` when it leaves them all. Once click-through, the page itself no
/// longer sees the mouse, so this is how it learns the pointer left.
pub const EVENT_POINTER: &str = "overlay:pointer";
/// Event carrying `false` when the buddies are hidden from the tray and
/// `true` when shown again. Hidden, the page draws nothing at all.
pub const EVENT_VISIBLE: &str = "overlay:visible";
/// Gap between the overlay and the work-area edges, in logical pixels.
const MARGIN: f64 = 8.0;

/// Where the overlay sits. Persisted by the settings window (#28); defaults
/// to the bottom-right corner of the primary monitor.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Anchor {
    pub corner: Corner,
    /// Index into the monitor list; `None` or out of range means primary.
    pub monitor: Option<usize>,
}

/// The expanded overlay, in logical pixels: its size (the work area) and
/// where the anchored area (`home`) lies inside it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ExpandedLayout {
    pub width: f64,
    pub height: f64,
    pub home: LogicalRect,
}

pub struct Overlay {
    window: WebviewWindow,
    anchor: Mutex<Anchor>,
    /// Logical size the buddies need, from the frontend's layout (#21).
    content_size: Mutex<(f64, f64)>,
    expanded: AtomicBool,
    visible: AtomicBool,
    tester: &'static Mutex<HitTester>,
}

impl Overlay {
    /// Configures the overlay window and places it. Call from `setup`, on the
    /// main thread.
    pub fn setup(app: &tauri::App) -> tauri::Result<Arc<Self>> {
        let window = app
            .get_webview_window(OVERLAY_LABEL)
            .ok_or_else(|| tauri::Error::WindowNotFound)?;

        // Click-through changes are applied off the hook thread, so the hook
        // never waits on the main thread.
        let (toggle_tx, toggle_rx) = mpsc::channel::<bool>();
        let applier = window.clone();
        std::thread::Builder::new()
            .name("overlay-click-through".into())
            .spawn(move || {
                for interactive in toggle_rx {
                    set_click_through(&applier, !interactive);
                    let _ = applier.emit(EVENT_POINTER, interactive);
                }
            })?;
        // Lives for the whole process: the mouse hook holds a reference to it.
        let tester: &'static Mutex<HitTester> = Box::leak(Box::new(Mutex::new(HitTester::new(
            Box::new(move |interactive| {
                let _ = toggle_tx.send(interactive);
            }),
        ))));

        set_click_through(&window, true);
        let overlay = Arc::new(Self {
            window,
            anchor: Mutex::new(Anchor::default()),
            content_size: Mutex::new(DEFAULT_SIZE),
            expanded: AtomicBool::new(false),
            visible: AtomicBool::new(true),
            tester,
        });
        overlay.place()?;

        #[cfg(windows)]
        {
            let hwnd = overlay.window.hwnd()?.0 as windows_sys::Win32::Foundation::HWND;
            win32::ensure_tool_window(hwnd);
            if let Err(error) = win32::start_mouse_hook(tester) {
                // Without the hook the overlay stays click-through: safe, just not clickable.
                eprintln!("claude-buddy: mouse hook not installed: {error}");
            }
            let weak = Arc::downgrade(&overlay);
            let _ = win32::watch_display_changes(
                hwnd,
                Box::new(move || {
                    if let Some(overlay) = weak.upgrade() {
                        let _ = overlay.place();
                    }
                }),
            );
        }
        Ok(overlay)
    }

    pub fn anchor(&self) -> Anchor {
        *lock(&self.anchor)
    }

    pub fn set_anchor(&self, anchor: Anchor) -> tauri::Result<()> {
        *lock(&self.anchor) = anchor;
        self.place()?;
        // The frontend lays buddies out from this corner.
        self.window.emit(EVENT_ANCHOR, anchor)
    }

    /// Resizes the anchored overlay to fit the group (logical pixels). The
    /// corner stays put; the size is clamped to the work area when placed.
    pub fn set_content_size(&self, width: f64, height: f64) -> tauri::Result<()> {
        let size = (width.max(1.0), height.max(1.0));
        if *lock(&self.content_size) == size {
            return Ok(());
        }
        *lock(&self.content_size) = size;
        self.place().map(|_| ())
    }

    /// Grows the overlay to cover its monitor's work area while a buddy is
    /// dragged or thrown (#24). Returns where the anchored area lies inside it.
    pub fn expand(&self) -> tauri::Result<Option<ExpandedLayout>> {
        self.expanded.store(true, Ordering::SeqCst);
        self.place()
    }

    /// Back to the anchored size and position.
    pub fn restore(&self) -> tauri::Result<()> {
        self.expanded.store(false, Ordering::SeqCst);
        self.place().map(|_| ())
    }

    pub fn visible(&self) -> bool {
        self.visible.load(Ordering::SeqCst)
    }

    /// Hides or shows the buddies (tray). Hidden, the window is gone and
    /// the page stops rendering; shown, it is re-anchored first.
    pub fn set_visible(&self, visible: bool) -> tauri::Result<()> {
        self.visible.store(visible, Ordering::SeqCst);
        if visible {
            self.place()?;
            self.window.show()?;
            set_click_through(&self.window, true);
        } else {
            self.window.hide()?;
        }
        self.window.emit(EVENT_VISIBLE, visible)
    }

    pub fn set_hit_regions(&self, regions: Vec<LogicalRect>) {
        lock(self.tester).set_regions(regions);
    }

    /// Moves and sizes the window for the current anchor, monitor and DPI.
    /// While expanded, returns where the anchored area lies inside the window.
    pub fn place(&self) -> tauri::Result<Option<ExpandedLayout>> {
        let Some(monitor) = self.monitor()? else {
            return Ok(None);
        };
        let scale = monitor.scale_factor();
        let work = monitor.work_area();
        let work = Rect {
            x: work.position.x,
            y: work.position.y,
            width: work.size.width,
            height: work.size.height,
        };
        let size = *lock(&self.content_size);
        let home = geometry::anchor(work, size, scale, self.anchor().corner, MARGIN);
        let (rect, layout) = if self.expanded.load(Ordering::SeqCst) {
            let layout = ExpandedLayout {
                width: f64::from(work.width) / scale,
                height: f64::from(work.height) / scale,
                home: geometry::inner_rect(work, home, scale),
            };
            (work, Some(layout))
        } else {
            (home, None)
        };
        self.window
            .set_size(PhysicalSize::new(rect.width, rect.height))?;
        self.window
            .set_position(PhysicalPosition::new(rect.x, rect.y))?;
        lock(self.tester).set_geometry((rect.x, rect.y), scale);
        Ok(layout)
    }

    fn monitor(&self) -> tauri::Result<Option<Monitor>> {
        let monitors = self.window.available_monitors()?;
        if let Some(monitor) = self.anchor().monitor.and_then(|i| monitors.get(i)) {
            return Ok(Some(monitor.clone()));
        }
        Ok(self
            .window
            .primary_monitor()?
            .or_else(|| monitors.into_iter().next()))
    }
}

/// Turns click-through on or off. On Windows, tao rewrites the extended
/// style when doing so, so the Alt+Tab exclusion is re-applied after it on
/// the main thread.
fn set_click_through(window: &WebviewWindow, click_through: bool) {
    let _ = window.set_ignore_cursor_events(click_through);
    #[cfg(windows)]
    {
        let reapply = window.clone();
        let _ = window.run_on_main_thread(move || {
            if let Ok(hwnd) = reapply.hwnd() {
                win32::ensure_tool_window(hwnd.0 as windows_sys::Win32::Foundation::HWND);
            }
        });
    }
}

/// Brings the terminal window of session process `pid` to the front (#23).
/// Returns whether it is now in the foreground; always `false` off Windows.
pub fn focus_process(pid: u32) -> bool {
    #[cfg(windows)]
    {
        focus_win32::focus_process(pid)
    }
    #[cfg(not(windows))]
    {
        let _ = pid;
        false
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Frontend: buddy and hover-card bounds, in CSS pixels relative to the overlay.
#[tauri::command]
pub fn set_hit_regions(overlay: tauri::State<'_, Arc<Overlay>>, regions: Vec<LogicalRect>) {
    overlay.set_hit_regions(regions);
}

/// Frontend: a buddy is grabbed. `null` when no monitor is known.
#[tauri::command]
pub fn overlay_expand(
    overlay: tauri::State<'_, Arc<Overlay>>,
) -> Result<Option<ExpandedLayout>, String> {
    overlay.expand().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn overlay_restore(overlay: tauri::State<'_, Arc<Overlay>>) -> Result<(), String> {
    overlay.restore().map_err(|e| e.to_string())
}

/// Frontend: the room the buddies need, in CSS pixels.
#[tauri::command]
pub fn set_overlay_content_size(
    overlay: tauri::State<'_, Arc<Overlay>>,
    width: f64,
    height: f64,
) -> Result<(), String> {
    if !(width.is_finite() && height.is_finite()) {
        return Err("size must be finite".into());
    }
    overlay
        .set_content_size(width, height)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_overlay_anchor(overlay: tauri::State<'_, Arc<Overlay>>) -> Anchor {
    overlay.anchor()
}

#[tauri::command]
pub fn set_overlay_anchor(
    overlay: tauri::State<'_, Arc<Overlay>>,
    anchor: Anchor,
) -> Result<(), String> {
    overlay.set_anchor(anchor).map_err(|e| e.to_string())
}
