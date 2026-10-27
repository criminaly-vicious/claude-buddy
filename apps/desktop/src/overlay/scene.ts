/**
 * What is on screen: one buddy per session, the clip it plays, the frame due
 * now, where it stands, and its hit region. Pure (time is passed in), so it
 * is tested without a canvas.
 *
 * The state → clip choice here is deliberately simple: one clip per state,
 * restarting on every state change. Transitions (sit down before working,
 * idle extras) come with the animation controller (#20); group layout rules
 * (stable slots, overflow) with #21.
 */

import { Atlas, Clip, Frame, frameAt } from "../sprites/atlas";
import type { BuddyState, SessionView } from "../types";
import type { HitRegion } from "./native";

/** Clip and looping per state, until the animation controller (#20). */
export const STATE_CLIPS: Record<BuddyState, { clip: string; loop: boolean }> = {
  spawning: { clip: "spawn", loop: false },
  idle: { clip: "idle", loop: true },
  working: { clip: "work_type", loop: true },
  researching: { clip: "research_read", loop: true },
  waiting: { clip: "wait_wave", loop: true },
  sleeping: { clip: "sleep", loop: true },
  leaving: { clip: "leave", loop: false },
};

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
  clip: Clip;
  loop: boolean;
  startedAt: number;
}

export class Scene {
  /** Insertion order is slot order: newest at the outer edge. */
  private buddies = new Map<string, Buddy>();

  constructor(private readonly atlas: Atlas) {}

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
    if (existing && existing.view.state === view.state) {
      existing.view = view;
      return;
    }
    const { clip, loop } = STATE_CLIPS[view.state];
    this.buddies.set(view.id, {
      view,
      clip: this.clip(clip),
      loop,
      startedAt: now,
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
      const { frame, nextChangeAt: next } = frameAt(buddy.clip, now - buddy.startedAt, buddy.loop);
      if (next !== null) {
        const at = buddy.startedAt + next;
        nextChangeAt = nextChangeAt === null ? at : Math.min(nextChangeAt, at);
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

  private clip(name: string): Clip {
    const clip = this.atlas.clips.get(name);
    if (!clip) throw new Error(`atlas has no "${name}" animation`);
    return clip;
  }
}
