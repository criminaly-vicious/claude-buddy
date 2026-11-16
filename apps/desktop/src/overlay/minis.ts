/**
 * Mini Clawds (#84, docs/animation-spec.md#subagents): one per running
 * subagent, standing on its buddy's head, stacked in start order with the
 * oldest at the bottom. When the buddy is thrown they fly off and scatter.
 * Each lands, walks back to its place in a line beside the buddy, and
 * climbs on again once the buddy is home and the minis below it are back,
 * so the stack always comes back in the same order.
 *
 * Pure: time and randomness are passed in. Positions are sprite corners in
 * CSS px relative to the overlay's home area, as throws use; heights are in
 * sprite px.
 */

import { Clip, Frame, frameAt } from "../sprites/atlas";
import type { SubagentView } from "../types";
import { gridOf, phaseOf } from "./controller";
import { Bounds, Point, Throw, ThrowClips } from "./throw";

/** First empty row under the feet in a 32×32 frame. */
const FLOOR = 30;
/** A mini's head when its frame does not say (standing). */
const MINI_HEAD = 21;
/** The buddy's head when its frame does not say (standing). */
const BUDDY_HEAD = 14;
/** How much one mini adds to a stack, in sprite px. */
export const MINI_HEIGHT = FLOOR - MINI_HEAD;
/** Minis on one buddy at most; its balloon counts the rest. */
export const MAX_MINIS = 5;
/** The part of a 32×32 frame that is a mini (arms up included), for hit regions. */
export const MINI_BOX = { x: 9, y: 19, width: 14, height: 11 };
/** Hopping back on: how long it takes, and how high it goes above the higher end. */
export const CLIMB_MS = 360;
const CLIMB_HEIGHT = 8;
/** Dropping a level when a mini below it leaves. */
const DROP_MS = 140;
/** Where minis wait beside their buddy: the first one, then each next, sprite px. */
const QUEUE_FIRST = 20;
const QUEUE_GAP = 12;
/** A throw scatters them this much on top of the buddy's speed, CSS px/s at 2×. */
const SCATTER_SIDEWAYS = 260;
const SCATTER_UP: [number, number] = [350, 700];
/** Redraw interval while a mini moves. */
const MOVE_FRAME_MS = 1000 / 30;

export interface MiniClips {
  /** Standing on the stack, busy (the subagent works). */
  work: Clip;
  /** Riding a buddy that walks or is held: arms out. */
  balance: Clip;
  pop: Clip;
  poof: Clip;
  fly: Clip;
  land: Clip;
  jump: Clip;
  walkLeft: Clip;
  walkRight: Clip;
}

/** The buddy a stack stands on, as drawn now. */
export interface Carrier {
  /** Its sprite's corner (CSS px, home-relative), hop included. */
  at: Point;
  frame: Frame;
  /** Its slot (CSS px), where scattered minis line up to climb back on. */
  slot: Point;
  /** -1 or 1: along the floor away from the corner, where the line goes. */
  outward: number;
  /** In its slot (not thrown, strolling or visiting): minis may climb on. */
  home: boolean;
  /** Walking, strolling or held: the minis on it balance. */
  moving: boolean;
  /** CSS px per sprite px. */
  scale: number;
}

export interface PlacedMini {
  view: SubagentView;
  frame: Frame;
  /** Sprite corner, CSS px home-relative. */
  at: Point;
  /** Standing on the stack (not flying, walking, waiting or climbing). */
  stacked: boolean;
}

type Loose =
  | { kind: "flying"; flight: Throw }
  | { kind: "waiting" }
  | { kind: "climbing"; from: Point; at: number };

interface Mini {
  view: SubagentView;
  bornAt: number;
  /** When its subagent stopped: it vanishes in a puff, then goes. */
  goneAt: number | null;
  /** Off the stack, and doing what; `null` while it stands on the stack. */
  loose: Loose | null;
  /** Dropping a level after a mini below it left: since when, from how high. */
  drop: { at: number; by: number } | null;
  /** Where it was last drawn, for a throw to scatter it from. */
  at: Point | null;
}

export interface MiniOptions {
  clips: MiniClips;
  random: () => number;
}

const KEY_SEPARATOR = "\u001f";

/** The hover and hit id of a mini: its buddy's session id and its subagent id. */
export function miniKey(parent: string, id: string): string {
  return `mini${KEY_SEPARATOR}${parent}${KEY_SEPARATOR}${id}`;
}

/** The buddy and subagent behind a {@link miniKey}, or `null` for any other id. */
export function parseMiniKey(key: string): { parent: string; id: string } | null {
  const [tag, parent, id, ...rest] = key.split(KEY_SEPARATOR);
  return tag === "mini" && parent !== undefined && id !== undefined && rest.length === 0
    ? { parent, id }
    : null;
}

/** Extra room above a buddy's 32 px cell for `count` minis on its head, in sprite px. */
export function stackRise(count: number): number {
  // From the buddy's highest head (stretching, row 12) up to raised mini arms.
  return Math.max(0, Math.min(count, MAX_MINIS) * MINI_HEIGHT + 2 - 12);
}

export class MiniStack {
  private minis: Mini[] = [];

  constructor(private readonly options: MiniOptions) {}

  /**
   * The subagents running now, oldest first (`SessionView.subagents`). New
   * ones pop in on top; stopped ones vanish in a puff. `offStack`: the
   * buddy is flying, so a newcomer waits beside its slot instead.
   */
  sync(subagents: readonly SubagentView[], now: number, offStack = false): void {
    const running = new Set(subagents.map((s) => s.id));
    for (const mini of this.minis) {
      if (mini.goneAt === null && !running.has(mini.view.id)) mini.goneAt = now;
    }
    for (const view of subagents) {
      const mini = this.minis.find((m) => m.goneAt === null && m.view.id === view.id);
      if (mini) {
        mini.view = view;
        continue;
      }
      if (this.size >= MAX_MINIS) continue;
      // Behind a mini that is still off the stack, it waits its turn too.
      const behind = offStack || this.minis.some((m) => m.goneAt === null && m.loose !== null);
      this.minis.push({
        view,
        bornAt: now,
        goneAt: null,
        loose: behind ? { kind: "waiting" } : null,
        drop: null,
        at: null,
      });
    }
  }

  /** Minis shown, leaving ones aside. */
  get size(): number {
    return this.minis.filter((m) => m.goneAt === null).length;
  }

  /** Whether any mini is off the stack: flying, walking back, waiting or climbing. */
  get loose(): boolean {
    return this.minis.some((m) => m.loose !== null);
  }

  /**
   * The buddy was thrown with `velocity` (CSS px/s): the minis on it fly
   * off, each with a share of that speed and a random push up and sideways.
   * `scale` is CSS px per sprite px.
   */
  scatter(
    velocity: { vx: number; vy: number },
    now: number,
    bounds: Bounds,
    scale: number,
    walkSpeed: number,
  ): void {
    const { random } = this.options;
    // A mini already vanishing just goes.
    this.minis = this.minis.filter((m) => m.goneAt === null || m.loose !== null);
    for (const mini of this.minis) {
      if (mini.loose !== null || mini.at === null) continue;
      const k = scale / 2;
      const flight = new Throw(mini.at, mini.at, now, {
        clips: this.throwClips(),
        bounds,
        walkSpeed,
      });
      const [low, high] = SCATTER_UP;
      flight.fling(
        {
          vx: velocity.vx * 0.6 + (random() * 2 - 1) * SCATTER_SIDEWAYS * k,
          vy: Math.min(0, velocity.vy * 0.6) - (low + random() * (high - low)) * k,
        },
        now,
      );
      mini.loose = { kind: "flying", flight };
      mini.drop = null;
    }
  }

  /** The work area changed (the overlay finished growing). */
  setBounds(bounds: Bounds): void {
    for (const mini of this.minis) {
      if (mini.loose?.kind === "flying") mini.loose.flight.setBounds(bounds);
    }
  }

  /**
   * Where every mini is and what it shows at `now`, stacked ones first
   * (bottom up), and when the picture next changes. `carrier` is the buddy
   * as drawn now; `null` when it is not on screen (behind the `+N` chip).
   */
  update(now: number, carrier: Carrier | null): { placed: PlacedMini[]; nextChangeAt: number | null } {
    let next: number | null = null;
    const changeAt = (t: number | null) => {
      if (t !== null) next = next === null ? t : Math.min(next, t);
    };
    this.removeGone(now);
    const placed: PlacedMini[] = [];
    if (carrier === null) return { placed, nextChangeAt: null };

    // On the stack: each stands on the head below it.
    let oy = (carrier.frame.head ?? BUDDY_HEAD) - FLOOR;
    for (const mini of this.minis) {
      if (mini.loose !== null) continue;
      const shown = this.stackedFrame(mini, now, carrier.moving);
      changeAt(shown.nextChangeAt);
      let y = oy;
      if (mini.drop) {
        const t = (now - mini.drop.at) / DROP_MS;
        if (t >= 1) mini.drop = null;
        else {
          y -= mini.drop.by * (1 - Math.max(0, t) ** 2);
          changeAt(now + MOVE_FRAME_MS);
        }
      }
      const at = { x: carrier.at.x, y: carrier.at.y + Math.round(y) * carrier.scale };
      mini.at = at;
      placed.push({ view: mini.view, frame: shown.frame, at, stacked: true });
      oy += (shown.frame.head ?? MINI_HEAD) - FLOOR;
    }
    const top = { x: carrier.at.x, y: carrier.at.y + oy * carrier.scale };

    // Off the stack: in flight, walking back, waiting in line or climbing on.
    const climbing = this.minis.some((m) => m.loose?.kind === "climbing");
    // Each mini's place in the line is its place in the stack, so nobody
    // shifts along when the one before it climbs on.
    let line = -1;
    for (let i = 0; i < this.minis.length; i++) {
      const mini = this.minis[i];
      const loose = mini.loose;
      if (mini.goneAt === null) line++;
      if (loose === null) continue;
      if (mini.goneAt !== null) {
        if (mini.at) {
          const shown = oneShot(this.options.clips.poof, now - mini.goneAt);
          changeAt(shown.nextChangeAt === null ? null : mini.goneAt + shown.nextChangeAt);
          placed.push({ view: mini.view, frame: shown.frame, at: mini.at, stacked: false });
        }
        continue;
      }
      // Its place in the line beside the buddy.
      const spot = {
        x: carrier.slot.x + carrier.outward * (QUEUE_FIRST + QUEUE_GAP * line) * carrier.scale,
        y: carrier.slot.y,
      };
      if (loose.kind === "flying") {
        const shown = loose.flight.update(now, spot);
        if (shown.phase !== "done") {
          mini.at = { x: shown.x, y: shown.y };
          placed.push({ view: mini.view, frame: shown.frame, at: mini.at, stacked: false });
          changeAt(now + MOVE_FRAME_MS);
          continue;
        }
        mini.loose = { kind: "waiting" };
      }
      if (mini.loose?.kind === "waiting") {
        const below = this.minis.slice(0, i).every((m) => m.loose === null || m.goneAt !== null);
        if (carrier.home && below && !climbing) {
          mini.loose = { kind: "climbing", from: spot, at: now };
        } else {
          mini.at = spot;
          const shown = onGrid(this.options.clips.work, now, phaseOf(mini.view.id));
          changeAt(shown.nextChangeAt);
          placed.push({ view: mini.view, frame: shown.frame, at: spot, stacked: false });
          continue;
        }
      }
      const climb = mini.loose as Extract<Loose, { kind: "climbing" }>;
      const t = (now - climb.at) / CLIMB_MS;
      if (t >= 1) {
        // On again: from the next frame it stands on the stack.
        mini.loose = null;
        mini.at = top;
        placed.push({
          view: mini.view,
          frame: this.stackedFrame(mini, now, carrier.moving).frame,
          at: top,
          stacked: true,
        });
        changeAt(now + MOVE_FRAME_MS);
        continue;
      }
      // A hop: straight towards the top of the stack, plus an arc.
      const arc = CLIMB_HEIGHT * carrier.scale * 4 * t * (1 - t);
      mini.at = {
        x: climb.from.x + (top.x - climb.from.x) * t,
        y: climb.from.y + (top.y - climb.from.y) * t - arc,
      };
      placed.push({
        view: mini.view,
        frame: frameAt(this.options.clips.jump, 0, true).frame,
        at: mini.at,
        stacked: false,
      });
      changeAt(now + MOVE_FRAME_MS);
    }
    return { placed, nextChangeAt: next };
  }

  /** Drops minis whose puff is over; the ones stacked above them fall a level. */
  private removeGone(now: number): void {
    const poofMs = this.options.clips.poof.duration;
    for (let i = 0; i < this.minis.length; ) {
      const mini = this.minis[i];
      const goneAt = mini.goneAt === null ? null : mini.goneAt + poofMs;
      if (goneAt === null || now < goneAt) {
        i++;
        continue;
      }
      if (mini.loose === null) {
        for (const above of this.minis.slice(i + 1)) {
          if (above.loose === null) above.drop = { at: goneAt, by: MINI_HEIGHT };
        }
      }
      this.minis.splice(i, 1);
    }
  }

  /** What a mini on the stack shows: popping in, vanishing, or busy. */
  private stackedFrame(mini: Mini, now: number, moving: boolean): { frame: Frame; nextChangeAt: number | null } {
    const { pop, poof, work, balance } = this.options.clips;
    if (mini.goneAt !== null) {
      const shown = oneShot(poof, now - mini.goneAt);
      return { frame: shown.frame, nextChangeAt: shown.nextChangeAt === null ? null : mini.goneAt + shown.nextChangeAt };
    }
    if (now - mini.bornAt < pop.duration) {
      const shown = oneShot(pop, now - mini.bornAt);
      return {
        frame: shown.frame,
        nextChangeAt: shown.nextChangeAt === null ? mini.bornAt + pop.duration : mini.bornAt + shown.nextChangeAt,
      };
    }
    return onGrid(moving ? balance : work, now, phaseOf(mini.view.id));
  }

  private throwClips(): ThrowClips {
    const { fly, land, jump, walkLeft, walkRight } = this.options.clips;
    return { grabbed: jump, fly, land, land_splat: land, dizzy: land, walkLeft, walkRight };
  }
}

/** A one-shot clip `elapsed` ms in; `nextChangeAt` is relative to its start. */
function oneShot(clip: Clip, elapsed: number): { frame: Frame; nextChangeAt: number | null } {
  return frameAt(clip, Math.max(0, elapsed), false);
}

/**
 * A loop on the shared clock (see the animation controller): frames change
 * on multiples of the clip's grid, started at a place picked by `phase`.
 */
function onGrid(clip: Clip, now: number, phase: number): { frame: Frame; nextChangeAt: number | null } {
  const grid = gridOf(clip) ?? 1;
  const shift = Math.floor((phase * clip.duration) / grid) * grid;
  const { frame, nextChangeAt } = frameAt(clip, now + shift, true);
  let next = nextChangeAt === null ? null : nextChangeAt - shift;
  if (next !== null && next <= now) next = now + 1;
  return { frame, nextChangeAt: next };
}
