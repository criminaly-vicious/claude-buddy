import type { BuddyState } from "../types";
import type { Rect } from "./hoverCard";
import type { PlacedChip, SceneFrame } from "./scene";

/** Draws one scene frame: nearest-neighbour, whole device pixels only. */
export function drawScene(
  ctx: CanvasRenderingContext2D,
  image: CanvasImageSource,
  scene: SceneFrame,
): void {
  ctx.imageSmoothingEnabled = false;
  ctx.clearRect(0, 0, ctx.canvas.width, ctx.canvas.height);
  for (const { frame, x, y, size, hop } of scene.sprites) {
    ctx.drawImage(image, frame.x, frame.y, frame.w, frame.h, x, y + hop, size, size);
  }
  // Speech bubbles go over every buddy, so a visitor never hides one.
  for (const { frame, x, y, size } of scene.effects) {
    ctx.drawImage(image, frame.x, frame.y, frame.w, frame.h, x, y, size, size);
  }
  if (scene.chip) drawChip(ctx, scene.chip);
}

/** The `+N` chip: a dark rounded tile with the count. */
function drawChip(ctx: CanvasRenderingContext2D, chip: PlacedChip): void {
  const { x, y, size, hidden } = chip;
  const inset = Math.round(size * 0.15);
  ctx.fillStyle = "rgba(30, 40, 55, 0.85)";
  ctx.beginPath();
  ctx.roundRect(x + inset, y + inset, size - 2 * inset, size - 2 * inset, Math.round(size * 0.12));
  ctx.fill();
  ctx.fillStyle = "#ffffff";
  ctx.font = `bold ${Math.round(size * 0.3)}px system-ui, sans-serif`;
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.fillText(`+${hidden}`, x + size / 2, y + size / 2);
}

/** Balloon text style and size limits, in CSS pixels. */
const FONT_PX = [12, 11, 11];
const LINE_PX = 16;
const PAD_X = 10;
const PAD_Y = 7;
/** The state dot before the first line, and the space it takes. */
const DOT_PX = 6;
const DOT_SPACE_PX = 11;
/** Widest balloon; longer lines end with "…". */
export const BALLOON_MAX_WIDTH = 280;
/** Tallest balloon (3 lines), plus its tail: the room the overlay keeps for it. */
export const BALLOON_MAX_HEIGHT = 3 * LINE_PX + 2 * PAD_Y;
/** Rows of the tail below (or above) the balloon, in sprite pixels. */
export const TAIL_ROWS = 2;

const COLOURS = {
  fill: "#FFFDF8",
  outline: "#1F1411",
  shadow: "rgba(0, 0, 0, 0.18)",
  text: ["#1F1411", "#6B5E57", "#9B8F88"],
};

export const STATE_COLOURS: Record<BuddyState, string> = {
  spawning: "#C9A227",
  idle: "#3FA66B",
  working: "#D97757",
  researching: "#4C8DF6",
  waiting: "#E5484D",
  sleeping: "#8F7FD9",
  leaving: "#9CA3AF",
};

function font(ctx: CanvasRenderingContext2D, line: number, dpr: number): void {
  ctx.font = `${line === 0 ? "600 " : ""}${(FONT_PX[line] ?? 11) * dpr}px system-ui, "Segoe UI", sans-serif`;
}

/** `text` cut with "…" so it fits in `width` device pixels. */
function fit(ctx: CanvasRenderingContext2D, text: string, width: number): string {
  if (ctx.measureText(text).width <= width) return text;
  const chars = Array.from(text);
  let lo = 0;
  let hi = chars.length;
  while (lo < hi) {
    const mid = Math.ceil((lo + hi) / 2);
    if (ctx.measureText(chars.slice(0, mid).join("") + "…").width <= width) lo = mid;
    else hi = mid - 1;
  }
  return chars.slice(0, lo).join("").trimEnd() + "…";
}

export interface MeasuredBalloon {
  width: number;
  height: number;
  /** The lines as they fit. */
  lines: string[];
}

/**
 * Size of the balloon for `lines`, in device pixels, snapped to whole
 * sprite pixels (`px`) so its pixel-art border lines up.
 */
export function measureBalloon(
  ctx: CanvasRenderingContext2D,
  lines: string[],
  dpr: number,
  px: number,
): MeasuredBalloon {
  const maxText = (BALLOON_MAX_WIDTH - 2 * PAD_X) * dpr;
  let widest = 0;
  const fitted = lines.map((line, i) => {
    font(ctx, i, dpr);
    const indent = i === 0 ? DOT_SPACE_PX * dpr : 0;
    const text = fit(ctx, line, maxText - indent);
    widest = Math.max(widest, ctx.measureText(text).width + indent);
    return text;
  });
  const snap = (v: number) => Math.ceil(v / px) * px;
  return {
    width: snap(widest + 2 * PAD_X * dpr),
    height: snap((fitted.length * LINE_PX + 2 * PAD_Y) * dpr),
    lines: fitted,
  };
}

/**
 * Draws the hover balloon: a white speech balloon with a dark pixel outline,
 * cut corners, a soft shadow and a tail towards the buddy at `tail.x`
 * (below the balloon, or above it when `tail.up`). `px` is one sprite pixel
 * in device pixels.
 */
export function drawBalloon(
  ctx: CanvasRenderingContext2D,
  rect: Rect,
  lines: string[],
  state: BuddyState,
  dpr: number,
  px: number,
  tail: { x: number; up: boolean },
): void {
  const { x, y, width: w, height: h } = rect;
  const block = (bx: number, by: number, bw: number, bh: number, colour: string) => {
    ctx.fillStyle = colour;
    ctx.fillRect(bx, by, bw, bh);
  };

  // Shadow, one pixel down and right.
  block(x + 2 * px, y + h, w - 2 * px, px, COLOURS.shadow);
  block(x + w, y + 2 * px, px, h - 2 * px, COLOURS.shadow);
  // Body and outline with cut corners.
  block(x + px, y + px, w - 2 * px, h - 2 * px, COLOURS.fill);
  block(x + px, y, w - 2 * px, px, COLOURS.outline);
  block(x + px, y + h - px, w - 2 * px, px, COLOURS.outline);
  block(x, y + px, px, h - 2 * px, COLOURS.outline);
  block(x + w - px, y + px, px, h - 2 * px, COLOURS.outline);

  // The tail, three pixels wide where it leaves the balloon.
  const tx = Math.min(Math.max(Math.round(tail.x / px) * px, x + 3 * px), x + w - 4 * px);
  if (tail.up) {
    block(tx - px, y, 3 * px, px, COLOURS.fill);
    block(tx - px, y - px, px, px, COLOURS.outline);
    block(tx, y - px, px, px, COLOURS.fill);
    block(tx + px, y - px, px, px, COLOURS.outline);
    block(tx, y - 2 * px, px, px, COLOURS.outline);
  } else {
    block(tx - px, y + h - px, 3 * px, px, COLOURS.fill);
    block(tx - px, y + h, px, px, COLOURS.outline);
    block(tx, y + h, px, px, COLOURS.fill);
    block(tx + px, y + h, px, px, COLOURS.outline);
    block(tx, y + h + px, px, px, COLOURS.outline);
  }

  // The state dot, then the text.
  const lineTop = (i: number) => y + (PAD_Y + i * LINE_PX) * dpr;
  const dot = Math.max(px, Math.round((DOT_PX * dpr) / px) * px);
  block(
    x + PAD_X * dpr,
    Math.round(lineTop(0) + ((LINE_PX - DOT_PX) / 2) * dpr),
    dot,
    dot,
    STATE_COLOURS[state],
  );
  ctx.textAlign = "left";
  ctx.textBaseline = "middle";
  lines.forEach((line, i) => {
    font(ctx, i, dpr);
    ctx.fillStyle = COLOURS.text[i] ?? COLOURS.text[2];
    const indent = i === 0 ? DOT_SPACE_PX * dpr : 0;
    ctx.fillText(line, x + PAD_X * dpr + indent, lineTop(i) + (LINE_PX / 2) * dpr);
  });
}
