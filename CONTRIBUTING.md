# Contributing to Claude Buddy

Claude Buddy is a lightweight Windows tray app (Tauri 2 + TypeScript frontend + Rust core) that shows one animated pixel-art buddy per running Claude Code CLI session. Thanks for helping.

By contributing you agree your work is licensed under Apache-2.0 and that you follow the [Code of Conduct](CODE_OF_CONDUCT.md).

## Setup

Requirements: Windows 10/11, Node 20+, Rust stable, pnpm.

> The project is not scaffolded yet. The commands below are placeholders; once scaffolded, see the issue tracker.

```bash
pnpm install      # install JS dependencies
pnpm tauri dev    # run the app in dev mode
pnpm test         # frontend tests
cargo test        # Rust core tests
```

## Workflow

1. **Issue** - every change starts from an issue (use the forms in `.github/ISSUE_TEMPLATE/`). Comment to claim it.
2. **Branch** - from `develop`: `feature/<issue>-<slug>` or `fix/<issue>-<slug>` (e.g. `feature/12-tray-menu`).
3. **Commits** - small, Conventional Commits, referencing the issue.
4. **PR** - target `develop`, fill in the template, title is a conventional commit.
5. **Review** - at least one approval, CI green, conversations resolved.
6. **Merge** - squash merge; the PR title becomes the commit message.

## Commit convention

[Conventional Commits 1.0](https://www.conventionalcommits.org/en/v1.0.0/): `type(scope): subject`

- Types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`
- Scopes: `core`, `bridge`, `hooks`, `ui`, `anim`, `tray`, `win`, `docs`, `ci`, `deps`
- Subject: imperative, lowercase, no trailing period, under 72 chars.
- Breaking changes: `feat(bridge)!:` and/or a `BREAKING CHANGE:` footer.
- Body references issues with `Refs #12` or `Closes #12`.

```
feat(anim): add idle-to-working buddy transition

Plays a 6-frame sprite when a session receives its first tool call.

Closes #12
```

```
fix(bridge): reject hook posts with a missing token

Refs #34
```

## Branching model (Git Flow)

```
main      o-----------------o-----------o   released only, tagged vX.Y.Z
           \               / \         /
release     \      o-----o   \       / hotfix/x.y.z
             \    /           \     /
develop   o---o--o-----o---o---o---o---o   integration
              \   /     \ /
feature        o-o       o   feature/<issue>-<slug>, fix/<issue>-<slug>
```

- `main`: released code only; every release is tagged `vX.Y.Z` ([SemVer](https://semver.org)).
- `develop`: integration branch; feature and fix PRs target it.
- `feature/<issue>-<slug>`, `fix/<issue>-<slug>`: branch from and merge into `develop`.
- `release/x.y.z`: branch from `develop`, stabilize and update `CHANGELOG.md`, merge to `main` (tag) and back to `develop`.
- `hotfix/x.y.z`: branch from `main` for urgent fixes, merge to `main` (tag) and back to `develop`.
- AI agent branches such as `claude/*` are allowed and follow the same rules.

## Pull request checklist

- [ ] Linked issue (`Closes #N`)
- [ ] PR title is a valid conventional commit
- [ ] Targets `develop` (or `main` for `hotfix/*`)
- [ ] Tests added or updated, and passing
- [ ] Docs updated; ADR added in `.context/decisions/` if an architecture decision was made
- [ ] `CHANGELOG.md` updated under `[Unreleased]` for user-visible changes
- [ ] Screenshots or GIF attached for UI or animation changes
- [ ] No unrelated changes

## Definition of done

- Acceptance criteria in the issue are met.
- CI is green (lint, type-check, `pnpm test`, `cargo test`, PR title check).
- Reviewed and approved.
- Behavior verified on Windows 10/11 where relevant.
- Docs, ADRs and changelog updated.
- No new telemetry, network exposure or prompt-content handling without a security review (see [SECURITY.md](SECURITY.md)).

## Issue types and labels

Types: `type:feature`, `type:bug`, `type:docs`, `type:chore`, `type:spike` (time-boxed research), `type:epic`. Areas: `area:core`, `area:bridge`, `area:ui`, `area:anim`, `area:win`. Priority: `priority:p0` to `priority:p2`. The full set is in `.github/labels.yml`.

## AI agents

AI agents (Claude Code and others) contribute under the same rules, plus:

- Read `CLAUDE.md` and `.context/` before starting; agent briefs live in `.context/agents/`.
- Log architecture decisions as ADRs in `.context/decisions/` using the template there.
- One issue per branch, one concern per PR.
- Do not push to `main` or `develop`; open a PR.
- Never invent requirements; if the issue is unclear, ask in the issue.

## Releases and changelog

SemVer. `CHANGELOG.md` follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Add entries under `[Unreleased]`; the release branch moves them under the new version.
