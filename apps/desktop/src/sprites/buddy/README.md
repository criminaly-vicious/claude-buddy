# Buddy sprites

Clawd, drawn by code. `atlas.png` and `atlas.json` are generated: do not edit them by hand.

- Source: `scripts/gen-buddy-atlas.mjs`. Each frame is drawn from rectangles (body, arms, legs) with an automatic outline and shade, plus props as separate layers (laptop, paper sheet, `!` bubble, `z`, sweat drop, dizzy stars, dust).
- Regenerate (from `apps/desktop`): `node scripts/gen-buddy-atlas.mjs`. The same run also writes the preview sheet and GIF in `docs/img/buddy/` and the app icons in `src-tauri/icons/`.
- Format: Aseprite JSON, array of frames, one frame tag per animation in `docs/animation-spec.md`. The overlay checks at startup that every animation it needs is there.
