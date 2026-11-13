/**
 * Excursions: a buddy away from its slot for a while, and back. Throws
 * (`throw.ts`), strolls along the floor, and visits to another buddy when
 * their sessions talk to each other. Each one says, for any time, where the
 * buddy is, what it shows and whether it is back.
 *
 * Positions are the sprite's top-left corner in CSS pixels relative to the
 * home area (the anchored corner rectangle), as in `throw.ts`. Pure: time is
 * passed in, and a position is a function of time, so the result does not
 * depend on how often the overlay redraws.
 */

import { Clip, Frame, frameAt } from "../sprites/atlas";
import type { Point } from "./throw";

/** Where a buddy is and what it shows at one moment. */
export interface Moment extends Point {
  frame: Frame;
  /** Back in its slot: the excursion is over. */
  done: boolean;
}

/**
 * How much room an excursion needs: the whole work area (a throw), a strip
 * along the floor (a stroll), or just the corner area (a visit).
 */
export type Room = "work" | "floor" | "home";

export interface Excursion {
  readonly room: Room;
  /**
   * The buddy at `now`. `slot` is where it walks back to (it moves when
   * other buddies come and go); `partner` is where a visit walks to.
   */
  update(now: number, slot: Point, partner?: Point): Moment;
}

/** A walk from `from` towards `to` that started at `at`, at `speed` px/s. */
interface Leg {
  from: Point;
  to: Point;
  at: number;
}

function legPosition(leg: Leg, speed: number, now: number): { at: Point; arrived: number | null } {
  const dx = leg.to.x - leg.from.x;
  const dy = leg.to.y - leg.from.y;
  const distance = Math.hypot(dx, dy);
  const duration = speed > 0 ? (distance / speed) * 1000 : 0;
  const t = Math.max(0, now - leg.at);
  if (t >= duration) return { at: { ...leg.to }, arrived: leg.at + duration };
  const k = t / duration;
  return { at: { x: leg.from.x + dx * k, y: leg.from.y + dy * k }, arrived: null };
}

/** Same place, to the pixel the overlay draws at. */
const same = (a: Point, b: Point) => Math.abs(a.x - b.x) < 0.01 && Math.abs(a.y - b.y) < 0.01;

export interface WalkClips {
  walkLeft: Clip;
  walkRight: Clip;
}

/** The walk clip facing the way a leg goes. */
function walkFrame(clips: WalkClips, leg: Leg, now: number): Frame {
  const clip = leg.to.x < leg.from.x ? clips.walkLeft : clips.walkRight;
  return frameAt(clip, now - leg.at, true).frame;
}

/**
 * Follows a moving slot home: when the slot moves (another buddy left),
 * the walk restarts from where the buddy is, towards the new place.
 */
class HomeWalk {
  private leg: Leg | null = null;

  constructor(
    private readonly clips: WalkClips,
    private readonly speed: number,
  ) {}

  /** Starts walking home from `from` at `at`. */
  start(from: Point, at: number, slot: Point): void {
    this.leg = { from: { ...from }, to: { ...slot }, at };
  }

  update(now: number, slot: Point): Moment {
    let leg = this.leg as Leg;
    if (!same(leg.to, slot)) {
      leg = { from: legPosition(leg, this.speed, now).at, to: { ...slot }, at: now };
      this.leg = leg;
    }
    const { at, arrived } = legPosition(leg, this.speed, now);
    return { ...at, frame: walkFrame(this.clips, leg, now), done: arrived !== null };
  }
}

export interface StrollOptions {
  clips: WalkClips & { look: Clip };
  /** Walking speed, CSS px/s. */
  speed: number;
  /** Where the stroll turns around. */
  turnAt: Point;
  /** How long it looks around there. */
  pauseMs: number;
}

/** An idle buddy takes a short walk along the floor, looks around and walks back. */
export class Stroll implements Excursion {
  readonly room: Room = "floor";
  private readonly out: Leg;
  private readonly home: HomeWalk;
  private homeFrom: number | null = null;

  constructor(
    from: Point,
    now: number,
    private readonly options: StrollOptions,
  ) {
    this.out = { from: { ...from }, to: { ...options.turnAt }, at: now };
    this.home = new HomeWalk(options.clips, options.speed);
  }

  /** Its session got busy: head back now, from wherever it is. */
  recall(now: number): void {
    if (this.homeFrom !== null) return;
    const here = legPosition(this.out, this.options.speed, now).at;
    this.homeFrom = now;
    // The first update re-aims at the real slot.
    this.home.start(here, now, here);
  }

  update(now: number, slot: Point): Moment {
    const { speed, clips, pauseMs } = this.options;
    if (this.homeFrom === null) {
      const { at, arrived } = legPosition(this.out, speed, now);
      if (arrived === null) return { ...at, frame: walkFrame(clips, this.out, now), done: false };
      const leaveAt = arrived + pauseMs;
      if (now < leaveAt) {
        return { ...at, frame: frameAt(clips.look, now - arrived, true).frame, done: false };
      }
      this.homeFrom = leaveAt;
      this.home.start(this.out.to, leaveAt, slot);
    }
    return this.home.update(now, slot);
  }
}

export interface VisitOptions {
  clips: WalkClips & { talk: Clip; standUp: Clip | null };
  speed: number;
  /** How long the two talk. */
  talkMs: number;
}

/**
 * A buddy whose session messaged another walks over to that buddy, talks
 * for a moment and walks back. A sitting buddy stands up first.
 */
export class Visit implements Excursion {
  readonly room: Room = "home";
  private phase: "rise" | "go" | "talk" | "back" = "rise";
  private go: Leg | null = null;
  private talkAt: number | null = null;
  private readonly home: HomeWalk;

  constructor(
    private readonly from: Point,
    private readonly startedAt: number,
    private readonly options: VisitOptions,
  ) {
    this.home = new HomeWalk(options.clips, options.speed);
  }

  /** When the talking started, or `null` before (or without) it. */
  get talkingSince(): number | null {
    return this.talkAt;
  }

  update(now: number, slot: Point, partner?: Point): Moment {
    const { clips, speed, talkMs } = this.options;
    const riseMs = clips.standUp?.duration ?? 0;

    if (this.phase === "rise") {
      if (clips.standUp && now < this.startedAt + riseMs) {
        return { ...this.from, frame: frameAt(clips.standUp, now - this.startedAt, false).frame, done: false };
      }
      this.phase = "go";
      this.go = { from: { ...this.from }, to: partner ? { ...partner } : { ...this.from }, at: this.startedAt + riseMs };
    }

    if (this.phase === "go") {
      let leg = this.go as Leg;
      if (!partner) {
        // The other session is gone: nobody to talk to.
        this.phase = "back";
        this.home.start(legPosition(leg, speed, now).at, now, slot);
      } else {
        if (!same(leg.to, partner)) {
          leg = { from: legPosition(leg, speed, now).at, to: { ...partner }, at: now };
          this.go = leg;
        }
        const { at, arrived } = legPosition(leg, speed, now);
        if (arrived === null) return { ...at, frame: walkFrame(clips, leg, now), done: false };
        this.phase = "talk";
        this.talkAt = arrived;
      }
    }

    if (this.phase === "talk") {
      const talkAt = this.talkAt as number;
      const here = (this.go as Leg).to;
      if (now < talkAt + talkMs) {
        return { ...here, frame: frameAt(clips.talk, now - talkAt, true).frame, done: false };
      }
      this.phase = "back";
      this.home.start(here, talkAt + talkMs, slot);
    }

    return this.home.update(now, slot);
  }
}
