# CLAUDE.md

Entry point for AI agents working on Claude Buddy. Read this, then `.context/README.md`.

## What this is

A Windows tray app (Tauri 2, Rust core, TypeScript Canvas overlay) that shows an animated pixel-art buddy for each running Claude Code CLI session. Start with `README.md` and `docs/`.

## Rules

- **Issue first.** Every change maps to one GitHub issue. Branch `<type>/<issue>-<description>` from `develop`, where type is `feature`, `bugfix`, `docs`, `chore` or `spike` (e.g. `feature/27-tray-menu`); full rules in `CONTRIBUTING.md#branch-naming`. Tool-assigned `claude/*` names are accepted.
- **Always open a PR.** Every piece of work an agent creates is pushed and opened as a pull request (target `develop`, template filled in, linked issue). Never leave work only on a branch.
- **Conventional Commits** for every commit and PR title (`feat(bridge): add token auth`). Reference the issue in the body (`Refs #12` / `Closes #12`).
- **Decisions are written down.** Any choice of library, format, protocol or architecture goes in `.context/decisions/NNNN-title.md` (copy `0000-template.md`). Do not silently reverse an accepted ADR: write a new one that supersedes it.
- **Docs move with code.** If behaviour in `docs/` changes, update the doc in the same PR.
- **Stay light.** Respect the perf budget in `docs/scope.md`. No polling loops in the render path.
- **Read-only towards Claude Code.** Never send input to sessions. `buddy-hook` must never block or fail a session.
- **Privacy.** Never log or display prompts, file contents or full commands.
- Do not ship official Anthropic logos or artwork (ADR-0005).

## Where things are

| Path | Contents |
| --- | --- |
| `docs/` | Vision, scope, architecture, session detection, animation spec, roadmap, backlog |
| `.context/agents/` | Role briefs: what each agent owns and how it works |
| `.context/decisions/` | ADRs |
| `.github/` | Issue forms, PR template, labels, workflows |

## Commands (once scaffolded, see issue B-02)

`pnpm install` · `pnpm tauri dev` · `pnpm dev:mock` · `pnpm test` · `cargo test --workspace` · `pnpm lint`
