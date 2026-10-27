/**
 * What is on screen: one buddy per session, the frame its animation
 * controller shows now, where it stands, and its hit region. Pure (time is
 * passed in), so it is tested without a canvas. Group layout rules (stable
 * slots, overflow) come with #21.
 */

import { Atlas, Frame } from "../sprites/atlas";
import type { SessionView } from "../types";
import { AnimationController, ControllerOptions } from "./controller";
import type { HitRegion } from "./native";

/** Sprite frame size in the atlas, in sprite pixels. */
export const SPRITE_SIZE = 32;
/** Gap between buddies, in sprite pixels (spec: 4 px). */
const GAP = 4;

export interface Viewport {
  /** Canvas size in device pixels. */
  width: number;
  height: number;
  /** Device pixels per CSS pixel. */
  devicePixelRatio: number;
  /** Buddy scale in CSS pixels per sprite pixel (settings: 1x/2x/3x). */
  scale: number;
}

export interface PlacedSprite {
  id: string;
  frame: Frame;
  /** Top-left corner and size on the canvas, in whole device pixels. */
  x: number;
  y: number;
  size: number;
}

export interface SceneFrame {
  sprites: PlacedSprite[];
  regions: HitRegion[];
  /** When the picture next changes (same clock as `now`), or `null` if static. */
  nextChangeAt: number | null;
}

interface Buddy {
  view: SessionView;
  animation: AnimationController;
}

export class Scene {
  /** Insertion order is slot order: newest at the outer edge. */
  private buddies = new Map<string, Buddy>();

  constructor(
    private readonly atlas: Atlas,
    private readonly options: Omit<ControllerOptions, "clips"> = {},
  ) {}

  get size(): number {
    return this.buddies.size;
  }

  /** Replaces all sessions (`sessions:snapshot`, mock feed). */
  setSessions(views: SessionView[], now: number): void {
    const ids = new Set(views.map((v) => v.id));
    for (const id of [...this.buddies.keys()]) {
      if (!ids.has(id)) this.buddies.delete(id);
    }
    for (const view of views) this.update(view, now);
  }

  /** One session changed (`sessions:update`). */
  update(view: SessionView, now: number): void {
    const existing = this.buddies.get(view.id);
    if (existing) {
      existing.view = view;
      existing.animation.setState(view.state, now);
      return;
    }
    this.buddies.set(view.id, {
      view,
      animation: new AnimationController(view.state, now, {
        ...this.options,
        clips: this.atlas.clips,
      }),
    });
  }

  /** A session is gone (`sessions:remove`). */
  remove(id: string): void {
    this.buddies.delete(id);
  }

  render(now: number, viewport: Viewport): SceneFrame {
    // Whole device pixels per sprite pixel, so pixel art stays crisp.
    const pixel = Math.max(1, Math.round(viewport.scale * viewport.devicePixelRatio));
    const size = SPRITE_SIZE * pixel;
    const gap = GAP * pixel;

    const sprites: PlacedSprite[] = [];
    const regions: HitRegion[] = [];
    let nextChangeAt: number | null = null;
    let slot = 0;
    for (const [id, buddy] of this.buddies) {
      const { frame, nextChangeAt: next } = buddy.animation.current(now);
      if (next !== null) {
        nextChangeAt = nextChangeAt === null ? next : Math.min(nextChangeAt, next);
      }
      // Bottom-right, newest at the outer edge, as in the default corner.
      const x = viewport.width - size - slot * (size + gap);
      const y = viewport.height - size;
      slot++;
      if (x + size <= 0) continue; // Overflow handling is #21.
      sprites.push({ id, frame, x, y, size });
      const css = viewport.devicePixelRatio;
      regions.push({ x: x / css, y: y / css, width: size / css, height: size / css });
    }
    return { sprites, regions, nextChangeAt };
  }
}
