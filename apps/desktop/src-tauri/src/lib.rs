//! Claude Buddy desktop core. Bridge and Windows integration modules are added
//! by their own issues (see docs/backlog.md).

pub mod registry;

use std::sync::Mutex;

use registry::Registry;

/// Dev-only view of the registry, for checking states without the overlay.
#[cfg(debug_assertions)]
#[tauri::command]
fn debug_sessions(registry: tauri::State<'_, Mutex<Registry>>) -> Vec<registry::SessionView> {
    registry
        .lock()
        .map(|registry| registry.snapshot())
        .unwrap_or_default()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default().manage(Mutex::new(Registry::default()));

    #[cfg(debug_assertions)]
    let builder = builder.invoke_handler(tauri::generate_handler![debug_sessions]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running Claude Buddy");
}
