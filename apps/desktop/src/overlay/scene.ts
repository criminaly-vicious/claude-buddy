/**
 * What is on screen: one buddy per session, the frame its animation
 * controller shows now, where it stands in the group layout, the `+N` chip,
 * and the hit regions. Buddies whose slot moves walk there (`walk` clip).
 * Pure (time is passed in), so it is tested without a canvas.
 */

import { Atlas, Clip, Frame, frameAt } from "../sprites/atlas";
import type { SessionView } from "../types";
import { AnimationController, ControllerOptions } from "./controller";
import { FloorPosition, layout, toCanvas } from "./layout";
import type { Corner, HitRegion } from "./native";

/** Sprite frame size in the atlas, in sprite pixels. */
export const SPRITE_SIZE = 32;
/** Gap between buddies and the waiting step, in sprite pixels (spec: 4 px). */
const GAP = 4;
const STEP = 4;
/** Walking speed, in sprite pixels per second. */
const WALK_SPEED = 64;
/** Redraw interval while something moves (the loop caps at 30 fps anyway). */
const MOVE_FRAME_MS = 1000 / 30;

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

export interface PlacedChip {
  hidden: number;
  x: number;
  y: number;
  size: number;
}

export interface SceneFrame {
  sprites: PlacedSprite[];
  chip: PlacedChip | null;
  regions: HitRegion[];
  /** Space the group needs, in CSS pixels, so the window can fit it. */
  contentSize: { width: number; height: number };
  /** When the picture next changes (same clock as `now`), or `null` if static. */
  nextChangeAt: number | null;
}

interface Walk {
  from: number;
  to: number;
  startedAt: number;
}

interface Buddy {
  view: SessionView;
  animation: AnimationController;
  /** Floor offset in sprite pixels while standing still; `null` until placed. */
  offset: number | null;
  walk: Walk | null;
}

export class Scene {
  /** Insertion order is creation order: oldest next to the corner. */
  private buddies = new Map<string, Buddy>();
  private corner: Corner = "bottom-right";
  private expanded = false;
  private chipRegion: HitRegion | null = null;
  /** When the slots last changed, so walks start then, not at the next render. */
  private layoutChangedAt = 0;
  private readonly walkClip: Clip;

  constructor(
    private readonly atlas: Atlas,
    private readonly options: Omit<ControllerOptions, "clips"> = {},
  ) {
    const walk = atlas.clips.get("walk");
    if (!walk) throw new Error('atlas has no "walk" animation');
    this.walkClip = walk;
  }

  get size(): number {
    return this.buddies.size;
  }

  /** The session behind a buddy, for the hover card. */
  view(id: string): SessionView | undefined {
    return this.buddies.get(id)?.view;
  }

  get currentCorner(): Corner {
    return this.corner;
  }

  setCorner(corner: Corner): void {
    this.corner = corner;
  }

  /** Show every buddy instead of the `+N` chip (pointer on the chip). */
  setExpanded(expanded: boolean): void {
    this.expanded = expanded;
  }

  /** Whether a CSS-pixel point is on the `+N` chip (as last rendered). */
  chipContains(x: number, y: number): boolean {
    const r = this.chipRegion;
    return r !== null && x >= r.x && y >= r.y && x < r.x + r.width && y < r.y + r.height;
  }

  /** Replaces all sessions (`sessions:snapshot`, mock feed). */
  setSessions(views: SessionView[], now: number): void {
    const ids = new Set(views.map((v) => v.id));
    for (const id of [...this.buddies.keys()]) {
      if (!ids.has(id)) this.remove(id, now);
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
      offset: null,
      walk: null,
    });
  }

  /** A session is gone (`sessions:remove`); the buddies behind it walk up. */
  remove(id: string, now: number): void {
    if (this.buddies.delete(id)) this.layoutChangedAt = now;
  }

  render(now: number, viewport: Viewport): SceneFrame {
    // Whole device pixels per sprite pixel, so pixel art stays crisp.
    const pixel = Math.max(1, Math.round(viewport.scale * viewport.devicePixelRatio));
    const size = SPRITE_SIZE * pixel;
    const css = viewport.devicePixelRatio;

    const waiting = new Set(
      [...this.buddies].filter(([, b]) => b.view.state === "waiting").map(([id]) => id),
    );
    // Laid out in sprite pixels, so a DPI or scale change never starts a walk.
    const plan = layout({
      ids: [...this.buddies.keys()],
      waiting,
      size: SPRITE_SIZE,
      gap: GAP,
      step: STEP,
      expanded: this.expanded,
    });
    // Rounded: mid-walk offsets are fractional, and pixel art must stay on whole pixels.
    const place = (pos: FloorPosition) =>
      toCanvas(
        { offset: Math.round(pos.offset * pixel), lift: pos.lift * pixel },
        this.corner,
        viewport,
        size,
      );

    const sprites: PlacedSprite[] = [];
    const regions: HitRegion[] = [];
    let nextChangeAt: number | null = null;
    const changeAt = (t: number | null) => {
      if (t !== null) nextChangeAt = nextChangeAt === null ? t : Math.min(nextChangeAt, t);
    };

    for (const [id, buddy] of this.buddies) {
      const target = plan.buddies.get(id);
      if (!target) {
        // Hidden behind the chip: placed afresh (no walk) when it reappears.
        buddy.offset = null;
        buddy.walk = null;
        continue;
      }
      const offset = this.moveTowards(buddy, target.offset, now);
      const walking = buddy.walk !== null;
      let frame: Frame;
      if (walking) {
        const walk = buddy.walk as Walk;
        frame = frameAt(this.walkClip, now - walk.startedAt, true).frame;
        changeAt(now + MOVE_FRAME_MS);
      } else {
        const showing = buddy.animation.current(now);
        frame = showing.frame;
        changeAt(showing.nextChangeAt);
      }
      const { x, y } = place({ offset, lift: target.lift });
      sprites.push({ id, frame, x, y, size });
      regions.push({ x: x / css, y: y / css, width: size / css, height: size / css });
    }

    let chip: PlacedChip | null = null;
    this.chipRegion = null;
    if (plan.chip) {
      const { x, y } = place(plan.chip);
      chip = { hidden: plan.chip.hidden, x, y, size };
      this.chipRegion = { x: x / css, y: y / css, width: size / css, height: size / css };
      regions.push(this.chipRegion);
    }

    return {
      sprites,
      chip,
      regions,
      contentSize: {
        width: plan.extent.along * viewport.scale,
        height: plan.extent.across * viewport.scale,
      },
      nextChangeAt,
    };
  }

  /**
   * Floor offset of `buddy` now, in sprite pixels, starting or finishing a
   * walk towards `target`. A position is a function of time, so the result
   * does not depend on how often this runs.
   */
  private moveTowards(buddy: Buddy, target: number, now: number): number {
    if (buddy.offset === null && buddy.walk === null) {
      // New or reappearing: stand in the slot straight away.
      buddy.offset = target;
      return target;
    }
    if (buddy.walk === null ? buddy.offset !== target : buddy.walk.to !== target) {
      // The slot changed (when the layout did): walk there from where the
      // buddy was at that moment, mid-walk or standing.
      const startedAt = Math.min(Math.max(this.layoutChangedAt, buddy.walk?.startedAt ?? 0), now);
      buddy.walk = { from: this.offsetAt(buddy, startedAt), to: target, startedAt };
    }
    const position = this.offsetAt(buddy, now);
    if (buddy.walk && position === buddy.walk.to) {
      buddy.offset = buddy.walk.to;
      buddy.walk = null;
    }
    return position;
  }

  private offsetAt(buddy: Buddy, now: number): number {
    const walk = buddy.walk;
    if (!walk) return buddy.offset ?? 0;
    const travelled = (WALK_SPEED * Math.max(0, now - walk.startedAt)) / 1000;
    const distance = Math.abs(walk.to - walk.from);
    if (travelled >= distance) return walk.to;
    return walk.from + Math.sign(walk.to - walk.from) * travelled;
  }
}
