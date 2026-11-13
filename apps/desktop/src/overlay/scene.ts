/**
 * What is on screen: one buddy per session, the frame its animation
 * controller shows now, where it stands in the group layout, the `+N` chip,
 * the speech bubbles over it, and the hit regions. Buddies whose slot moves
 * walk there (`walk_left` / `walk_right`); buddies on an excursion (thrown, strolling,
 * visiting another buddy) are drawn wherever the excursion puts them.
 * Pure (time and randomness are passed in), so it is tested without a canvas.
 */

import { Atlas, Clip, Frame, frameAt } from "../sprites/atlas";
import type { SessionView, SubagentView } from "../types";
import { AnimationController, ControllerOptions, phaseOf } from "./controller";
import { Excursion, Room, Stroll, Visit } from "./excursion";
import { Carrier, MINI_BOX, MiniClips, miniKey, MiniStack, parseMiniKey, stackRise } from "./minis";
import { FloorPosition, layout, toCanvas } from "./layout";
import type { Corner, HitRegion } from "./native";
import { Bounds, Point, Throw, ThrowClips } from "./throw";

/** Sprite frame size in the atlas, in sprite pixels. */
export const SPRITE_SIZE = 32;
/**
 * The part of a 32×32 frame that is Clawd (arms up included), in sprite
 * pixels: hit regions and the hover balloon use it, so the empty air above
 * him stays click-through.
 */
export const BODY_BOX = { x: 2, y: 10, width: 28, height: 21 };
/** Gap between buddies and the waiting step, in sprite pixels (spec: 4 px). */
const GAP = 4;
const STEP = 4;
/** Walking speed back to a slot, in sprite pixels per second. */
const WALK_SPEED = 64;
/** Strolls and visits are a little slower, in sprite pixels per second. */
const STROLL_SPEED = 40;
/** Minis walking back after a throw, on their little legs, in sprite pixels per second. */
const MINI_WALK_SPEED = 48;
/** Redraw interval while something flies or hops (the loop caps at 30 fps anyway). */
const MOVE_FRAME_MS = 1000 / 30;
/** Redraw interval while buddies walk: 20 fps is plenty for pixel art at walking pace. */
const WALK_FRAME_MS = 1000 / 20;
/** The happy hop after a click (#23): how long and how high, in sprite pixels. */
export const HOP_MS = 320;
const HOP_HEIGHT = 6;
/** How long two buddies talk on a visit, and a lone speech bubble lasts. */
export const TALK_MS = 2600;
/** The other buddy answers this long after the talking starts. */
const REPLY_AFTER_MS = 900;

/** When idle buddies go for a stroll, in ms: the first one, then the next ones. */
export interface StrollTiming {
  first: [number, number];
  every: [number, number];
  /** How far they walk, in sprite pixels. */
  distance: [number, number];
  /** How long they look around before walking back. */
  pause: [number, number];
  /** Quiet time after any stroll before the next one, whoever goes. */
  gap: [number, number];
}

export const DEFAULT_STROLLS: StrollTiming = {
  first: [30_000, 90_000],
  every: [60_000, 180_000],
  distance: [40, 120],
  pause: [1_200, 2_800],
  gap: [45_000, 90_000],
};

export interface SceneOptions extends Omit<ControllerOptions, "clips"> {
  strolls?: StrollTiming;
}

export interface Viewport {
  /** Canvas size in device pixels. */
  width: number;
  height: number;
  /** Device pixels per CSS pixel. */
  devicePixelRatio: number;
  /** Buddy scale in CSS pixels per sprite pixel (settings: 1x/2x/3x). */
  scale: number;
  /**
   * Where the group's corner area lies on the canvas, in device pixels.
   * The whole canvas unless the overlay is expanded.
   */
  home?: { x: number; y: number; width: number; height: number };
}

/** An area in CSS pixels relative to the home area's top-left corner. */
export interface Area {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export interface PlacedSprite {
  id: string;
  frame: Frame;
  /** Top-left corner and size on the canvas, in whole device pixels. */
  x: number;
  y: number;
  size: number;
  /**
   * Vertical drawing offset of a hop, in whole device pixels. `x`, `y` and
   * the hit region stay at the buddy's place, so the balloon does not bounce.
   */
  hop: number;
  /** Set for a mini Clawd (a subagent): its buddy and whether it is on the stack. */
  mini?: { parent: string; view: SubagentView; stacked: boolean };
}

/** A bubble drawn over a buddy (same cell as its sprite), with no hit region. */
export interface PlacedEffect {
  id: string;
  frame: Frame;
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
  effects: PlacedEffect[];
  chip: PlacedChip | null;
  regions: HitRegion[];
  /** Space the group needs, in CSS pixels, so the window can fit it. */
  contentSize: { width: number; height: number };
  /** When the picture next changes (same clock as `now`), or `null` if static. */
  nextChangeAt: number | null;
  /** Device pixels per sprite pixel. */
  pixel: number;
}

interface Walk {
  from: number;
  to: number;
  startedAt: number;
}

/** A bubble shown over a buddy for a while. */
interface Effect {
  clip: Clip;
  from: number;
  until: number;
}

interface Buddy {
  view: SessionView;
  animation: AnimationController;
  /** Floor offset in sprite pixels while standing still; `null` until placed. */
  offset: number | null;
  walk: Walk | null;
  /** When the last hop started. */
  hopAt: number | null;
  /** When this buddy may go for its next stroll; `null` until scheduled. */
  strollAt: number | null;
  /** Its subagents, as mini Clawds on its head (#84). */
  minis: MiniStack;
}

export { phaseOf };

/** A throw seen as an excursion; the scene still drags and releases it. */
class Thrown implements Excursion {
  readonly room: Room = "work";
  constructor(readonly flight: Throw) {}
  update(now: number, slot: Point) {
    const shown = this.flight.update(now, slot);
    return { x: shown.x, y: shown.y, frame: shown.frame, done: shown.phase === "done" };
  }
}

/** A visit and who it is for. */
interface Visiting {
  visit: Visit;
  to: string;
  replied: boolean;
}

export class Scene {
  /** Insertion order is creation order: oldest next to the corner. */
  private buddies = new Map<string, Buddy>();
  private corner: Corner = "bottom-right";
  private expanded = false;
  private chipRegion: HitRegion | null = null;
  /** When the slots last changed, so walks start then, not at the next render. */
  private layoutChangedAt = 0;
  private readonly throwClips: ThrowClips;
  private readonly clips: {
    walkLeft: Clip;
    walkRight: Clip;
    look: Clip;
    talk: Clip;
    standUp: Clip;
    fxTalk: Clip;
    fxReply: Clip;
  };
  /** Buddies away from their slot (thrown, strolling, visiting), until they are back. */
  private away = new Map<string, Excursion>();
  private visits = new Map<string, Visiting>();
  private effects = new Map<string, Effect[]>();
  private grabbed: string | null = null;
  private hovered: string | null = null;
  /** Where buddies may go: the expanded overlay, relative to home (CSS px). */
  private workArea: Area | null = null;
  /** Last drawn place of each buddy (home-relative CSS px). */
  private placed = new Map<string, Point>();
  /** Last slot of each buddy (home-relative CSS px). */
  private slots = new Map<string, Point>();
  private spriteCss = SPRITE_SIZE;
  private scaleCss = 1;
  private homeCss = { width: 0, height: 0 };
  private readonly random: () => number;
  private readonly strolls: StrollTiming;
  /** No stroll starts before this, so a big group is not always on the move. */
  private nextStrollAnyAt = 0;
  private readonly miniClips: MiniClips;

  constructor(
    private readonly atlas: Atlas,
    private options: SceneOptions = {},
  ) {
    const clip = (name: string) => {
      const found = atlas.clips.get(name);
      if (!found) throw new Error(`atlas has no "${name}" animation`);
      return found;
    };
    this.throwClips = {
      grabbed: clip("grabbed"),
      fly: clip("fly"),
      land: clip("land"),
      land_splat: clip("land_splat"),
      dizzy: clip("dizzy"),
      walkLeft: clip("walk_left"),
      walkRight: clip("walk_right"),
    };
    this.clips = {
      walkLeft: clip("walk_left"),
      walkRight: clip("walk_right"),
      look: clip("idle_look"),
      talk: clip("talk"),
      standUp: clip("stand_up"),
      fxTalk: clip("fx_talk"),
      fxReply: clip("fx_reply"),
    };
    this.miniClips = {
      work: clip("mini_work"),
      balance: clip("mini_balance"),
      pop: clip("mini_pop"),
      poof: clip("mini_poof"),
      fly: clip("mini_fly"),
      land: clip("mini_land"),
      jump: clip("mini_jump"),
      walkLeft: clip("mini_walk_left"),
      walkRight: clip("mini_walk_right"),
    };
    this.random = options.random ?? Math.random;
    this.strolls = options.strolls ?? DEFAULT_STROLLS;
  }

  /** Settings: random idle extras and strolls off or on, for every buddy now and later. */
  setReduceMotion(on: boolean, now: number): void {
    this.options = { ...this.options, reduceMotion: on };
    for (const buddy of this.buddies.values()) buddy.animation.setReduceMotion(on, now);
  }

  /** Whether any buddy (or mini) is held, flying or on its way back from a throw. */
  get busy(): boolean {
    return [...this.away.values()].some((e) => e instanceof Thrown) || this.minisLoose();
  }

  private minisLoose(): boolean {
    return [...this.buddies.values()].some((b) => b.minis.loose);
  }

  /**
   * How much the overlay must grow: the work area while someone is thrown,
   * a floor strip while someone strolls, nothing otherwise.
   */
  roomNeeded(): "work" | "floor" | null {
    // Scattered minis may be anywhere a throw can reach.
    if (this.minisLoose()) return "work";
    let need: "work" | "floor" | null = null;
    for (const e of this.away.values()) {
      if (e.room === "work") return "work";
      if (e.room === "floor") need = "floor";
    }
    return need;
  }

  isThrown(id: string): boolean {
    return this.away.get(id) instanceof Thrown;
  }

  /** The buddy under the pointer (or under a mini on it): it never wanders off while you look at it. */
  setHovered(id: string | null): void {
    this.hovered = id === null ? null : (parseMiniKey(id)?.parent ?? id);
  }

  /** The area buddies may go (the expanded overlay), home-relative CSS px. */
  setWorkArea(area: Area): void {
    this.workArea = area;
    for (const e of this.away.values()) {
      if (e instanceof Thrown) e.flight.setBounds(this.bounds());
    }
    for (const buddy of this.buddies.values()) buddy.minis.setBounds(this.bounds());
  }

  /**
   * Picks up buddy `id` at `pointer` (home-relative CSS px). Works on a
   * buddy in mid-air or on a walk too. Returns whether there was a buddy to grab.
   */
  grab(id: string, pointer: Point, now: number): boolean {
    const at = this.placed.get(id);
    if (!at || !this.buddies.has(id)) return false;
    this.visits.delete(id);
    this.away.set(
      id,
      new Thrown(
        new Throw({ x: at.x, y: at.y }, pointer, now, {
          clips: this.throwClips,
          bounds: this.bounds(),
          walkSpeed: WALK_SPEED * this.scaleCss,
        }),
      ),
    );
    this.grabbed = id;
    return true;
  }

  /** The held buddy follows the pointer (home-relative CSS px). */
  drag(pointer: Point, now: number): void {
    const held = this.grabbed ? this.away.get(this.grabbed) : undefined;
    if (held instanceof Thrown) held.flight.drag(pointer, now);
  }

  /** Lets go of the held buddy; the minis on its head fly off, scattering. */
  release(now: number): void {
    const held = this.grabbed ? this.away.get(this.grabbed) : undefined;
    const buddy = this.grabbed ? this.buddies.get(this.grabbed) : undefined;
    if (held instanceof Thrown) {
      held.flight.release(now);
      buddy?.minis.scatter(
        held.flight.velocity,
        now,
        this.bounds(),
        this.scaleCss,
        MINI_WALK_SPEED * this.scaleCss,
      );
    }
    this.grabbed = null;
  }

  /**
   * Session `from` messaged session `to` (`null`: someone the app could not
   * tell). Its buddy walks over and they talk; when that cannot be shown
   * (someone is away, hidden, or reduce motion is on), a speech bubble over
   * the sender (and an answer over the recipient) says it instead.
   */
  interact(from: string, to: string | null, now: number): void {
    const sender = this.buddies.get(from);
    if (!sender) return;
    const recipient = to !== null && to !== from ? this.buddies.get(to) : undefined;
    const canWalk =
      recipient !== undefined &&
      to !== null &&
      !this.options.reduceMotion &&
      !this.away.has(from) &&
      !this.away.has(to) &&
      this.placed.has(from) &&
      this.slots.has(from) &&
      this.slots.has(to) &&
      sender.view.state !== "leaving" &&
      recipient.view.state !== "leaving";
    if (!canWalk) {
      this.addEffect(from, this.clips.fxTalk, now, now + TALK_MS);
      if (recipient && to !== null) {
        this.addEffect(to, this.clips.fxReply, now + REPLY_AFTER_MS, now + TALK_MS);
      }
      return;
    }
    const sitting = sender.animation.posture(now) === "sitting";
    const visit = new Visit(this.placed.get(from) as Point, now, {
      clips: { ...this.clips, standUp: sitting ? this.clips.standUp : null },
      speed: STROLL_SPEED * this.scaleCss,
      talkMs: TALK_MS,
    });
    this.away.set(from, visit);
    this.visits.set(from, { visit, to, replied: false });
  }

  /** Where a sprite's corner may go: the work area minus the sprite (or the home area). */
  private bounds(): Bounds {
    const area = this.workArea ?? {
      left: 0,
      top: 0,
      right: this.homeCss.width,
      bottom: this.homeCss.height,
    };
    return {
      left: area.left,
      top: area.top,
      right: Math.max(area.left, area.right - this.spriteCss),
      bottom: Math.max(area.top, area.bottom - this.spriteCss),
    };
  }

  get size(): number {
    return this.buddies.size;
  }

  /** The session behind a buddy, for the hover balloon. */
  view(id: string): SessionView | undefined {
    return this.buddies.get(id)?.view;
  }

  /** The subagent behind a mini ({@link miniKey}) and its session, for its balloon. */
  miniView(key: string): { parent: SessionView; subagent: SubagentView } | undefined {
    const ref = parseMiniKey(key);
    const parent = ref ? this.buddies.get(ref.parent)?.view : undefined;
    const subagent = parent?.subagents?.find((s) => s.id === ref?.id);
    return parent && subagent ? { parent, subagent } : undefined;
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
      existing.minis.sync(view.subagents ?? [], now, this.flying(view.id));
      // Called away while strolling: walk back to the desk now.
      const out = this.away.get(view.id);
      if (view.state !== "idle" && out instanceof Stroll) out.recall(now);
      return;
    }
    this.buddies.set(view.id, {
      view,
      animation: new AnimationController(view.state, now, {
        ...this.options,
        clips: this.atlas.clips,
        phase: phaseOf(view.id),
      }),
      offset: null,
      walk: null,
      hopAt: null,
      strollAt: null,
      minis: new MiniStack({ clips: this.miniClips, random: this.random }),
    });
    this.buddies.get(view.id)?.minis.sync(view.subagents ?? [], now);
  }

  /** Thrown and let go: in the air, landing or walking back. */
  private flying(id: string): boolean {
    const away = this.away.get(id);
    return away instanceof Thrown && away.flight.phase !== "grabbed";
  }

  /** The buddy does a small happy hop (its terminal was focused). */
  hop(id: string, now: number): void {
    const buddy = this.buddies.get(id);
    if (buddy) buddy.hopAt = now;
  }

  /** A session is gone (`sessions:remove`); the buddies behind it walk up. */
  remove(id: string, now: number): void {
    if (this.buddies.delete(id)) this.layoutChangedAt = now;
    this.away.delete(id);
    this.visits.delete(id);
    this.effects.delete(id);
    this.placed.delete(id);
    this.slots.delete(id);
    if (this.grabbed === id) this.grabbed = null;
    if (this.hovered === id) this.hovered = null;
  }

  /** Shows `clip` over buddy `id` from `from` until `until`; it replaces a bubble it would overlap. */
  private addEffect(id: string, clip: Clip, from: number, until: number): void {
    const list = (this.effects.get(id) ?? []).filter((e) => e.until <= from || e.from >= until);
    list.push({ clip, from, until });
    this.effects.set(id, list);
  }

  render(now: number, viewport: Viewport): SceneFrame {
    // Whole device pixels per sprite pixel, so pixel art stays crisp.
    const pixel = Math.max(1, Math.round(viewport.scale * viewport.devicePixelRatio));
    const size = SPRITE_SIZE * pixel;
    const css = viewport.devicePixelRatio;
    const home = viewport.home ?? { x: 0, y: 0, width: viewport.width, height: viewport.height };
    this.spriteCss = size / css;
    this.scaleCss = pixel / css;
    this.homeCss = { width: home.width / css, height: home.height / css };

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
    const place = (pos: FloorPosition) => {
      const at = toCanvas(
        { offset: Math.round(pos.offset * pixel), lift: pos.lift * pixel },
        this.corner,
        home,
        size,
      );
      return { x: at.x + home.x, y: at.y + home.y };
    };
    const toHome = (p: Point): Point => ({ x: (p.x - home.x) / css, y: (p.y - home.y) / css });
    const toCanvasPoint = (p: Point): Point => ({
      x: Math.round(home.x + p.x * css),
      y: Math.round(home.y + p.y * css),
    });
    const boxRegion = (box: typeof BODY_BOX, x: number, y: number): HitRegion => ({
      x: (x + box.x * pixel) / css,
      y: (y + box.y * pixel) / css,
      width: (box.width * pixel) / css,
      height: (box.height * pixel) / css,
    });
    const region = (x: number, y: number) => boxRegion(BODY_BOX, x, y);
    const outward = this.corner.endsWith("right") ? -1 : 1;

    const sprites: PlacedSprite[] = [];
    // Minis go over every buddy, so one walking back is never hidden.
    const miniSprites: PlacedSprite[] = [];
    const effects: PlacedEffect[] = [];
    const regions: HitRegion[] = [];
    let nextChangeAt: number | null = null;
    const changeAt = (t: number | null) => {
      if (t !== null) nextChangeAt = nextChangeAt === null ? t : Math.min(nextChangeAt, t);
    };

    const drawMinis = (id: string, buddy: Buddy, carrier: Carrier | null) => {
      const { placed, nextChangeAt: next } = buddy.minis.update(now, carrier);
      changeAt(next);
      for (const mini of placed) {
        const { x, y } = toCanvasPoint(mini.at);
        miniSprites.push({
          id: miniKey(id, mini.view.id),
          frame: mini.frame,
          x,
          y,
          size,
          hop: 0,
          mini: { parent: id, view: mini.view, stacked: mini.stacked },
        });
        regions.push(boxRegion(MINI_BOX, x, y));
      }
    };

    // Slots first: visits walk to the slot of the buddy they visit.
    for (const [id] of this.buddies) {
      const target = plan.buddies.get(id);
      if (target) this.slots.set(id, toHome(place(target)));
      else this.slots.delete(id);
    }

    for (const [id, buddy] of this.buddies) {
      const target = plan.buddies.get(id);
      const slot = this.slots.get(id) ?? this.placed.get(id) ?? { x: 0, y: 0 };
      const away = this.away.get(id);
      if (away) {
        // Away: logically it already holds its slot.
        if (target) {
          buddy.offset = target.offset;
          buddy.walk = null;
        }
        const shown = away.update(now, slot, this.partnerSpot(id));
        if (shown.done) {
          this.away.delete(id);
          this.visits.delete(id);
          buddy.animation.restart(now);
          // A stroll already booked the next one; after anything else, start over.
          if (away instanceof Stroll) this.nextStrollAnyAt = now + this.between(this.strolls.gap);
          else buddy.strollAt = null;
        } else {
          this.noteReply(id, now);
          const { x, y } = toCanvasPoint(shown);
          sprites.push({ id, frame: shown.frame, x, y, size, hop: 0 });
          regions.push(region(x, y));
          this.placed.set(id, { x: shown.x, y: shown.y });
          this.pushEffects(id, x, y, size, now, effects, changeAt);
          // Held, the minis cling on; let go, they are already off.
          drawMinis(id, buddy, {
            at: toHome({ x, y }),
            frame: shown.frame,
            slot,
            outward,
            home: false,
            moving: true,
            scale: this.scaleCss,
          });
          changeAt(now + (away instanceof Thrown ? MOVE_FRAME_MS : WALK_FRAME_MS));
          continue;
        }
      }
      if (!target) {
        // Hidden behind the chip: placed afresh (no walk) when it reappears.
        buddy.offset = null;
        buddy.walk = null;
        drawMinis(id, buddy, null);
        continue;
      }
      const offset = this.moveTowards(buddy, target.offset, now);
      const walking = buddy.walk !== null;
      let frame: Frame;
      if (walking) {
        // Facing the way it goes: offsets grow away from the corner.
        const walk = buddy.walk as Walk;
        const outward = this.corner.endsWith("right") ? -1 : 1;
        const left = Math.sign(walk.to - walk.from) * outward < 0;
        const clip = left ? this.clips.walkLeft : this.clips.walkRight;
        frame = frameAt(clip, now - walk.startedAt, true).frame;
        changeAt(now + WALK_FRAME_MS);
      } else {
        const showing = buddy.animation.current(now);
        frame = showing.frame;
        changeAt(showing.nextChangeAt);
        changeAt(this.considerStroll(id, buddy, showing.clip, slot, now));
      }
      const { x, y } = place({ offset, lift: target.lift });
      const lift = this.hopLift(buddy, now);
      if (lift > 0) changeAt(now + MOVE_FRAME_MS);
      // Away from the floor: up in bottom corners, down in top ones.
      const hop = lift === 0 ? 0 : (this.corner.startsWith("bottom") ? -lift : lift) * pixel;
      sprites.push({ id, frame, x, y, size, hop });
      this.placed.set(id, toHome({ x, y }));
      regions.push(region(x, y));
      this.pushEffects(id, x, y + hop, size, now, effects, changeAt);
      drawMinis(id, buddy, {
        at: toHome({ x, y: y + hop }),
        frame,
        slot,
        outward,
        home: !walking,
        moving: walking,
        scale: this.scaleCss,
      });
    }
    sprites.push(...miniSprites);

    let chip: PlacedChip | null = null;
    this.chipRegion = null;
    if (plan.chip) {
      const { x, y } = place(plan.chip);
      chip = { hidden: plan.chip.hidden, x, y, size };
      this.chipRegion = { x: x / css, y: y / css, width: size / css, height: size / css };
      regions.push(this.chipRegion);
    }

    // Room above the group for the tallest stack of minis.
    let rise = 0;
    for (const [id, buddy] of this.buddies) {
      if (plan.buddies.has(id)) rise = Math.max(rise, stackRise(buddy.minis.size));
    }
    return {
      sprites,
      effects,
      chip,
      regions,
      contentSize: {
        width: plan.extent.along * viewport.scale,
        height: (plan.extent.across + rise) * viewport.scale,
      },
      nextChangeAt,
      pixel,
    };
  }

  /** Where a visiting buddy stands: beside the buddy it visits, on its own side. */
  private partnerSpot(id: string): Point | undefined {
    const visiting = this.visits.get(id);
    if (!visiting) return undefined;
    const them = this.placed.get(visiting.to) ?? this.slots.get(visiting.to);
    const mine = this.slots.get(id);
    if (!them || !mine || !this.buddies.has(visiting.to)) return undefined;
    const side = mine.x < them.x ? -1 : 1;
    return { x: them.x + side * this.spriteCss * 0.85, y: them.y };
  }

  /** Once a visit's talk starts, the visited buddy answers a moment later. */
  private noteReply(id: string, now: number): void {
    const visiting = this.visits.get(id);
    const since = visiting?.visit.talkingSince ?? null;
    if (!visiting || visiting.replied || since === null || since > now) return;
    visiting.replied = true;
    this.addEffect(visiting.to, this.clips.fxReply, since + REPLY_AFTER_MS, since + TALK_MS);
  }

  /** Adds the bubbles showing over buddy `id` now, and when they change. */
  private pushEffects(
    id: string,
    x: number,
    y: number,
    size: number,
    now: number,
    out: PlacedEffect[],
    changeAt: (t: number | null) => void,
  ): void {
    const list = this.effects.get(id);
    if (!list) return;
    const live = list.filter((e) => e.until > now);
    if (live.length === 0) {
      this.effects.delete(id);
      return;
    }
    this.effects.set(id, live);
    for (const effect of live) {
      if (effect.from > now) {
        changeAt(effect.from);
        continue;
      }
      const { frame, nextChangeAt } = frameAt(effect.clip, now - effect.from, true);
      out.push({ id, frame, x, y, size });
      changeAt(nextChangeAt === null ? null : effect.from + nextChangeAt);
      changeAt(effect.until);
    }
  }

  /**
   * Starts a stroll for an idle buddy when its time comes, one buddy at a
   * time and never under the pointer. Returns when to check again.
   */
  private considerStroll(
    id: string,
    buddy: Buddy,
    clip: string,
    slot: Point,
    now: number,
  ): number | null {
    if (this.options.reduceMotion || buddy.view.state !== "idle") {
      buddy.strollAt = null;
      return null;
    }
    if (buddy.strollAt === null) {
      buddy.strollAt = now + this.between(this.strolls.first);
      return buddy.strollAt;
    }
    if (now < buddy.strollAt) return buddy.strollAt;
    if (now < this.nextStrollAnyAt) {
      // Someone strolled a moment ago: wait for the quiet time to pass.
      buddy.strollAt = this.nextStrollAnyAt + this.between([0, 10_000]);
      return buddy.strollAt;
    }
    const someoneOut = this.away.size > 0;
    if (someoneOut || clip !== "idle" || this.hovered === id || buddy.walk !== null) {
      // Not now: try again a little later.
      buddy.strollAt = now + this.between([4_000, 9_000]);
      return buddy.strollAt;
    }
    const direction = this.corner.endsWith("right") ? -1 : 1;
    const distance = this.between(this.strolls.distance) * this.scaleCss;
    let x = slot.x + direction * distance;
    if (this.workArea) {
      x = Math.min(Math.max(x, this.workArea.left), this.workArea.right - this.spriteCss);
    }
    this.away.set(
      id,
      new Stroll(slot, now, {
        clips: { walkLeft: this.clips.walkLeft, walkRight: this.clips.walkRight, look: this.clips.look },
        speed: STROLL_SPEED * this.scaleCss,
        turnAt: { x, y: slot.y },
        pauseMs: this.between(this.strolls.pause),
      }),
    );
    buddy.strollAt = now + this.between(this.strolls.every);
    return now + WALK_FRAME_MS;
  }

  private between([min, max]: [number, number]): number {
    return min + this.random() * (max - min);
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

  /** Height of the current hop in whole sprite pixels (a parabola), 0 when done. */
  private hopLift(buddy: Buddy, now: number): number {
    if (buddy.hopAt === null) return 0;
    const t = (now - buddy.hopAt) / HOP_MS;
    if (t < 0 || t >= 1) {
      if (t >= 1) buddy.hopAt = null;
      return 0;
    }
    return Math.round(HOP_HEIGHT * 4 * t * (1 - t));
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
