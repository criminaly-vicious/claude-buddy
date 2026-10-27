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
2. **Branch** - from `develop`, named `<type>/<issue>-<description>` (e.g. `feature/27-tray-menu`). See [Branch naming](#branch-naming).
3. **Commits** - small, Conventional Commits, referencing the issue.
4. **PR** - target `develop`, fill in the template, title is a conventional commit.
5. **Review** - at least one approval, CI green, conversations resolved.
6. **Merge** - merge commit, never squash or rebase (ADR-0008); the PR title becomes the merge commit title.

## Commit convention

[Conventional Commits 1.0](https://www.conventionalcommits.org/en/v1.0.0/): `type(scope): subject`

- Types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`
- Scopes: `core`, `bridge`, `hooks`, `ui`, `anim`, `tray`, `win`, `docs`, `ci`, `deps`, `release`
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
release     \      o-----o   \       / hotfix/X.Y.Z
             \    /           \     /
develop   o---o--o-----o---o---o---o---o   integration
              \   /     \ /
feature        o-o       o   feature/<issue>-<description>, bugfix/..., docs/..., ...
```

- `main`: released code only; every release is tagged `vX.Y.Z` ([SemVer](https://semver.org)).
- `develop`: integration branch; all work branches target it.
- Work branches (`feature/`, `bugfix/`, `docs/`, `chore/`, `spike/`): branch from and merge into `develop`.
- `release/X.Y.Z`: branch from `develop`, stabilize and update `CHANGELOG.md`, merge to `main` (tag) and back to `develop`.
- `hotfix/X.Y.Z`: branch from `main` for urgent fixes, merge to `main` (tag) and back to `develop`.

## Branch naming

Branch names follow Git Flow and the [Conventional Branch](https://conventional-branch.github.io/) spec. CI checks them on every PR (`.github/workflows/branch-name.yml`).

| Branch | From → into | Issue type | Commit types it usually carries | Example |
| --- | --- | --- | --- | --- |
| `main` | — | — | — | `main` |
| `develop` | — | — | — | `develop` |
| `feature/<issue>-<description>` | `develop` → `develop` | `type:feature` | `feat` | `feature/27-tray-menu` |
| `bugfix/<issue>-<description>` | `develop` → `develop` | `type:bug` | `fix` | `bugfix/41-hook-timeout` |
| `docs/<issue>-<description>` | `develop` → `develop` | `type:docs` | `docs` | `docs/32-user-guide` |
| `chore/<issue>-<description>` | `develop` → `develop` | `type:chore` | `chore`, `ci`, `build`, `refactor`, `test`, `perf` | `chore/7-windows-ci` |
| `spike/<issue>-<description>` | `develop` → `develop` | `type:spike` | `docs`, throwaway code | `spike/9-hook-formats` |
| `release/X.Y.Z` | `develop` → `main` + `develop` | — | `chore(release)` | `release/0.1.0` |
| `hotfix/X.Y.Z` | `main` → `main` + `develop` | `type:bug` | `fix` | `hotfix/0.1.1` |

Epics (`type:epic`) never get a branch; their child issues do.

Rules:

- Lowercase ASCII letters, digits and hyphens only. No spaces, underscores, uppercase or consecutive/trailing hyphens.
- Work branches start with the issue number, then a short English description of 2 to 5 words (`feature/18-sprite-atlas-loader`, not `feature/sprites` or `feature/18_Sprite_Loader`).
- Dots appear only in version numbers. Versions in branches have no `v` (`release/0.1.0`); tags do (`v0.1.0`).
- Whole name at most 50 characters.
- The prefix says what the branch is, the commit scope says where: `bugfix/41-hook-timeout` carries `fix(bridge): ...`.
- Only `release/*` and `hotfix/*` (and nothing else) open PRs into `main`.
- Delete the branch after merge.

AI agent branches: when the tool picks the name (for example Claude Code on the web creates `claude/<random-name>`), `claude/*` is accepted as is. Agents that can choose their own name use the table above. Either way the PR links its issue and follows every other rule.

## Pull request checklist

- [ ] Linked issue (`Closes #N`)
- [ ] PR title is a valid conventional commit
- [ ] Branch name follows [Branch naming](#branch-naming)
- [ ] Targets `develop` (`main` only for `release/*` and `hotfix/*`)
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

Types: `type:feature`, `type:bug`, `type:docs`, `type:chore`, `type:spike` (time-boxed research), `type:epic`. Each type maps to one branch prefix (see [Branch naming](#branch-naming)). Areas: `area:core`, `area:bridge`, `area:ui`, `area:anim`, `area:win`, `area:ci`, `area:docs`. Priority: `priority:p0` to `priority:p2`. The full set is in `.github/labels.yml`.

## AI agents

AI agents (Claude Code and others) contribute under the same rules, plus:

- Read `CLAUDE.md` and `.context/` before starting; agent briefs live in `.context/agents/`.
- Log architecture decisions as ADRs in `.context/decisions/` using the template there.
- One issue per branch, one concern per PR.
- Do not push to `main` or `develop`; open a PR.
- Never invent requirements; if the issue is unclear, ask in the issue.

## Releases and changelog

SemVer. `CHANGELOG.md` follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Add entries under `[Unreleased]`; the release branch moves them under the new version.
