import { parseAtlas, requireClips } from "./sprites/atlas";
import atlasJson from "./sprites/placeholder/atlas.json";
import atlasUrl from "./sprites/placeholder/atlas.png";
import { HitRegionSync } from "./overlay/hitSync";
import { RenderLoop } from "./overlay/loop";
import { setHitRegions } from "./overlay/native";
import { drawScene } from "./overlay/renderer";
import { Scene, STATE_CLIPS, Viewport } from "./overlay/scene";
import { subscribeSessions } from "./overlay/sessions";

/** CSS pixels per sprite pixel until the settings window exists (#28). */
const BUDDY_SCALE = 2;

const inTauri = "__TAURI_INTERNALS__" in window;

async function start(): Promise<void> {
  const canvas = document.getElementById("overlay") as HTMLCanvasElement;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("no 2D canvas");

  // Placeholder art until #19; fails loudly if an animation is missing.
  const atlas = parseAtlas(atlasJson);
  requireClips(atlas, Object.values(STATE_CLIPS).map((s) => s.clip));
  const image = new Image();
  image.src = atlasUrl;
  await image.decode();

  const scene = new Scene(atlas);
  // In the browser (`pnpm dev:mock`) there is no backend to tell.
  const sync = new HitRegionSync(inTauri ? setHitRegions : async () => {});

  function viewport(): Viewport {
    const dpr = window.devicePixelRatio || 1;
    const width = Math.round(canvas.clientWidth * dpr);
    const height = Math.round(canvas.clientHeight * dpr);
    if (canvas.width !== width || canvas.height !== height) {
      canvas.width = width;
      canvas.height = height;
    }
    return { width, height, devicePixelRatio: dpr, scale: BUDDY_SCALE };
  }

  const loop = new RenderLoop((now) => {
    // Hidden overlay: draw nothing and sleep until it is shown again.
    if (document.hidden) return null;
    const frame = scene.render(now, viewport());
    drawScene(ctx, image, frame);
    sync.update(frame.regions);
    return frame.nextChangeAt;
  });

  await subscribeSessions({
    snapshot: (views) => {
      scene.setSessions(views, performance.now());
      loop.invalidate();
    },
    update: (view) => {
      scene.update(view, performance.now());
      loop.invalidate();
    },
    remove: (id) => {
      scene.remove(id);
      loop.invalidate();
    },
  });
  window.addEventListener("resize", () => loop.invalidate());
  document.addEventListener("visibilitychange", () => loop.invalidate());
  loop.invalidate();
}

start().catch((error) => console.error("claude-buddy overlay:", error));
