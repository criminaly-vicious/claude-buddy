# 0007. Branch naming convention

- **Status:** Accepted
- **Date:** 2026-10-27
- **Issue:** —

## Context
ADR-0006 adopted Git Flow but only named `feature/*` and `fix/*` work branches, while the issue forms have five types (feature, bug, docs, chore, spike). Agents and humans need one predictable name per issue so branches, issues, labels and PRs line up, and CI needs a rule it can check.

## Decision
Use Git Flow prefixes plus the [Conventional Branch](https://conventional-branch.github.io/) spec, one prefix per issue type:

- Long-lived: `main`, `develop`.
- Work: `feature/`, `bugfix/`, `docs/`, `chore/`, `spike/`, each followed by `<issue>-<description>` (e.g. `feature/27-tray-menu`).
- Release flow: `release/X.Y.Z` and `hotfix/X.Y.Z`; tags are `vX.Y.Z`.
- Lowercase, digits and hyphens; dots only in versions; at most 50 characters.
- Tool-assigned agent branches `claude/*` are accepted.

`.github/workflows/branch-name.yml` enforces this on PRs, and only `release/*` and `hotfix/*` may target `main`. Full table in `CONTRIBUTING.md#branch-naming`.

## Alternatives considered
- `fix/` instead of `bugfix/`: matches the commit type, but `bugfix/` is the Git Flow and Conventional Branch default and keeps `fix` free for `hotfix/`.
- `<type>/<description>` without issue number: shorter, but loses the branch-to-issue link that ADR-0006 relies on.
- One prefix per commit type (`ci/`, `refactor/`, `test/`, ...): too many prefixes; those changes ride in `chore/` or in the feature that needs them.

## Consequences
- A branch name tells you the issue and its type at a glance.
- PRs from a badly named branch fail CI and must be re-pushed under a new name.
- Supersedes the `fix/*` prefix mentioned in ADR-0006.
