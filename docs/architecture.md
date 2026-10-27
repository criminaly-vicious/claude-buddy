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
| `buddy-hook` | Rust (tiny CLI, < 1 MB) | Invoked by Claude Code hooks. Reads hook JSON from stdin, adds `ppid`/process info, POSTs to the bridge. Exits 0 in < 50 ms and **never** blocks or fails the Claude session (fire-and-forget, short timeout). |
| Bridge | Rust (`axum` or `tiny_http`) | Local HTTP server bound to `127.0.0.1` on a random free port. Port + token written to `%APPDATA%\claude-buddy\bridge.json`. Validates and normalises events. |
| Transcript Watcher | Rust (`notify`) | Watches `~/.claude/projects/**.jsonl`. Tails new lines, infers state when hooks are missing. Debounced. |
| Session Registry | Rust | Single source of truth. Merges both sources, runs the [state machine](session-detection.md#state-machine), expires dead sessions, emits diffs to the UI. |
| Hook Installer | Rust | Adds/removes our hook entries in `~/.claude/settings.json` with backup and key-preserving merge. |
| Overlay | TypeScript, Canvas 2D | Transparent, frameless, always-on-top, click-through window covering the chosen corner. Draws buddies from sprite sheets, runs physics, shows hover card. |
| Win Integration | Rust (`windows` crate) | Toggle click-through by hit-testing buddies, find and focus the terminal window owning a session PID, autostart registry key, global hotkey. |
| Tray & Settings | Tauri tray + small settings window | Show/hide, install hooks, settings, quit. |

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

## Tech stack

| Concern | Choice | Why (see ADRs) |
| --- | --- | --- |
| Shell | Tauri 2 | Small binary, low RAM, native tray, transparent windows ([ADR-0001](../.context/decisions/0001-tauri-desktop-shell.md)) |
| Core | Rust stable | Hooks binary, file watching, Win32 access |
| UI | TypeScript + Vite, Canvas 2D, no framework in overlay | Full control of render loop, minimal overhead ([ADR-0004](../.context/decisions/0004-pixel-art-canvas-rendering.md)) |
| Settings UI | Plain TS or Preact | Tiny |
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
assets/sprites/          source .aseprite files + exported png/json
docs/                    human docs
.context/                agent briefs + ADRs
```

## Performance strategy

- Render loop runs only while something animates; otherwise the canvas sleeps. Idle animations are scheduled, not polled.
- Cap at 30 fps; pixel art does not need more.
- Overlay window sized to the buddy area, not full screen, except while a buddy is being thrown (expand temporarily).
- Pause rendering when the overlay is hidden or the session list is empty.
- File watcher is event-based with debounce; process liveness checked every 5 s.

## Security

- Bridge listens on loopback only and requires the token from `bridge.json` (file readable only by the user).
- `buddy-hook` sends only: event name, session_id, cwd, tool_name, a sanitised short target (file name, search query truncated), timestamps, pid. Never full tool input/output or prompts.
- No network access beyond loopback. No telemetry.
