import type { PlacedChip, SceneFrame } from "./scene";

/** Draws one scene frame: nearest-neighbour, whole device pixels only. */
export function drawScene(
  ctx: CanvasRenderingContext2D,
  image: CanvasImageSource,
  scene: SceneFrame,
): void {
  ctx.imageSmoothingEnabled = false;
  ctx.clearRect(0, 0, ctx.canvas.width, ctx.canvas.height);
  for (const { frame, x, y, size } of scene.sprites) {
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
