# Agent context

Shared memory for humans and AI agents. Kept small and current.

```
.context/
  README.md          this file: how agents collaborate
  agents/            one brief per role (ownership, inputs, outputs, checklists)
  sessions.md        live registry of agent sessions (id, role, branch, status)
  decisions/         ADRs, numbered, never deleted (superseded instead)
```

## How agents collaborate

1. The **master agent** ([brief](agents/master.md), loop in `.claude/skills/master/SKILL.md`) owns planning: keeps `docs/roadmap.md` and `docs/backlog.md` in sync with GitHub issues, splits work, and assigns issues to role agents.
2. A role agent takes **one issue**, creates its branch from `develop` (`<type>/<issue>-<description>`, see `CONTRIBUTING.md#branch-naming`), reads the docs listed in its brief, and works only inside its area. Cross-area changes go back to the master as a new issue.
3. Decisions made while working become ADRs in the same PR.
4. The PR description lists what was done, what was verified, and open questions. The master (or a human) reviews and merges.

## Roles

| Role | Brief | Owns |
| --- | --- | --- |
| Master | [master.md](agents/master.md) | Planning, issues, reviews, docs coherence |
| Bridge | [bridge.md](agents/bridge.md) | `buddy-hook`, bridge server, watcher, registry, hook installer |
| Overlay & animation | [overlay.md](agents/overlay.md) | Overlay rendering, sprites, animation controller, physics, hover card |
| Windows integration | [windows.md](agents/windows.md) | Win32: click-through, focus, autostart, hotkey, tray, installer |
| QA & docs | [qa-docs.md](agents/qa-docs.md) | Tests, CI, perf budget, user docs, changelog |

## Decisions

| ADR | Title | Status |
| --- | --- | --- |
| [0001](decisions/0001-tauri-desktop-shell.md) | Tauri 2 as desktop shell | Accepted |
| [0002](decisions/0002-hooks-first-session-detection.md) | Hooks first, transcript watcher as fallback | Accepted |
| [0003](decisions/0003-loopback-http-bridge.md) | Loopback HTTP bridge with token | Accepted |
| [0004](decisions/0004-pixel-art-canvas-rendering.md) | Pixel art on Canvas 2D, no UI framework in overlay | Accepted |
| [0005](decisions/0005-original-character-art.md) | Original character art, no official branding | Accepted |
| [0006](decisions/0006-git-flow-conventional-commits.md) | Git Flow + Conventional Commits | Accepted |
| [0007](decisions/0007-branch-naming.md) | Branch naming convention | Accepted |
| [0008](decisions/0008-merge-commits.md) | Merge commits instead of squash | Accepted |
| [0009](decisions/0009-tiny-http-bridge-server.md) | tiny_http for the bridge server | Accepted |
