/**
 * Animation controller: one per buddy. Turns the registry's state into the
 * clip sequence from docs/animation-spec.md#state--animation (transition
 * first, then the loop) and says what to show at any time.
 *
 * Rules:
 * - A non-looping clip (transition, spawn, idle extra) always plays to its
 *   end; a state change that arrives meanwhile is applied when it finishes.
 *   (`grabbed`, the one exception in the spec, is played over the controller
 *   by the scene while a buddy is thrown: see `throw.ts`.)
 * - Clips follow each other at exact times, so the result depends only on
 *   the timestamps given, not on how often the controller is asked.
 * - Idle extras are scheduled deadlines, not timers: the render loop sleeps
 *   until the next one.
 *
 * Pure: time and randomness are passed in.
 */

import { Clip, frameAt, Frame } from "../sprites/atlas";
import type { BuddyState } from "../types";

/** Every animation the controller can play; the atlas must have them all. */
export const CONTROLLER_CLIPS = [
  "spawn",
  "idle",
  "idle_look",
  "idle_stretch",
  "idle_scratch",
  "sit_down",
  "work_type",
  "research_read",
  "stand_up",
  "wait_wave",
  "sleep",
  "leave",
] as const;

const IDLE_EXTRAS = ["idle_look", "idle_stretch", "idle_scratch"] as const;
/** Random idle extras: one every 8–20 s (spec). */
const EXTRA_MIN_MS = 8_000;
const EXTRA_SPREAD_MS = 12_000;

type Posture = "standing" | "sitting";

interface Step {
  clip: string;
  loop: boolean;
}

/** Posture a buddy is in while (and after) a clip plays. */
const SITTING_CLIPS = new Set(["sit_down", "work_type", "research_read"]);

/** The steps that take a buddy in `posture` to `state` (spec table). */
export function plan(state: BuddyState, posture: Posture): Step[] {
  const stand: Step[] = posture === "sitting" ? [{ clip: "stand_up", loop: false }] : [];
  const sit: Step[] = posture === "standing" ? [{ clip: "sit_down", loop: false }] : [];
  switch (state) {
    case "spawning":
      return [
        { clip: "spawn", loop: false },
        { clip: "idle", loop: true },
      ];
    case "idle":
      return [...stand, { clip: "idle", loop: true }];
    case "working":
      return [...sit, { clip: "work_type", loop: true }];
    case "researching":
      // Already sitting: just swap the laptop for the paper.
      return [...sit, { clip: "research_read", loop: true }];
    case "waiting":
      return [...stand, { clip: "wait_wave", loop: true }];
    case "sleeping":
      return [{ clip: "sleep", loop: true }];
    case "leaving":
      return [...stand, { clip: "leave", loop: false }];
  }
}

export interface ControllerOptions {
  clips: ReadonlyMap<string, Clip>;
  /** Returns a number in [0, 1). Injected so tests are deterministic. */
  random?: () => number;
  /** No random idle extras (accessibility setting). */
  reduceMotion?: boolean;
}

export interface Showing {
  /** Clip (frame tag) playing now. */
  clip: string;
  frame: Frame;
  /** When the picture next changes, or `null` if it never will on its own. */
  nextChangeAt: number | null;
}

export class AnimationController {
  private readonly clips: ReadonlyMap<string, Clip>;
  private readonly random: () => number;
  private readonly reduceMotion: boolean;

  private target: BuddyState;
  private step: Step;
  private startedAt: number;
  /** Steps after the current one, for the current target. */
  private queue: Step[] = [];
  /** When the next idle extra starts, while looping `idle`. */
  private nextExtraAt: number | null = null;

  constructor(state: BuddyState, now: number, options: ControllerOptions) {
    this.clips = options.clips;
    this.random = options.random ?? Math.random;
    this.reduceMotion = options.reduceMotion ?? false;
    this.target = state;
    // A new buddy always pops in first (spec: spawning → spawn → idle);
    // one first seen mid-turn then goes straight to its state.
    const steps: Step[] =
      state === "spawning"
        ? plan("spawning", "standing")
        : [{ clip: "spawn", loop: false }, ...plan(state, "standing")];
    const [first, ...rest] = steps;
    this.step = first;
    this.queue = rest;
    this.startedAt = now;
    this.enterStep(now);
  }

  get state(): BuddyState {
    return this.target;
  }

  setState(state: BuddyState, now: number): void {
    this.advance(now);
    if (state === this.target) return;
    this.target = state;
    if (this.step.loop) {
      // Loops can be left at any time.
      this.startPlan(plan(state, this.postureAfter(this.step)), now);
    } else {
      // Finish the current clip, then head for the new target.
      this.queue = [];
    }
  }

  current(now: number): Showing {
    this.advance(now);
    const clip = this.clip(this.step.clip);
    const { frame, nextChangeAt } = frameAt(clip, now - this.startedAt, this.step.loop);
    let next = nextChangeAt === null ? null : this.startedAt + nextChangeAt;
    // A one-shot followed by another step changes when it ends.
    if (!this.step.loop && next === null && this.queue.length > 0) {
      next = this.startedAt + clip.duration;
    }
    if (this.nextExtraAt !== null && (next === null || this.nextExtraAt < next)) {
      next = this.nextExtraAt;
    }
    return { clip: this.step.clip, frame, nextChangeAt: next };
  }

  /** Moves through finished one-shots and due extras, up to `now`. */
  private advance(now: number): void {
    for (;;) {
      if (!this.step.loop) {
        const endsAt = this.startedAt + this.clip(this.step.clip).duration;
        if (now < endsAt) return;
        const next =
          this.queue.length > 0
            ? this.queue
            : this.step.clip === "leave"
              ? [] // Leaving holds its last frame until the session is removed.
              : plan(this.target, this.postureAfter(this.step));
        if (next.length === 0) return;
        this.startPlan(next, endsAt);
        continue;
      }
      if (this.nextExtraAt !== null && now >= this.nextExtraAt) {
        const extra = IDLE_EXTRAS[Math.floor(this.random() * IDLE_EXTRAS.length)];
        this.startPlan(
          [
            { clip: extra, loop: false },
            { clip: "idle", loop: true },
          ],
          this.nextExtraAt,
        );
        continue;
      }
      return;
    }
  }

  private startPlan(steps: Step[], at: number): void {
    const [first, ...rest] = steps;
    this.step = first;
    this.queue = rest;
    this.startedAt = at;
    this.enterStep(at);
  }

  private enterStep(at: number): void {
    this.nextExtraAt =
      this.step.clip === "idle" && this.step.loop && !this.reduceMotion
        ? at + EXTRA_MIN_MS + this.random() * EXTRA_SPREAD_MS
        : null;
  }

  private postureAfter(step: Step): Posture {
    return SITTING_CLIPS.has(step.clip) ? "sitting" : "standing";
  }

  private clip(name: string): Clip {
    const clip = this.clips.get(name);
    if (!clip) throw new Error(`atlas has no "${name}" animation`);
    return clip;
  }
}
