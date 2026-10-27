import type { SceneFrame } from "./scene";

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
}
