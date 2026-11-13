import { describe, expect, it } from "vitest";

import { parseAtlas } from "../sprites/atlas";
import buddy from "../sprites/buddy/atlas.json";
import type { SubagentView } from "../types";
import {
  Carrier,
  CLIMB_MS,
  MAX_MINIS,
  MINI_HEIGHT,
  MiniClips,
  miniKey,
  MiniStack,
  parseMiniKey,
  stackRise,
} from "./minis";
import type { Bounds } from "./throw";

const atlas = parseAtlas(buddy);
const clip = (name: string) => atlas.clips.get(name)!;
const clips: MiniClips = {
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
/** A 1000x600 work area for a 64 px sprite (2 CSS px per sprite px). */
const bounds: Bounds = { left: 0, top: 0, right: 936, bottom: 536 };
const SCALE = 2;
const idleFrame = clip("idle").frames[0];

function sub(id: string, since = 0): SubagentView {
  return { id, kind: "Explore", since };
}

/** The buddy standing in its slot at (800, 536), idle. */
function carrier(over: Partial<Carrier> = {}): Carrier {
  return {
    at: { x: 800, y: 536 },
    frame: idleFrame,
    slot: { x: 800, y: 536 },
    outward: -1,
    home: true,
    moving: false,
    scale: SCALE,
    ...over,
  };
}

/** A stack with `ids` that have finished popping in. */
function settled(ids: string[], random = () => 0.5): MiniStack {
  const stack = new MiniStack({ clips, random });
  stack.sync(ids.map((id) => sub(id)), 0);
  stack.update(1_000, carrier());
  return stack;
}

const ids = (placed: { view: SubagentView }[]) => placed.map((p) => p.view.id);

describe("MiniStack", () => {
  it("stacks the minis on the buddy's head in start order", () => {
    const { placed } = settled(["a", "b", "c"]).update(2_000, carrier());
    expect(ids(placed)).toEqual(["a", "b", "c"]);
    expect(placed.every((p) => p.stacked && p.at.x === 800)).toBe(true);
    // Feet on the buddy's head (row 14), then each on the head below.
    const head = idleFrame.head!;
    expect(placed.map((p) => p.at.y)).toEqual([0, 1, 2].map((k) => 536 + (head - 30 - k * MINI_HEIGHT) * SCALE));
  });

  it("follows the buddy's head as it breathes, sits or hops", () => {
    const stack = settled(["a"]);
    const sitting = clip("work_type").frames[0];
    const [standing] = stack.update(2_000, carrier()).placed;
    const [sat] = stack.update(2_000, carrier({ frame: sitting })).placed;
    expect(sat.at.y - standing.at.y).toBe((sitting.head! - idleFrame.head!) * SCALE);
    const [hopped] = stack.update(2_000, carrier({ at: { x: 800, y: 526 } })).placed;
    expect(hopped.at.y).toBe(standing.at.y - 10);
  });

  it("pops in, then works; balances while the buddy moves", () => {
    const stack = new MiniStack({ clips, random: () => 0.5 });
    stack.sync([sub("a")], 0);
    expect(clips.pop.frames).toContain(stack.update(50, carrier()).placed[0].frame);
    expect(clips.work.frames).toContain(stack.update(1_000, carrier()).placed[0].frame);
    expect(clips.balance.frames).toContain(stack.update(1_000, carrier({ moving: true })).placed[0].frame);
  });

  it("works on the shared clock: frame changes fall on its grid", () => {
    const stack = settled(["a", "b"]);
    const { nextChangeAt } = stack.update(1_234.5, carrier());
    expect(nextChangeAt! % clips.work.frames[0].duration).toBeCloseTo(0);
    expect(nextChangeAt!).toBeGreaterThan(1_234.5);
  });

  it("a stopped subagent vanishes in a puff, and the ones above drop a level", () => {
    const stack = settled(["a", "b", "c"]);
    const before = stack.update(2_000, carrier()).placed;
    stack.sync([sub("a"), sub("c")], 2_000);
    expect(clips.poof.frames).toContain(stack.update(2_050, carrier()).placed[1].frame);
    const poofEnd = 2_000 + clips.poof.duration;
    // Just after the puff, "c" starts falling from where it stood...
    const falling = stack.update(poofEnd + 1, carrier()).placed;
    expect(ids(falling)).toEqual(["a", "c"]);
    expect(falling[1].at.y).toBeLessThan(before[1].at.y + 2);
    // ...and lands on "a".
    const landed = stack.update(poofEnd + 500, carrier()).placed;
    expect(landed[1].at.y).toBe(before[1].at.y);
  });

  it("shows at most MAX_MINIS", () => {
    const stack = new MiniStack({ clips, random: () => 0.5 });
    stack.sync(
      Array.from({ length: MAX_MINIS + 3 }, (_, i) => sub(`s${i}`)),
      0,
    );
    expect(stack.size).toBe(MAX_MINIS);
    expect(stack.update(1_000, carrier()).placed).toHaveLength(MAX_MINIS);
  });

  describe("thrown", () => {
    /** Scatters a settled stack of `names` at t=2000 with the buddy flung up and left. */
    function scattered(names: string[], random = () => 0.5) {
      const stack = settled(names, random);
      stack.update(2_000, carrier());
      stack.scatter({ vx: -900, vy: -600 }, 2_000, bounds, SCALE, 96);
      return stack;
    }

    it("flies off the buddy, scattering", () => {
      const stack = scattered(["a", "b"], (() => {
        let i = 0;
        return () => [0.1, 0.9, 0.9, 0.1][i++ % 4];
      })());
      expect(stack.loose).toBe(true);
      const { placed } = stack.update(2_200, carrier({ home: false }));
      expect(placed.every((p) => !p.stacked)).toBe(true);
      expect(placed[0].at.x).not.toBe(placed[1].at.x);
      expect(placed.every((p) => clips.fly.frames.includes(p.frame))).toBe(true);
    });

    it("comes back in the same order, once the buddy is home", () => {
      // The last mini lands nearest: it is back first, but waits its turn.
      const stack = scattered(["a", "b", "c"]);
      const away = carrier({ home: false });
      let now = 2_000;
      for (; now < 12_000; now += 50) stack.update(now, away);
      // All in line beside the slot, outwards, in stack order, waiting.
      const waiting = stack.update(now, away).placed;
      expect(ids(waiting)).toEqual(["a", "b", "c"]);
      expect(waiting.every((p) => !p.stacked && p.at.y === 536)).toBe(true);
      const xs = waiting.map((p) => p.at.x);
      expect(xs[0]).toBeGreaterThan(xs[1]);
      expect(xs[1]).toBeGreaterThan(xs[2]);

      // Home: they climb on one at a time, bottom first; the others keep their place.
      const stackedAt: Record<string, number> = {};
      const cWaitsAt = xs[2];
      for (; now < 16_000; now += 20) {
        for (const p of stack.update(now, carrier()).placed) {
          if (p.stacked && stackedAt[p.view.id] === undefined) stackedAt[p.view.id] = now;
          if (p.view.id === "c" && !p.stacked && stackedAt.b === undefined) expect(p.at.x).toBe(cWaitsAt);
        }
      }
      expect(stackedAt.a).toBeLessThan(stackedAt.b);
      expect(stackedAt.b).toBeLessThan(stackedAt.c);
      expect(stackedAt.b - stackedAt.a).toBeGreaterThanOrEqual(CLIMB_MS - 40);
      expect(stack.loose).toBe(false);
      const back = stack.update(now, carrier()).placed;
      expect(ids(back)).toEqual(["a", "b", "c"]);
      expect(back.every((p) => p.stacked)).toBe(true);
    });

    it("a newcomer while the others are off waits its turn behind them", () => {
      const stack = scattered(["a"]);
      stack.sync([sub("a"), sub("b", 2_100)], 2_100);
      let now = 2_100;
      for (; now < 20_000; now += 25) stack.update(now, carrier({ home: now > 9_000 }));
      expect(ids(stack.update(now, carrier()).placed)).toEqual(["a", "b"]);
      expect(stack.loose).toBe(false);
    });

    it("a mini whose subagent stops while it is off just vanishes", () => {
      const stack = scattered(["a", "b"]);
      stack.update(2_300, carrier({ home: false }));
      stack.sync([sub("b")], 2_300);
      const { placed } = stack.update(2_350, carrier({ home: false }));
      expect(clips.poof.frames).toContain(placed.find((p) => p.view.id === "a")!.frame);
      stack.update(2_300 + clips.poof.duration + 1, carrier({ home: false }));
      expect(stack.size).toBe(1);
    });
  });

  it("asks for room above the buddy only when the stack is taller than its cell", () => {
    expect(stackRise(0)).toBe(0);
    expect(stackRise(1)).toBe(0);
    expect(stackRise(3)).toBe(3 * MINI_HEIGHT + 2 - 12);
    expect(stackRise(99)).toBe(stackRise(MAX_MINIS));
  });

  it("names a mini by its buddy and its subagent", () => {
    expect(parseMiniKey(miniKey("session-1", "a0001"))).toEqual({ parent: "session-1", id: "a0001" });
    expect(parseMiniKey("session-1")).toBeNull();
  });
});
