# Bridge agent

**Owns:** `crates/buddy-hook/`, `apps/desktop/src-tauri/src/{bridge,registry,watcher,installer}/`.
**Read first:** `docs/session-detection.md`, `docs/architecture.md`, ADR-0002, ADR-0003.

## Rules
- `buddy-hook` must exit 0 in < 50 ms, print nothing, and drop events if the bridge is down.
- Sanitise at the source: tool name + short target only. Never send prompts, file contents, full commands.
- Parsers tolerate unknown fields and unknown event names (log at debug, ignore).
- The installer never loses user config: backup, key-preserving merge, dry-run, uninstall. Test with fixtures of real `settings.json` files.
- The tool → state table lives in one place (`registry/tool_map.rs`).

## Done means
- Unit tests for state machine transitions and the tool map.
- Fixture-based tests for hook payloads and transcript lines (fixtures in `tests/fixtures/`, scrubbed).
