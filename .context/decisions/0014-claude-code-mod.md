# 0014. Connect Claude Code through a Claude Code mod, classic hooks as fallback

- **Status:** Accepted
- **Date:** 2026-11-13
- **Issue:** #70
- **Supersedes:** the connection part of [ADR-0002](0002-hooks-first-session-detection.md) (the tray writing `buddy-hook` entries into `settings.json` as the only primary source). The transcript fallback in ADR-0002 is unchanged.

## Context
Claude Code 2.1.3xx added mods: plugins with a TypeScript hooks module that can hook the session's events, read files and fetch over HTTP through the host. Until now, connecting Claude Code meant the tray editing the user's `~/.claude/settings.json` to run `buddy-hook.exe` (ADR-0002). A mod installs with one line, touches no settings file, and updates with the plugin. The v1.0.0 plan (owner, 2026-11-13) asks for one.

## Decision
- Ship a mod, **`buddy-bridge`**, in `mods/buddy-bridge/`, listed in `.claude-plugin/marketplace.json` at the repository root: `/plugin install buddy-bridge --marketplace criminaly-vicious/claude-buddy`. Plugin names starting with `claude-` are reserved for Anthropic, so the mod is named for what it does.
- It hooks the same nine `classic.<Event>` events the tray installer registers and sends the **same wire events** as `buddy-hook` to `POST /v1/events`, with the same fields, limits, privacy rules and `target` rules. The bridge protocol does not change. The rules live twice, in Rust (`bridge_protocol`) and TypeScript (`mods/buddy-bridge/hooks/protocol.ts`). One shared case file, `mods/buddy-bridge/test/target-cases.js`, is tested on both sides, so they cannot drift apart.
- **It never blocks a session.** Each hook returns `next(e)` at once and reports on the side, with a `.catch` that hands the event on. A send gives up after 300 ms, and every error is swallowed.
- **No double reporting.** When any settings layer (`$.settings.read({ key: "hooks" })`) has a hook running `buddy-hook(.exe)`, the mod stays quiet for the session and the classic hook reports. Removing the classic hooks (tray → Uninstall hooks…) hands over to the mod at the next session.
- **The PID comes from a child process.** No `$` call returns Claude Code's PID, and `CLAUDE_PID` is not set for mods. The mod starts one child per session, `sh -c 'echo $PPID'` (PowerShell `Get-CimInstance` on Windows), and sends its parent PID. Reports wait for it for at most 2 s; without it, liveness falls back to the 30-minute transcript TTL.
- The classic hooks stay, as the **fallback** for Claude Code versions or setups where mods do not load (mods are early access; hooks modules of installed plugins are behind a rollout switch on Claude Code's side).

## Alternatives considered
- **Mod only, drop `buddy-hook`:** rejected for v1.0.0. Mods are early access and may not load for every user; the classic path is proven (#53).
- **Mod blocks the classic hooks to avoid duplicates:** a `classic.*` hook that does not call `next` would also stop the user's own settings hooks. Rejected.
- **Bridge-side de-duplication** (an event id, or a `source` field): needs a protocol change and still sends every event twice. Detecting the classic hook in the mod is enough.
- **`$.env.get("CLAUDE_PID")` for the PID:** tested, not set for mods or their children.
- **Drawing the buddy inside Claude Code:** out of scope (#70); a possible follow-up.

## Consequences
- Connecting Claude Code becomes one line, without the tray editing `settings.json`. The tray's hook installer is still needed for the fallback.
- Two implementations of the event and `target` rules must move together; the shared cases and both test suites guard this.
- One short-lived child process per session (a PowerShell start on Windows, not measured yet), off the turn's path.
- ⚠ Windows: not verified. The mod was tested end to end on Linux (Claude Code 2.1.323, `claude -p --plugin-dir`, fake bridge): all events arrived with token, PID and target, and none with the classic hook installed. The PowerShell PID lookup, `APPDATA` resolution and the installed-plugin path on Windows still need the owner's check.
