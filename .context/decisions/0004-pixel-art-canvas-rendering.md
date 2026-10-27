# 0004. Pixel art on Canvas 2D, no UI framework in overlay

- **Status:** Accepted
- **Date:** 2026-10-27

## Context
Buddies need many small frame animations, simple physics, and must cost almost nothing when idle.

## Decision
Pixel-art sprite sheets (32x32 frames, Aseprite source, PNG atlas + JSON) drawn on a single Canvas 2D with integer scaling. Our own tiny render loop that stops when nothing animates. No React/Vue in the overlay; the settings window may use Preact.

## Alternatives considered
- **Lottie/Rive:** smooth vector animation but heavier runtimes and harder to draw "cute pixel" style.
- **PixiJS/WebGL:** more power than needed; GPU context cost for a corner overlay.
- **DOM/CSS sprites:** fine for few buddies, but physics and many layers get awkward.

## Consequences
- Art pipeline depends on Aseprite exports; frame tags are the contract between art and code.
- We write our own small animation controller and physics step (both unit-testable).
