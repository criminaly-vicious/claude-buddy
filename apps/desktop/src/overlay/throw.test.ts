import { describe, expect, it } from "vitest";

import { parseAtlas } from "../sprites/atlas";
import placeholder from "../sprites/placeholder/atlas.json";
import {
  Body,
  Bounds,
  capSpeed,
  GRAVITY,
  HARD_LANDING,
  MAX_SPEED,
  RESTITUTION,
  STEP_MS,
  stepFlight,
  stepSpring,
  Throw,
  ThrowClips,
} from "./throw";

const atlas = parseAtlas(placeholder);
const clip = (name: string) => atlas.clips.get(name)!;
const clips: ThrowClips = {
  grabbed: clip("grabbed"),
  fly: clip("fly"),
  land: clip("land"),
  land_splat: clip("land_splat"),
  dizzy: clip("dizzy"),
  walk: clip("walk"),
};
/** A 1000x600 work area for a 64 px sprite. */
const bounds: Bounds = { left: 0, top: 0, right: 936, bottom: 536 };
const DT = STEP_MS / 1000;

function fly(body: Body, steps: number): Body {
  for (let i = 0; i < steps; i++) body = stepFlight(body, bounds).body;
  return body;
}

describe("flight physics", () => {
  it("falls with gravity", () => {
    const start: Body = { x: 100, y: 0, vx: 0, vy: 0 };
    const after = fly(start, 60); // 0.5 s
    expect(after.vy).toBeCloseTo(GRAVITY * 0.5, 6);
    // Semi-implicit Euler: within a step's worth of ½gt².
    expect(after.y).toBeGreaterThan(0.5 * GRAVITY * 0.25 - GRAVITY * 0.5 * DT);
    expect(after.y).toBeLessThan(0.5 * GRAVITY * 0.25 + GRAVITY * 0.5 * DT);
    expect(after.x).toBe(100);
  });

  it("bounces off the walls and ceiling with restitution 0.4", () => {
    const wall = stepFlight({ x: 935, y: 100, vx: 1000, vy: 0 }, bounds);
    expect(wall.body.x).toBe(936);
    expect(wall.body.vx).toBeCloseTo(-1000 * RESTITUTION);
    const left = stepFlight({ x: 1, y: 100, vx: -500, vy: 0 }, bounds);
    expect(left.body).toMatchObject({ x: 0, vx: 500 * RESTITUTION });
    const ceiling = stepFlight({ x: 100, y: 1, vy: -1000, vx: 0 }, bounds);
    expect(ceiling.body.y).toBe(0);
    expect(ceiling.body.vy).toBeGreaterThan(0);
  });

  it("bounces off the floor, reporting the impact speed", () => {
    const hit = stepFlight({ x: 100, y: 535, vx: 0, vy: 1000 }, bounds);
    expect(hit.body.y).toBe(536);
    expect(hit.impact).toBeCloseTo(1000 + GRAVITY * DT);
    expect(hit.body.vy).toBeCloseTo(-(1000 + GRAVITY * DT) * RESTITUTION);
    expect(hit.resting).toBe(false);
  });

  it("settles a slow bounce and slides to a stop with friction", () => {
    let step = stepFlight({ x: 100, y: 536, vx: 300, vy: 50 }, bounds);
    expect(step.body.vy).toBe(0);
    expect(step.body.vx).toBeLessThan(300);
    let steps = 1;
    while (!step.resting) {
      step = stepFlight(step.body, bounds);
      steps++;
      expect(step.body.y).toBe(536);
    }
    expect(step.body.vx).toBe(0);
    // 300 px/s at 1600 px/s² stops in about 0.19 s.
    expect(steps * DT).toBeCloseTo(300 / 1600, 1);
    expect(step.body.x).toBeGreaterThan(100);
  });

  it("caps throw speed, keeping the direction", () => {
    const capped = capSpeed({ x: 0, y: 0, vx: 6000, vy: -8000 });
    expect(Math.hypot(capped.vx, capped.vy)).toBeCloseTo(MAX_SPEED);
    expect(capped.vx / capped.vy).toBeCloseTo(6000 / -8000);
  });

  it("the spring follows the target without overshooting", () => {
    let body: Body = { x: 0, y: 0, vx: 0, vy: 0 };
    let max = 0;
    for (let i = 0; i < 120; i++) {
      body = stepSpring(body, { x: 100, y: 0 });
      max = Math.max(max, body.x);
    }
    expect(body.x).toBeCloseTo(100, 0);
    expect(max).toBeLessThanOrEqual(100.0001);
    // It lags: after 50 ms it is still well short of the target.
    let early: Body = { x: 0, y: 0, vx: 0, vy: 0 };
    for (let i = 0; i < 6; i++) early = stepSpring(early, { x: 100, y: 0 });
    expect(early.x).toBeLessThan(60);
  });
});

/** Runs a throw to the end, sampling every `every` ms; returns the samples. */
function run(t: Throw, from: number, every: number, slot = { x: 800, y: 520 }) {
  const samples = [];
  for (let now = from; now < from + 20_000; now += every) {
    const frame = t.update(now, slot);
    samples.push({ now, ...frame });
    if (frame.phase === "done") break;
  }
  return samples;
}

function thrown(): Throw {
  // Grab at the slot, pull up-left quickly, let go.
  const t = new Throw({ x: 800, y: 520 }, { x: 832, y: 540 }, 0, {
    clips,
    bounds,
    walkSpeed: 128,
  });
  for (let i = 1; i <= 10; i++) t.drag({ x: 832 - 30 * i, y: 540 - 25 * i }, i * 10);
  t.release(100);
  return t;
}

describe("Throw", () => {
  it("goes grab → fly → land → walk back → done", () => {
    const t = thrown();
    expect(t.phase).toBe("flying");
    const phases = run(t, 100, 5).map((s) => s.phase);
    const order = phases.filter((p, i) => p !== phases[i - 1]);
    expect(order).toEqual(["flying", "landing", "returning", "done"]);
  });

  it("ends exactly in the slot", () => {
    const last = run(thrown(), 100, 7).at(-1)!;
    expect(last).toMatchObject({ phase: "done", x: 800, y: 520 });
  });

  it("is deterministic whatever the redraw rate", () => {
    const a = thrown();
    const b = thrown();
    // Same moments, one sampled 30x more often in between.
    for (let now = 100; now < 3000; now += 1) {
      const fb = b.update(now, { x: 800, y: 520 });
      if (now % 30 === 10) {
        const fa = a.update(now, { x: 800, y: 520 });
        expect(fa).toEqual(fb);
      }
    }
  });

  it("follows the pointer with lag while held, using the grip point", () => {
    const t = new Throw({ x: 100, y: 100 }, { x: 110, y: 120 }, 0, {
      clips,
      bounds,
      walkSpeed: 128,
    });
    t.drag({ x: 310, y: 120 }, 0);
    const soon = t.update(50, { x: 0, y: 0 });
    expect(soon.phase).toBe("grabbed");
    expect(soon.x).toBeGreaterThan(100);
    expect(soon.x).toBeLessThan(300);
    const later = t.update(1000, { x: 0, y: 0 });
    expect(later.x).toBeCloseTo(300, 0); // pointer minus grip
    expect(later.y).toBeCloseTo(100, 0);
  });

  it("a release without velocity drops and lands softly", () => {
    const t = new Throw({ x: 400, y: 300 }, { x: 410, y: 310 }, 0, {
      clips,
      bounds,
      walkSpeed: 128,
    });
    t.release(1000); // held still for a second
    const samples = run(t, 1000, 5);
    const touchdown = samples.find((s) => s.phase === "landing")!;
    expect(touchdown.x).toBeCloseTo(400, 3);
    expect(touchdown.y).toBe(536);
    // 236 px drop → about 970 px/s: a plain landing, no dizzy.
    const landing = samples.filter((s) => s.phase === "landing");
    expect(landing[0].frame).toBe(clips.land.frames[0]);
    expect(landing.every((s) => clips.land.frames.includes(s.frame))).toBe(true);
  });

  it("a hard landing plays land_splat then dizzy", () => {
    // Dropped from the ceiling of a tall work area.
    const tall: Bounds = { left: 0, top: 0, right: 936, bottom: 1400 };
    expect(Math.sqrt(2 * GRAVITY * 1400)).toBeGreaterThan(HARD_LANDING);
    const t = new Throw({ x: 400, y: 0 }, { x: 410, y: 10 }, 0, {
      clips,
      bounds: tall,
      walkSpeed: 128,
    });
    t.release(0);
    const samples = run(t, 0, 5, { x: 800, y: 1380 });
    const landing = samples.filter((s) => s.phase === "landing");
    const start = landing[0].now;
    const frameAt = (ms: number) => landing.find((s) => s.now >= start + ms)!.frame;
    expect(frameAt(0)).toEqual(clips.land_splat.frames[0]);
    expect(frameAt(clips.land_splat.duration + 1)).toBe(clips.dizzy.frames[0]);
  });

  it("walks to the slot's new place when it moves", () => {
    const t = thrown();
    let frame = t.update(100, { x: 800, y: 520 });
    let now = 100;
    while (frame.phase !== "returning") frame = t.update((now += 5), { x: 800, y: 520 });
    while (frame.phase !== "done") frame = t.update((now += 5), { x: 700, y: 520 });
    expect(frame).toMatchObject({ x: 700, y: 520 });
  });
});
