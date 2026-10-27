//! Claude Buddy desktop core. Bridge, registry and Windows integration modules
//! are added by their own issues (see docs/backlog.md).

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running Claude Buddy");
}
