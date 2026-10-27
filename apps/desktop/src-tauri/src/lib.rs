//! Claude Buddy desktop core.

pub mod bridge;
pub mod claude_dir;
pub mod config;
pub mod installer;
pub mod liveness;
pub mod registry;
pub mod settings;
mod single_instance;
pub mod tray;
pub mod watcher;
pub mod win;

use std::sync::{Arc, Mutex, OnceLock};

use tauri::{AppHandle, Emitter, Manager, RunEvent, WindowEvent};

use bridge::Bridge;
use registry::{Change, Registry, EVENT_REMOVE, EVENT_SNAPSHOT, EVENT_UPDATE};
use watcher::TranscriptWatcher;
use win::Overlay;

type SharedRegistry = Arc<Mutex<Registry>>;

/// Current sessions, for the overlay when it starts (the startup
/// `sessions:snapshot` event fires before the page listens).
#[tauri::command]
fn get_sessions(registry: tauri::State<'_, SharedRegistry>) -> Vec<registry::SessionView> {
    registry
        .lock()
        .map(|registry| registry.snapshot())
        .unwrap_or_default()
}

/// Click on a buddy: focus the terminal window of session `id` (#23).
/// Best effort; `true` only when the window is now in front, so the buddy
/// hops on success.
#[tauri::command]
fn focus_session(registry: tauri::State<'_, SharedRegistry>, id: String) -> bool {
    let pid = registry.lock().ok().and_then(|registry| registry.pid(&id));
    pid.is_some_and(win::focus_process)
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
    // A second launch shows the first instance's buddies and exits.
    let app_handle: Arc<OnceLock<AppHandle>> = Arc::default();
    let first = Arc::clone(&app_handle);
    let claim = single_instance::claim(move || {
        if let Some(app) = first.get() {
            tray::set_buddies_visible(app, true);
        }
    });
    if let single_instance::Instance::Second = claim {
        return;
    }

    // Settings first: the registry starts with the saved idle timeout.
    let config = settings::ConfigState::load();
    settings::sync_autostart(&config);
    let saved = config.get();
    let registry: SharedRegistry = Arc::new(Mutex::new(Registry::new(registry::Config {
        idle_timeout_ms: saved.idle_timeout_ms(),
        ..registry::Config::default()
    })));
    let builder = tauri::Builder::default()
        .manage(config)
        .manage(Arc::clone(&registry))
        .manage(Mutex::new(None::<Bridge>))
        .manage(Mutex::new(None::<TranscriptWatcher>))
        .setup(move |app| {
            let overlay = Overlay::setup(app)?;
            app.manage(overlay);
            app.manage(settings::Integrations::start(app.handle()));
            settings::apply(app.handle(), &saved);
            tray::setup(app)?;
            let _ = app_handle.set(app.handle().clone());

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
                if window.label() != win::OVERLAY_LABEL {
                    return;
                }
                if let Some(overlay) = window.try_state::<Arc<Overlay>>() {
                    let _ = overlay.place();
                }
            }
        });

    let builder = builder.invoke_handler(tauri::generate_handler![
        win::set_hit_regions,
        win::overlay_expand,
        win::overlay_restore,
        win::set_overlay_content_size,
        win::get_overlay_anchor,
        win::set_overlay_anchor,
        get_sessions,
        focus_session,
        settings::get_config,
        settings::set_config,
        settings::list_monitors,
        settings::hook_status,
        settings::integration_status
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
