//! The settings window (#28) and the commands behind it. The window exists
//! only while it is open: the tray creates it, closing destroys it.
//!
//! Changes apply live: corner and monitor move the overlay, the idle
//! timeout reaches the registry, scale and reduce motion go to the overlay
//! page through [`EVENT_CONFIG`], start with Windows writes the Run key and
//! the hotkey is re-registered (#29). Every change is saved at once.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::bridge::{Bridge, Input};
use crate::config::{self, AppConfig};
use crate::installer::HookInstaller;
use crate::registry::{Millis, Registry};
use crate::win::{autostart, hotkey, Anchor, Overlay};

/// Label of the settings window.
pub const SETTINGS_LABEL: &str = "settings";
/// Event carrying the new [`AppConfig`] whenever it changes.
pub const EVENT_CONFIG: &str = "config:changed";

/// The current config and where it is saved (`None`: no data folder, keep it in memory).
pub struct ConfigState {
    pub path: Option<PathBuf>,
    pub config: Mutex<AppConfig>,
}

impl ConfigState {
    pub fn load() -> Self {
        let path = config::default_path();
        let config = path.as_deref().map(config::load).unwrap_or_default();
        Self {
            path,
            config: Mutex::new(config),
        }
    }

    pub fn get(&self) -> AppConfig {
        lock(&self.config).clone()
    }
}

/// Start with Windows and the global hotkey: what went wrong last time
/// they were applied, for the settings window.
#[derive(Default)]
pub struct Integrations {
    #[cfg(windows)]
    hotkey: Option<hotkey::HotkeyService>,
    hotkey_error: Mutex<Option<String>>,
    autostart_error: Mutex<Option<String>>,
}

impl Integrations {
    /// Starts the hotkey thread; a press shows or hides the buddies.
    pub fn start(app: &AppHandle) -> Self {
        #[cfg(windows)]
        {
            let handle = app.clone();
            let service = hotkey::HotkeyService::start(move || {
                let visible = handle.state::<Arc<Overlay>>().visible();
                crate::tray::set_buddies_visible(&handle, !visible);
            });
            if let Err(error) = &service {
                eprintln!("claude-buddy: hotkey thread not started: {error}");
            }
            Self {
                hotkey: service.ok(),
                ..Self::default()
            }
        }
        #[cfg(not(windows))]
        {
            let _ = app;
            Self::default()
        }
    }

    fn set_hotkey(&self, text: Option<&str>) -> Result<(), String> {
        let parsed = text
            .map(hotkey::parse)
            .transpose()
            .map_err(|e| format!("{}: {e}", text.unwrap_or_default()))?;
        #[cfg(windows)]
        {
            match &self.hotkey {
                Some(service) => service.set(parsed),
                None if parsed.is_none() => Ok(()),
                None => Err("the hotkey could not be set up".into()),
            }
        }
        #[cfg(not(windows))]
        {
            match parsed {
                None => Ok(()),
                Some(_) => Err("global hotkeys are only available on Windows".into()),
            }
        }
    }
}

/// Startup: point an existing autostart entry at this executable, and let
/// the saved config follow the real autostart state (Task Manager can change it).
pub fn sync_autostart(state: &ConfigState) {
    autostart::repair();
    let Some(real) = autostart::is_enabled() else {
        return;
    };
    let mut config = lock(&state.config);
    if config.start_with_windows != real {
        config.start_with_windows = real;
        if let Some(path) = &state.path {
            let _ = config::save(path, &config);
        }
    }
}

/// Opens the settings window, or brings it forward if it is already open.
pub fn open(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(SETTINGS_LABEL) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let built =
        WebviewWindowBuilder::new(app, SETTINGS_LABEL, WebviewUrl::App("settings.html".into()))
            .title("Claude Buddy settings")
            .inner_size(440.0, 600.0)
            .resizable(false)
            .maximizable(false)
            .center()
            .build();
    if let Err(error) = built {
        eprintln!("claude-buddy: settings window not opened: {error}");
    }
}

/// Applies `config` to the running app: overlay anchor and idle timeout.
/// Scale and reduce motion are read by the overlay page from the event.
pub fn apply(app: &AppHandle, config: &AppConfig) {
    if let Some(overlay) = app.try_state::<Arc<Overlay>>() {
        let anchor = Anchor {
            corner: config.corner,
            monitor: config.monitor,
        };
        if overlay.anchor() != anchor {
            let _ = overlay.set_anchor(anchor);
        }
    }
    if let Some(registry) = app.try_state::<Arc<Mutex<Registry>>>() {
        lock(&registry).set_idle_timeout(config.idle_timeout_ms());
    }
    // The driver sleeps until its next deadline, which may have moved.
    if let Some(bridge) = app.try_state::<Mutex<Option<Bridge>>>() {
        if let Some(bridge) = lock(&bridge).as_ref() {
            let _ = bridge.inputs().send(Input::Wake);
        }
    }
    if let Some(integrations) = app.try_state::<Integrations>() {
        *lock(&integrations.hotkey_error) = integrations.set_hotkey(config.hotkey.as_deref()).err();
        let autostart = match autostart::is_enabled() {
            Some(real) if real != config.start_with_windows => {
                autostart::set_enabled(config.start_with_windows).err()
            }
            // Unsupported here: only complain when someone asks for it.
            None if config.start_with_windows => autostart::set_enabled(true).err(),
            _ => None,
        };
        *lock(&integrations.autostart_error) = autostart;
    }
}

#[tauri::command]
pub fn get_config(state: tauri::State<'_, ConfigState>) -> AppConfig {
    state.get()
}

/// Saves and applies a new config. Returns it as stored (values in range).
#[tauri::command]
pub fn set_config(
    app: AppHandle,
    state: tauri::State<'_, ConfigState>,
    config: AppConfig,
) -> Result<AppConfig, String> {
    let mut config = config.sanitized();
    apply(&app, &config);
    // Start with Windows shows what the registry really says, even if
    // turning it on or off just failed.
    if let Some(real) = autostart::is_enabled() {
        config.start_with_windows = real;
    }
    if let Some(path) = &state.path {
        config::save(path, &config).map_err(|e| format!("could not save settings: {e}"))?;
    }
    *lock(&state.config) = config.clone();
    let _ = app.emit(EVENT_CONFIG, &config);
    Ok(config)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationStatus {
    /// Whether Claude Buddy starts with Windows; `None` where unsupported.
    pub autostart: Option<bool>,
    pub autostart_error: Option<String>,
    /// Why the hotkey is not active (bad text, taken by another app).
    pub hotkey_error: Option<String>,
}

#[tauri::command]
pub fn integration_status(integrations: tauri::State<'_, Integrations>) -> IntegrationStatus {
    IntegrationStatus {
        autostart: autostart::is_enabled(),
        autostart_error: lock(&integrations.autostart_error).clone(),
        hotkey_error: lock(&integrations.hotkey_error).clone(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorInfo {
    pub index: usize,
    pub name: Option<String>,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
}

/// Monitors to choose from, in the order `Anchor::monitor` indexes them.
#[tauri::command]
pub fn list_monitors(app: AppHandle) -> Result<Vec<MonitorInfo>, String> {
    let monitors = app.available_monitors().map_err(|e| e.to_string())?;
    let primary = app.primary_monitor().ok().flatten();
    Ok(monitors
        .iter()
        .enumerate()
        .map(|(index, m)| MonitorInfo {
            index,
            name: m.name().cloned(),
            width: m.size().width,
            height: m.size().height,
            primary: primary
                .as_ref()
                .is_some_and(|p| p.position() == m.position() && p.size() == m.size()),
        })
        .collect())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookStatus {
    /// `None` when it could not be checked (no config folder, unreadable file).
    pub installed: Option<bool>,
    pub settings_path: Option<String>,
    /// Epoch ms of the last hook event since Claude Buddy started.
    pub last_event_at: Option<Millis>,
    pub bridge_running: bool,
}

#[tauri::command]
pub fn hook_status(app: AppHandle) -> HookStatus {
    let actions = crate::tray::hook_actions().ok();
    let installed = actions
        .as_ref()
        .and_then(|a| HookInstaller::new(&a.settings, &a.hook).is_installed().ok());
    let last_event_at = app
        .try_state::<Arc<Mutex<Registry>>>()
        .and_then(|r| lock(&r).last_hook_event_at());
    let bridge_running = app
        .try_state::<Mutex<Option<Bridge>>>()
        .is_some_and(|b| lock(&b).is_some());
    HookStatus {
        installed,
        settings_path: actions.map(|a| a.settings.display().to_string()),
        last_event_at,
        bridge_running,
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
