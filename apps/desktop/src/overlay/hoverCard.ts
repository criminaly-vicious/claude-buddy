/**
 * Hover card (docs/animation-spec.md#hover-card-content-rules): what a
 * session is doing, shown 300 ms after the pointer rests on its buddy. At
 * most 3 lines of 80 characters. The text comes from the backend already
 * sanitised; it is cut again here as a second barrier.
 *
 * Pure: content, timing and placement are computed from inputs, so they are
 * tested without a canvas.
 */

import type { BuddyState, SessionView } from "../types";
import type { Corner } from "./native";

export const HOVER_DELAY_MS = 300;
export const MAX_LINES = 3;
export const MAX_LINE_CHARS = 80;

export const STATE_LABELS: Record<BuddyState, string> = {
  spawning: "Starting",
  idle: "Idle",
  working: "Working",
  researching: "Researching",
  waiting: "Waiting for you",
  sleeping: "Sleeping",
  leaving: "Leaving",
};

const APPROXIMATE_HINT = "Approximate: install hooks for exact states";

/** Cuts `text` to `max` characters (Unicode code points), ending with `…` when cut. */
export function clip(text: string, max = MAX_LINE_CHARS): string {
  // Control characters (newlines, escapes) never reach the card.
  const isControl = (ch: string) => ch.charCodeAt(0) < 0x20 || ch.charCodeAt(0) === 0x7f;
  const chars = Array.from(
    Array.from(text, (ch) => (isControl(ch) ? " " : ch))
      .join("")
      .trim(),
  );
  if (chars.length <= max) return chars.join("");
  return chars.slice(0, max - 1).join("") + "…";
}

/** `14s`, `2m 14s`, `1h 5m`. */
export function formatElapsed(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ${s % 60}s`;
  return `${Math.floor(m / 60)}h ${m % 60}m`;
}

/** The card's lines for `view` at wall-clock time `now` (epoch ms). */
export function cardLines(view: SessionView, now: number): string[] {
  const lines = [
    `${view.project || "session"} · ${STATE_LABELS[view.state]} · ${formatElapsed(now - view.since)}`,
  ];
  const activity = view.summary ?? view.tool;
  if (activity) lines.push(activity);
  if (view.source === "transcript") lines.push(APPROXIMATE_HINT);
  return lines.slice(0, MAX_LINES).map((line) => clip(line));
}

/**
 * Hover timing. The pointer is over a buddy (its id), over the card itself,
 * or nowhere. The card opens after the pointer rests on one buddy for
 * {@link HOVER_DELAY_MS}, stays open while the pointer is on the buddy or
 * the card, and closes as soon as it leaves both.
 */
export class HoverState {
  private target: string | null = null;
  private since = 0;
  private open: string | null = null;

  /** `over`: a buddy id, `"card"`, or `null` for nothing. */
  pointer(over: string | null, now: number): void {
    if (over === "card") {
      if (this.open !== null) return; // Reading the card keeps it open.
      over = null;
    }
    if (over !== this.target) {
      this.target = over;
      this.since = now;
      if (this.open !== over) this.open = null;
    }
  }

  /** The session whose card shows at `now`, if any. */
  openId(now: number): string | null {
    if (this.open === null && this.target !== null && now - this.since >= HOVER_DELAY_MS) {
      this.open = this.target;
    }
    return this.open;
  }

  /**
   * When the card next needs a redraw: the end of the hover delay, or the
   * next whole second of the elapsed time while open. `null` when idle.
   */
  nextChangeAt(now: number, since: number | null): number | null {
    if (this.openId(now) !== null && since !== null) {
      const elapsed = now - since;
      return now + (1000 - (((elapsed % 1000) + 1000) % 1000));
    }
    if (this.target !== null && this.open === null) return this.since + HOVER_DELAY_MS;
    return null;
  }
}

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/**
 * Where the card goes, in canvas pixels: on the side of the buddy away from
 * the screen edge (above it in bottom corners, below in top ones), aligned
 * with the buddy's outer side, and pushed back inside the canvas when it
 * would cross an edge.
 */
export function placeCard(
  buddy: Rect,
  card: { width: number; height: number },
  canvas: { width: number; height: number },
  corner: Corner,
  gap: number,
): Rect {
  const right = corner.endsWith("right");
  const bottom = corner.startsWith("bottom");
  // Extend towards the screen centre, away from the corner's side.
  let x = right ? buddy.x + buddy.width - card.width : buddy.x;
  x = Math.min(Math.max(0, x), Math.max(0, canvas.width - card.width));
  let y = bottom ? buddy.y - gap - card.height : buddy.y + buddy.height + gap;
  y = Math.min(Math.max(0, y), Math.max(0, canvas.height - card.height));
  return { x, y, width: card.width, height: card.height };
}
