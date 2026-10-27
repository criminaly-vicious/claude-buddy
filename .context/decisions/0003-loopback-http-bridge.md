# 0003. Loopback HTTP bridge with token

- **Status:** Accepted
- **Date:** 2026-10-27

## Context
`buddy-hook` runs as a short-lived process per hook event and must hand the event to the long-running app quickly.

## Decision
The app runs an HTTP server on `127.0.0.1` with a random free port and a random per-install token. Both are written to `%APPDATA%\claude-buddy\bridge.json` (user-only ACL). `buddy-hook` reads it and sends `POST /v1/events` with `Authorization: Bearer <token>`, 300 ms timeout.

## Alternatives considered
- **Named pipes:** Windows-native and slightly faster, but harder to port and to debug with curl.
- **Writing event files to a spool folder:** no server needed, but adds disk churn and latency.

## Consequences
- Easy to test (`curl`), portable to other OSes.
- Must guard against other local processes: token required, loopback only, payload size limit, schema validation.
