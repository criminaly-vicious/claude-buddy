# 0011. Overlay click-through via a low-level mouse hook

- **Status:** Accepted
- **Date:** 2026-10-27
- **Issue:** #17

## Context
The overlay is a transparent WebView2 window over the desktop. It must let every click through to the windows below, except on the buddies and the hover card, which must get hover, click and drag. WebView2 renders through DirectComposition, so transparent pixels still capture the mouse: per-pixel hit-testing by alpha does not work. A window that ignores the mouse (`WS_EX_TRANSPARENT`) also receives no mouse events, so the webview cannot tell when the cursor reaches a buddy.

## Decision
- The overlay is click-through by default (Tauri `set_ignore_cursor_events(true)`, i.e. `WS_EX_TRANSPARENT | WS_EX_LAYERED` set by tao).
- The frontend registers its interactive rectangles with `set_hit_regions` (CSS pixels, overlay-relative). Rust converts them to screen pixels for the current position and DPI.
- A **`WH_MOUSE_LL` hook** on a dedicated thread feeds every cursor move and left-button press/release to a pure `HitTester`. The overlay becomes interactive while the cursor is over a region, and stays interactive during a drag that started on one, until the button is released.
- Only a change of state toggles click-through. The toggle runs on a separate thread, so the hook never waits for the UI thread.
- No-focus and taskbar come from Tauri (`focusable: false` → `WS_EX_NOACTIVATE`, `skipTaskbar`). Alt+Tab exclusion is `WS_EX_TOOLWINDOW`, re-applied after every toggle because tao rewrites the extended style.
- Monitor, work-area and DPI changes re-anchor the window: `WM_DISPLAYCHANGE`, `WM_SETTINGCHANGE` and `WM_DPICHANGED` through a window subclass, plus Tauri's `ScaleFactorChanged`.

## Alternatives considered
- **Polling `GetCursorPos` on a timer:** simple, but wakes the CPU tens of times a second forever, against the idle budget.
- **`WM_NCHITTEST` returning `HTTRANSPARENT`:** only passes the click to windows of the same thread, not to other apps.
- **Layered window with per-pixel alpha (`UpdateLayeredWindow`):** would give free hit-testing, but WebView2 does not render that way; it would mean drawing the overlay without a webview (see ADR-0001).

## Consequences
- No polling: the hook runs only when the mouse moves, and does a few rectangle checks under a `try_lock`, then returns.
- Low-level hooks are system-wide. A slow callback would lag every mouse move on the machine, so the callback must stay trivial; it never blocks (`try_lock`, channel send) and never swallows input.
- Some security tools flag processes with low-level hooks; worth a line in the user guide (#32).
- The frontend must resend regions whenever buddies move, appear or leave. When the overlay moves, Rust recomputes the screen rectangles itself.
