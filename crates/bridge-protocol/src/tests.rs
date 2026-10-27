use std::collections::HashMap;
use std::path::Path;

use super::*;

/// The `POST /v1/events` example from docs/architecture.md#bridge-protocol.
const DOC_EVENT: &str = r#"{
  "v": 1,
  "event": "PreToolUse",
  "session_id": "11111111-1111-4111-8111-111111111111",
  "cwd": "C:\\Users\\me\\projects\\demo",
  "at": 1793072412345,
  "claude_pid": 9184,
  "tool_name": "Read",
  "target": "index.ts"
}"#;

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn doc_event() -> EventBody {
    serde_json::from_str(DOC_EVENT).unwrap()
}

fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
    let vars: HashMap<String, OsString> = vars
        .iter()
        .map(|(k, v)| (k.to_string(), OsString::from(v)))
        .collect();
    move |name| vars.get(name).cloned()
}

#[test]
fn doc_event_round_trips() {
    let event = doc_event();
    assert_eq!(event.cwd, r"C:\Users\me\projects\demo");
    assert_eq!(event.claude_pid, Some(9184));
    assert_eq!(event.notification_type, None);

    let json: serde_json::Value = serde_json::to_value(&event).unwrap();
    let expected: serde_json::Value = serde_json::from_str(DOC_EVENT).unwrap();
    assert_eq!(json, expected, "absent optional fields are not serialised");
    assert_eq!(event.validate(), Ok(()));
}

#[test]
fn unknown_fields_are_ignored() {
    let mut value: serde_json::Value = serde_json::from_str(DOC_EVENT).unwrap();
    value["added_in_a_later_version"] = serde_json::json!(true);
    let event: EventBody = serde_json::from_value(value).unwrap();
    assert_eq!(event, doc_event());
}

#[test]
fn missing_required_field_fails_to_parse() {
    let mut value: serde_json::Value = serde_json::from_str(DOC_EVENT).unwrap();
    value.as_object_mut().unwrap().remove("session_id");
    assert!(serde_json::from_value::<EventBody>(value).is_err());
}

#[test]
fn doc_bridge_file_round_trips() {
    let json = format!(
        r#"{{"protocol":1,"port":51234,"token":"{TOKEN}","pid":7312,"started_at":1793072400000}}"#
    );
    let file: BridgeFile = serde_json::from_str(&json).unwrap();
    assert_eq!(file.port, 51234);
    assert!(file.is_usable());
    assert_eq!(serde_json::to_string(&file).unwrap(), json);
}

#[test]
fn bridge_file_needs_supported_version_and_token() {
    let file = BridgeFile {
        protocol: 1,
        port: 1,
        token: TOKEN.into(),
        pid: 1,
        started_at: 0,
    };
    assert!(file.is_usable());
    assert!(!BridgeFile {
        protocol: 2,
        ..file.clone()
    }
    .is_usable());
    assert!(!BridgeFile {
        token: TOKEN.to_uppercase(),
        ..file.clone()
    }
    .is_usable());
    assert!(!BridgeFile {
        token: TOKEN[..63].into(),
        ..file
    }
    .is_usable());
}

#[test]
fn wrong_version_is_rejected() {
    let event = EventBody {
        v: 2,
        ..doc_event()
    };
    assert_eq!(
        event.validate(),
        Err(ValidationError::UnsupportedVersion(2))
    );
}

#[test]
fn per_event_required_fields() {
    let tool_event = EventBody {
        tool_name: None,
        ..doc_event()
    };
    assert_eq!(
        tool_event.validate(),
        Err(ValidationError::Missing("tool_name"))
    );

    let notification = EventBody {
        event: "Notification".into(),
        tool_name: None,
        ..doc_event()
    };
    assert_eq!(
        notification.validate(),
        Err(ValidationError::Missing("notification_type"))
    );

    let start = EventBody {
        event: "SessionStart".into(),
        tool_name: None,
        ..doc_event()
    };
    assert_eq!(
        start.validate(),
        Err(ValidationError::Missing("start_source"))
    );
}

#[test]
fn unknown_events_validate_but_are_not_known() {
    let event = EventBody {
        event: "SomethingNew".into(),
        tool_name: None,
        ..doc_event()
    };
    assert_eq!(event.validate(), Ok(()));
    assert!(!event.is_known_event());
    assert!(doc_event().is_known_event());
}

#[test]
fn over_limit_strings_are_rejected_and_clamped() {
    let mut event = EventBody {
        target: Some("é".repeat(limits::TARGET + 1)),
        cwd: "x".repeat(limits::CWD + 5),
        ..doc_event()
    };
    assert_eq!(
        event.validate(),
        Err(ValidationError::TooLong {
            field: "cwd",
            max: limits::CWD
        })
    );

    event.clamp_lengths();
    assert_eq!(event.validate(), Ok(()));
    assert_eq!(
        event.target.as_ref().unwrap().chars().count(),
        limits::TARGET
    );
    assert_eq!(event.cwd.chars().count(), limits::CWD);
}

#[test]
fn exactly_at_limit_is_fine() {
    let event = EventBody {
        target: Some("t".repeat(limits::TARGET)),
        ..doc_event()
    };
    assert_eq!(event.validate(), Ok(()));
}

#[test]
fn truncate_counts_scalar_values() {
    assert_eq!(truncate("héllo", 2), "hé");
    assert_eq!(truncate("hi", 10), "hi");
    assert_eq!(truncate("🦀🦀🦀", 1), "🦀");
    assert_eq!(truncate("", 0), "");
}

#[test]
fn doc_event_fits_the_body_limit() {
    assert!(DOC_EVENT.len() < MAX_BODY_BYTES);
}

#[test]
fn windows_path_uses_appdata() {
    let appdata = r"C:\Users\me\AppData\Roaming";
    let path = resolve_bridge_file_path(true, env(&[("APPDATA", appdata)])).unwrap();
    assert_eq!(
        path,
        Path::new(appdata).join("claude-buddy").join("bridge.json")
    );
}

#[test]
fn windows_without_appdata_has_no_path() {
    assert_eq!(resolve_bridge_file_path(true, env(&[])), None);
    assert_eq!(
        resolve_bridge_file_path(true, env(&[("APPDATA", "")])),
        None
    );
}

#[test]
fn unix_path_prefers_absolute_xdg_config_home() {
    let path = resolve_bridge_file_path(
        false,
        env(&[("XDG_CONFIG_HOME", "/xdg"), ("HOME", "/home/me")]),
    );
    assert_eq!(path, Some(PathBuf::from("/xdg/claude-buddy/bridge.json")));
}

#[test]
fn unix_path_falls_back_to_home_config() {
    for vars in [
        vec![("HOME", "/home/me")],
        vec![("HOME", "/home/me"), ("XDG_CONFIG_HOME", "relative/dir")],
    ] {
        assert_eq!(
            resolve_bridge_file_path(false, env(&vars)),
            Some(PathBuf::from("/home/me/.config/claude-buddy/bridge.json"))
        );
    }
}

#[test]
fn override_wins_on_every_os() {
    for windows in [true, false] {
        let vars = env(&[
            (BRIDGE_FILE_ENV, "/tmp/test-bridge.json"),
            ("APPDATA", r"C:\x"),
            ("HOME", "/home/me"),
        ]);
        assert_eq!(
            resolve_bridge_file_path(windows, vars),
            Some(PathBuf::from("/tmp/test-bridge.json"))
        );
    }
}
