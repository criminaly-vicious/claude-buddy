//! `target` rules from docs/architecture.md#target-rules: a short, sanitised
//! hint of what a tool is working on. When in doubt, no target.

use serde_json::Value;

use crate::{limits, truncate};

/// The `target` for `tool_name` called with `tool_input`, or `None` when the
/// tool has no target rule or the input does not look safe.
pub fn derive(tool_name: &str, tool_input: &Value) -> Option<String> {
    let field = |name: &str| tool_input.get(name).and_then(Value::as_str);
    let target = match tool_name {
        "Read" | "Edit" | "MultiEdit" | "Write" => file_name(field("file_path")?),
        "NotebookEdit" => file_name(field("notebook_path")?),
        "Grep" | "Glob" => field("pattern")?,
        "WebSearch" => field("query")?,
        "WebFetch" => url_host(field("url")?)?,
        "Bash" | "PowerShell" => program_name(field("command")?)?,
        "Agent" | "Task" => field("subagent_type")?,
        _ => return None,
    };
    let target = truncate(target.trim(), limits::TARGET);
    (!target.is_empty()).then(|| target.to_string())
}

/// Last path segment, accepting `/` and `\` separators.
fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\'])
        .find(|s| !s.is_empty())
        .unwrap_or_default()
}

/// Host of an absolute URL, without user info, port, path or query.
fn url_host(url: &str) -> Option<&str> {
    let (_, rest) = url.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host_port = authority.rsplit('@').next()?;
    let host = match host_port.strip_prefix('[') {
        // IPv6 literal: `[::1]:8080`.
        Some(v6) => v6.split(']').next()?,
        None => host_port.split(':').next()?,
    };
    (!host.is_empty()).then_some(host)
}

/// Program name of a shell command. Leading `NAME=value` assignments are
/// skipped; anything that still looks like an assignment, a variable or a
/// quoted string gives no target, because it may hold a secret.
fn program_name(command: &str) -> Option<&str> {
    let first = command
        .split_whitespace()
        .find(|token| !is_env_assignment(token))?;
    let risky = first.contains(['=', '$', '"', '\'', '`'])
        || first.to_ascii_lowercase().starts_with("$env:");
    if risky {
        return None;
    }
    Some(file_name(first))
}

/// `NAME=value` with a shell-style identifier before `=`.
fn is_env_assignment(token: &str) -> bool {
    let Some((name, _)) = token.split_once('=') else {
        return false;
    };
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn target(tool: &str, input: Value) -> Option<String> {
        derive(tool, &input)
    }

    #[test]
    fn file_tools_send_the_file_name_only() {
        for tool in ["Read", "Edit", "MultiEdit", "Write"] {
            assert_eq!(
                target(tool, json!({"file_path": "/home/me/src/app.ts"})),
                Some("app.ts".into()),
                "{tool}"
            );
        }
        assert_eq!(
            target(
                "Write",
                json!({"file_path": r"C:\Users\me\proj\src\index.ts"})
            ),
            Some("index.ts".into())
        );
        assert_eq!(
            target(
                "NotebookEdit",
                json!({"notebook_path": "/n/analysis.ipynb"})
            ),
            Some("analysis.ipynb".into())
        );
    }

    #[test]
    fn search_tools_send_the_pattern_or_query() {
        assert_eq!(
            target("Grep", json!({"pattern": "fn main"})),
            Some("fn main".into())
        );
        assert_eq!(
            target("Glob", json!({"pattern": "**/*.rs"})),
            Some("**/*.rs".into())
        );
        assert_eq!(
            target("WebSearch", json!({"query": "tauri transparent window"})),
            Some("tauri transparent window".into())
        );
    }

    #[test]
    fn web_fetch_sends_the_host_only() {
        for (url, host) in [
            ("https://docs.rs/tauri/latest?search=x#y", "docs.rs"),
            ("http://user:secret@example.com:8080/private", "example.com"),
            ("http://[::1]:3000/", "::1"),
        ] {
            assert_eq!(
                target("WebFetch", json!({ "url": url })),
                Some(host.into()),
                "{url}"
            );
        }
        assert_eq!(target("WebFetch", json!({"url": "not a url"})), None);
    }

    #[test]
    fn shell_sends_the_program_name_only() {
        for (command, program) in [
            ("git push origin main", "git"),
            ("/usr/bin/cargo test --workspace", "cargo"),
            (r"C:\tools\node.exe build.js", "node.exe"),
            ("RUST_LOG=debug cargo run", "cargo"),
            ("A=1 B=2 make", "make"),
        ] {
            assert_eq!(
                target("Bash", json!({ "command": command })),
                Some(program.into()),
                "{command}"
            );
        }
    }

    #[test]
    fn shell_never_leaks_assignments_or_variables() {
        for command in [
            "API_KEY=sk-123",
            "$env:TOKEN='x'; git push",
            "$TOKEN do-something",
            "\"quoted program\" arg",
            "'single' arg",
            "`backtick`",
            "--flag=value cmd",
            "",
            "   ",
        ] {
            assert_eq!(
                target("Bash", json!({ "command": command })),
                None,
                "{command:?}"
            );
            assert_eq!(
                target("PowerShell", json!({ "command": command })),
                None,
                "{command:?}"
            );
        }
    }

    #[test]
    fn review_cases_from_the_protocol_table() {
        let bash = |command: &str| target("Bash", json!({ "command": command }));
        assert_eq!(bash("API_KEY=x curl https://x.dev"), Some("curl".into()));
        assert_eq!(bash("$env:T='x'; git push"), None);
        assert_eq!(bash(r#""C:\Program Files\git.exe" status"#), None);
        assert_eq!(
            target(
                "WebFetch",
                json!({"url": "https://example.com/p?token=abc"})
            ),
            Some("example.com".into())
        );
        assert_eq!(
            target("Read", json!({"file_path": r"C:\Users\me\secret\plan.md"})),
            Some("plan.md".into())
        );
        assert_eq!(
            target("Read", json!({"file_path": "/home/me/secret/plan.md"})),
            Some("plan.md".into())
        );
        assert_eq!(target("SomeFutureTool", json!({"file_path": "/x/y"})), None);
    }

    #[test]
    fn agents_send_the_subagent_type() {
        let input =
            json!({"subagent_type": "Explore", "prompt": "secret plan", "description": "d"});
        assert_eq!(target("Agent", input.clone()), Some("Explore".into()));
        assert_eq!(target("Task", input), Some("Explore".into()));
    }

    #[test]
    fn other_tools_and_bad_input_send_nothing() {
        assert_eq!(target("TodoWrite", json!({"todos": []})), None);
        assert_eq!(
            target("mcp__github__search_issues", json!({"query": "x"})),
            None
        );
        assert_eq!(target("Read", json!({})), None);
        assert_eq!(target("Read", json!({"file_path": 42})), None);
        assert_eq!(target("Read", json!("not an object")), None);
    }

    #[test]
    fn long_targets_are_cut_to_the_limit() {
        let long = "q".repeat(200);
        let cut = target("WebSearch", json!({ "query": long })).unwrap();
        assert_eq!(cut.chars().count(), limits::TARGET);
    }
}
