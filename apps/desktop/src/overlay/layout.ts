/**
 * Group layout (docs/animation-spec.md#layout): buddies stand on an
 * invisible floor along the edge of the chosen corner, oldest next to the
 * corner and newest at the outer end, a 4 px gap apart. At most 10 are
 * shown; the rest collapse into a `+N` chip that expands on hover. A buddy
 * waiting for the user steps 4 px forward, away from the screen edge.
 *
 * Positions are given as an offset along the floor plus a "lift" away from
 * it, so they do not depend on the canvas size: resizing the window never
 * makes anyone walk. `toCanvas` turns them into canvas coordinates.
 *
 * Pure and unit-agnostic: callers pass sizes in device pixels.
 */

import type { Corner } from "./native";

export const MAX_VISIBLE = 10;

export interface LayoutInput {
  /** Session ids in creation order (oldest first). */
  ids: readonly string[];
  waiting: ReadonlySet<string>;
  /** Buddy (and chip) size. */
  size: number;
  /** Space between neighbours. */
  gap: number;
  /** How far a waiting buddy steps forward. */
  step: number;
  /** Show everyone, e.g. while the pointer is on the chip. */
  expanded: boolean;
  maxVisible?: number;
}

export interface FloorPosition {
  /** Distance from the corner along the floor, to the buddy's near edge. */
  offset: number;
  /** Distance away from the floor (towards the screen centre). */
  lift: number;
}

export interface Layout {
  buddies: Map<string, FloorPosition>;
  /** The `+N` chip, when some buddies are hidden. */
  chip: (FloorPosition & { hidden: number }) | null;
  /** Space the group needs, along the floor and away from it. */
  extent: { along: number; across: number };
}

export function layout(input: LayoutInput): Layout {
  const { ids, waiting, size, gap, step, expanded } = input;
  const max = input.maxVisible ?? MAX_VISIBLE;
  const overflow = !expanded && ids.length > max;
  const visible = overflow ? ids.slice(0, max) : ids;
  const slot = (i: number) => i * (size + gap);

  const buddies = new Map<string, FloorPosition>();
  visible.forEach((id, i) => {
    buddies.set(id, { offset: slot(i), lift: waiting.has(id) ? step : 0 });
  });
  const chip = overflow
    ? { offset: slot(visible.length), lift: 0, hidden: ids.length - visible.length }
    : null;
  const slots = visible.length + (chip ? 1 : 0);
  return {
    buddies,
    chip,
    extent: {
      along: slots === 0 ? 0 : slots * size + (slots - 1) * gap,
      across: size + step,
    },
  };
}

/** Top-left canvas coordinates of an item at `pos`, for a canvas anchored to `corner`. */
export function toCanvas(
  pos: FloorPosition,
  corner: Corner,
  canvas: { width: number; height: number },
  size: number,
): { x: number; y: number } {
  const right = corner.endsWith("right");
  const bottom = corner.startsWith("bottom");
  return {
    x: right ? canvas.width - size - pos.offset : pos.offset,
    y: bottom ? canvas.height - size - pos.lift : pos.lift,
  };
}
