# 0015. Follow subagents through SubagentStart and SubagentStop

- **Status:** Accepted
- **Date:** 2026-11-13
- **Issue:** #83

## Context
The owner wants each subagent shown as a mini Clawd on its session's buddy (#84). Until now the app ignored subagents: `SubagentStart` and `SubagentStop` were not registered, and tool events from inside a subagent (`agent_id` set) counted toward the parent buddy ([architecture](../../docs/architecture.md#server-event--registry)). Claude Code reports a subagent's start (`agent_id`, `agent_type`) and its end (plus its last message, which must never leave the hook). Several subagents can run at once, and background subagents keep running after the parent's turn ends.

## Decision
- **Register `SubagentStart` and `SubagentStop`**, in the tray's hook installer and in the `buddy-bridge` mod, which now reports 11 events. The mod's version goes to 0.2.0.
- **Wire protocol:** an optional `agent_type` (64 chars) beside `agent_id`. It is additive: protocol version 1 stays, and an older app ignores the field.
- **Registry:** a session keeps its running subagents in start order (`SessionView.subagents`: id, type, current tool and summary, since).
  - A subagent starts on `SubagentStart`, or on its first event with an `agent_id`, so older hooks still show it. It ends on `SubagentStop`.
  - If no end ever comes, it is dropped after 10 minutes of silence. A subagent seen only through its tools (older hooks, so no `SubagentStop` will come) is dropped after 90 seconds of silence.
  - The parent leaving takes its subagents with it. At most 16 are tracked per session.
  - `agent_type` without `agent_id` (a session started with `--agent`) is not a subagent.
- **A subagent's tool events update the subagent, not the parent buddy.**
  - The parent keeps the tool and pose of its own `Agent` call ("Delegating to Explore").
  - A permission prompt inside a subagent still makes the parent wait, since the user must answer in that terminal. The subagent's next tool means the user answered.
  - Subagent events keep the parent awake, so it does not fall asleep while its minis work.
- **Transcript-only sessions** show no subagents. Subagent transcripts stay ignored by the watcher.

## Alternatives considered
- **Count running `Agent` tool calls** (PreToolUse/PostToolUse of the parent): no id links a call to the subagent's own events, and background subagents outlive their call. Rejected.
- **Keep counting subagent tools toward the parent:** with several subagents the parent's pose would flicker between theirs, and the minis would have nothing of their own to show. Rejected.
- **Read `subagents/agent-*.meta.json`** (`finished`) for transcript sessions: more file watching for an approximate mode. Left for later.

## Consequences
- Existing classic installs lack the two events until **Install / repair hooks…** runs again. The settings window now says "out of date" instead of "not installed". Until then minis still appear (from tool events) and go after 90 s of silence.
- The registry holds one more timer per session (the silent-subagent deadline), folded into `next_deadline`, so nothing polls.
- The mod and `buddy-hook` copy `agent_type`; the shared protocol tests cover it on both sides.
