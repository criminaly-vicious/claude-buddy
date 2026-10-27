//! The overlay window: transparent, always on top, out of the taskbar and
//! Alt+Tab, never focused, anchored to a corner of a monitor's work area, and
//! click-through except over the regions the frontend registers (buddies,
//! hover card). Decisions: ADR-0011. Geometry and hit-testing are pure and
//! tested everywhere; the Win32 parts live in `win32.rs`.

mod geometry;
mod hit;
#[cfg(windows)]
mod win32;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewWindow};

pub use geometry::{Corner, LogicalRect, Rect};
use hit::HitTester;

/// Label of the overlay window in `tauri.conf.json`.
pub const OVERLAY_LABEL: &str = "overlay";
/// Overlay size in logical pixels while anchored.
const OVERLAY_SIZE: (f64, f64) = (480.0, 160.0);
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

pub struct Overlay {
    window: WebviewWindow,
    anchor: Mutex<Anchor>,
    expanded: AtomicBool,
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
            expanded: AtomicBool::new(false),
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
        self.place()
    }

    /// Grows the overlay to cover its whole monitor, e.g. while a buddy is thrown (#24).
    pub fn expand(&self) -> tauri::Result<()> {
        self.expanded.store(true, Ordering::SeqCst);
        self.place()
    }

    /// Back to the anchored size and position.
    pub fn restore(&self) -> tauri::Result<()> {
        self.expanded.store(false, Ordering::SeqCst);
        self.place()
    }

    pub fn set_hit_regions(&self, regions: Vec<LogicalRect>) {
        lock(self.tester).set_regions(regions);
    }

    /// Moves and sizes the window for the current anchor, monitor and DPI.
    pub fn place(&self) -> tauri::Result<()> {
        let Some(monitor) = self.monitor()? else {
            return Ok(());
        };
        let scale = monitor.scale_factor();
        let rect = if self.expanded.load(Ordering::SeqCst) {
            Rect {
                x: monitor.position().x,
                y: monitor.position().y,
                width: monitor.size().width,
                height: monitor.size().height,
            }
        } else {
            let work = monitor.work_area();
            let work = Rect {
                x: work.position.x,
                y: work.position.y,
                width: work.size.width,
                height: work.size.height,
            };
            geometry::anchor(work, OVERLAY_SIZE, scale, self.anchor().corner, MARGIN)
        };
        self.window
            .set_size(PhysicalSize::new(rect.width, rect.height))?;
        self.window
            .set_position(PhysicalPosition::new(rect.x, rect.y))?;
        lock(self.tester).set_geometry((rect.x, rect.y), scale);
        Ok(())
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

#[tauri::command]
pub fn overlay_expand(overlay: tauri::State<'_, Arc<Overlay>>) -> Result<(), String> {
    overlay.expand().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn overlay_restore(overlay: tauri::State<'_, Arc<Overlay>>) -> Result<(), String> {
    overlay.restore().map_err(|e| e.to_string())
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
