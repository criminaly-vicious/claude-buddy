/**
 * While a buddy is dragged or thrown the overlay grows to the work area
 * (`overlay_expand`) and shrinks back afterwards. The window resizes a
 * little after it is asked to, so what the canvas shows is decided by its
 * actual size: at the expanded size the group is drawn in the layout's home
 * area, otherwise the canvas is the home area. Pure, for testing.
 */

import type { ExpandedLayout } from "./native";
import type { Area } from "./scene";

export interface Size {
  width: number;
  height: number;
}

export interface CssRect extends Size {
  x: number;
  y: number;
}

/** Window sizes are whole device pixels; allow for rounding. */
const TOLERANCE = 2;

function isExpanded(canvas: Size, layout: ExpandedLayout | null): layout is ExpandedLayout {
  return (
    layout !== null &&
    Math.abs(canvas.width - layout.width) <= TOLERANCE &&
    Math.abs(canvas.height - layout.height) <= TOLERANCE
  );
}

/** Where the corner area is on a canvas of `canvas` CSS size. */
export function homeArea(canvas: Size, layout: ExpandedLayout | null): CssRect {
  if (isExpanded(canvas, layout)) return { ...layout.home };
  return { x: 0, y: 0, width: canvas.width, height: canvas.height };
}

/**
 * Where thrown buddies may fly, relative to the home area: the work area
 * once known (it is the expanded window), else the canvas.
 */
export function flightArea(canvas: Size, layout: ExpandedLayout | null): Area {
  if (layout === null) return { left: 0, top: 0, right: canvas.width, bottom: canvas.height };
  const { home } = layout;
  return { left: -home.x, top: -home.y, right: layout.width - home.x, bottom: layout.height - home.y };
}
