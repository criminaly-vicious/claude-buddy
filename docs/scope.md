# Scope

## MVP (v0.1.0) — in scope

**Platform**
- Windows 10 (21H2+) and Windows 11, x64.
- Claude Code **CLI** sessions only (any terminal). The Claude desktop app is out of scope.

**Background app**
- Tray icon with menu: Show/Hide buddies, Install/Repair hooks, Settings, Quit.
- Starts with Windows (opt-in).
- Global hotkey to toggle the overlay (default `Ctrl+Alt+B`).

**Session discovery**
- Detect running sessions through Claude Code hooks (primary) and transcript files (fallback). See [session-detection.md](session-detection.md).
- One buddy per live session, up to 10 visible (extra ones collapse into a `+N` badge).
- One-click hook installer that edits `~/.claude/settings.json` safely (backup, merge, uninstall).

**Buddies**
- States: `spawning`, `idle`, `working`, `researching`, `waiting`, `sleeping`, `leaving`. See [animation-spec.md](animation-spec.md).
- Grouped in a configurable screen corner, on the chosen monitor.
- Hover card: project folder name, state, current/last tool, short last action, elapsed time.
- Click: focus the terminal window that owns the session (best effort).
- Drag and throw with simple physics (gravity, bounce, floor/walls = screen edges), then walk back to the group.

**Settings**
- Corner, monitor, buddy scale (1x/2x/3x), start with Windows, hotkey, idle-to-sleep timeout.

## Later (post-MVP)

- Buddy names/colours per project, accessories (hats).
- Sounds (opt-in).
- Desktop notifications when a session waits for input.
- Inter-buddy interactions (high-fives when two finish together, chatting).
- macOS and Linux.
- Claude desktop app sessions, if a stable signal exists.
- Showing subagents as tiny helper buddies.

## Non-goals

- Controlling Claude Code (sending prompts, approving permissions). We only observe and focus.
- Reading or displaying full prompts, code or tool outputs.
- Cloud features, accounts, telemetry.
- Being a full session manager or log viewer.

## Success criteria for v0.1.0

| Criterion | Target |
| --- | --- |
| State change visible after a hook event | < 300 ms |
| Session detected after it starts (hooks installed) | < 2 s |
| Session detected via fallback only | < 5 s |
| Idle CPU (5 buddies, nothing changing) | < 1% |
| RAM | < 80 MB |
| Installer size | < 15 MB |
| Crash-free day of use with 5 sessions | yes |

## Assumptions and risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Claude Code hook/transcript formats change | Wrong states | Versioned parser, tolerant to unknown fields, fallback to "unknown" pose; spike issue tracks format |
| Focusing a specific Windows Terminal tab is not possible via Win32 | Click brings the window but not the tab | Accept window-level focus for MVP; document; investigate UI Automation later |
| Transparent always-on-top WebView2 window costs CPU | Breaks "light" | Render only on change, cap at 30 fps, pause when hidden; perf budget in CI later |
| Editing the user's `settings.json` | Breaks their config | Backup, JSON merge that preserves unknown keys, dry-run preview, one-click uninstall |
| Character resembles official branding | Legal | Original art, no logos, disclaimer (ADR-0005) |
