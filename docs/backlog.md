# Backlog

Every item here exists as a GitHub issue (linked next to each ID). Issues are the source of truth for status; this file is the planning snapshot. Labels: `type:*`, `area:*`, `priority:*` (see [CONTRIBUTING.md](../CONTRIBUTING.md)).

## M0 — Foundation

| ID | Type | Title | Area | Pri |
| --- | --- | --- | --- | --- |
| B-01 [#1](https://github.com/criminaly-vicious/claude-buddy/issues/1) | epic | M0 Foundation: repo, tooling, spikes | — | p0 |
| B-02 [#6](https://github.com/criminaly-vicious/claude-buddy/issues/6) | chore | Scaffold Tauri 2 app + `buddy-hook` crate (pnpm workspace) | core | p0 |
| B-03 [#7](https://github.com/criminaly-vicious/claude-buddy/issues/7) | chore | CI on `windows-latest`: lint, typecheck, `cargo test`, Vitest, build | ci | p0 |
| B-04 [#8](https://github.com/criminaly-vicious/claude-buddy/issues/8) | chore | Sync labels from `.github/labels.yml` and create milestones | ci | p0 |
| B-05 [#9](https://github.com/criminaly-vicious/claude-buddy/issues/9) | spike | Verify Claude Code hook and transcript formats on Windows (2 days) | bridge | p0 |
| B-06 [#10](https://github.com/criminaly-vicious/claude-buddy/issues/10) | spike | Transparent, click-through, always-on-top Tauri window on Windows: CPU/RAM cost (2 days) | win | p0 |

## M1 — Session bridge

| ID | Type | Title | Area | Pri |
| --- | --- | --- | --- | --- |
| B-07 [#2](https://github.com/criminaly-vicious/claude-buddy/issues/2) | epic | M1 Session bridge: know every session's state | bridge | p0 |
| B-08 [#11](https://github.com/criminaly-vicious/claude-buddy/issues/11) | feature | `buddy-hook` CLI: read hook stdin, sanitise, POST to bridge, never block | bridge | p0 |
| B-09 [#12](https://github.com/criminaly-vicious/claude-buddy/issues/12) | feature | Local bridge server on 127.0.0.1 with token + `bridge.json` | bridge | p0 |
| B-10 [#13](https://github.com/criminaly-vicious/claude-buddy/issues/13) | feature | Session registry + state machine + tool→state mapping | core | p0 |
| B-11 [#14](https://github.com/criminaly-vicious/claude-buddy/issues/14) | feature | Transcript watcher fallback (`~/.claude/projects/**.jsonl`) | bridge | p1 |
| B-12 [#15](https://github.com/criminaly-vicious/claude-buddy/issues/15) | feature | Process liveness + session expiry | core | p1 |
| B-13 [#16](https://github.com/criminaly-vicious/claude-buddy/issues/16) | feature | Hook installer/uninstaller for `~/.claude/settings.json` (backup, merge, dry-run) | bridge | p0 |

## M2 — Overlay & buddies

| ID | Type | Title | Area | Pri |
| --- | --- | --- | --- | --- |
| B-14 [#3](https://github.com/criminaly-vicious/claude-buddy/issues/3) | epic | M2 Overlay & buddies: living characters in the corner | ui | p0 |
| B-15 [#17](https://github.com/criminaly-vicious/claude-buddy/issues/17) | feature | Overlay window: corner anchoring, multi-monitor, hit-test click-through | win | p0 |
| B-16 [#18](https://github.com/criminaly-vicious/claude-buddy/issues/18) | feature | Sprite pipeline: Aseprite atlas loader + render loop that sleeps when static | anim | p0 |
| B-17 [#19](https://github.com/criminaly-vicious/claude-buddy/issues/19) | feature | Buddy character art v1 (all animations in the spec) | anim | p0 |
| B-18 [#20](https://github.com/criminaly-vicious/claude-buddy/issues/20) | feature | Animation controller: state → transitions → loops, random idle extras | anim | p0 |
| B-19 [#21](https://github.com/criminaly-vicious/claude-buddy/issues/21) | feature | Group layout: stable slots, `+N` overflow, waiting buddy steps forward | ui | p1 |

## M3 — Interactions

| ID | Type | Title | Area | Pri |
| --- | --- | --- | --- | --- |
| B-20 [#4](https://github.com/criminaly-vicious/claude-buddy/issues/4) | epic | M3 Interactions: hover, click, throw | ui | p1 |
| B-21 [#22](https://github.com/criminaly-vicious/claude-buddy/issues/22) | feature | Hover card with sanitised session summary | ui | p0 |
| B-22 [#23](https://github.com/criminaly-vicious/claude-buddy/issues/23) | feature | Click to focus the session's terminal window | win | p1 |
| B-23 [#24](https://github.com/criminaly-vicious/claude-buddy/issues/24) | feature | Drag & throw physics with bounce, land, dizzy, walk back | anim | p1 |
| B-24 [#25](https://github.com/criminaly-vicious/claude-buddy/issues/25) | spike | Focus a specific Windows Terminal tab via UI Automation (1 day) | win | p2 |

## M4 — Background app & release v0.1.0

| ID | Type | Title | Area | Pri |
| --- | --- | --- | --- | --- |
| B-25 [#5](https://github.com/criminaly-vicious/claude-buddy/issues/5) | epic | M4 Background app & first release | core | p1 |
| B-26 [#27](https://github.com/criminaly-vicious/claude-buddy/issues/27) | feature | Tray icon + menu (show/hide, install hooks, settings, quit) | core | p0 |
| B-27 [#28](https://github.com/criminaly-vicious/claude-buddy/issues/28) | feature | Settings window + persisted config | ui | p1 |
| B-28 [#29](https://github.com/criminaly-vicious/claude-buddy/issues/29) | feature | Start with Windows + global hotkey | win | p1 |
| B-29 [#30](https://github.com/criminaly-vicious/claude-buddy/issues/30) | chore | Performance budget check (idle CPU < 1%, RAM < 80 MB) | core | p1 |
| B-30 [#31](https://github.com/criminaly-vicious/claude-buddy/issues/31) | chore | NSIS installer + release workflow (tag on `main` → GitHub Release) | ci | p1 |
| B-31 [#32](https://github.com/criminaly-vicious/claude-buddy/issues/32) | docs | User guide: install, hooks, troubleshooting, FAQ | docs | p1 |
