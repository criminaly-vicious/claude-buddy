import { parseAtlas, requireClips } from "./sprites/atlas";
import atlasJson from "./sprites/placeholder/atlas.json";
import atlasUrl from "./sprites/placeholder/atlas.png";
import { HitRegionSync } from "./overlay/hitSync";
import { RenderLoop } from "./overlay/loop";
import { flightArea, homeArea } from "./overlay/expand";
import { cardLines, HoverState, placeCard, Rect } from "./overlay/hoverCard";
import {
  expandOverlay,
  ExpandedLayout,
  focusSession,
  getOverlayAnchor,
  HitRegion,
  onOverlayAnchor,
  onOverlayPointer,
  onOverlayVisible,
  restoreOverlay,
  setHitRegions,
  setOverlayContentSize,
} from "./overlay/native";
import { drawCard, drawScene, measureCard } from "./overlay/renderer";
import { CONTROLLER_CLIPS } from "./overlay/controller";
import { Scene, Viewport } from "./overlay/scene";
import { subscribeSessions } from "./overlay/sessions";

/** CSS pixels per sprite pixel until the settings window exists (#28). */
const BUDDY_SCALE = 2;
/** The `+N` group collapses this long after the pointer leaves it. */
const COLLAPSE_AFTER_MS = 1500;
/** Space between a buddy and its hover card, in CSS pixels. */
const CARD_GAP = 6;
/** A press that moves further than this (CSS pixels) is a drag, not a click. */
const DRAG_THRESHOLD = 4;

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

  /** The overlay as the backend last expanded it, for throws (#24). */
  let expanded: ExpandedLayout | null = null;
  /** Asked the backend to expand and not yet to restore. */
  let expandRequested = false;
  const canvasSize = () => ({ width: canvas.clientWidth, height: canvas.clientHeight });

  function viewport(): Viewport {
    const dpr = window.devicePixelRatio || 1;
    const width = Math.round(canvas.clientWidth * dpr);
    const height = Math.round(canvas.clientHeight * dpr);
    if (canvas.width !== width || canvas.height !== height) {
      canvas.width = width;
      canvas.height = height;
    }
    const home = homeArea(canvasSize(), expanded);
    return {
      width,
      height,
      devicePixelRatio: dpr,
      scale: BUDDY_SCALE,
      home: {
        x: Math.round(home.x * dpr),
        y: Math.round(home.y * dpr),
        width: Math.round(home.width * dpr),
        height: Math.round(home.height * dpr),
      },
    };
  }

  /** A canvas point (CSS px) relative to the home area, where the scene's physics lives. */
  function toHome(x: number, y: number): { x: number; y: number } {
    const home = homeArea(canvasSize(), expanded);
    return { x: x - home.x, y: y - home.y };
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

  const hover = new HoverState();
  /** The open card's rectangle in canvas pixels, for pointer tests. */
  let cardRect: Rect | null = null;
  let sprites: { id: string; x: number; y: number; size: number }[] = [];

  /** Hidden from the tray: nothing is drawn and the loop sleeps. */
  let hiddenByUser = false;

  const loop = new RenderLoop((now) => {
    // Hidden overlay: draw nothing and sleep until it is shown again.
    if (document.hidden || hiddenByUser) return null;
    const vp = viewport();
    const dpr = vp.devicePixelRatio;
    const frame = scene.render(now, vp);
    sprites = frame.sprites;
    drawScene(ctx, image, frame);

    const regions: HitRegion[] = [...frame.regions];
    const content = { ...frame.contentSize };
    let next = frame.nextChangeAt;
    cardRect = null;
    const openId = hover.openId(now);
    const sprite = openId && !scene.isThrown(openId) && frame.sprites.find((s) => s.id === openId);
    const view = openId ? scene.view(openId) : undefined;
    if (sprite && view) {
      const lines = cardLines(view, Date.now());
      const size = measureCard(ctx, lines, dpr);
      const buddy = { x: sprite.x, y: sprite.y, width: sprite.size, height: sprite.size };
      cardRect = placeCard(buddy, size, vp, scene.currentCorner, CARD_GAP * dpr);
      drawCard(ctx, cardRect, lines, dpr);
      regions.push({
        x: cardRect.x / dpr,
        y: cardRect.y / dpr,
        width: cardRect.width / dpr,
        height: cardRect.height / dpr,
      });
      // Make room for the card above (or below) the group.
      content.width = Math.max(content.width, size.width / dpr);
      content.height += CARD_GAP + size.height / dpr;
    }
    // The elapsed time is wall-clock based; express its start on this clock.
    const since = view ? view.since - (Date.now() - now) : null;
    const hoverNext = hover.nextChangeAt(now, since);
    if (hoverNext !== null) next = next === null ? hoverNext : Math.min(next, hoverNext);

    sync.update(regions);
    fitWindow(content);
    // Everyone is back in their slot: shrink to the corner again.
    if (expandRequested && !scene.busy) {
      expandRequested = false;
      restoreOverlay().catch(() => {});
    }
    return next;
  });

  /** What the pointer is over: a buddy id, the card, or nothing. */
  function pointerTarget(x: number, y: number): string | null {
    const dpr = window.devicePixelRatio || 1;
    const [px, py] = [x * dpr, y * dpr];
    const inside = (r: { x: number; y: number; width: number; height: number }) =>
      px >= r.x && py >= r.y && px < r.x + r.width && py < r.y + r.height;
    if (cardRect && inside(cardRect)) return "card";
    const sprite = sprites.find((s) => inside({ x: s.x, y: s.y, width: s.size, height: s.size }));
    return sprite?.id ?? null;
  }

  // Hovering the +N chip shows everyone; leaving the group collapses it again.
  let collapseTimer: ReturnType<typeof setTimeout> | null = null;

  // Press on a buddy: a click if it stays put, a drag once it moves.
  let press: { id: string; x: number; y: number } | null = null;
  let dragging = false;
  canvas.addEventListener("pointerdown", (event) => {
    const id = pointerTarget(event.offsetX, event.offsetY);
    if (event.button !== 0 || id === null || id === "card") return;
    press = { id, x: event.offsetX, y: event.offsetY };
    canvas.setPointerCapture(event.pointerId);
  });

  function startDrag(id: string, from: { x: number; y: number }, now: number): void {
    if (!scene.grab(id, toHome(from.x, from.y), now)) return;
    dragging = true;
    hover.pointer(null, now);
    scene.setWorkArea(flightArea(canvasSize(), null));
    if (!inTauri) return; // The browser page is the work area already.
    expandRequested = true;
    expandOverlay()
      .then((layout) => {
        if (!layout) return;
        expanded = layout;
        scene.setWorkArea(flightArea(canvasSize(), layout));
        loop.invalidate();
      })
      .catch(() => {}); // Not expanded: it just flies inside the corner area.
  }

  canvas.addEventListener("pointermove", (event) => {
    const now = performance.now();
    if (press && !dragging) {
      const moved = Math.hypot(event.offsetX - press.x, event.offsetY - press.y);
      if (moved > DRAG_THRESHOLD) startDrag(press.id, press, now);
    }
    if (dragging) {
      scene.drag(toHome(event.offsetX, event.offsetY), now);
      loop.invalidate();
      return;
    }
    hover.pointer(pointerTarget(event.offsetX, event.offsetY), now);
    loop.invalidate();
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

  // Let go: throw what was dragged, or treat it as a click.
  const endPress = (event: PointerEvent, cancelled: boolean) => {
    const pressed = press;
    const wasDragging = dragging;
    press = null;
    dragging = false;
    if (wasDragging) {
      scene.release(performance.now());
      loop.invalidate();
    } else if (pressed && !cancelled) {
      click(pressed.id, event);
    }
  };
  canvas.addEventListener("pointerup", (event) => endPress(event, false));
  canvas.addEventListener("pointercancel", (event) => endPress(event, true));

  // Click on a buddy: focus its terminal; it hops when that worked. The
  // browser mock has no terminals, so it always hops.
  function click(id: string, event: PointerEvent): void {
    if (pointerTarget(event.offsetX, event.offsetY) !== id) return;
    const focused = inTauri ? focusSession(id) : Promise.resolve(true);
    focused
      .then((ok) => {
        if (!ok) return;
        scene.hop(id, performance.now());
        loop.invalidate();
      })
      .catch(() => {}); // Best effort: nothing to show when focus fails.
  }

  // In the browser the page sees the pointer leave; in the app the overlay
  // turns click-through first, so the backend says when it left.
  const pointerGone = () => {
    hover.pointer(null, performance.now());
    loop.invalidate();
  };
  canvas.addEventListener("pointerleave", pointerGone);

  if (inTauri) {
    await onOverlayPointer((inside) => {
      if (!inside) pointerGone();
    });
    await onOverlayVisible((visible) => {
      hiddenByUser = !visible;
      if (!visible) {
        hover.pointer(null, performance.now());
        ctx.clearRect(0, 0, canvas.width, canvas.height);
      }
      loop.invalidate();
    });
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
