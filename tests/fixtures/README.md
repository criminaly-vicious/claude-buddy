# Test fixtures

Scrubbed samples of what Claude Code sends to hooks and writes to transcripts. Produced by spike [#9](https://github.com/criminaly-vicious/claude-buddy/issues/9); the formats are explained in [`docs/session-detection.md`](../../docs/session-detection.md).

**This repository is public.** Every fixture must stay free of real prompts, file contents, personal paths, tokens and real IDs. Add new captures only after scrubbing them the same way.

## Layout

| Path | Source | Contents |
| --- | --- | --- |
| `hooks/captured/` | Real capture, Claude Code 2.1.311, Linux | Stdin of a command hook, one file per event (and per tool for tool events) |
| `hooks/http/` | Real capture, Claude Code 2.1.311, Linux | One POST received by a local server from an `http` hook: headers and body |
| `hooks/from-docs/` | Official hook docs, not captured | Events the capture run could not trigger (`Notification`, `PermissionRequest`, `PostToolUseFailure`) and a Windows-style payload. **Not verified on Windows.** |
| `transcripts/session.jsonl` | Real capture, Claude Code 2.1.311, Linux | Main transcript of a short session: one prompt, `Read`, `Bash`, an `Agent` subagent, final answer |
| `transcripts/subagents/` | Real capture | The subagent's own transcript and its `.meta.json` |
| `transcripts/http-hook-connection-refused.jsonl` | Real capture | Lines Claude Code writes when an `http` hook cannot connect (evidence for ADR-0002) |

## How they were captured

A throwaway, non-interactive session (`claude -p` with `--settings <temp file>`) ran in an empty folder with a synthetic `notes.txt`. Its settings registered a command hook that saved stdin for every event, plus `http` hooks pointing at a local test server. The user's own settings were not touched. The synthetic prompt asked Claude to read the file, run `echo done`, and delegate one read to a subagent.

## Scrubbing

| Real value | Placeholder |
| --- | --- |
| Session ID | `11111111-1111-4111-8111-111111111111` |
| Prompt ID | `22222222-2222-4222-8222-222222222222` |
| Subagent ID | `a0000000000000001` |
| Other UUIDs | `00000000-0000-4000-8000-0000000000NN` (consistent within the set) |
| `toolu_…`, `msg_…`, `req_…` IDs | `toolu_FIXTURE0001`, `msg_FIXTURE0001`, `req_FIXTURE0001`, … |
| Working directory | `/home/user/projects/demo` |
| Config dir | `/home/user/.claude` |
| Thinking signatures | `FIXTURE_SIGNATURE` |
| Test token | `FIXTURE_TOKEN` |

Transcript lines were also trimmed:
- `attachment` lines (system reminders, tool and skill listings, environment snapshots) were dropped, except one `environment` attachment kept as a shape example with its content replaced.
- `atis-latch` lines (internal) were dropped.

Everything else is byte-for-byte what Claude Code wrote, including fields we do not use. Parsers must tolerate unknown fields.

Caveat: the capture ran inside a Claude Code cloud session, so `entrypoint` is `remote_desktop` and the child session reused its parent's session ID. Locally on Windows, expect `entrypoint` such as `cli` and a fresh ID per session.
