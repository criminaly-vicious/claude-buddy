# Roadmap

Each milestone is a GitHub milestone with one `type:epic` issue that links its children. Full list: [backlog.md](backlog.md).

| Milestone | Goal | Exit criteria | Version |
| --- | --- | --- | --- |
| **M0 Foundation** | Repo builds on Windows CI; the two big unknowns are answered | Scaffold runs `pnpm tauri dev`; CI green; spike reports for hook formats and transparent-window cost merged into docs | v0.0.1 |
| **M1 Session bridge** | Core knows every live session and its state | With hooks installed, 5 parallel sessions show correct states in a debug list; fallback works without hooks | v0.0.2 |
| **M2 Overlay & buddies** | Animated buddies in the corner reflect real state | All spec animations play; transitions correct; overlay click-through except on buddies | v0.0.3 |
| **M3 Interactions** | Hover, click-to-focus, throw | Hover card shows summary; click focuses terminal window; throw/bounce/walk-back works on multi-monitor | v0.0.4 |
| **M4 Background app & release** | Daily-driver quality | Tray, settings, autostart, hotkey; perf budget met; signed-off installer published | **v0.1.0** |

## Order and parallelism

```
M0 ──► M1 (bridge) ─────────────┐
   └─► M2 (art + overlay) ──────┼─► M3 ──► M4
       (can mock sessions)      │
```

M1 and M2 run in parallel: the overlay develops against a mock session feed (`pnpm dev:mock`) until the bridge lands.

## After v0.1.0 (ideas, not committed)

Per-project colours and names, desktop notification when a buddy waits, opt-in sounds, helper buddies for subagents, buddy-to-buddy interactions, macOS/Linux ports.
