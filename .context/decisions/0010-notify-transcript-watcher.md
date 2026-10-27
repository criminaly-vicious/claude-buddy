# 0010. notify for the transcript watcher

- **Status:** Accepted
- **Date:** 2026-10-27
- **Issue:** #14

## Context
The transcript fallback (ADR-0002) must notice new lines in `~/.claude/projects/**/*.jsonl` within a second or two, without burning CPU while nothing changes, on Windows first and Linux for development.

## Decision
Use the **`notify`** crate (v8, `RecommendedWatcher`, recursive) on the projects folder. A single thread blocks on its event channel, waits 150 ms for a burst to settle, then reads only the new complete lines of each changed file (offset per file; a shrunk file is read again from the start).

## Alternatives considered
- **Polling file sizes on a timer:** simple, but wakes the CPU forever and adds latency, against the "light" principle.
- **Raw `ReadDirectoryChangesW` / inotify:** what `notify` wraps; writing it twice ourselves adds `unsafe` and bugs for no gain.

## Consequences
- One well-maintained dependency, native change events on each OS, no polling.
- `notify` can drop events under heavy load; a missed write is picked up on the next one, because each read starts from the stored offset rather than from the event.
