# 0002. Hooks first, transcript watcher as fallback

- **Status:** Accepted
- **Date:** 2026-10-27

## Context
We must know which Claude Code CLI sessions exist and what each is doing, in near real time, without modifying Claude Code and without heavy polling.

## Decision
Primary source: **Claude Code hooks** configured in `~/.claude/settings.json`, which run `buddy-hook.exe` to forward events to our local bridge. Fallback: **watch transcript JSONL files** in `~/.claude/projects/` to discover sessions and infer approximate state. See `docs/session-detection.md`.

## Alternatives considered
- **Transcripts only:** zero setup, but cannot reliably detect "waiting for permission" and lags.
- **Process/terminal scraping:** fragile, invasive, terminal-specific.
- **Wrapping the `claude` command:** changes how users start sessions; rejected.
- **Built-in `http` hooks instead of `buddy-hook.exe`** (evaluated in spike #9): rejected. Every hook shows an error in the user's transcript when the app is not running, `SessionStart` does not support `http`, and the payload has no PID. See `docs/session-detection.md`.

## Consequences
- Requires a one-click, reversible hook installer that must never damage user config.
- Formats can change between Claude Code versions: tolerant parsers, fixtures, and spike B-05 to verify.
- Sessions started before hooks were installed still appear (via fallback).
