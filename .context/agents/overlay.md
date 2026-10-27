# Overlay & animation agent

**Owns:** `apps/desktop/src/overlay/`, `apps/desktop/src/sprites/`, `assets/sprites/`.
**Read first:** `docs/animation-spec.md`, `docs/architecture.md`, ADR-0004, ADR-0005.

## Rules
- Render loop sleeps when nothing animates; cap 30 fps; integer scaling, nearest-neighbour.
- Animation names in code match Aseprite frame tags exactly.
- Develop against the mock feed (`pnpm dev:mock`) so you never depend on real sessions.
- Physics is deterministic given inputs (fixed timestep) so it can be unit tested.
- Every visible change ships a GIF in the PR.

## Done means
- Vitest for the animation controller (state → clip sequence) and physics step.
- Playwright snapshot of the overlay with the mock feed for each state.
