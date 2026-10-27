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

Implemented in `apps/desktop/src/overlay/controller.ts`:
- Posture decides the transition: `sit_down`, `work_type` and `research_read` leave the buddy sitting, everything else standing.
- A new buddy always plays `spawn` first, even if it is first seen mid-turn.
- A state that arrives during a non-looping clip (transition, `spawn`, idle extra) is applied when that clip ends. Changing your mind mid-transition plays it out and comes back (e.g. `sit_down` then `stand_up`).
- `leave` plays once and holds its last frame until the session is removed.
- Idle extras (`idle_look`, `idle_stretch`, `idle_scratch`) start 8–20 s into each `idle` loop, picked at random. They are deadlines the render loop sleeps until, not timers. Reduce motion turns them off.
- Clips follow each other at exact times, so what shows depends only on the timestamps, not on how often the overlay redraws.

## Layout

- Buddies stand side by side on an invisible "floor" in the chosen corner, 4 px gap, newest at the outer edge.
- Max 10 visible; extras collapse into a `+N` chip that expands on hover.
- A buddy in `waiting` steps 4 px forward so it stands out.
- Slots are stable: a buddy keeps its slot until it leaves.

Implemented in `apps/desktop/src/overlay/layout.ts` and `scene.ts`:
- **Order:** creation order, oldest touching the corner, mirrored for left and top corners; "forward" means away from the screen edge.
- **Leaving:** when a buddy leaves, the ones behind it play `walk` and move up one slot at 64 sprite px/s. Positions are kept in sprite pixels along the floor, so resizing the window or changing DPI never makes anyone walk.
- **Overflow:** the 10 oldest stay visible and the newest go behind the chip. Hovering the chip shows everyone; the group collapses 1.5 s after the pointer leaves it. Hidden buddies reappear in place, without walking.
- **Window size:** the overlay window resizes around the group from its corner (`set_overlay_content_size`), instead of keeping a fixed size.

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

Implemented in `apps/desktop/src/overlay/hoverCard.ts`:
- **Lines:** `<project> · <state label> · <elapsed>`, then the tool summary (or the tool name), then "Approximate: install hooks for exact states" for transcript-only sessions.
- **Sanitising:** lines are cut to 80 Unicode code points with `…`, and control characters are replaced, a second barrier after the backend's sanitising.
- **Timing:** the card opens after 300 ms on one buddy and stays open while the pointer is on the buddy or the card. The elapsed time redraws once per second, only while the card is open.
- **Placement:** the card sits on the side away from the screen edge (above in bottom corners), aligned with the buddy's outer side, and stays inside the overlay. The window grows to make room for it. While open, the card is a hit region.
- **Leaving:** in the app, the backend's `overlay:pointer` event says when the pointer left every region, since the click-through window no longer sees it.

### Click

- A click on a buddy asks the backend to focus its session's terminal (docs/session-detection.md#window-focus-click-on-a-buddy). Clicks on the hover card do nothing.
- **Happy hop:** when the window came to the front, the buddy hops 6 sprite px away from the floor and back in 320 ms (a parabola, whole sprite pixels), over whatever clip it is playing, sitting or standing, so no extra art is needed. The hit region and the hover card stay in place during the hop. No hop when nothing was focused.
- In `pnpm dev:mock` there are no terminals, so every click hops.

## Accessibility & comfort

- Option to reduce motion (no random idle extras, no screen shake).
- `waiting` uses shape (bubble + wave), not only colour.
- Tooltip text respects system font scaling.
