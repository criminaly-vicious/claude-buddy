# 0009. tiny_http for the bridge server

- **Status:** Accepted
- **Date:** 2026-10-27
- **Issue:** #12

## Context
The bridge (ADR-0003) serves one tiny endpoint on loopback: a few requests per second at most, one user, bodies under 16 KiB. It must add little to the binary size and idle cost, and its behaviour (status codes, size limits) must be easy to test.

## Decision
Use **`tiny_http`** on a dedicated thread. Requests are handled synchronously and turned into registry inputs sent over a channel to a single driver thread, which owns the registry timers (`Registry::next_deadline`) and emits the Tauri events.

## Alternatives considered
- **axum (on Tauri's tokio runtime):** idiomatic and powerful, but pulls in hyper, tower and their dependencies for a single route, and an async stack is not needed at this load.
- **Raw `TcpListener` with hand-written HTTP parsing:** smallest, but parsing HTTP ourselves is where security bugs come from.
- **Named pipes:** rejected in ADR-0003.

## Consequences
- Small dependency tree; no async in the bridge.
- Blocking I/O: `tiny_http` exposes no per-request read timeout. Each request is therefore handled on its own short-lived thread, so a client that stalls mid-body only holds its own thread and never blocks other requests. Authentication runs before the body is read, so only a process holding the user's token can stall a thread at all, and the body read is capped at 16 KiB.
- If the bridge ever needs streaming or many concurrent clients, revisit with a new ADR.
