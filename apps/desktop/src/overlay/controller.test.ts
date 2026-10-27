import { describe, expect, it } from "vitest";

import { parseAtlas } from "../sprites/atlas";
import placeholder from "../sprites/placeholder/atlas.json";
import type { BuddyState } from "../types";
import { AnimationController, CONTROLLER_CLIPS, plan } from "./controller";

const atlas = parseAtlas(placeholder);
const ms = (clip: string) => atlas.clips.get(clip)!.duration;

/** No idle extras unless a test wants them. */
function controller(state: BuddyState, now = 0, random = () => 0.5, reduceMotion = true) {
  return new AnimationController(state, now, { clips: atlas.clips, random, reduceMotion });
}

/** Clips shown from `from` to `until`, following `nextChangeAt`, collapsed. */
function timeline(c: AnimationController, from: number, until: number): string[] {
  const seen: string[] = [];
  let t = from;
  for (let guard = 0; guard < 10_000 && t <= until; guard++) {
    const { clip, nextChangeAt } = c.current(t);
    if (seen[seen.length - 1] !== clip) seen.push(clip);
    if (nextChangeAt === null) break;
    t = nextChangeAt;
  }
  return seen;
}

/** A controller already looping `state`'s clip, at the returned time. */
function settled(state: BuddyState): [AnimationController, number] {
  const c = controller("spawning");
  let t = ms("spawn");
  c.setState(state, t);
  t += 5_000; // Long enough for any transition.
  c.current(t);
  return [c, t];
}

describe("plan (spec state table)", () => {
  it.each<[BuddyState, "standing" | "sitting", string[]]>([
    ["spawning", "standing", ["spawn", "idle"]],
    ["idle", "standing", ["idle"]],
    ["idle", "sitting", ["stand_up", "idle"]],
    ["working", "standing", ["sit_down", "work_type"]],
    ["working", "sitting", ["work_type"]],
    ["researching", "standing", ["sit_down", "research_read"]],
    ["researching", "sitting", ["research_read"]],
    ["waiting", "standing", ["wait_wave"]],
    ["waiting", "sitting", ["stand_up", "wait_wave"]],
    ["sleeping", "standing", ["sleep"]],
    ["sleeping", "sitting", ["sleep"]],
    ["leaving", "standing", ["leave"]],
    ["leaving", "sitting", ["stand_up", "leave"]],
  ])("%s from %s → %j", (state, posture, clips) => {
    expect(plan(state, posture).map((s) => s.clip)).toEqual(clips);
  });

  it("every clip it uses exists in the atlas", () => {
    for (const name of CONTROLLER_CLIPS) expect(atlas.clips.has(name), name).toBe(true);
  });
});

describe("AnimationController", () => {
  it("a new buddy spawns, then idles", () => {
    const c = controller("spawning");
    expect(timeline(c, 0, ms("spawn") + 10)).toEqual(["spawn", "idle"]);
  });

  it("a buddy first seen mid-turn still pops in, then goes to its state", () => {
    const c = controller("working");
    expect(timeline(c, 0, ms("spawn") + ms("sit_down") + 10)).toEqual([
      "spawn",
      "sit_down",
      "work_type",
    ]);
  });

  it("sits down before working and stands up before waiting", () => {
    const [c, t] = settled("idle");
    c.setState("working", t);
    expect(timeline(c, t, t + ms("sit_down") + 10)).toEqual(["sit_down", "work_type"]);
    const t2 = t + 5_000;
    c.setState("waiting", t2);
    expect(timeline(c, t2, t2 + ms("stand_up") + 10)).toEqual(["stand_up", "wait_wave"]);
  });

  it("swaps the prop without standing up between working and researching", () => {
    const [c, t] = settled("working");
    c.setState("researching", t);
    expect(c.current(t).clip).toBe("research_read");
  });

  it("finishes a transition before following a state that arrives mid-way", () => {
    const [c, t] = settled("idle");
    c.setState("working", t); // sit_down starts
    const mid = t + ms("sit_down") / 2;
    c.setState("waiting", mid); // arrives during sit_down
    expect(c.current(mid).clip).toBe("sit_down");
    expect(timeline(c, mid, t + ms("sit_down") + ms("stand_up") + 10)).toEqual([
      "sit_down",
      "stand_up",
      "wait_wave",
    ]);
  });

  it("a state flip back during a transition ends where it started", () => {
    const [c, t] = settled("idle");
    c.setState("working", t);
    c.setState("idle", t + 10); // changed its mind
    expect(timeline(c, t + 10, t + ms("sit_down") + ms("stand_up") + 10)).toEqual([
      "sit_down",
      "stand_up",
      "idle",
    ]);
  });

  it("leaving stands up, plays once and then holds still", () => {
    const [c, t] = settled("working");
    c.setState("leaving", t);
    const end = t + ms("stand_up") + ms("leave");
    expect(timeline(c, t, end + 1000)).toEqual(["stand_up", "leave"]);
    expect(c.current(end + 1000)).toMatchObject({ clip: "leave", nextChangeAt: null });
  });

  it("the same state again does not restart the loop", () => {
    const [c, t] = settled("working");
    const before = c.current(t);
    c.setState("working", t);
    expect(c.current(t)).toEqual(before);
  });

  it("plays a random idle extra every 8–20 s, deterministically", () => {
    const rolls = [0.5, 0, 0.99];
    let i = 0;
    const random = () => rolls[i++ % rolls.length];
    const c = new AnimationController("idle", 0, { clips: atlas.clips, random });
    const idleStart = ms("spawn");
    // First roll 0.5 → extra after 8 s + 0.5 × 12 s = 14 s; second roll 0 picks idle_look.
    const extraAt = idleStart + 14_000;
    expect(c.current(extraAt - 1).clip).toBe("idle");
    expect(c.current(extraAt - 1).nextChangeAt).toBeLessThanOrEqual(extraAt);
    expect(c.current(extraAt).clip).toBe("idle_look");
    expect(c.current(extraAt + ms("idle_look")).clip).toBe("idle");
  });

  it("reduce motion means no extras", () => {
    const c = controller("idle", 0, () => 0, true);
    expect(timeline(c, 0, 120_000)).toEqual(["spawn", "idle"]);
  });

  it("is continuous: the result does not depend on how often it is asked", () => {
    const sparse = controller("spawning");
    const dense = controller("spawning");
    const changeAt = 3_000;
    sparse.setState("working", changeAt);
    for (let t = 0; t <= changeAt; t += 7) dense.current(t);
    dense.setState("working", changeAt);
    for (const t of [3_050, 3_400, 4_000, 9_000]) {
      expect(sparse.current(t)).toEqual(dense.current(t));
    }
  });
});
