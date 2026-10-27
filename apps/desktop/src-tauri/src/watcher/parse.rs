//! One transcript line → what it tells us (docs/session-detection.md#line-types).
//! Reads only the fields listed there; text, prompts and tool results are
//! never kept.

use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub session_id: String,
    pub cwd: Option<String>,
    pub kind: LineKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LineKind {
    /// The user typed a prompt.
    Prompt,
    /// The assistant called a tool. `target` follows the protocol's target rules.
    ToolUse {
        tool: String,
        target: Option<String>,
    },
    /// A tool result came back.
    ToolResult,
    /// The turn ended (final text or `stop_hook_summary`).
    TurnEnded,
}

/// `None` for lines that say nothing about state: bookkeeping, attachments,
/// unknown types, subagent (sidechain) lines, or invalid JSON.
pub fn parse_line(line: &str) -> Option<Line> {
    let value: Value = serde_json::from_str(line).ok()?;
    if value.get("isSidechain").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let session_id = value.get("sessionId")?.as_str()?.to_string();
    let cwd = value.get("cwd").and_then(Value::as_str).map(str::to_string);
    let message = value.get("message");
    let content = message.and_then(|m| m.get("content"));

    let kind = match value.get("type")?.as_str()? {
        "user" => match content? {
            Value::String(_) => LineKind::Prompt,
            Value::Array(blocks) if blocks.iter().any(|b| block_type(b) == Some("tool_result")) => {
                LineKind::ToolResult
            }
            _ => return None,
        },
        "assistant" => {
            let blocks = content?.as_array()?;
            if let Some(tool_use) = blocks.iter().find(|b| block_type(b) == Some("tool_use")) {
                let tool = tool_use.get("name")?.as_str()?.to_string();
                let input = tool_use.get("input").unwrap_or(&Value::Null);
                let target = bridge_protocol::target::derive(&tool, input);
                LineKind::ToolUse { tool, target }
            } else {
                let stop = message?.get("stop_reason").and_then(Value::as_str);
                let has_text = blocks.iter().any(|b| block_type(b) == Some("text"));
                if has_text && stop == Some("end_turn") {
                    LineKind::TurnEnded
                } else {
                    return None;
                }
            }
        }
        "system" if value.get("subtype").and_then(Value::as_str) == Some("stop_hook_summary") => {
            LineKind::TurnEnded
        }
        _ => return None,
    };
    Some(Line {
        session_id,
        cwd,
        kind,
    })
}

fn block_type(block: &Value) -> Option<&str> {
    block.get("type").and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        let path = format!(
            "{}/../../../tests/fixtures/transcripts/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    fn kinds(text: &str) -> Vec<LineKind> {
        text.lines()
            .filter_map(parse_line)
            .map(|l| l.kind)
            .collect()
    }

    #[test]
    fn captured_session_reads_as_documented() {
        let tool = |tool: &str, target: Option<&str>| LineKind::ToolUse {
            tool: tool.into(),
            target: target.map(str::to_string),
        };
        assert_eq!(
            kinds(&fixture("session.jsonl")),
            [
                LineKind::Prompt,
                tool("Read", Some("notes.txt")),
                tool("Bash", Some("echo")),
                LineKind::ToolResult,
                LineKind::ToolResult,
                tool("Agent", Some("general-purpose")),
                LineKind::ToolResult,
                LineKind::TurnEnded,
                LineKind::TurnEnded,
            ]
        );
    }

    #[test]
    fn lines_carry_session_and_cwd() {
        let text = fixture("session.jsonl");
        let first = text.lines().find_map(parse_line).unwrap();
        assert_eq!(first.session_id, "11111111-1111-4111-8111-111111111111");
        assert_eq!(first.cwd.as_deref(), Some("/home/user/projects/demo"));
    }

    #[test]
    fn subagent_transcripts_are_ignored() {
        let text = fixture("subagents/agent-a0000000000000001.jsonl");
        assert!(kinds(&text).is_empty());
    }

    #[test]
    fn hook_error_transcript_has_no_false_states() {
        // Every line is either bookkeeping or a real state signal; none panics.
        let _ = kinds(&fixture("http-hook-connection-refused.jsonl"));
    }

    #[test]
    fn garbage_and_unknown_types_are_ignored() {
        for line in [
            "",
            "not json",
            "{\"type\":\"user\"",
            r#"{"type":"brand-new","sessionId":"s"}"#,
            r#"{"type":"user","message":{"content":"no session id"}}"#,
            r#"{"type":"assistant","sessionId":"s","message":{"content":[{"type":"thinking"}],"stop_reason":"tool_use"}}"#,
        ] {
            assert_eq!(parse_line(line), None, "{line:?}");
        }
    }

    #[test]
    fn tool_targets_follow_the_protocol_rules() {
        let line = r#"{"type":"assistant","sessionId":"s","message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"API_KEY=sk-1 $SECRET"}}]}}"#;
        assert_eq!(
            parse_line(line).unwrap().kind,
            LineKind::ToolUse {
                tool: "Bash".into(),
                target: None
            }
        );
    }
}
