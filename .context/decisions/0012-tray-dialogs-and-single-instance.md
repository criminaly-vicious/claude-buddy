# 0012. Tray dialogs and single instance with plain Win32

- **Status:** Accepted
- **Date:** 2026-10-27
- **Issue:** #27

## Context
The tray (#27) needs two things Tauri core does not provide: a confirmation dialog before the hook installer changes `settings.json`, with a result message afterwards; and a single running instance, where a second launch brings the first one forward. Official plugins exist for both (`tauri-plugin-dialog`, `tauri-plugin-single-instance`). The app is Windows-first, already depends on `windows-sys`, and must stay light (docs/scope.md).

## Decision
- **Dialogs:** `MessageBoxW` (OK / Cancel, or OK), top-most, no owner, called from a worker thread so the tray's event loop never blocks. Off Windows the confirmation answers "no" and messages go to stderr, so nothing changes without a person agreeing. The summary shows the file path and line counts, never the settings' contents, which can hold secrets.
- **Single instance:** a named mutex `Local\io.github.criminaly-vicious.claude-buddy.instance` claimed before Tauri starts. A second launch finds it, signals the auto-reset event `...show`, and exits. The first instance waits on that event on a background thread and shows its buddies. `Local\` keeps it per Windows session. Off Windows there is no check (development only).

## Alternatives considered
- `tauri-plugin-dialog` / `tauri-plugin-single-instance`: more dependencies, capabilities and JS bindings for two small features; the single-instance plugin uses its own IPC. Worth revisiting if a macOS/Linux port happens.
- Detecting a running instance through `bridge.json` and `GET /v1/health`: no way to ask it to show itself without a new endpoint in the bridge protocol, which is meant for hooks only.
- A settings-style webview window for the preview: heavier; the settings window (#28) can host a full diff later.

## Consequences
- No new crates and no new `windows-sys` features: `MessageBoxW` and the mutex/event calls are in features already enabled.
- Dialogs look like Windows message boxes, not the app's own UI.
- A crash leaves no stale lock: Windows releases the mutex with the process.
