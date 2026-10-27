# Session detection

How Claude Buddy learns which Claude Code sessions exist and what each one is doing. Decision record: [ADR-0002](../.context/decisions/0002-hooks-first-session-detection.md).

> **Verified** by spike [#9](https://github.com/criminaly-vicious/claude-buddy/issues/9) on 2026-10-27 against Claude Code **2.1.311 on Linux**, by real capture and the [official hooks reference](https://code.claude.com/docs/en/hooks). Items marked **⚠ Windows: not verified** come from the docs only and must be checked on Windows 10/11 before the code depends on them (see [Windows checklist](#windows-checklist)). Scrubbed samples live in [`tests/fixtures/`](../tests/fixtures/README.md). Parsers must tolerate unknown fields and unknown event names: both change between versions.

## Source 1: hooks (primary)

Claude Code runs user-configured hooks on lifecycle events, configured under `hooks` in `~/.claude/settings.json` (or `%CLAUDE_CONFIG_DIR%\settings.json` if that variable is set). Edits to the file are picked up by Claude Code's settings watcher, so new sessions see the hooks without a restart.

### Installer output

The installer writes **exec-form command hooks** (with `args`), which spawn `buddy-hook.exe` directly with no shell. That avoids every Git Bash / PowerShell / cmd quoting problem, and paths with spaces need no escaping. On Windows exec form requires a real `.exe` (not a `.cmd` shim), which `buddy-hook.exe` is.

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "C:\\Users\\<user>\\AppData\\Local\\Claude Buddy\\buddy-hook.exe",
            "args": ["PreToolUse"],
            "async": true,
            "timeout": 5
          }
        ]
      }
    ]
  }
}
```

- One entry per event in the table below. No `matcher`, so every tool and every notification type is matched.
- `async: true` runs the hook in the background, so it adds no latency to the session. Claude Code does not enforce `timeout` on async hooks, so `buddy-hook` keeps its own hard timeout.
- ⚠ Windows: not verified. Exec form, `async` and the settings watcher are documented for Windows but were tested on Linux only.

### Payload (stdin JSON)

Common fields, present on (almost) every event:

| Field | Notes |
| --- | --- |
| `session_id` | UUID. Key of the registry |
| `transcript_path` | Absolute path of the session's JSONL transcript |
| `cwd` | Session working directory. Use its last segment as the buddy's project name |
| `hook_event_name` | Event name |
| `prompt_id` | UUID of the current user prompt. Absent before the first prompt (not on `SessionStart`) |
| `permission_mode` | `default`, `plan`, `acceptEdits`, `auto`, `dontAsk`, `bypassPermissions`. Not on `SessionStart`, `SubagentStart`, `SessionEnd` |
| `effort` | `{ "level": "low" … "max" }`. Only on tool events, `Stop` and `SubagentStop` |
| `agent_id`, `agent_type` | Present when the event comes from inside a subagent. Same `session_id` as the parent |
| `scratchpad_dir` | Session scratch folder (v2.1.257+). Not used |

On Windows every path arrives with backslash separators, e.g. `C:\Users\me\proj\src\index.ts` (⚠ Windows: not verified; [sample](../tests/fixtures/hooks/from-docs/pre-tool-use-write-windows.json)). Normalise separators before comparing.

Per event (fields beyond the common ones):

| Hook event | Extra fields | Meaning for us | Sample |
| --- | --- | --- | --- |
| `SessionStart` | `source`: `startup`, `resume`, `clear`, `compact`, `fork`; sometimes `model` | Spawn a buddy (on `startup`/`resume`/`fork`); keep it on `clear`/`compact` | [captured](../tests/fixtures/hooks/captured/session-start.json) |
| `UserPromptSubmit` | `prompt` (full prompt text: **never forward**) | Turn started → `working` | [captured](../tests/fixtures/hooks/captured/user-prompt-submit.json) |
| `PreToolUse` | `tool_name`, `tool_input`, `tool_use_id` | Map tool → `working` / `researching`; summary from `tool_input` | [Read](../tests/fixtures/hooks/captured/pre-tool-use-read.json), [Bash](../tests/fixtures/hooks/captured/pre-tool-use-bash.json), [Agent](../tests/fixtures/hooks/captured/pre-tool-use-agent.json) |
| `PostToolUse` | `tool_name`, `tool_input`, `tool_response` (full result: **never forward**), `tool_use_id`, `duration_ms` | Keep state | [captured](../tests/fixtures/hooks/captured/post-tool-use-bash.json) |
| `PostToolUseFailure` | `tool_name`, `tool_input`, `tool_use_id`, `error`, `is_interrupt`, `duration_ms` | Keep state (optional: brief "oops" pose) | [docs](../tests/fixtures/hooks/from-docs/post-tool-use-failure-bash.json) |
| `Notification` | `message`, `title`, `notification_type` | `permission_prompt`, `idle_prompt`, `elicitation_dialog`, `elicitation_url_dialog`, `agent_needs_input` → `waiting`; other types ignored | [docs](../tests/fixtures/hooks/from-docs/notification-permission-prompt.json) |
| `PermissionRequest` | `tool_name`, `tool_input`, `permission_suggestions` (no `tool_use_id`) | Earliest "asks for permission" signal → `waiting`. Optional, `Notification` covers it | [docs](../tests/fixtures/hooks/from-docs/permission-request-bash.json) |
| `Stop` | `stop_hook_active`, `last_assistant_message` (**never forward**), `background_tasks`, `session_crons` | Turn finished → `idle` | [captured](../tests/fixtures/hooks/captured/stop.json) |
| `SubagentStart` | `agent_id`, `agent_type` | Post-MVP: helper buddy appears | [captured](../tests/fixtures/hooks/captured/subagent-start.json) |
| `SubagentStop` | as `Stop`, plus `agent_id`, `agent_type`, `agent_transcript_path` | Post-MVP: helper buddy leaves | [captured](../tests/fixtures/hooks/captured/subagent-stop.json) |
| `SessionEnd` | `reason`: `clear`, `resume`, `logout`, `prompt_input_exit`, `other` | `leaving`, then remove | [captured](../tests/fixtures/hooks/captured/session-end.json) |

Tool events fired inside a subagent carry `agent_id`/`agent_type` ([sample](../tests/fixtures/hooks/captured/pre-tool-use-read-in-subagent.json)). For the MVP they count toward the parent buddy's state.

Not captured: `Notification` and `PermissionRequest` only fire in interactive sessions, and the capture ran with `claude -p`. Their samples come from the docs.

### Environment of a command hook

Command hooks inherit Claude Code's environment, plus these documented variables:

| Variable | Use |
| --- | --- |
| `CLAUDE_PID` | PID of the Claude Code process. `buddy-hook` sends it with every event, used for liveness and window focus |
| `CLAUDE_CODE_SESSION_ID` | Same as `session_id` |
| `CLAUDE_PROJECT_DIR` | Project root |

HTTP hooks receive none of this. That is one reason they are not used (below).

### Rules for `buddy-hook`

- Must exit 0 fast (< 50 ms target, hard timeout 300 ms on the HTTP call) and print nothing, so it never affects the session. A non-zero exit or stdout output shows up as a hook error in the user's transcript.
- If the bridge is not running, drop the event silently.
- Sanitise before sending: only the tool name and a short target (file basename, first 60 chars of a search query or command name). Never forward `prompt`, `tool_response`, `last_assistant_message` or file contents.

### Why not the built-in `http` hook type

Claude Code has an `http` hook type that POSTs the payload as JSON to a URL, with headers that can interpolate allow-listed env vars ([sample request](../tests/fixtures/hooks/http/pre-tool-use-request.json)). It would remove `buddy-hook.exe`, but the spike found three blockers:

1. **Errors are visible when Buddy is not running.** A refused connection is a non-blocking hook error: Claude Code writes a `hook_non_blocking_error` attachment for every tool call, and the user sees "Stop hook error occurred" ([captured](../tests/fixtures/transcripts/http-hook-connection-refused.jsonl)). Users close tray apps; their sessions must stay clean.
2. **`SessionStart` does not support `http` hooks** (only `command` and `mcp_tool`), so a command hook is needed anyway.
3. **No PID.** HTTP payloads do not include `CLAUDE_PID`, so liveness would fall back to guesswork.

ADR-0002 stands: command hooks running `buddy-hook.exe`.

## Source 2: transcript watcher (fallback)

### Location

`<config>/projects/<encoded-cwd>/<session_id>.jsonl`, where `<config>` is `~/.claude` (Windows: `%USERPROFILE%\.claude`) unless `CLAUDE_CONFIG_DIR` is set.

- `<encoded-cwd>`: the working directory with every character that is not a letter or digit replaced by `-`. Observed on Linux: `/home/user/projects/demo` → `-home-user-projects-demo`, and `/tmp/a_b.c d-é` → `-tmp-a-b-c-d--` (`_`, `.`, space, `-` and non-ASCII letters each become one `-`). Expected on Windows: `C:\Users\me\proj` → `C--Users-me-proj` (⚠ Windows: not verified). `CLAUDE_CODE_PROJECT_DIR_NAME` can override the name.
- The encoding is lossy (`-` and `/` collide), so **never decode the folder name**. Read `cwd` from the transcript lines instead.
- Subagents write their own transcript at `<config>/projects/<encoded-cwd>/<session_id>/subagents/agent-<agent_id>.jsonl`, plus `agent-<agent_id>.meta.json` (`agentType`, `description`, `toolUseId`, `finished`). The watcher must not treat those as new sessions.

### Line types

Each line is one JSON object with a `type`. Most lines also carry `uuid`, `parentUuid`, `timestamp`, `sessionId`, `cwd`, `version`, `gitBranch`, `isSidechain` (true inside subagent transcripts). Sample: [`session.jsonl`](../tests/fixtures/transcripts/session.jsonl).

| `type` | What it is | Use |
| --- | --- | --- |
| `user` | `message.content` is a **string** for a typed prompt, or a list of `tool_result` blocks after a tool ran | String content → turn started → `working` |
| `assistant` | One line **per content block** (`thinking`, `tool_use`, `text`), several lines can share one `message.id`. `message.stop_reason` is `tool_use` or `end_turn` | `tool_use` block → tool name → mapping; `text` with `stop_reason: end_turn` → `idle` |
| `system` | Subtypes, e.g. `stop_hook_summary` written when a turn ends | `stop_hook_summary` → turn ended → `idle` |
| `attachment` | Context injected by Claude Code (environment, reminders, listings, `hook_non_blocking_error`, …) | Ignore |
| `queue-operation` | Prompt queued / dequeued (`content` = prompt text) | Ignore |
| `last-prompt`, `tools-used`, `cost-state`, `atis-latch` | Bookkeeping | Ignore |

Unknown `type` values must be ignored, not fail the parser.

**Privacy:** transcripts contain full prompts, file contents, tool results and model output. The watcher reads only `type`, `subtype`, `isSidechain`, `message.content[].type`, `message.content[].name`, `message.stop_reason`, `cwd` and `sessionId`. For a `tool_use` block it also passes `message.content[].input` to `bridge_protocol::target::derive`, the same [target rules](architecture.md#target-rules) `buddy-hook` uses, and keeps only the resulting short target. It never stores or forwards any other text.

### Inference

Implemented in `apps/desktop/src-tauri/src/watcher/` ([ADR-0010](../.context/decisions/0010-notify-transcript-watcher.md)). When hooks are not installed (or for sessions started before the app), the watcher:

1. At startup, reads transcripts modified in the last **30 minutes** (the registry's transcript TTL), skipping `subagents/`.
2. Watches the projects folder with `notify` and, 150 ms after a burst of changes, reads only the **new complete lines** of each changed file. It stores an offset per file; a file that shrank is read again from the start. A file seen for the first time is read from its last 64 KiB only, so a long session does not replay hours of history.
3. Maps each relevant line to a registry event with `source: transcript`:

| Line | Registry event |
| --- | --- |
| `user` with string content (typed prompt) | `PromptSubmitted` |
| `assistant` with a `tool_use` block | `ToolStarted` (tool name + derived target) |
| `user` with `tool_result` blocks | `ToolFinished` (last tool of that file) |
| `assistant` `text` with `stop_reason: end_turn`, or `system`/`stop_hook_summary` | `TurnEnded` |
| Sidechain (`isSidechain: true`), bookkeeping, attachments, unknown types, invalid JSON | Ignored |

`cwd` and the session ID come from the lines themselves, never from the folder name.

Not implemented yet: inferring `waiting` from silence while a tool is pending. Without hooks, a permission prompt shows as the tool's state until the user answers.

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
- `waiting` and `leaving` skip the minimum display time: the user must see them at once. A state queued behind the minimum is replaced by any newer one, so only the latest is shown.
- Implemented in `apps/desktop/src-tauri/src/registry/` as pure logic: the caller passes the time in, `Registry::apply` takes a normalised event, `Registry::tick` runs the timers, and `Registry::next_deadline` says when the next tick is due, so nothing polls.

## Liveness

- `buddy-hook` sends `CLAUDE_PID` (set by Claude Code in every command hook's environment) with each event as `claude_pid`; the registry stores it per session. No parent-chain walking is needed to find the Claude process.
- The bridge driver starts watching a `(session, pid)` pair the first time it sees it (`apps/desktop/src-tauri/src/liveness/`):
  - **Windows:** opens the process with `SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION` and blocks a small thread in `WaitForSingleObject`. Nothing polls, and because the handle keeps the process object alive, a reused PID cannot be mistaken for the old process. If the process is already gone, the exit is reported at once. If it cannot be opened (e.g. access denied), the session only leaves on `SessionEnd`.
  - **Linux (development only):** checks `/proc/<pid>/stat` every 5 s and compares the process start time, so a reused PID counts as an exit. Other OSes are not watched.
- When the process exits, the registry waits a **15 s grace period**, then the buddy leaves. A hook event from the same session with a different PID (e.g. `claude --resume`) cancels the exit. This covers terminals closed without `SessionEnd`.
- Transcript-only sessions expire after **30 min** without writes.
- Both expiries are registry deadlines (`Registry::next_deadline`), so the driver sleeps until they are due: no extra thread or timer.
- ⚠ Windows: not verified. Which process `CLAUDE_PID` points to under the native installer vs npm (`claude.exe` or `node.exe`), and whether it is the process that lives as long as the session.

## Window focus (click on a buddy)

1. From `CLAUDE_PID`, walk up the parent chain to the first process that owns a visible top-level window (`WindowsTerminal.exe`, `conhost.exe`, `Code.exe`, ...).
2. `AllowSetForegroundWindow` + `SetForegroundWindow` on that window (restore if minimised).
3. Windows Terminal hosts many tabs in one window: MVP focuses the window only. Tab-level focus is a later spike (UI Automation).

## Windows checklist

Not verifiable from the Linux capture. Check on Windows 10/11 before implementing the parts that depend on it, and update this doc:

- [ ] Capture payloads with a temporary exec-form hook: confirm backslash paths in `cwd`, `transcript_path`, `tool_input.file_path`.
- [ ] Transcript folder name for `C:\Users\<user>\<project>` (expected `C--Users-<user>-<project>`), including a path with spaces and non-ASCII characters.
- [ ] Exec-form `buddy-hook.exe` with a path containing spaces runs with and without Git for Windows installed.
- [ ] `async: true` hooks run and never show a hook error, including when `buddy-hook.exe` exits early.
- [ ] `CLAUDE_PID` is set and points to the long-lived Claude process. Record the process tree under Windows Terminal, PowerShell, cmd and the VS Code terminal.
- [ ] `Notification` with `permission_prompt` and `idle_prompt` fires in an interactive session.
- [ ] Settings watcher picks up installer changes in a running session.
