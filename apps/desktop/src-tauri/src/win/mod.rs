//! The overlay window: transparent, always on top, out of the taskbar and
//! Alt+Tab, never focused, anchored to a corner of a monitor's work area, and
//! click-through except over the regions the frontend registers (buddies,
//! hover balloon). Decisions: ADR-0011. Geometry and hit-testing are pure and
//! tested everywhere; the Win32 parts live in `win32.rs`.

pub mod autostart;
pub mod focus;
#[cfg(windows)]
mod focus_win32;
mod geometry;
mod hit;
pub mod hotkey;
#[cfg(windows)]
mod win32;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewWindow};

pub use geometry::{Corner, ExpandMode, LogicalRect, Rect};
use hit::{HitTester, PointerChange};

/// Label of the overlay window in `tauri.conf.json`.
pub const OVERLAY_LABEL: &str = "overlay";
/// Overlay size in logical pixels until the frontend reports the group's size.
const DEFAULT_SIZE: (f64, f64) = (480.0, 160.0);
/// Event carrying the new [`Anchor`] whenever it changes.
pub const EVENT_ANCHOR: &str = "overlay:anchor";
/// Event carrying a [`PointerEvent`] when the pointer enters a registered
/// region and when it leaves them all. Once click-through, the page itself
/// no longer sees the mouse, so this is how it learns the pointer left; on
/// entering, it also learns where the pointer is before its first own event.
pub const EVENT_POINTER: &str = "overlay:pointer";

/// Payload of [`EVENT_POINTER`]; `x` and `y` are logical pixels relative to
/// the overlay.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct PointerEvent {
    pub inside: bool,
    pub x: Option<f64>,
    pub y: Option<f64>,
}

impl From<PointerChange> for PointerEvent {
    fn from(change: PointerChange) -> Self {
        Self {
            inside: change.interactive,
            x: change.at.map(|(x, _)| x),
            y: change.at.map(|(_, y)| y),
        }
    }
}
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
    expanded: Mutex<Option<ExpandMode>>,
    /// Anchored corner on screen and scale at the last placement: when they
    /// change, the buddies moved on screen (see `HitTester::set_geometry`).
    placed_at: Mutex<Option<((i32, i32), f64)>>,
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
        let (toggle_tx, toggle_rx) = mpsc::channel::<PointerChange>();
        let applier = window.clone();
        std::thread::Builder::new()
            .name("overlay-click-through".into())
            .spawn(move || {
                for change in toggle_rx {
                    set_click_through(&applier, !change.interactive);
                    let _ = applier.emit(EVENT_POINTER, PointerEvent::from(change));
                }
            })?;
        // Lives for the whole process: the mouse hook holds a reference to it.
        let tester: &'static Mutex<HitTester> = Box::leak(Box::new(Mutex::new(HitTester::new(
            Box::new(move |change| {
                let _ = toggle_tx.send(change);
            }),
        ))));

        set_click_through(&window, true);
        let overlay = Arc::new(Self {
            window,
            anchor: Mutex::new(Anchor::default()),
            content_size: Mutex::new(DEFAULT_SIZE),
            expanded: Mutex::new(None),
            placed_at: Mutex::new(None),
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

    /// Grows the overlay while buddies leave their corner: to the whole work
    /// area for a throw (#24), or to a strip along the floor for a stroll.
    /// Returns where the anchored area lies inside it.
    pub fn expand(&self, mode: ExpandMode) -> tauri::Result<Option<ExpandedLayout>> {
        *lock(&self.expanded) = Some(mode);
        self.place()
    }

    /// Back to the anchored size and position.
    pub fn restore(&self) -> tauri::Result<()> {
        *lock(&self.expanded) = None;
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
        let corner = self.anchor().corner;
        let home = geometry::anchor(work, size, scale, corner, MARGIN);
        let expanded = *lock(&self.expanded);
        let (rect, layout) = match expanded {
            Some(mode) => {
                let outer = geometry::expanded_rect(work, home, mode);
                let layout = ExpandedLayout {
                    width: f64::from(outer.width) / scale,
                    height: f64::from(outer.height) / scale,
                    home: geometry::inner_rect(outer, home, scale),
                };
                (outer, Some(layout))
            }
            None => (home, None),
        };
        self.window
            .set_size(PhysicalSize::new(rect.width, rect.height))?;
        self.window
            .set_position(PhysicalPosition::new(rect.x, rect.y))?;
        // Growing or shrinking around the corner leaves the buddies where
        // they are on screen; anything else moved them.
        let placed = (geometry::anchor_point(home, corner), scale);
        let moved = lock(&self.placed_at).replace(placed) != Some(placed);
        lock(self.tester).set_geometry((rect.x, rect.y), scale, moved);
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

/// Frontend: a buddy leaves its corner (`"work"` for a throw, `"floor"` for
/// a stroll; work when omitted). `null` when no monitor is known.
#[tauri::command]
pub fn overlay_expand(
    overlay: tauri::State<'_, Arc<Overlay>>,
    mode: Option<ExpandMode>,
) -> Result<Option<ExpandedLayout>, String> {
    overlay
        .expand(mode.unwrap_or(ExpandMode::Work))
        .map_err(|e| e.to_string())
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
