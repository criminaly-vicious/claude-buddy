//! Claude Buddy desktop core. Windows integration modules are added by their
//! own issues (see docs/backlog.md).

pub mod bridge;
pub mod registry;

use std::sync::{Arc, Mutex};

use tauri::{Emitter, Manager, RunEvent};

use bridge::Bridge;
use registry::{Change, Registry, EVENT_REMOVE, EVENT_SNAPSHOT, EVENT_UPDATE};

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let registry: SharedRegistry = Arc::default();
    let builder = tauri::Builder::default()
        .manage(Arc::clone(&registry))
        .manage(Mutex::new(None::<Bridge>))
        .setup(move |app| {
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
                    *app.state::<Mutex<Option<Bridge>>>().lock().unwrap() = Some(bridge);
                }
                Err(error) => eprintln!("claude-buddy: bridge not started: {error}"),
            }
            let snapshot = registry.lock().map(|r| r.snapshot()).unwrap_or_default();
            let _ = app.emit(EVENT_SNAPSHOT, snapshot);
            Ok(())
        });

    #[cfg(debug_assertions)]
    let builder = builder.invoke_handler(tauri::generate_handler![debug_sessions]);

    builder
        .build(tauri::generate_context!())
        .expect("error while building Claude Buddy")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                // Stops the server and deletes bridge.json.
                if let Ok(mut bridge) = app.state::<Mutex<Option<Bridge>>>().lock() {
                    bridge.take();
                }
            }
        });
}
