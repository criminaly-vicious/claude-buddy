# Architecture

## Overview

```
 Claude Code session(s)                       Claude Buddy (one Tauri 2 process)
 ┌──────────────────────┐   hook event (JSON)  ┌───────────────────────────────────────────┐
 │ claude (CLI)         │ ───────────────────► │ Bridge: HTTP server 127.0.0.1:<port>      │
 │  hooks in            │  buddy-hook.exe      │   + token auth                            │
 │  ~/.claude/settings  │  POST /v1/events     │                    │                      │
 └──────────────────────┘                      │                    ▼                      │
 ┌──────────────────────┐   file changes       │ Session Registry (Rust)                   │
 │ ~/.claude/projects/  │ ───────────────────► │   session_id → SessionState               │
 │   <proj>/<id>.jsonl  │  Transcript Watcher  │   state machine + timers (idle, sleep)    │
 └──────────────────────┘  (fallback)          │   process liveness check (sysinfo)        │
                                               │                    │ Tauri event          │
                                               │                    ▼ "sessions:update"    │
                                               │ Overlay window (WebView2, transparent)    │
                                               │   TypeScript + Canvas 2D                  │
                                               │   buddies, animation, physics, hover card │
                                               │                    │ invoke("focus")      │
                                               │                    ▼                      │
                                               │ Win Integration (Rust, windows-rs)        │
                                               │   window focus, click-through, autostart  │
                                               │ Tray + Settings window                    │
                                               └───────────────────────────────────────────┘
```

## Components

| Component | Language | Responsibility |
| --- | --- | --- |
| `buddy-hook` | Rust (tiny CLI, < 1 MB) | Invoked by Claude Code hooks. Reads hook JSON from stdin, sanitises it, adds `CLAUDE_PID`, POSTs it to the bridge ([protocol](#bridge-protocol)). Exits 0 in < 50 ms and **never** blocks or fails the Claude session (fire-and-forget, short timeout). |
| Bridge | Rust (`tiny_http`, [ADR-0009](../.context/decisions/0009-tiny-http-bridge-server.md)) | Local HTTP server bound to `127.0.0.1` on a random free port. Port + token written to `bridge.json` ([protocol](#bridge-protocol)). Validates events and normalises them into registry events. |
| Transcript Watcher | Rust (`notify`) | Watches `~/.claude/projects/**.jsonl`. Tails new lines, infers state when hooks are missing. Debounced. |
| Session Registry | Rust | Single source of truth. Merges both sources, runs the [state machine](session-detection.md#state-machine), expires dead sessions, emits diffs to the UI. |
| Hook Installer | Rust | Adds/removes our hook entries in `~/.claude/settings.json` with backup and key-preserving merge. |
| Overlay | TypeScript, Canvas 2D | Transparent, frameless, always-on-top, click-through window covering the chosen corner. Draws buddies from sprite sheets, runs physics, shows hover card. |
| Win Integration | Rust (`windows-sys`), `src-tauri/src/win/` | Overlay window: corner anchoring on the chosen monitor's work area, DPI and display changes, click-through except over regions registered with `set_hit_regions` ([ADR-0011](../.context/decisions/0011-overlay-click-through.md)), expand to the work area for throws, focus the terminal window owning a session PID. Later: autostart, global hotkey. |
| Tray & Settings | Tauri tray (`src-tauri/src/tray/`) + settings window (`src-tauri/src/settings.rs`, `src/settings/`) | Show/hide buddies (hidden: window hidden, nothing rendered), install/repair and uninstall hooks with a confirmation and a bridge health check, settings ([below](#app-config)), quit. One instance per Windows session; a second launch shows the first one's buddies ([ADR-0012](../.context/decisions/0012-tray-dialogs-and-single-instance.md)). |

## Data model

```ts
type BuddyState =
  | "spawning" | "idle" | "working" | "researching"
  | "waiting" | "sleeping" | "leaving";

interface SessionView {
  id: string;              // Claude Code session_id
  project: string;         // basename of cwd
  cwd: string;
  state: BuddyState;
  tool?: string;           // current or last tool name, e.g. "Edit"
  summary?: string;        // short, sanitised, max 80 chars ("Editing src/app.ts")
  since: number;           // epoch ms when state started
  source: "hooks" | "transcript";
  pid?: number;            // claude process, for focus + liveness
}
```

UI receives the full list on start (`sessions:snapshot`) then diffs (`sessions:update`, `sessions:remove`).

## Bridge protocol

Contract between `buddy-hook` (client, #11) and the bridge (server, #12). Protocol version **1**. Types, limits and path resolution are implemented once in `crates/bridge-protocol`, which both sides depend on. Decision record: [ADR-0003](../.context/decisions/0003-loopback-http-bridge.md).

### App config: `config.json`

The app's own settings live next to `bridge.json` (same folder rules as below), in `config.json`:

```json
{
  "version": 1,
  "corner": "bottom-right",
  "monitor": null,
  "scale": 2,
  "idleSleepMinutes": 5,
  "reduceMotion": false,
  "startWithWindows": false,
  "hotkey": null
}
```

- `monitor`: index into the monitor list, `null` for the primary monitor. `scale`: 1, 2 or 3 CSS px per sprite pixel. `idleSleepMinutes`: 1–120.
- `startWithWindows` and `hotkey` are stored only; autostart and the hotkey come with #29.
- Read leniently (`src-tauri/src/config.rs`): a bad or unknown field falls back to its default without losing the others, out-of-range values are clamped, a newer `version` is read field by field. A file that is not JSON is renamed to `config.json.invalid` and the defaults are used. Writes go through a temporary file and a rename.
- **Applied live** from the settings window (`set_config` saves, applies and emits `config:changed`): corner and monitor re-anchor the overlay, the idle timeout reaches the registry (the driver is woken to recompute its deadline), scale and reduce motion are picked up by the overlay page. At startup the saved values are applied before the first frame.
- The settings window also shows the hook status (`hook_status`): installed or not, the settings file, and when the last hook event arrived since start. It refreshes when the window gets focus or on "Refresh", never on a timer.

### Discovery file: `bridge.json`

| OS | Path |
| --- | --- |
| Windows | `%APPDATA%\claude-buddy\bridge.json` (`%APPDATA%` is the roaming profile folder, e.g. `C:\Users\<user>\AppData\Roaming`) |
| Linux / macOS (development only) | `$XDG_CONFIG_HOME/claude-buddy/bridge.json`, or `~/.config/claude-buddy/bridge.json` when `XDG_CONFIG_HOME` is unset |

- The path is fixed and computed from environment variables only, so `buddy-hook` finds it without Tauri. It is deliberately **not** the Tauri app data dir (which follows the app identifier).
- `CLAUDE_BUDDY_BRIDGE_FILE`, if set, overrides the path on both sides. It is meant for tests and should not be set by users.

```json
{
  "protocol": 1,
  "port": 51234,
  "token": "9f2c…64 hex chars…",
  "pid": 7312,
  "started_at": 1793072400000
}
```

| Field | Type | Meaning |
| --- | --- | --- |
| `protocol` | integer | Protocol version. `buddy-hook` sends nothing if it is not a version it supports |
| `port` | integer | Port on `127.0.0.1` |
| `token` | string | 64 lowercase hex chars (32 random bytes from the OS CSPRNG), new on every app start |
| `pid` | integer | PID of the Claude Buddy process. Lets a client tell a stale file from a live app |
| `started_at` | integer | Epoch milliseconds when the app wrote the file |

Lifecycle:
- The app writes the file at startup, after the server is listening. It writes to a temporary file in the same folder, then renames it into place, so a reader never sees a half-written file.
- The file is readable by the current user only. On Windows its ACL grants access to the current user's SID only, with no inherited entries. On Unix the mode is `0600`.
- The app deletes the file on a clean exit. After a crash a stale file can remain. Clients need no special handling: the connection is refused, or the token is rejected by the next app instance, and the event is dropped.
- A missing, unreadable or invalid file means "Buddy is not running": `buddy-hook` drops the event.

### Endpoint: `POST /v1/events`

```
POST /v1/events HTTP/1.1
Host: 127.0.0.1:51234
Authorization: Bearer <token>
Content-Type: application/json
```

Body: one event as a JSON object. Field names use the hook payload's own `snake_case`. Unknown fields are ignored, so new fields can be added without a version bump.

```json
{
  "v": 1,
  "event": "PreToolUse",
  "session_id": "11111111-1111-4111-8111-111111111111",
  "cwd": "C:\\Users\\me\\projects\\demo",
  "at": 1793072412345,
  "claude_pid": 9184,
  "tool_name": "Read",
  "target": "index.ts"
}
```

| Field | Type | Required | Limit | Content |
| --- | --- | --- | --- | --- |
| `v` | integer | yes | | Protocol version, `1` |
| `event` | string | yes | 32 chars | Hook event name exactly as Claude Code sends it in `hook_event_name` |
| `session_id` | string | yes | 64 chars | Hook `session_id` |
| `cwd` | string | yes | 1024 chars | Hook `cwd`, unchanged (Windows separators kept) |
| `at` | integer | yes | | Epoch milliseconds when `buddy-hook` started |
| `claude_pid` | integer | no | | `CLAUDE_PID` from the hook environment, if set and numeric |
| `tool_name` | string | tool events, `PermissionRequest` | 128 chars | Hook `tool_name` |
| `target` | string | no | 60 chars | Sanitised short target, see below |
| `notification_type` | string | `Notification` | 32 chars | Hook `notification_type` |
| `start_source` | string | `SessionStart` | 16 chars | Hook `source` (`startup`, `resume`, `clear`, `compact`, `fork`) |
| `agent_id` | string | no | 64 chars | Hook `agent_id` when the event comes from a subagent |

The whole body must be at most **16 KiB**. Strings are counted in Unicode scalar values. A client cuts any string over its limit before sending. The server rejects a request whose field exceeds its limit.

**Never sent:** `prompt`, `tool_input` (except the `target` derived from it), `tool_response`, `last_assistant_message`, `message`, `title`, `transcript_path`, file contents, full commands, environment variables.

#### `target` rules

`buddy-hook` derives `target` from `tool_input` before sending, then cuts it to 60 chars. The rules are implemented once in `bridge_protocol::target::derive`, which the transcript watcher also uses. Any tool not listed sends no `target`. When in doubt, send no target: an empty hover line is fine, a leaked secret is not.

| Tool | `target` |
| --- | --- |
| `Read`, `Edit`, `MultiEdit`, `Write`, `NotebookEdit` | File name only (last segment of `file_path` / `notebook_path`, either separator) |
| `Grep`, `Glob` | `pattern` |
| `WebSearch` | `query` |
| `WebFetch` | Host of `url` only (no path, no query string) |
| `Bash`, `PowerShell` | Program name only. Split `command` on whitespace and skip leading `NAME=value` tokens (shell env assignments). If the first remaining token contains `=`, `$` or a quote, or starts with `$env:`, send no `target`. Otherwise send its last path segment (`git`, `cargo`) |
| `Agent`, `Task` | `subagent_type` |

#### Responses

`buddy-hook` ignores the response body and status, and never retries. Codes exist for tests and debugging.

| Status | When |
| --- | --- |
| `204 No Content` | Event accepted (including recognised events the MVP ignores, e.g. `SubagentStart`) |
| `202 Accepted` | Valid body with an `event` name the server does not know. Ignored, for forward compatibility |
| `400 Bad Request` | Invalid JSON, missing required field, wrong type, or a field over its limit |
| `401 Unauthorized` | Missing or wrong bearer token (compared in constant time) |
| `404 Not Found` / `405 Method Not Allowed` | Other paths or methods |
| `413 Payload Too Large` | Body over 16 KiB (checked from `Content-Length` and while reading) |
| `415 Unsupported Media Type` | `Content-Type` is not `application/json` |

`GET /v1/health` (same token) returns `200` with `{"protocol":1}`. The hook installer's "Test" button uses it.

#### Client timing

`buddy-hook` sends with a 300 ms total timeout (connect + write + read), does not wait for the body, and exits 0 silently in every case (see [rules](session-detection.md#rules-for-buddy-hook)).

### Server: event → registry

The bridge stamps each event with its own receive time and applies events in arrival order. Async hooks can arrive slightly out of order; the registry's minimum display time absorbs this.

| `event` | Registry event |
| --- | --- |
| `SessionStart` | `SessionStarted` |
| `UserPromptSubmit` | `PromptSubmitted` |
| `PreToolUse` | `ToolStarted { tool: tool_name, summary }`, where `summary` is `"<tool_name> · <target>"` or just `tool_name` |
| `PostToolUse`, `PostToolUseFailure` | `ToolFinished { tool: tool_name }` |
| `Notification` with `notification_type` in `permission_prompt`, `idle_prompt`, `elicitation_dialog`, `elicitation_url_dialog`, `agent_needs_input` | `NeedsInput` |
| `Notification` with another type | Ignored (`204`) |
| `PermissionRequest` | `NeedsInput` |
| `Stop` | `TurnEnded` |
| `SubagentStart`, `SubagentStop` | Ignored for the MVP (`204`) |
| `SessionEnd` | `SessionEnded` |

`claude_pid` becomes the registry's `pid`; `cwd` is passed through. Events inside a subagent (`agent_id` set) count toward the parent session.

## Tech stack

| Concern | Choice | Why (see ADRs) |
| --- | --- | --- |
| Shell | Tauri 2 | Small binary, low RAM, native tray, transparent windows ([ADR-0001](../.context/decisions/0001-tauri-desktop-shell.md)) |
| Core | Rust stable | Hooks binary, file watching, Win32 access |
| UI | TypeScript + Vite, Canvas 2D, no framework in overlay | Full control of render loop, minimal overhead ([ADR-0004](../.context/decisions/0004-pixel-art-canvas-rendering.md)) |
| Settings UI | Plain TS, a second Vite page (`settings.html`) | Tiny; the window exists only while open |
| Package manager | pnpm | |
| Tests | `cargo test`, Vitest, Playwright for overlay snapshots | |
| CI | GitHub Actions on `windows-latest` | |
| Installer | Tauri bundler (NSIS) | Per-user install, no admin |

## Repository layout (planned)

```
apps/desktop/            Tauri app
  src/                   overlay + settings (TS)
    overlay/             render loop, buddy, physics, hover card
    sprites/             sprite sheet loader + atlas json
  src-tauri/             Rust: bridge, registry, watcher, installer, win
crates/buddy-hook/       hook CLI
crates/bridge-protocol/  wire contract shared by buddy-hook and the bridge
assets/sprites/          source .aseprite files + exported png/json
docs/                    human docs
.context/                agent briefs + ADRs
```

## Performance strategy

- Render loop runs only while something animates; otherwise the canvas sleeps. Idle animations are scheduled, not polled. The scene reports when the next sprite frame changes, and the loop (`apps/desktop/src/overlay/loop.ts`) sleeps on a timer until then: with five buddies it redraws about 16 times a second, not 30, and not at all with none.
- Cap at 30 fps; pixel art does not need more.
- Overlay window sized to the buddy area, not full screen, except while a buddy is dragged or thrown (expanded to the work area until it is back in its slot).
- Click-through follows the cursor through a low-level mouse hook that runs only when the mouse moves (ADR-0011), not a timer.
- Pause rendering when the overlay is hidden or the session list is empty.
- File watcher is event-based with debounce; process liveness checked every 5 s.

## Security

- Bridge listens on loopback only and requires the token from `bridge.json` (file readable only by the user).
- `buddy-hook` sends only the fields in the [bridge protocol](#bridge-protocol): event name, session_id, cwd, tool_name, a sanitised short target, a timestamp and the Claude PID. Never full tool input/output or prompts.
- No network access beyond loopback. No telemetry.
