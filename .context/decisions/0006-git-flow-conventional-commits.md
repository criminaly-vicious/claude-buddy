# 0006. Git Flow + Conventional Commits

- **Status:** Accepted
- **Date:** 2026-10-27

## Context
Several AI agents and humans work in parallel; history must stay readable and releases predictable.

## Decision
Git Flow branches (`main`, `develop`, `feature/*`, `fix/*`, `release/*`, `hotfix/*`), Conventional Commits for commits and PR titles (linted in CI), squash merges into `develop`, SemVer tags on `main`. Details in `CONTRIBUTING.md`.

## Consequences
- Changelog and version bumps can be generated from history later (e.g. release-please).
- Every change needs an issue and a branch; small overhead accepted for traceability.
