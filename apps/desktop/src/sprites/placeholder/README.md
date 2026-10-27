# Placeholder sprites

**Not the real art.** An abstract, generated atlas so the sprite pipeline (#18) can be built and tested before the buddy art (#19) exists. Each animation is a grey-blue block with a coloured bar whose length shows the frame index.

- Format: Aseprite JSON, array of frames, one frame tag per animation in `docs/animation-spec.md`.
- Regenerate: `node scripts/gen-placeholder-atlas.mjs` (from `apps/desktop`).
- Replace with the real atlas from `assets/sprites/buddy.aseprite` in #19. The real art must be original, not the official Claude mascot (ADR-0005).
