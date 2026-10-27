//! The single tool → state table (docs/session-detection.md#tool--state-mapping).

use super::BuddyState;

const RESEARCH_TOOLS: &[&str] = &["WebSearch", "WebFetch", "Read", "Grep", "Glob", "LS"];
const WORK_TOOLS: &[&str] = &[
    "Edit",
    "MultiEdit",
    "Write",
    "NotebookEdit",
    "Bash",
    "PowerShell",
    "Task",
    "Agent",
];
const RESEARCH_MCP_WORDS: &[&str] = &["search", "fetch", "read", "get", "list"];

/// State a tool puts its buddy in, or `None` to keep the current state.
pub fn state_for_tool(tool: &str) -> Option<BuddyState> {
    if RESEARCH_TOOLS.contains(&tool) {
        return Some(BuddyState::Researching);
    }
    if WORK_TOOLS.contains(&tool) {
        return Some(BuddyState::Working);
    }
    // MCP tools are named `mcp__<server>__<tool>`; classify by the tool part.
    if let Some(rest) = tool.strip_prefix("mcp__") {
        let name = rest
            .rsplit("__")
            .next()
            .unwrap_or(rest)
            .to_ascii_lowercase();
        let researching = RESEARCH_MCP_WORDS.iter().any(|w| name.contains(w));
        return Some(if researching {
            BuddyState::Researching
        } else {
            BuddyState::Working
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_research_tools() {
        for tool in RESEARCH_TOOLS {
            assert_eq!(
                state_for_tool(tool),
                Some(BuddyState::Researching),
                "{tool}"
            );
        }
    }

    #[test]
    fn built_in_work_tools() {
        for tool in WORK_TOOLS {
            assert_eq!(state_for_tool(tool), Some(BuddyState::Working), "{tool}");
        }
    }

    #[test]
    fn mcp_tools_by_name() {
        assert_eq!(
            state_for_tool("mcp__github__search_issues"),
            Some(BuddyState::Researching)
        );
        assert_eq!(
            state_for_tool("mcp__notion__notion-fetch"),
            Some(BuddyState::Researching)
        );
        assert_eq!(
            state_for_tool("mcp__github__create_pull_request"),
            Some(BuddyState::Working)
        );
    }

    #[test]
    fn mcp_server_name_does_not_count() {
        // "reader" server, but the tool itself writes.
        assert_eq!(
            state_for_tool("mcp__reader__write_note"),
            Some(BuddyState::Working)
        );
    }

    #[test]
    fn todo_and_unknown_tools_keep_state() {
        assert_eq!(state_for_tool("TodoWrite"), None);
        assert_eq!(state_for_tool("SomethingNew"), None);
    }
}
