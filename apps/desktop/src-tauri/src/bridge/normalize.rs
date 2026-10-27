//! Wire event → registry event, per docs/architecture.md#server-event--registry.

use bridge_protocol::EventBody;

use crate::registry::{EventKind, Millis, RegistryEvent, Source};

/// Notification types that mean the user is needed.
const WAITING_NOTIFICATIONS: &[&str] = &[
    "permission_prompt",
    "idle_prompt",
    "elicitation_dialog",
    "elicitation_url_dialog",
    "agent_needs_input",
];

/// Maps a validated body to a registry event stamped with the bridge's
/// receive time. `None` for events the MVP knowingly ignores.
pub fn to_registry_event(body: EventBody, received_at: Millis) -> Option<RegistryEvent> {
    let tool = || body.tool_name.clone().unwrap_or_default();
    let kind = match body.event.as_str() {
        "SessionStart" => EventKind::SessionStarted,
        "UserPromptSubmit" => EventKind::PromptSubmitted,
        "PreToolUse" => EventKind::ToolStarted {
            summary: Some(match &body.target {
                Some(target) => format!("{} · {target}", tool()),
                None => tool(),
            }),
            tool: tool(),
        },
        "PostToolUse" | "PostToolUseFailure" => EventKind::ToolFinished { tool: tool() },
        "Notification" => {
            let kind = body.notification_type.as_deref().unwrap_or_default();
            if !WAITING_NOTIFICATIONS.contains(&kind) {
                return None;
            }
            EventKind::NeedsInput
        }
        "PermissionRequest" => EventKind::NeedsInput,
        "Stop" => EventKind::TurnEnded,
        "SessionEnd" => EventKind::SessionEnded,
        _ => return None,
    };
    Some(RegistryEvent {
        session_id: body.session_id,
        source: Source::Hooks,
        at: received_at,
        cwd: Some(body.cwd),
        pid: body.claude_pid,
        kind,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(event: &str) -> EventBody {
        EventBody {
            v: 1,
            event: event.into(),
            session_id: "s".into(),
            cwd: r"C:\dev\demo".into(),
            at: 1,
            claude_pid: Some(9),
            tool_name: None,
            target: None,
            notification_type: None,
            start_source: None,
            agent_id: None,
        }
    }

    fn kind(body: EventBody) -> Option<EventKind> {
        to_registry_event(body, 100).map(|e| e.kind)
    }

    #[test]
    fn carries_session_cwd_pid_and_receive_time() {
        let event = to_registry_event(body("Stop"), 100).unwrap();
        assert_eq!(event.session_id, "s");
        assert_eq!(event.cwd.as_deref(), Some(r"C:\dev\demo"));
        assert_eq!(event.pid, Some(9));
        assert_eq!(event.at, 100, "receive time, not the client's `at`");
        assert_eq!(event.source, Source::Hooks);
    }

    #[test]
    fn lifecycle_events() {
        assert_eq!(kind(body("SessionStart")), Some(EventKind::SessionStarted));
        assert_eq!(
            kind(body("UserPromptSubmit")),
            Some(EventKind::PromptSubmitted)
        );
        assert_eq!(kind(body("Stop")), Some(EventKind::TurnEnded));
        assert_eq!(kind(body("SessionEnd")), Some(EventKind::SessionEnded));
        assert_eq!(kind(body("PermissionRequest")), Some(EventKind::NeedsInput));
    }

    #[test]
    fn tool_events_build_a_summary() {
        let with_target = EventBody {
            tool_name: Some("Read".into()),
            target: Some("index.ts".into()),
            ..body("PreToolUse")
        };
        assert_eq!(
            kind(with_target),
            Some(EventKind::ToolStarted {
                tool: "Read".into(),
                summary: Some("Read · index.ts".into())
            })
        );

        let without_target = EventBody {
            tool_name: Some("Bash".into()),
            ..body("PreToolUse")
        };
        assert_eq!(
            kind(without_target),
            Some(EventKind::ToolStarted {
                tool: "Bash".into(),
                summary: Some("Bash".into())
            })
        );

        for event in ["PostToolUse", "PostToolUseFailure"] {
            let finished = EventBody {
                tool_name: Some("Bash".into()),
                ..body(event)
            };
            assert_eq!(
                kind(finished),
                Some(EventKind::ToolFinished {
                    tool: "Bash".into()
                })
            );
        }
    }

    #[test]
    fn only_waiting_notifications_count() {
        for kind_name in WAITING_NOTIFICATIONS {
            let event = EventBody {
                notification_type: Some(kind_name.to_string()),
                ..body("Notification")
            };
            assert_eq!(kind(event), Some(EventKind::NeedsInput), "{kind_name}");
        }
        let other = EventBody {
            notification_type: Some("auth_success".into()),
            ..body("Notification")
        };
        assert_eq!(kind(other), None);
    }

    #[test]
    fn subagent_and_unknown_events_are_ignored() {
        assert_eq!(kind(body("SubagentStart")), None);
        assert_eq!(kind(body("SubagentStop")), None);
        assert_eq!(kind(body("SomethingNew")), None);
    }
}
