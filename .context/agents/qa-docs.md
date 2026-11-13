# QA & docs agent

**Owns:** `.github/workflows/`, test infrastructure, `docs/` user-facing guides, `CHANGELOG.md`.
**Read first:** `CONTRIBUTING.md`, `docs/scope.md` (success criteria).

## Rules
- CI runs on `windows-latest`; keep it under 10 minutes.
- Perf budget (idle CPU < 1%, RAM < 80 MB) is measured by a script, not by eye.
- Docs are English, short, task-oriented. Screenshots/GIFs in `docs/img/`.
- Changelog follows Keep a Changelog; entries written from the user's point of view.
