# 0008. Merge commits instead of squash

- **Status:** Accepted
- **Date:** 2026-10-27
- **Issue:** —

## Context
ADR-0006 chose squash merges into `develop`. The project owner decided that PRs keep their full history instead. Squash also breaks branches stacked on an unmerged PR: they still carry the original commits, so merging them later conflicts with the squashed copy.

## Decision
Merge every PR with a merge commit. Never squash or rebase-merge. The merge commit title is the PR title (a Conventional Commit) followed by `(#N)`.

## Alternatives considered
- Squash (ADR-0006): one commit per PR on `develop`, but loses commit history and breaks stacked branches.
- Rebase merge: linear history, but rewrites commit SHAs, with the same stacked-branch problem.

## Consequences
- `develop` keeps every commit; read it with `git log --first-parent` for one line per PR.
- Individual commits must also follow Conventional Commits, since they land as is.
- Supersedes the squash rule in ADR-0006.
