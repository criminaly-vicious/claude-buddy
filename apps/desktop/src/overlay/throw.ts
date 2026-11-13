/**
 * Drag and throw (docs/animation-spec.md#interactions, #24): a grabbed
 * buddy follows the pointer on a spring, flies with gravity when released,
 * bounces off the work-area edges, slides to a stop on the floor, plays its
 * landing, then walks back to its slot.
 *
 * Physics runs at a fixed step ({@link STEP_MS}) on a grid that starts when
 * the buddy is grabbed, so the path depends only on the input timestamps,
 * never on how often the overlay redraws. Units: CSS pixels and seconds,
 * positions are the sprite's top-left corner relative to the overlay's home
 * area (the anchored corner rectangle), y pointing down. Pure: time is
 * passed in.
 */

import { Clip, Frame, frameAt } from "../sprites/atlas";

/** Physics step: 120 Hz. */
export const STEP_MS = 1000 / 120;
const DT = STEP_MS / 1000;
/** Gravity, px/s² (spec ≈ 2000). */
export const GRAVITY = 2000;
/** Share of speed kept by a bounce (spec 0.4). */
export const RESTITUTION = 0.4;
/** Sliding deceleration on the floor, px/s². */
export const FRICTION = 1600;
/** A floor bounce slower than this just settles, px/s. */
const SETTLE_SPEED = 60;
/** Below this the buddy stops sliding, px/s. */
const STOP_SPEED = 8;
/** A first floor impact at least this fast is a hard landing (`land_splat` then `dizzy`). */
export const HARD_LANDING = 1400;
/** Throws are capped so a flick cannot launch the buddy through a wall. */
export const MAX_SPEED = 4000;
/** Spring stiffness (1/s²); critically damped, so it lags without wobbling. */
const SPRING_K = 600;
const SPRING_DAMPING = 2 * Math.sqrt(SPRING_K);

export interface Point {
  x: number;
  y: number;
}

export interface Body extends Point {
  vx: number;
  vy: number;
}

/** Where the sprite's top-left corner may go: the work area minus the sprite. */
export interface Bounds {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/** One spring step towards `target` (semi-implicit Euler). */
export function stepSpring(body: Body, target: Point, dt = DT): Body {
  const ax = SPRING_K * (target.x - body.x) - SPRING_DAMPING * body.vx;
  const ay = SPRING_K * (target.y - body.y) - SPRING_DAMPING * body.vy;
  const vx = body.vx + ax * dt;
  const vy = body.vy + ay * dt;
  return { x: body.x + vx * dt, y: body.y + vy * dt, vx, vy };
}

export interface FlightStep {
  body: Body;
  /** Downward speed when it hit the floor during this step, or `null`. */
  impact: number | null;
  /** On the floor and stopped: the flight is over. */
  resting: boolean;
}

/**
 * One flight step: gravity, then bounces off the bounds (restitution) and
 * friction while touching the floor. `bounds` limits the top-left corner.
 */
export function stepFlight(body: Body, bounds: Bounds, dt = DT): FlightStep {
  let { x, y, vx, vy } = body;
  vy += GRAVITY * dt;
  x += vx * dt;
  y += vy * dt;
  let impact: number | null = null;

  if (x < bounds.left) {
    x = bounds.left;
    vx = Math.abs(vx) * RESTITUTION;
  } else if (x > bounds.right) {
    x = bounds.right;
    vx = -Math.abs(vx) * RESTITUTION;
  }
  if (y < bounds.top) {
    y = bounds.top;
    vy = Math.abs(vy) * RESTITUTION;
  }
  let onFloor = false;
  if (y >= bounds.bottom) {
    y = bounds.bottom;
    onFloor = true;
    if (vy > 0) {
      impact = vy;
      vy = vy * RESTITUTION < SETTLE_SPEED ? 0 : -vy * RESTITUTION;
    }
  }
  if (onFloor && vy === 0) {
    const slow = FRICTION * dt;
    vx = Math.abs(vx) <= slow ? 0 : vx - Math.sign(vx) * slow;
  }
  const resting = onFloor && vy === 0 && Math.abs(vx) < STOP_SPEED;
  if (resting) vx = 0;
  return { body: { x, y, vx, vy }, impact, resting };
}

/** Caps the speed of `body` at {@link MAX_SPEED}, keeping its direction. */
export function capSpeed(body: Body): Body {
  const speed = Math.hypot(body.vx, body.vy);
  if (speed <= MAX_SPEED) return body;
  const k = MAX_SPEED / speed;
  return { ...body, vx: body.vx * k, vy: body.vy * k };
}

export type ThrowPhase = "grabbed" | "flying" | "landing" | "returning" | "done";

export interface ThrowClips {
  grabbed: Clip;
  fly: Clip;
  land: Clip;
  land_splat: Clip;
  dizzy: Clip;
  walk: Clip;
}

export interface ThrowOptions {
  clips: ThrowClips;
  bounds: Bounds;
  /** Walking speed back to the slot, px/s. */
  walkSpeed: number;
}

export interface ThrowFrame extends Point {
  phase: ThrowPhase;
  frame: Frame;
}

interface Step {
  clip: Clip;
  loop: boolean;
}

/** One buddy from the moment it is grabbed until it is back in its slot. */
export class Throw {
  private body: Body;
  private target: Point;
  /** Pointer offset from the sprite's corner, so it hangs where it was caught. */
  private readonly grip: Point;
  /** Time up to which physics has run, on the step grid. */
  private simAt: number;
  private phaseName: ThrowPhase = "grabbed";
  private phaseAt: number;
  private bounds: Bounds;
  private hardLanding = false;
  private firstImpact = true;
  private landing: Step[] = [];
  /** Walk back: from where, since when, towards which slot. */
  private walk: { from: Point; at: number; to: Point } | null = null;

  constructor(
    sprite: Point,
    pointer: Point,
    now: number,
    private readonly options: ThrowOptions,
  ) {
    this.body = { x: sprite.x, y: sprite.y, vx: 0, vy: 0 };
    this.grip = { x: pointer.x - sprite.x, y: pointer.y - sprite.y };
    this.target = { x: sprite.x, y: sprite.y };
    this.simAt = now;
    this.phaseAt = now;
    this.bounds = options.bounds;
  }

  get phase(): ThrowPhase {
    return this.phaseName;
  }

  /** The work area changed (the overlay finished expanding). */
  setBounds(bounds: Bounds): void {
    this.bounds = bounds;
  }

  /** The pointer moved while holding the buddy. */
  drag(pointer: Point, now: number): void {
    this.simulate(now);
    this.target = { x: pointer.x - this.grip.x, y: pointer.y - this.grip.y };
  }

  /** Let go: it flies with the spring's velocity (a slow release just drops). */
  release(now: number): void {
    if (this.phaseName !== "grabbed") return;
    this.simulate(now);
    this.body = capSpeed(this.body);
    this.enter("flying", this.simAt);
  }

  /** Where the buddy is and what it shows at `now`; `slot` is where it walks back to. */
  update(now: number, slot: Point): ThrowFrame {
    this.simulate(now);
    if (this.phaseName === "landing") {
      const total = this.landing.reduce((sum, s) => sum + s.clip.duration, 0);
      if (now >= this.phaseAt + total) {
        this.enter("returning", this.phaseAt + total);
        this.walk = { from: { x: this.body.x, y: this.body.y }, at: this.phaseAt, to: slot };
      }
    }
    if (this.phaseName === "returning" && this.walk) {
      if (this.walk.to.x !== slot.x || this.walk.to.y !== slot.y) {
        // The slot moved (another buddy left): head there from here.
        this.walk = { from: this.walkPosition(now), at: now, to: slot };
      }
      const position = this.walkPosition(now);
      if (position.x === slot.x && position.y === slot.y) {
        const distance = Math.hypot(slot.x - this.walk.from.x, slot.y - this.walk.from.y);
        this.enter("done", this.walk.at + (distance / this.options.walkSpeed) * 1000);
      }
      this.body = { ...this.body, ...position };
    }
    return { x: this.body.x, y: this.body.y, phase: this.phaseName, frame: this.frame(now) };
  }

  /** Runs fixed physics steps up to `now`. */
  private simulate(now: number): void {
    while (this.simAt + STEP_MS <= now) {
      this.simAt += STEP_MS;
      if (this.phaseName === "grabbed") {
        this.body = stepSpring(this.body, this.target);
      } else if (this.phaseName === "flying") {
        const step = stepFlight(this.body, this.bounds);
        this.body = step.body;
        if (step.impact !== null && this.firstImpact) {
          this.firstImpact = false;
          this.hardLanding = step.impact >= HARD_LANDING;
        }
        if (step.resting) this.land(this.simAt);
      } else {
        return;
      }
    }
  }

  private land(at: number): void {
    const { land, land_splat, dizzy } = this.options.clips;
    this.landing = this.hardLanding
      ? [
          { clip: land_splat, loop: false },
          { clip: dizzy, loop: false },
        ]
      : [{ clip: land, loop: false }];
    this.enter("landing", at);
  }

  private enter(phase: ThrowPhase, at: number): void {
    this.phaseName = phase;
    this.phaseAt = at;
  }

  private walkPosition(now: number): Point {
    const walk = this.walk as NonNullable<Throw["walk"]>;
    const dx = walk.to.x - walk.from.x;
    const dy = walk.to.y - walk.from.y;
    const distance = Math.hypot(dx, dy);
    const travelled = (this.options.walkSpeed * Math.max(0, now - walk.at)) / 1000;
    if (travelled >= distance) return { ...walk.to };
    const k = travelled / distance;
    return { x: walk.from.x + dx * k, y: walk.from.y + dy * k };
  }

  private frame(now: number): Frame {
    const { clips } = this.options;
    const t = now - this.phaseAt;
    switch (this.phaseName) {
      case "grabbed":
        return frameAt(clips.grabbed, t, true).frame;
      case "flying":
        return frameAt(clips.fly, t, true).frame;
      case "landing": {
        let start = 0;
        for (const step of this.landing) {
          if (t < start + step.clip.duration) return frameAt(step.clip, t - start, false).frame;
          start += step.clip.duration;
        }
        const last = this.landing[this.landing.length - 1].clip;
        return frameAt(last, last.duration, false).frame;
      }
      case "returning":
      case "done":
        return frameAt(clips.walk, t, true).frame;
    }
  }
}
