# Session detection

How Claude Buddy learns which Claude Code sessions exist and what each one is doing. Decision record: [ADR-0002](../.context/decisions/0002-hooks-first-session-detection.md).

> Formats below reflect Claude Code as of late 2026. They must be re-verified by the spike issue "Verify Claude Code hook and transcript formats on Windows" before implementation, and the parser must tolerate unknown fields.

## Source 1: hooks (primary)

Claude Code runs user-configured commands on lifecycle events, configured in `~/.claude/settings.json`. Each hook receives a JSON payload on stdin that includes at least `session_id`, `transcript_path`, `cwd` and `hook_event_name`; tool events also include `tool_name` and `tool_input`.

The installer adds entries that run `buddy-hook.exe <event>` for:

| Hook event | Meaning for us |
| --- | --- |
| `SessionStart` | Spawn a buddy |
| `UserPromptSubmit` | Turn started → `working` (until a tool says otherwise) |
| `PreToolUse` | Tool starting → map tool to `working` or `researching` |
| `PostToolUse` | Tool finished → keep state, update summary |
| `Notification` | Claude needs the user (permission, question, idle prompt) → `waiting` |
| `Stop` | Turn finished → `idle` |
| `SubagentStop` | Subagent finished (post-MVP: helper buddies) |
| `SessionEnd` | `leaving`, then remove |

Rules for `buddy-hook`:
- Must exit 0 fast (< 50 ms target, hard timeout 300 ms on the HTTP call) and print nothing, so it never affects the session.
- If the bridge is not running, drop the event silently.
- Sanitise before sending: only the tool name and a short target (file basename, first 60 chars of a search query or command name).

## Source 2: transcript watcher (fallback)

Each session writes a JSONL transcript under `~/.claude/projects/<encoded-cwd>/<session_id>.jsonl`. When hooks are not installed (or for sessions started before the app), the watcher:

1. Lists transcripts modified in the last 10 minutes at startup.
2. Watches the folder for changes and tails appended lines.
3. Infers state from the last entries: an assistant `tool_use` block gives the tool; a final assistant text with no pending tool → `idle`; no writes for N seconds while a tool is pending → probably `waiting`.

Fallback states are lower confidence; the hover card shows a small "approximate" hint and suggests installing hooks.

## Tool → state mapping

| Tools | State |
| --- | --- |
| `WebSearch`, `WebFetch`, `Read`, `Grep`, `Glob`, `LS`, MCP tools whose names contain `search`, `fetch`, `read`, `get`, `list` | `researching` |
| `Edit`, `MultiEdit`, `Write`, `NotebookEdit`, `Bash`, `PowerShell`, `Task`/`Agent`, other MCP tools | `working` |
| `TodoWrite` and unknown tools | keep current state |

The mapping lives in one table in the registry so it is easy to tune.

## State machine

```
           SessionStart / first transcript line
                       │
                       ▼
                  ┌─────────┐
                  │spawning │ (plays once, ~1 s)
                  └────┬────┘
                       ▼
   ┌──────────────► ┌──────┐ ◄──────────── Stop
   │                │ idle │ ───── no events for idle_timeout (5 min) ──► sleeping
   │                └──┬───┘                                               │
   │  UserPromptSubmit │ PreToolUse                     any event ◄────────┘
   │                   ▼
   │     ┌─────────┐ ◄──► ┌─────────────┐
   │     │ working │      │ researching │   (by tool mapping)
   │     └────┬────┘      └──────┬──────┘
   │          └──── Notification ┴─────► ┌─────────┐
   │                                     │ waiting │ ── PreToolUse / UserPromptSubmit ──► working
   └──────────────── Stop ◄──────────────┴─────────┘

   any state ── SessionEnd or process gone for 15 s ──► leaving ──► removed
```

- Minimum display time per state: 600 ms, so quick tool flips do not cause jitter.
- `waiting` has priority: it is only left on a new tool/prompt event or `Stop`.

## Liveness

- `buddy-hook` reports its parent process chain; the registry stores the `claude` (node) PID.
- Every 5 s the registry checks the PID is alive (`sysinfo`). If gone for 15 s, the session leaves.
- Transcript-only sessions expire after 30 min without writes.

## Window focus (click on a buddy)

1. From the session PID, walk up the parent chain to the first process that owns a visible top-level window (`WindowsTerminal.exe`, `conhost.exe`, `Code.exe`, ...).
2. `AllowSetForegroundWindow` + `SetForegroundWindow` on that window (restore if minimised).
3. Windows Terminal hosts many tabs in one window: MVP focuses the window only. Tab-level focus is a later spike (UI Automation).
