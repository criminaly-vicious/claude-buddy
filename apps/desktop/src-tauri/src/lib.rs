//! Claude Buddy desktop core.

pub mod bridge;
pub mod claude_dir;
pub mod installer;
pub mod liveness;
pub mod registry;
pub mod watcher;
pub mod win;

use std::sync::{Arc, Mutex};

use tauri::{Emitter, Manager, RunEvent, WindowEvent};

use bridge::Bridge;
use registry::{Change, Registry, EVENT_REMOVE, EVENT_SNAPSHOT, EVENT_UPDATE};
use watcher::TranscriptWatcher;
use win::Overlay;

type SharedRegistry = Arc<Mutex<Registry>>;

/// Dev-only view of the registry, for checking states without the overlay.
#[cfg(debug_assertions)]
#[tauri::command]
fn debug_sessions(registry: tauri::State<'_, SharedRegistry>) -> Vec<registry::SessionView> {
    registry
        .lock()
        .map(|registry| registry.snapshot())
        .unwrap_or_default()
}

/// Dev-only dry run of the hook installer: the diff it would apply to
/// `settings.json`, with `buddy-hook` expected next to the app executable.
/// The real install action comes with the tray menu (#27).
#[cfg(debug_assertions)]
#[tauri::command]
fn debug_hook_install_plan() -> Result<String, String> {
    let settings = installer::default_settings_path().ok_or("no Claude config folder")?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let hook = exe.with_file_name(if cfg!(windows) {
        "buddy-hook.exe"
    } else {
        "buddy-hook"
    });
    let plan = installer::HookInstaller::new(settings, hook)
        .plan_install()
        .map_err(|e| e.to_string())?;
    Ok(plan.diff())
}

/// Fallback source for sessions without hooks. Missing `~/.claude/projects`
/// (Claude Code never ran) just means no fallback.
fn start_transcript_watcher(app: &tauri::App, inputs: std::sync::mpsc::Sender<bridge::Input>) {
    let Some(dir) = watcher::projects_dir().filter(|dir| dir.is_dir()) else {
        return;
    };
    match TranscriptWatcher::start(&dir, inputs, bridge::now) {
        Ok(watcher) => {
            *app.state::<Mutex<Option<TranscriptWatcher>>>()
                .lock()
                .unwrap() = Some(watcher);
        }
        Err(error) => eprintln!("claude-buddy: transcript watcher not started: {error}"),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let registry: SharedRegistry = Arc::default();
    let builder = tauri::Builder::default()
        .manage(Arc::clone(&registry))
        .manage(Mutex::new(None::<Bridge>))
        .manage(Mutex::new(None::<TranscriptWatcher>))
        .setup(move |app| {
            let overlay = Overlay::setup(app)?;
            app.manage(overlay);

            let handle = app.handle().clone();
            let emit = move |change: Change| {
                let _ = match change {
                    Change::Update(view) => handle.emit(EVENT_UPDATE, view),
                    Change::Remove(id) => handle.emit(EVENT_REMOVE, id),
                };
            };
            // Without a bridge the app still runs; buddies just never appear.
            match Bridge::start(Arc::clone(&registry), emit) {
                Ok(bridge) => {
                    start_transcript_watcher(app, bridge.inputs());
                    *app.state::<Mutex<Option<Bridge>>>().lock().unwrap() = Some(bridge);
                }
                Err(error) => eprintln!("claude-buddy: bridge not started: {error}"),
            }
            let snapshot = registry.lock().map(|r| r.snapshot()).unwrap_or_default();
            let _ = app.emit(EVENT_SNAPSHOT, snapshot);
            Ok(())
        })
        .on_window_event(|window, event| {
            // Moving to a monitor with another DPI: re-anchor at the new scale.
            if let WindowEvent::ScaleFactorChanged { .. } = event {
                if let Some(overlay) = window.try_state::<Arc<Overlay>>() {
                    let _ = overlay.place();
                }
            }
        });

    #[cfg(debug_assertions)]
    let builder = builder.invoke_handler(tauri::generate_handler![
        win::set_hit_regions,
        win::overlay_expand,
        win::overlay_restore,
        win::get_overlay_anchor,
        win::set_overlay_anchor,
        debug_sessions,
        debug_hook_install_plan
    ]);
    #[cfg(not(debug_assertions))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        win::set_hit_regions,
        win::overlay_expand,
        win::overlay_restore,
        win::get_overlay_anchor,
        win::set_overlay_anchor
    ]);

    builder
        .build(tauri::generate_context!())
        .expect("error while building Claude Buddy")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                if let Ok(mut watcher) = app.state::<Mutex<Option<TranscriptWatcher>>>().lock() {
                    watcher.take();
                }
                // Stops the server and deletes bridge.json.
                if let Ok(mut bridge) = app.state::<Mutex<Option<Bridge>>>().lock() {
                    bridge.take();
                }
            }
        });
}
