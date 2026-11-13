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

/** Card text style, in CSS pixels. */
const CARD_FONT_PX = 12;
const CARD_LINE_PX = 16;
const CARD_PADDING_PX = 8;

function cardFont(ctx: CanvasRenderingContext2D, dpr: number, bold: boolean): void {
  ctx.font = `${bold ? "600 " : ""}${CARD_FONT_PX * dpr}px system-ui, sans-serif`;
}

/** Size of the hover card for `lines`, in device pixels. */
export function measureCard(
  ctx: CanvasRenderingContext2D,
  lines: string[],
  dpr: number,
): { width: number; height: number } {
  let widest = 0;
  lines.forEach((line, i) => {
    cardFont(ctx, dpr, i === 0);
    widest = Math.max(widest, ctx.measureText(line).width);
  });
  return {
    width: Math.ceil(widest + 2 * CARD_PADDING_PX * dpr),
    height: Math.ceil((lines.length * CARD_LINE_PX + 2 * CARD_PADDING_PX) * dpr),
  };
}

/** Draws the hover card: a dark rounded panel, first line bold. */
export function drawCard(
  ctx: CanvasRenderingContext2D,
  rect: Rect,
  lines: string[],
  dpr: number,
): void {
  ctx.fillStyle = "rgba(24, 28, 36, 0.92)";
  ctx.beginPath();
  ctx.roundRect(rect.x, rect.y, rect.width, rect.height, 6 * dpr);
  ctx.fill();
  ctx.textAlign = "left";
  ctx.textBaseline = "top";
  lines.forEach((line, i) => {
    cardFont(ctx, dpr, i === 0);
    ctx.fillStyle = i === 0 ? "#ffffff" : "#c9d1d9";
    const y = rect.y + (CARD_PADDING_PX + i * CARD_LINE_PX + 2) * dpr;
    ctx.fillText(line, rect.x + CARD_PADDING_PX * dpr, y);
  });
}
