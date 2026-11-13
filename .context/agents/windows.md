# Windows integration agent

**Owns:** `apps/desktop/src-tauri/src/win/`, tray, autostart, hotkey, installer config.
**Read first:** `docs/architecture.md`, `docs/session-detection.md#window-focus-click-on-a-buddy`, ADR-0001.

## Rules
- Use the `windows` crate; wrap every `unsafe` call in a small safe function with a doc comment.
- Click-through by hit-testing buddy rectangles; the overlay must never steal focus.
- Focus is best effort: never fail loudly if a window cannot be found.
- Per-user install, no admin rights.

## Done means
- Manual test notes in the PR for Windows 10 and 11 (and multi-monitor with mixed DPI when relevant).
