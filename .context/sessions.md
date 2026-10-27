# Agent sessions

Live registry of the Claude Code sessions working on this repo. The master agent keeps it current (see `.claude/skills/master/SKILL.md`). Session ids are for the claude-code-remote tools (`send_message`, `list_events`).

## Active

| Role | Session | Branch | Working on |
| --- | --- | --- | --- |
| Master | `session_01VWGg3Q3xyLXSSWxMrwmZEJ` | `claude/amazing-darwin-awrqcv` | Coordination, reviews, merges |
| Core | `session_01VWGg3PhThAGaSW52z4JdPX` "Claude Bunny project setup" | `claude/pensive-brown-izonp9` | #30 performance budget check (PR #64) |
| Bridge / research | `session_01VWGg3PPHo8B38FaxCaWAYo` "Normalização de ranges e nomenclatura" | `claude/festive-volta-qtqfnb` | #25 Windows Terminal tab spike (PR #65) |

## Waiting on the owner

| Issue | What is needed |
| --- | --- |
| #19 | Art direction for the buddy character |
| #10 | CPU/RAM measurement on a real Windows machine |
| #8 | Milestones M0-M4 (GitHub UI) |
| #66 | Run the Windows Terminal tab prototype (`tools/spikes/wt-tabs`) |
| Release | `release/0.1.0` branch, merge into `main`, `v0.1.0` tag |

## Done

| PR | Session | Outcome |
| --- | --- | --- |
| #26 | Core | Docs, planning, issues #1-#32 |
| #33 | Bridge | Branch naming convention (ADR-0007) |
| #34 | Core | Label sync workflow |
| #35, #36 | Master | Master skill, session registry, merge-commit rule (ADR-0008) |
| #37-#53 | Core, Bridge | M0-M2: scaffold, hook formats, bridge, registry, liveness, transcript watcher, hook installer, overlay, animations |
| #55, #56 | Core | Hover card (#22), click to focus (#23) |
| #57 | Core | Drag and throw (#24) |
| #58 | Master | Workflow actions off Node 20 (#54) |
| #59 | Core | Tray icon and menu (#27, ADR-0012) |
| #60 | Core | Settings window and config (#28) |
| #61 | Bridge | User guide (#32, GIFs pending) |
| #62 | Core | Autostart and global hotkey (#29) |
| #63 | Bridge | NSIS installer and release workflow (#31) |
