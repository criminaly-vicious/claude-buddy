import { describe, expect, it } from "vitest";

import { parseAtlas } from "../sprites/atlas";
import buddy from "../sprites/buddy/atlas.json";
import { Moment, Stroll, Visit } from "./excursion";

const atlas = parseAtlas(buddy);
const clip = (name: string) => atlas.clips.get(name)!;
const walks = { walkLeft: clip("walk_left"), walkRight: clip("walk_right") };
const SPEED = 80; // px/s

/** Samples an excursion every `every` ms until it is done. */
function run(
  update: (now: number) => Moment,
  from: number,
  every: number,
): (Moment & { now: number })[] {
  const out = [];
  for (let now = from; now < from + 60_000; now += every) {
    const m = update(now);
    out.push({ ...m, now });
    if (m.done) break;
  }
  return out;
}

describe("Stroll", () => {
  const stroll = (pauseMs = 1000) =>
    new Stroll({ x: 400, y: 50 }, 0, {
      clips: { ...walks, look: clip("idle_look") },
      speed: SPEED,
      turnAt: { x: 240, y: 50 },
      pauseMs,
    });

  it("walks out facing the way it goes, looks around, walks back", () => {
    const s = stroll();
    const out = s.update(1000, { x: 400, y: 50 });
    expect(out).toMatchObject({ x: 320, y: 50, done: false });
    expect(clip("walk_left").frames).toContain(out.frame);
    // 160 px at 80 px/s: there at 2 s, then a 1 s look.
    const look = s.update(2500, { x: 400, y: 50 });
    expect(look).toMatchObject({ x: 240, done: false });
    expect(clip("idle_look").frames).toContain(look.frame);
    // It leaves at 3 s: half a second later it is 40 px on its way back.
    const back = s.update(3500, { x: 400, y: 50 });
    expect(back.x).toBeCloseTo(280);
    expect(clip("walk_right").frames).toContain(back.frame);
    expect(s.update(5000, { x: 400, y: 50 })).toMatchObject({ x: 400, done: true });
  });

  it("is the same whatever the redraw rate", () => {
    const a = stroll();
    const b = stroll();
    const sparse = run((now) => a.update(now, { x: 400, y: 50 }), 0, 250);
    const dense = new Map(run((now) => b.update(now, { x: 400, y: 50 }), 0, 50).map((m) => [m.now, m]));
    for (const m of sparse) expect(dense.get(m.now)).toEqual(m);
  });

  it("heads home when recalled, to wherever its slot is now", () => {
    const s = stroll();
    s.update(1000, { x: 400, y: 50 });
    s.recall(1000);
    const back = run((now) => s.update(now, { x: 360, y: 50 }), 1000, 100);
    expect(back.at(-1)).toMatchObject({ x: 360, y: 50, done: true });
    // From x 320 to 360 at 80 px/s: half a second.
    expect(back.at(-1)!.now).toBe(1500);
    expect(back.every((m) => m.x >= 320)).toBe(true);
  });
});

describe("Visit", () => {
  const options = (sitting: boolean) => ({
    clips: { ...walks, talk: clip("talk"), standUp: sitting ? clip("stand_up") : null },
    speed: SPEED,
    talkMs: 2000,
  });

  it("stands up, walks over, talks, then walks back", () => {
    const v = new Visit({ x: 400, y: 50 }, 0, options(true));
    const slot = { x: 400, y: 50 };
    const partner = { x: 320, y: 50 };
    const rise = clip("stand_up").duration;
    expect(clip("stand_up").frames).toContain(v.update(0, slot, partner).frame);
    const going = v.update(rise + 500, slot, partner);
    expect(going.x).toBeCloseTo(360);
    expect(clip("walk_left").frames).toContain(going.frame);
    // 80 px: there one second after standing up, then it talks.
    const talking = v.update(rise + 1500, slot, partner);
    expect(talking).toMatchObject({ x: 320, done: false });
    expect(clip("talk").frames).toContain(talking.frame);
    expect(v.talkingSince).toBe(rise + 1000);
    const end = run((now) => v.update(now, slot, partner), rise + 1500, 100).at(-1)!;
    expect(end).toMatchObject({ x: 400, done: true });
    expect(end.now).toBeCloseTo(rise + 1000 + 2000 + 1000, -2);
  });

  it("a standing buddy sets off at once", () => {
    const v = new Visit({ x: 400, y: 50 }, 0, options(false));
    expect(v.update(500, { x: 400, y: 50 }, { x: 320, y: 50 }).x).toBeCloseTo(360);
  });

  it("goes back when the other buddy leaves", () => {
    const v = new Visit({ x: 400, y: 50 }, 0, options(false));
    v.update(500, { x: 400, y: 50 }, { x: 320, y: 50 });
    const end = run((now) => v.update(now, { x: 400, y: 50 }), 500, 100).at(-1)!;
    expect(end).toMatchObject({ x: 400, done: true });
    expect(v.talkingSince).toBeNull();
  });
});
