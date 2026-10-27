//! Wire contract between `buddy-hook` (client) and the Claude Buddy bridge
//! (server), protocol version 1. The prose spec is the "Bridge protocol"
//! section of docs/architecture.md; this crate is its single implementation.
//!
//! Types, limits, path resolution and the `target` sanitising rules live
//! here; HTTP handling is the server's job. [`target::derive`] is shared by
//! every client that builds a `target` (`buddy-hook` and the transcript
//! watcher), so the privacy rules exist once.

use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub mod target;

pub const PROTOCOL_VERSION: u32 = 1;
pub const EVENTS_PATH: &str = "/v1/events";
pub const HEALTH_PATH: &str = "/v1/health";
/// Largest accepted request body.
pub const MAX_BODY_BYTES: usize = 16 * 1024;
/// Overrides the bridge file path on both sides. For tests only.
pub const BRIDGE_FILE_ENV: &str = "CLAUDE_BUDDY_BRIDGE_FILE";
/// Length of [`BridgeFile::token`] in hex chars (32 random bytes).
pub const TOKEN_HEX_LEN: usize = 64;

/// Maximum lengths of [`EventBody`] strings, in Unicode scalar values.
pub mod limits {
    pub const EVENT: usize = 32;
    pub const SESSION_ID: usize = 64;
    pub const CWD: usize = 1024;
    pub const TOOL_NAME: usize = 128;
    pub const TARGET: usize = 60;
    pub const NOTIFICATION_TYPE: usize = 32;
    pub const START_SOURCE: usize = 16;
    pub const AGENT_ID: usize = 64;
}

/// Hook events the server maps to registry events (or knowingly ignores).
/// Anything else is accepted with `202` and dropped.
pub const KNOWN_EVENTS: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "Notification",
    "PermissionRequest",
    "Stop",
    "SubagentStart",
    "SubagentStop",
    "SessionEnd",
];

const TOOL_EVENTS: &[&str] = &[
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "PermissionRequest",
];

/// Contents of `bridge.json`, written by the app and read by `buddy-hook`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeFile {
    pub protocol: u32,
    pub port: u16,
    pub token: String,
    pub pid: u32,
    pub started_at: u64,
}

impl BridgeFile {
    /// Whether a client should use this file: supported version and a well-formed token.
    pub fn is_usable(&self) -> bool {
        self.protocol == PROTOCOL_VERSION && is_valid_token(&self.token)
    }
}

/// `true` for exactly [`TOKEN_HEX_LEN`] lowercase hex chars.
pub fn is_valid_token(token: &str) -> bool {
    token.len() == TOKEN_HEX_LEN
        && token
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Body of `POST /v1/events`. Unknown fields are ignored on input.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventBody {
    pub v: u32,
    pub event: String,
    pub session_id: String,
    pub cwd: String,
    pub at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claude_pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notification_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationError {
    UnsupportedVersion(u32),
    Missing(&'static str),
    TooLong { field: &'static str, max: usize },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion(v) => write!(f, "unsupported protocol version {v}"),
            Self::Missing(field) => write!(f, "missing field `{field}`"),
            Self::TooLong { field, max } => write!(f, "`{field}` is longer than {max} chars"),
        }
    }
}

impl std::error::Error for ValidationError {}

impl EventBody {
    /// Whether the server maps this event (otherwise it answers `202`).
    pub fn is_known_event(&self) -> bool {
        KNOWN_EVENTS.contains(&self.event.as_str())
    }

    /// Server-side check: version, per-event required fields and length limits.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.v != PROTOCOL_VERSION {
            return Err(ValidationError::UnsupportedVersion(self.v));
        }
        let required = |field, value: &Option<String>| {
            value
                .as_ref()
                .map(|_| ())
                .ok_or(ValidationError::Missing(field))
        };
        let event = self.event.as_str();
        if TOOL_EVENTS.contains(&event) {
            required("tool_name", &self.tool_name)?;
        }
        if event == "Notification" {
            required("notification_type", &self.notification_type)?;
        }
        if event == "SessionStart" {
            required("start_source", &self.start_source)?;
        }
        for (field, value, max) in self.strings() {
            if value.is_some_and(|v| v.chars().count() > max) {
                return Err(ValidationError::TooLong { field, max });
            }
        }
        Ok(())
    }

    /// Client-side: cuts every string to its limit so [`EventBody::validate`] passes.
    pub fn clamp_lengths(&mut self) {
        let clamp = |s: &mut String, max| {
            let cut = truncate(s, max).len();
            s.truncate(cut);
        };
        clamp(&mut self.event, limits::EVENT);
        clamp(&mut self.session_id, limits::SESSION_ID);
        clamp(&mut self.cwd, limits::CWD);
        for (value, max) in [
            (&mut self.tool_name, limits::TOOL_NAME),
            (&mut self.target, limits::TARGET),
            (&mut self.notification_type, limits::NOTIFICATION_TYPE),
            (&mut self.start_source, limits::START_SOURCE),
            (&mut self.agent_id, limits::AGENT_ID),
        ] {
            if let Some(s) = value {
                clamp(s, max);
            }
        }
    }

    fn strings(&self) -> [(&'static str, Option<&str>, usize); 8] {
        [
            ("event", Some(self.event.as_str()), limits::EVENT),
            (
                "session_id",
                Some(self.session_id.as_str()),
                limits::SESSION_ID,
            ),
            ("cwd", Some(self.cwd.as_str()), limits::CWD),
            ("tool_name", self.tool_name.as_deref(), limits::TOOL_NAME),
            ("target", self.target.as_deref(), limits::TARGET),
            (
                "notification_type",
                self.notification_type.as_deref(),
                limits::NOTIFICATION_TYPE,
            ),
            (
                "start_source",
                self.start_source.as_deref(),
                limits::START_SOURCE,
            ),
            ("agent_id", self.agent_id.as_deref(), limits::AGENT_ID),
        ]
    }
}

/// The first `max` Unicode scalar values of `s`.
pub fn truncate(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((cut, _)) => &s[..cut],
        None => s,
    }
}

/// Path of `bridge.json` for this OS, from environment variables only.
/// `None` when the needed variables are missing.
pub fn bridge_file_path() -> Option<PathBuf> {
    resolve_bridge_file_path(cfg!(windows), |name| std::env::var_os(name))
}

/// [`bridge_file_path`] with the OS and environment injected, for tests.
pub fn resolve_bridge_file_path(
    windows: bool,
    env: impl Fn(&str) -> Option<OsString>,
) -> Option<PathBuf> {
    let non_empty = |name: &str| env(name).filter(|v| !v.is_empty());
    if let Some(path) = non_empty(BRIDGE_FILE_ENV) {
        return Some(PathBuf::from(path));
    }
    let base = if windows {
        PathBuf::from(non_empty("APPDATA")?)
    } else {
        // Unix rules on purpose (not the host's `Path::is_absolute`), so the
        // result does not depend on which OS runs the resolver.
        let xdg = non_empty("XDG_CONFIG_HOME").filter(|dir| dir.to_string_lossy().starts_with('/'));
        match xdg {
            Some(dir) => PathBuf::from(dir),
            None => PathBuf::from(non_empty("HOME")?).join(".config"),
        }
    };
    Some(base.join("claude-buddy").join("bridge.json"))
}

#[cfg(test)]
mod tests;
