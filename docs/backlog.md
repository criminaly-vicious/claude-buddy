# Backlog

Every item here exists as a GitHub issue (number in brackets once created). Issues are the source of truth for status; this file is the planning snapshot. Labels: `type:*`, `area:*`, `priority:*` (see [CONTRIBUTING.md](../CONTRIBUTING.md)).

## M0 — Foundation

| ID | Type | Title | Area | Pri |
| --- | --- | --- | --- | --- |
| B-01 | epic | M0 Foundation: repo, tooling, spikes | — | p0 |
| B-02 | chore | Scaffold Tauri 2 app + `buddy-hook` crate (pnpm workspace) | core | p0 |
| B-03 | chore | CI on `windows-latest`: lint, typecheck, `cargo test`, Vitest, build | ci | p0 |
| B-04 | chore | Sync labels from `.github/labels.yml` and create milestones | ci | p1 |
| B-05 | spike | Verify Claude Code hook and transcript formats on Windows (2 days) | bridge | p0 |
| B-06 | spike | Transparent, click-through, always-on-top Tauri window on Windows: CPU/RAM cost (2 days) | win | p0 |

## M1 — Session bridge

| ID | Type | Title | Area | Pri |
| --- | --- | --- | --- | --- |
| B-07 | epic | M1 Session bridge: know every session's state | bridge | p0 |
| B-08 | feature | `buddy-hook` CLI: read hook stdin, sanitise, POST to bridge, never block | bridge | p0 |
| B-09 | feature | Local bridge server on 127.0.0.1 with token + `bridge.json` | bridge | p0 |
| B-10 | feature | Session registry + state machine + tool→state mapping | core | p0 |
| B-11 | feature | Transcript watcher fallback (`~/.claude/projects/**.jsonl`) | bridge | p1 |
| B-12 | feature | Process liveness + session expiry | core | p1 |
| B-13 | feature | Hook installer/uninstaller for `~/.claude/settings.json` (backup, merge, dry-run) | bridge | p0 |

## M2 — Overlay & buddies

| ID | Type | Title | Area | Pri |
| --- | --- | --- | --- | --- |
| B-14 | epic | M2 Overlay & buddies: living characters in the corner | ui | p0 |
| B-15 | feature | Overlay window: corner anchoring, multi-monitor, hit-test click-through | win | p0 |
| B-16 | feature | Sprite pipeline: Aseprite atlas loader + render loop that sleeps when static | anim | p0 |
| B-17 | feature | Buddy character art v1 (all animations in the spec) | anim | p0 |
| B-18 | feature | Animation controller: state → transitions → loops, random idle extras | anim | p0 |
| B-19 | feature | Group layout: stable slots, `+N` overflow, waiting buddy steps forward | ui | p1 |

## M3 — Interactions

| ID | Type | Title | Area | Pri |
| --- | --- | --- | --- | --- |
| B-20 | epic | M3 Interactions: hover, click, throw | ui | p1 |
| B-21 | feature | Hover card with sanitised session summary | ui | p0 |
| B-22 | feature | Click to focus the session's terminal window | win | p1 |
| B-23 | feature | Drag & throw physics with bounce, land, dizzy, walk back | anim | p1 |
| B-24 | spike | Focus a specific Windows Terminal tab via UI Automation (1 day) | win | p2 |

## M4 — Background app & release v0.1.0

| ID | Type | Title | Area | Pri |
| --- | --- | --- | --- | --- |
| B-25 | epic | M4 Background app & first release | core | p1 |
| B-26 | feature | Tray icon + menu (show/hide, install hooks, settings, quit) | core | p0 |
| B-27 | feature | Settings window + persisted config | ui | p1 |
| B-28 | feature | Start with Windows + global hotkey | win | p1 |
| B-29 | chore | Performance budget check (idle CPU < 1%, RAM < 80 MB) | core | p1 |
| B-30 | chore | NSIS installer + release workflow (tag on `main` → GitHub Release) | ci | p1 |
| B-31 | docs | User guide: install, hooks, troubleshooting, FAQ | docs | p1 |
