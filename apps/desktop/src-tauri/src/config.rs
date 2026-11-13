//! The app's own settings (#28), in `config.json` next to `bridge.json`:
//! `%APPDATA%\claude-buddy\config.json` on Windows, the XDG config folder
//! elsewhere. Versioned JSON, read leniently: a bad or unknown field falls
//! back to its default instead of losing the rest, and a file that is not
//! JSON at all is set aside (`config.json.invalid`) rather than overwritten.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::win::Corner;

/// Format version written to the file.
pub const VERSION: u32 = 1;
/// Allowed buddy scales, CSS pixels per sprite pixel.
pub const SCALES: [u32; 3] = [1, 2, 3];
/// The show/hide hotkey until someone picks another one.
pub const DEFAULT_HOTKEY: &str = "Ctrl+Alt+B";
/// Idle → sleeping timeout bounds, minutes.
pub const IDLE_MINUTES: (u32, u32) = (1, 120);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub version: u32,
    pub corner: Corner,
    /// Monitor index; `None` means the primary monitor.
    pub monitor: Option<usize>,
    pub scale: u32,
    /// Minutes without events before an idle buddy falls asleep.
    pub idle_sleep_minutes: u32,
    pub reduce_motion: bool,
    /// Mirrors the real Run-key state (see `win::autostart`).
    pub start_with_windows: bool,
    /// Global show/hide hotkey, e.g. "Ctrl+Alt+B"; `None` for none.
    pub hotkey: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            version: VERSION,
            corner: Corner::default(),
            monitor: None,
            scale: 2,
            idle_sleep_minutes: 5,
            reduce_motion: false,
            start_with_windows: false,
            hotkey: Some(DEFAULT_HOTKEY.to_string()),
        }
    }
}

impl AppConfig {
    /// Reads every known field it can from `value`; the rest stay default.
    pub fn from_value(value: &Value) -> Self {
        let mut config = Self::default();
        let Some(fields) = value.as_object() else {
            return config;
        };
        let field = |name: &str| fields.get(name).cloned();
        if let Some(corner) = field("corner").and_then(|v| serde_json::from_value(v).ok()) {
            config.corner = corner;
        }
        if let Some(monitor) = field("monitor") {
            config.monitor = monitor.as_u64().map(|i| i as usize);
        }
        if let Some(scale) = field("scale").and_then(|v| v.as_u64()) {
            config.scale = scale as u32;
        }
        if let Some(minutes) = field("idleSleepMinutes").and_then(|v| v.as_u64()) {
            config.idle_sleep_minutes = minutes.min(u64::from(u32::MAX)) as u32;
        }
        if let Some(on) = field("reduceMotion").and_then(|v| v.as_bool()) {
            config.reduce_motion = on;
        }
        if let Some(on) = field("startWithWindows").and_then(|v| v.as_bool()) {
            config.start_with_windows = on;
        }
        if let Some(hotkey) = field("hotkey") {
            config.hotkey = hotkey.as_str().map(str::to_owned);
        }
        config.sanitized()
    }

    /// Values brought into range: nearest allowed scale, timeout clamped,
    /// empty hotkey dropped, a valid hotkey spelled canonically ("Ctrl+Alt+B";
    /// an invalid one is kept so the settings window can say why), current version.
    pub fn sanitized(mut self) -> Self {
        self.version = VERSION;
        self.scale = *SCALES
            .iter()
            .min_by_key(|s| s.abs_diff(self.scale))
            .unwrap_or(&2);
        self.idle_sleep_minutes = self
            .idle_sleep_minutes
            .clamp(IDLE_MINUTES.0, IDLE_MINUTES.1);
        self.hotkey = self
            .hotkey
            .map(|h| h.trim().chars().take(64).collect::<String>())
            .filter(|h| !h.is_empty())
            .map(|h| match crate::win::hotkey::parse(&h) {
                Ok(parsed) => crate::win::hotkey::format(parsed),
                Err(_) => h,
            });
        self
    }

    pub fn idle_timeout_ms(&self) -> u64 {
        u64::from(self.idle_sleep_minutes) * 60_000
    }
}

/// `config.json` in the app's data folder (next to `bridge.json`).
pub fn default_path() -> Option<PathBuf> {
    Some(bridge_protocol::bridge_file_path()?.with_file_name("config.json"))
}

/// Loads the config. A missing file gives the defaults; a file that is not
/// JSON is renamed to `config.json.invalid` (so saving does not destroy
/// it) and gives the defaults too.
pub fn load(path: &Path) -> AppConfig {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(_) => return AppConfig::default(),
    };
    match serde_json::from_slice::<Value>(&bytes) {
        Ok(value) => AppConfig::from_value(&value),
        Err(_) => {
            let _ = std::fs::rename(path, path.with_extension("json.invalid"));
            AppConfig::default()
        }
    }
}

/// Writes `config` through a temporary file and a rename, so a crash never
/// leaves half a file.
pub fn save(path: &Path, config: &AppConfig) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut json = serde_json::to_string_pretty(config).map_err(io::Error::other)?;
    json.push('\n');
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, json)?;
    std::fs::rename(&temp, path)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("claude-buddy-config-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("config.json")
    }

    #[test]
    fn missing_file_gives_defaults() {
        assert_eq!(load(&temp("missing")), AppConfig::default());
    }

    #[test]
    fn saves_and_loads_back() {
        let path = temp("roundtrip");
        let config = AppConfig {
            corner: Corner::TopLeft,
            monitor: Some(1),
            scale: 3,
            idle_sleep_minutes: 12,
            reduce_motion: true,
            hotkey: Some("Ctrl+Alt+B".into()),
            ..AppConfig::default()
        };
        save(&path, &config).unwrap();
        assert_eq!(load(&path), config);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"version\": 1") && text.contains("\"idleSleepMinutes\": 12"));
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn bad_fields_fall_back_alone() {
        let config = AppConfig::from_value(&json!({
            "version": 7,
            "corner": "middle",
            "scale": 3,
            "idleSleepMinutes": "soon",
            "reduceMotion": true,
            "future": { "anything": 1 }
        }));
        assert_eq!(
            config,
            AppConfig {
                scale: 3,
                reduce_motion: true,
                ..AppConfig::default()
            }
        );
    }

    #[test]
    fn values_are_brought_into_range() {
        let config = AppConfig::from_value(&json!({
            "scale": 9, "idleSleepMinutes": 0, "hotkey": "   ", "monitor": -1
        }));
        assert_eq!(config.scale, 3);
        assert_eq!(config.idle_sleep_minutes, 1);
        assert_eq!(config.hotkey, None);
        let spelled = AppConfig::from_value(&json!({ "hotkey": " alt+ctrl+b " }));
        assert_eq!(spelled.hotkey.as_deref(), Some("Ctrl+Alt+B"));
        let kept = AppConfig::from_value(&json!({ "hotkey": "Hyper+Q" }));
        assert_eq!(kept.hotkey.as_deref(), Some("Hyper+Q"));
        // Missing means the default; null means no hotkey.
        assert_eq!(
            AppConfig::from_value(&json!({})).hotkey.as_deref(),
            Some(DEFAULT_HOTKEY)
        );
        assert_eq!(
            AppConfig::from_value(&json!({ "hotkey": null })).hotkey,
            None
        );
        assert_eq!(config.monitor, None);
        let long = AppConfig::from_value(&json!({ "idleSleepMinutes": 100000, "scale": 0 }));
        assert_eq!((long.idle_sleep_minutes, long.scale), (120, 1));
        assert_eq!(long.idle_timeout_ms(), 120 * 60_000);
    }

    #[test]
    fn a_file_that_is_not_json_is_set_aside() {
        let path = temp("garbage");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{ oops").unwrap();
        assert_eq!(load(&path), AppConfig::default());
        assert!(!path.exists());
        let kept = path.with_extension("json.invalid");
        assert_eq!(std::fs::read_to_string(kept).unwrap(), "{ oops");
    }

    #[test]
    fn not_an_object_gives_defaults() {
        assert_eq!(AppConfig::from_value(&json!([1, 2])), AppConfig::default());
    }
}
