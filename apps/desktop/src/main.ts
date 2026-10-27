import { parseAtlas, requireClips } from "./sprites/atlas";
import atlasJson from "./sprites/placeholder/atlas.json";
import atlasUrl from "./sprites/placeholder/atlas.png";
import { HitRegionSync } from "./overlay/hitSync";
import { RenderLoop } from "./overlay/loop";
import {
  getOverlayAnchor,
  onOverlayAnchor,
  setHitRegions,
  setOverlayContentSize,
} from "./overlay/native";
import { drawScene } from "./overlay/renderer";
import { CONTROLLER_CLIPS } from "./overlay/controller";
import { Scene, Viewport } from "./overlay/scene";
import { subscribeSessions } from "./overlay/sessions";

/** CSS pixels per sprite pixel until the settings window exists (#28). */
const BUDDY_SCALE = 2;
/** The `+N` group collapses this long after the pointer leaves it. */
const COLLAPSE_AFTER_MS = 1500;

const inTauri = "__TAURI_INTERNALS__" in window;

async function start(): Promise<void> {
  const canvas = document.getElementById("overlay") as HTMLCanvasElement;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("no 2D canvas");

  // Placeholder art until #19; fails loudly if an animation is missing.
  const atlas = parseAtlas(atlasJson);
  requireClips(atlas, CONTROLLER_CLIPS);
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

  // The window follows the group's size; only real changes are sent.
  let lastContentSize = "";
  function fitWindow(size: { width: number; height: number }): void {
    const key = `${Math.ceil(size.width)}x${Math.ceil(size.height)}`;
    if (!inTauri || key === lastContentSize) return;
    lastContentSize = key;
    setOverlayContentSize(Math.ceil(size.width), Math.ceil(size.height)).catch(() => {
      lastContentSize = "";
    });
  }

  const loop = new RenderLoop((now) => {
    // Hidden overlay: draw nothing and sleep until it is shown again.
    if (document.hidden) return null;
    const frame = scene.render(now, viewport());
    drawScene(ctx, image, frame);
    sync.update(frame.regions);
    fitWindow(frame.contentSize);
    return frame.nextChangeAt;
  });

  // Hovering the +N chip shows everyone; leaving the group collapses it again.
  let collapseTimer: ReturnType<typeof setTimeout> | null = null;
  canvas.addEventListener("pointermove", (event) => {
    if (collapseTimer !== null) clearTimeout(collapseTimer);
    if (scene.chipContains(event.offsetX, event.offsetY)) {
      scene.setExpanded(true);
      loop.invalidate();
    }
    collapseTimer = setTimeout(() => {
      scene.setExpanded(false);
      loop.invalidate();
    }, COLLAPSE_AFTER_MS);
  });

  if (inTauri) {
    scene.setCorner((await getOverlayAnchor()).corner);
    await onOverlayAnchor((anchor) => {
      scene.setCorner(anchor.corner);
      loop.invalidate();
    });
  }

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
      scene.remove(id, performance.now());
      loop.invalidate();
    },
  });
  window.addEventListener("resize", () => loop.invalidate());
  document.addEventListener("visibilitychange", () => loop.invalidate());
  loop.invalidate();
}

start().catch((error) => console.error("claude-buddy overlay:", error));
