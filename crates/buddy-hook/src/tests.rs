use super::*;

fn fixture(name: &str) -> String {
    let path = format!(
        "{}/../../tests/fixtures/hooks/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn event(name: &str) -> EventBody {
    build_event(None, &fixture(name), Some("4242"), 1_000).expect(name)
}

/// Every field a body may carry (docs/architecture.md#bridge-protocol).
const ALLOWED_KEYS: &[&str] = &[
    "v",
    "event",
    "session_id",
    "cwd",
    "at",
    "claude_pid",
    "tool_name",
    "target",
    "notification_type",
    "start_source",
    "agent_id",
];

#[test]
fn every_fixture_yields_a_valid_body_with_only_protocol_fields() {
    for dir in ["captured", "from-docs"] {
        let path = format!(
            "{}/../../tests/fixtures/hooks/{dir}",
            env!("CARGO_MANIFEST_DIR")
        );
        for entry in std::fs::read_dir(path).unwrap() {
            let name = format!("{dir}/{}", entry.unwrap().file_name().to_string_lossy());
            let body = event(&name);
            assert_eq!(body.validate(), Ok(()), "{name}");
            let json = serde_json::to_value(&body).unwrap();
            for key in json.as_object().unwrap().keys() {
                assert!(ALLOWED_KEYS.contains(&key.as_str()), "{name}: {key}");
            }
        }
    }
}

#[test]
fn never_forwards_prompts_inputs_outputs_or_contents() {
    for name in [
        "captured/user-prompt-submit.json",
        "captured/pre-tool-use-agent.json",
        "captured/post-tool-use-bash.json",
        "captured/stop.json",
        "from-docs/pre-tool-use-write-windows.json",
        "from-docs/notification-permission-prompt.json",
    ] {
        let payload: Value = serde_json::from_str(&fixture(name)).unwrap();
        let sent = serde_json::to_string(&event(name)).unwrap();
        for field in [
            "prompt",
            "tool_response",
            "last_assistant_message",
            "message",
            "title",
            "transcript_path",
        ] {
            assert!(
                !sent.contains(&format!("\"{field}\"")),
                "{name} leaks {field}"
            );
        }
        // No text from those fields appears anywhere in what we send.
        let secrets = [
            payload.get("prompt"),
            payload.pointer("/tool_input/prompt"),
            payload.pointer("/tool_input/content"),
            payload.pointer("/tool_input/command"),
            payload.get("message"),
            payload.get("last_assistant_message"),
        ];
        for secret in secrets.into_iter().flatten().filter_map(Value::as_str) {
            if secret.len() > 8 {
                assert!(!sent.contains(secret), "{name} leaks {secret:?}");
            }
        }
    }
}

#[test]
fn pre_tool_use_carries_the_tool_and_a_derived_target() {
    let read = event("captured/pre-tool-use-read.json");
    assert_eq!(read.event, "PreToolUse");
    assert_eq!(read.session_id, "11111111-1111-4111-8111-111111111111");
    assert_eq!(read.cwd, "/home/user/projects/demo");
    assert_eq!(read.claude_pid, Some(4242));
    assert_eq!(read.at, 1_000);
    assert_eq!(read.tool_name.as_deref(), Some("Read"));
    assert_eq!(read.target.as_deref(), Some("notes.txt"));

    assert_eq!(
        event("captured/pre-tool-use-bash.json").target.as_deref(),
        Some("echo")
    );
    assert_eq!(
        event("captured/pre-tool-use-agent.json").target.as_deref(),
        Some("general-purpose")
    );
    let windows = event("from-docs/pre-tool-use-write-windows.json");
    assert_eq!(windows.target.as_deref(), Some("index.ts"));
}

#[test]
fn only_pre_tool_use_gets_a_target() {
    let post = event("captured/post-tool-use-read.json");
    assert_eq!(post.tool_name.as_deref(), Some("Read"));
    assert_eq!(post.target, None);
    assert_eq!(event("from-docs/permission-request-bash.json").target, None);
}

#[test]
fn event_specific_fields() {
    assert_eq!(
        event("captured/session-start.json").start_source.as_deref(),
        Some("startup")
    );
    assert_eq!(
        event("from-docs/notification-permission-prompt.json")
            .notification_type
            .as_deref(),
        Some("permission_prompt")
    );
    assert_eq!(
        event("captured/pre-tool-use-read-in-subagent.json")
            .agent_id
            .as_deref(),
        Some("a0000000000000001")
    );
    assert_eq!(event("captured/stop.json").start_source, None);
}

#[test]
fn bad_input_gives_nothing_to_send() {
    for stdin in ["", "not json", "[]", r#"{"hook_event_name":"Stop"}"#] {
        assert!(
            build_event(Some("Stop"), stdin, None, 0).is_none(),
            "{stdin:?}"
        );
    }
    // Tool event without its tool name fails validation.
    let no_tool = r#"{"hook_event_name":"PreToolUse","session_id":"s","cwd":"/x"}"#;
    assert!(build_event(None, no_tool, None, 0).is_none());
}

#[test]
fn the_argument_names_the_event_when_the_payload_does_not() {
    let stdin = r#"{"session_id":"s","cwd":"/x"}"#;
    assert_eq!(
        build_event(Some("Stop"), stdin, None, 0).unwrap().event,
        "Stop"
    );
}

#[test]
fn bad_pid_is_dropped_and_long_values_are_clamped() {
    let long_cwd = "/x".repeat(2_000);
    let stdin = format!(r#"{{"hook_event_name":"Stop","session_id":"s","cwd":"{long_cwd}"}}"#);
    let body = build_event(None, &stdin, Some("not a pid"), 0).unwrap();
    assert_eq!(body.claude_pid, None);
    assert_eq!(body.cwd.chars().count(), bridge_protocol::limits::CWD);
}

#[test]
fn bridge_file_must_be_usable() {
    let token = "a".repeat(64);
    let good = format!(r#"{{"protocol":1,"port":5000,"token":"{token}","pid":1,"started_at":0}}"#);
    let (addr, read_token) = read_bridge_file(&good).unwrap();
    assert_eq!(addr.to_string(), "127.0.0.1:5000");
    assert_eq!(read_token, token);
    assert!(read_bridge_file(&good.replace(r#""protocol":1"#, r#""protocol":2"#)).is_none());
    assert!(read_bridge_file(
        r#"{"protocol":1,"port":5000,"token":"short","pid":1,"started_at":0}"#
    )
    .is_none());
    assert!(read_bridge_file("garbage").is_none());
}
