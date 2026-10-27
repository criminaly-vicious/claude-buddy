# Character & animation spec

## The buddy

- Original pixel-art character: a small, chunky orange creature (warm orange `#D97757`-ish body, darker outline, two dot eyes, stubby arms and legs). Inspired by the Claude Code mascot vibe, **not** a copy of official art ([ADR-0005](../.context/decisions/0005-original-character-art.md)).
- Base canvas **32×32 px** per frame, drawn at integer scale (1x/2x/3x) with nearest-neighbour filtering.
- Props are separate layers in the source file: laptop, paper sheet, `!` bubble, `z` bubble, sweat drop.
- Source files in Aseprite (`assets/sprites/buddy.aseprite`), exported to a PNG atlas + JSON (frame tags = animation names).
- Until the real art lands (#19), `apps/desktop/src/sprites/placeholder/` holds a generated, deliberately abstract atlas with every animation tag, so the pipeline runs end to end. The overlay checks at startup that every animation it needs exists in the atlas.

## Animations

| Name | Frames (approx.) | Loop | Used for |
| --- | --- | --- | --- |
| `spawn` | 8 | no | Pops in with a small hop and dust puff |
| `idle` | 4 | yes | Breathing; random extras every 8–20 s: `idle_look`, `idle_stretch`, `idle_scratch` |
| `sit_down` | 4 | no | Transition into working/researching |
| `work_type` | 6 | yes | Sitting, laptop open, typing; occasional screen glow |
| `research_read` | 6 | yes | Sitting, holding a paper sheet, eyes moving; occasional page flip |
| `stand_up` | 4 | no | Leaving a sitting state |
| `wait_wave` | 6 | yes | Standing, waving, `!` bubble bobbing |
| `sleep` | 4 | yes | Curled up, `z` bubbles |
| `walk` | 6 | yes | Walking back to its slot after being thrown or reordered |
| `grabbed` | 4 | yes | Dangling, legs kicking |
| `fly` | 2 | yes | Tumbling in the air |
| `land` | 5 | no | Squash on landing; hard landings play `land_splat` then `dizzy` |
| `dizzy` | 6 | no | Stars circling, then shakes it off |
| `leave` | 8 | no | Waves goodbye and walks off screen |

### State → animation

| Buddy state | Enter | Loop |
| --- | --- | --- |
| `spawning` | `spawn` | then `idle` |
| `idle` | `stand_up` if sitting | `idle` (+ random extras) |
| `working` | `sit_down` if standing | `work_type` |
| `researching` | `sit_down` if standing (swap prop if already sitting) | `research_read` |
| `waiting` | `stand_up` if sitting | `wait_wave` |
| `sleeping` | — | `sleep` |
| `leaving` | `stand_up` if sitting | `leave`, then removed |

Animation playback is driven by an **animation controller** per buddy: target state comes from the registry; the controller plays the needed transition before the loop, and never interrupts a non-looping clip except for `grabbed`.

## Layout

- Buddies stand side by side on an invisible "floor" in the chosen corner, 4 px gap, newest at the outer edge.
- Max 10 visible; extras collapse into a `+N` chip that expands on hover.
- A buddy in `waiting` steps 4 px forward so it stands out.
- Slots are stable: a buddy keeps its slot until it leaves.

## Interactions

| Input | Behaviour |
| --- | --- |
| Hover (300 ms) | Hover card above the buddy: project name, state label, tool, summary, elapsed time, "approximate" hint for transcript-only sessions |
| Click | Focus the session's terminal window; buddy does a small happy hop |
| Double-click | Buddy does a random silly animation (spin, jump) |
| Drag | `grabbed`; buddy follows the cursor with a little lag (spring) |
| Release with velocity | `fly` with gravity (≈ 2000 px/s²), bounces off screen edges (restitution 0.4), friction on floor; `land` / `land_splat` → `dizzy` on hard landings; then `walk` back to slot and resume state |
| Release without velocity | Drops, `land`, walks back |
| Right-click | Context menu: Focus, Copy session id, Hide this buddy |

While a buddy is in the air the overlay temporarily grows to full screen and shrinks back after landing.

## Hover card content rules

- Max 3 lines, 80 chars each, sanitised (no secrets, no full commands). Example:
  - `claude-buddy · Working · 2m 14s`
  - `Edit · src/overlay/physics.ts`
- Card is the only clickable-through exception besides the buddies.

## Accessibility & comfort

- Option to reduce motion (no random idle extras, no screen shake).
- `waiting` uses shape (bubble + wave), not only colour.
- Tooltip text respects system font scaling.
