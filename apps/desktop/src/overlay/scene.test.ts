import { describe, expect, it, vi } from "vitest";

import { parseAtlas } from "../sprites/atlas";
import buddy from "../sprites/buddy/atlas.json";
import type { BuddyState, SessionView } from "../types";
import { HitRegionSync } from "./hitSync";
import { BODY_BOX, HOP_MS, Scene, SPRITE_SIZE, StrollTiming, Viewport } from "./scene";

const atlas = parseAtlas(buddy);
const viewport: Viewport = { width: 960, height: 320, devicePixelRatio: 2, scale: 2 };

function view(id: string, state: BuddyState): SessionView {
  return { id, project: id, cwd: `/p/${id}`, state, since: 0, source: "hooks" };
}

describe("Scene", () => {
  it("places buddies bottom-right at whole-pixel integer scale, newest outermost", () => {
    const scene = new Scene(atlas);
    scene.setSessions([view("a", "idle"), view("b", "working")], 0);
    const { sprites } = scene.render(0, viewport);
    // 2 CSS px per sprite px at DPR 2 → 4 device px per sprite px.
    const size = SPRITE_SIZE * 4;
    expect(sprites.map((s) => [s.id, s.x, s.y, s.size])).toEqual([
      ["a", 960 - size, 320 - size, size],
      ["b", 960 - 2 * size - 16, 320 - size, size],
    ]);
  });

  it("rounds fractional DPI to whole device pixels", () => {
    const scene = new Scene(atlas);
    scene.setSessions([view("a", "idle")], 0);
    const { sprites } = scene.render(0, { ...viewport, devicePixelRatio: 1.25 });
    expect(sprites[0].size).toBe(SPRITE_SIZE * 3); // round(2 * 1.25) = 3
    expect(Number.isInteger(sprites[0].x)).toBe(true);
  });

  it("reports hit regions in CSS pixels, around Clawd only", () => {
    const scene = new Scene(atlas);
    scene.setSessions([view("a", "idle")], 0);
    const { regions } = scene.render(0, viewport);
    // The cell is at (416, 96), 64 px; at 2 CSS px per sprite pixel the
    // body box starts 2 px right and 10 px down, 28 × 21 sprite pixels.
    expect(regions).toEqual([
      {
        x: 480 - 64 + BODY_BOX.x * 2,
        y: 160 - 64 + BODY_BOX.y * 2,
        width: BODY_BOX.width * 2,
        height: BODY_BOX.height * 2,
      },
    ]);
  });

  it("asks for the next frame change and sleeps when nothing is on screen", () => {
    const scene = new Scene(atlas, { reduceMotion: true });
    expect(scene.render(0, viewport).nextChangeAt).toBeNull();

    scene.setSessions([view("a", "sleeping")], 1000);
    const spawnFrame = atlas.clips.get("spawn")!.frames[0].duration;
    expect(scene.render(1000, viewport).nextChangeAt).toBe(1000 + spawnFrame);
  });

  it("follows state changes through the animation controller", () => {
    const scene = new Scene(atlas, { reduceMotion: true });
    scene.update(view("a", "idle"), 0);
    const spawn = atlas.clips.get("spawn")!.duration;
    scene.update(view("a", "working"), spawn);
    // sit_down plays first: the next change is its first frame's end.
    const sitFrame = atlas.clips.get("sit_down")!.frames[0].duration;
    expect(scene.render(spawn, viewport).nextChangeAt).toBe(spawn + sitFrame);
  });

  it("a finished leave stops asking for frames", () => {
    const scene = new Scene(atlas, { reduceMotion: true });
    scene.update(view("a", "idle"), 0);
    const spawn = atlas.clips.get("spawn")!.duration;
    scene.update(view("a", "leaving"), spawn);
    const leave = atlas.clips.get("leave")!.duration;
    expect(scene.render(spawn + leave + 1, viewport).nextChangeAt).toBeNull();
  });

  it("snapshots drop missing sessions and remove deletes one", () => {
    const scene = new Scene(atlas);
    scene.setSessions([view("a", "idle"), view("b", "idle")], 0);
    scene.setSessions([view("b", "idle")], 10);
    expect(scene.render(10, viewport).sprites.map((s) => s.id)).toEqual(["b"]);
    scene.remove("b", 20);
    expect(scene.size).toBe(0);
  });
});

describe("HitRegionSync", () => {
  it("sends only when the regions change", async () => {
    const send = vi.fn().mockResolvedValue(undefined);
    const sync = new HitRegionSync(send);
    const a = [{ x: 1, y: 2, width: 3, height: 4 }];
    sync.update(a);
    sync.update([{ x: 1.0001, y: 2, width: 3, height: 4 }]); // sub-pixel noise
    sync.update([...a]);
    expect(send).toHaveBeenCalledTimes(1);
    sync.update([]);
    expect(send).toHaveBeenCalledTimes(2);
    expect(send).toHaveBeenLastCalledWith([]);
  });

  it("resends after a failed send", async () => {
    const send = vi.fn().mockRejectedValueOnce(new Error("ipc")).mockResolvedValue(undefined);
    const sync = new HitRegionSync(send);
    const regions = [{ x: 0, y: 0, width: 1, height: 1 }];
    sync.update(regions);
    await Promise.resolve();
    await Promise.resolve();
    sync.update(regions);
    expect(send).toHaveBeenCalledTimes(2);
  });
});

describe("Scene group layout", () => {
  const ms = (clip: string) => atlas.clips.get(clip)!.duration;
  const settledAt = ms("spawn") + 10;
  /** Device px per sprite px at the test viewport (scale 2, DPR 2). */
  const px = 4;
  const slot = (SPRITE_SIZE + 4) * px;
  const xOf = (scene: Scene, id: string, now: number) =>
    scene.render(now, viewport).sprites.find((s) => s.id === id)?.x;

  it("buddies behind a leaver walk up to close the gap", () => {
    const scene = new Scene(atlas, { reduceMotion: true });
    scene.setSessions([view("a", "idle"), view("b", "idle"), view("c", "idle")], 0);
    const start = xOf(scene, "c", settledAt)!;
    scene.remove("b", settledAt);

    // 1 slot = 36 sprite px at 64 sprite px/s ≈ 563 ms of walking.
    const mid = scene.render(settledAt + 200, viewport);
    const c = mid.sprites.find((s) => s.id === "c")!;
    expect(c.x).toBeGreaterThan(start);
    expect(c.x).toBeLessThan(start + slot);
    expect(Number.isInteger(c.x)).toBe(true);
    expect(c.frame).toEqual(
      atlas.clips.get("walk")!.frames.find((f) => f === c.frame) ?? "walk frame",
    );
    expect(mid.nextChangeAt).toBeLessThanOrEqual(settledAt + 200 + 50);

    expect(xOf(scene, "c", settledAt + 2000)).toBe(start + slot);
    expect(xOf(scene, "a", settledAt + 2000)).toBe(960 - SPRITE_SIZE * px);
  });

  it("the walk depends only on time, not on how often it renders", () => {
    const make = () => {
      const scene = new Scene(atlas, { reduceMotion: true });
      scene.setSessions([view("a", "idle"), view("b", "idle"), view("c", "idle")], 0);
      scene.render(settledAt, viewport);
      scene.remove("a", settledAt);
      return scene;
    };
    const sparse = make();
    const dense = make();
    sparse.render(settledAt, viewport);
    for (let t = settledAt; t < settledAt + 400; t += 16) dense.render(t, viewport);
    expect(xOf(sparse, "c", settledAt + 400)).toBe(xOf(dense, "c", settledAt + 400));
  });

  it("a DPI change moves nobody's slot and starts no walk", () => {
    const scene = new Scene(atlas, { reduceMotion: true });
    scene.setSessions([view("a", "idle"), view("b", "idle")], 0);
    scene.render(settledAt, viewport);
    const after = scene.render(settledAt + 10, { ...viewport, devicePixelRatio: 1 });
    expect(after.sprites[1].x).toBe(960 - 2 * SPRITE_SIZE * 2 - 4 * 2);
  });

  it("a waiting buddy stands 4 sprite px higher", () => {
    const scene = new Scene(atlas, { reduceMotion: true });
    scene.setSessions([view("a", "idle"), view("b", "waiting")], 0);
    const { sprites } = scene.render(settledAt, viewport);
    expect(sprites[0].y - sprites[1].y).toBe(4 * px);
  });

  it("overflow shows ten plus a +N chip with its own hit region", () => {
    const scene = new Scene(atlas, { reduceMotion: true });
    const many = Array.from({ length: 12 }, (_, i) => view(`s${i}`, "idle"));
    scene.setSessions(many, 0);
    const frame = scene.render(settledAt, { ...viewport, width: 4000 });
    expect(frame.sprites).toHaveLength(10);
    expect(frame.chip).toMatchObject({ hidden: 2, x: 4000 - SPRITE_SIZE * px - 10 * slot });
    expect(frame.regions).toHaveLength(11);
    const chipRegion = frame.regions[10];
    expect(scene.chipContains(chipRegion.x + 1, chipRegion.y + 1)).toBe(true);
    expect(scene.chipContains(frame.regions[0].x + 1, frame.regions[0].y + 1)).toBe(false);
  });

  it("expanding shows everyone; hidden buddies appear without walking", () => {
    const scene = new Scene(atlas, { reduceMotion: true });
    scene.setSessions(Array.from({ length: 12 }, (_, i) => view(`s${i}`, "idle")), 0);
    scene.render(settledAt, { ...viewport, width: 4000 });
    scene.setExpanded(true);
    const frame = scene.render(settledAt + 1, { ...viewport, width: 4000 });
    expect(frame.sprites).toHaveLength(12);
    expect(frame.chip).toBeNull();
    expect(scene.chipContains(0, 0)).toBe(false);
    expect(frame.sprites[11].x).toBe(4000 - SPRITE_SIZE * px - 11 * slot);
  });

  it("follows the overlay corner", () => {
    const scene = new Scene(atlas, { reduceMotion: true });
    scene.setCorner("top-left");
    scene.setSessions([view("a", "idle"), view("b", "waiting")], 0);
    const { sprites } = scene.render(settledAt, viewport);
    expect(sprites[0]).toMatchObject({ x: 0, y: 0 });
    expect(sprites[1]).toMatchObject({ x: slot, y: 4 * px });
  });

  it("reports the CSS size the group needs", () => {
    const scene = new Scene(atlas, { reduceMotion: true });
    scene.setSessions([view("a", "idle"), view("b", "idle")], 0);
    expect(scene.render(settledAt, viewport).contentSize).toEqual({
      width: (2 * SPRITE_SIZE + 4) * 2,
      height: (SPRITE_SIZE + 4) * 2,
    });
  });

  it("hops away from the floor after a click, then lands and sleeps again", () => {
    const scene = new Scene(atlas, { reduceMotion: true });
    scene.setSessions([view("a", "sleeping")], 0);
    const rest = scene.render(settledAt, viewport);
    expect(rest.sprites[0].hop).toBe(0);
    scene.hop("a", settledAt);

    const top = scene.render(settledAt + HOP_MS / 2, viewport);
    expect(top.sprites[0].hop).toBe(-6 * px); // 6 sprite px up in a bottom corner
    // Place and hit region stay put, so the hover card does not bounce.
    expect(top.sprites[0]).toMatchObject({ x: rest.sprites[0].x, y: rest.sprites[0].y });
    expect(top.regions).toEqual(rest.regions);
    expect(top.nextChangeAt).toBeLessThanOrEqual(settledAt + HOP_MS / 2 + 34);

    const landed = scene.render(settledAt + HOP_MS, viewport);
    expect(landed.sprites[0].hop).toBe(0);
    // Back to the sleep animation's own pace, as if it had never hopped.
    const untouched = new Scene(atlas, { reduceMotion: true });
    untouched.setSessions([view("a", "sleeping")], 0);
    expect(landed.nextChangeAt).toBe(untouched.render(settledAt + HOP_MS, viewport).nextChangeAt);
  });

  it("hops downwards in top corners and ignores unknown ids", () => {
    const scene = new Scene(atlas, { reduceMotion: true });
    scene.setCorner("top-left");
    scene.setSessions([view("a", "idle")], 0);
    scene.hop("nobody", settledAt);
    scene.hop("a", settledAt);
    expect(scene.render(settledAt + HOP_MS / 2, viewport).sprites[0].hop).toBe(6 * px);
  });

  describe("drag and throw", () => {
    // Bottom-right, 4 device px per sprite px: a buddy is 64 CSS px.
    const restingScene = () => {
      const scene = new Scene(atlas, { reduceMotion: true });
      scene.setSessions([view("a", "idle"), view("b", "idle")], 0);
      scene.render(settledAt, viewport);
      return scene;
    };
    const grabbedClip = atlas.clips.get("grabbed")!;

    it("keeps the group in place when the overlay expands around it", () => {
      const scene = restingScene();
      const small = scene.render(settledAt, viewport).sprites;
      const home = { x: 2000, y: 1000, width: viewport.width, height: viewport.height };
      const big = scene.render(settledAt, { ...viewport, width: 4000, height: 2000, home });
      expect(big.sprites.map((s) => [s.x - home.x, s.y - home.y])).toEqual(
        small.map((s) => [s.x, s.y]),
      );
      expect(big.regions[0]).toEqual({
        x: 1000 + 480 - 64 + BODY_BOX.x * 2,
        y: 500 + 160 - 64 + BODY_BOX.y * 2,
        width: BODY_BOX.width * 2,
        height: BODY_BOX.height * 2,
      });
    });

    it("only grabs buddies that are on screen", () => {
      const scene = restingScene();
      expect(scene.grab("nobody", { x: 0, y: 0 }, settledAt)).toBe(false);
      expect(scene.busy).toBe(false);
      expect(scene.grab("a", { x: 440, y: 120 }, settledAt)).toBe(true);
      expect(scene.busy).toBe(true);
      expect(scene.isThrown("a")).toBe(true);
      expect(scene.isThrown("b")).toBe(false);
    });

    it("a held buddy dangles after the pointer; its hit region follows", () => {
      const scene = restingScene();
      scene.grab("a", { x: 440, y: 120 }, settledAt); // a sits at 416,96 (CSS)
      scene.drag({ x: 340, y: 20 }, settledAt);
      const frame = scene.render(settledAt + 1000, viewport);
      const a = frame.sprites.find((s) => s.id === "a")!;
      expect(grabbedClip.frames).toContain(a.frame);
      expect(a.x / 2).toBeCloseTo(316, 0);
      expect(a.y / 2).toBeCloseTo(-4, 0);
      expect(frame.regions[0].x).toBe(a.x / 2 + BODY_BOX.x * 2);
      expect(frame.nextChangeAt).not.toBeNull();
    });

    it("flies, lands and walks back to its slot, then the scene sleeps again", () => {
      const scene = restingScene();
      const rest = scene.render(settledAt, viewport).sprites[0];
      scene.setWorkArea({ left: -1000, top: -500, right: 480, bottom: 160 });
      scene.grab("a", { x: 440, y: 120 }, settledAt);
      for (let i = 1; i <= 5; i++) scene.drag({ x: 440 - 40 * i, y: 120 - 40 * i }, settledAt + i * 10);
      scene.release(settledAt + 50);
      let now = settledAt + 50;
      let frame = scene.render(now, viewport);
      let left = Infinity;
      while (scene.busy && now < settledAt + 30_000) {
        frame = scene.render((now += 20), viewport);
        left = Math.min(left, frame.sprites[0].x);
      }
      expect(scene.busy).toBe(false);
      expect(left).toBeLessThan(rest.x - 100); // it really flew
      expect(frame.sprites[0]).toMatchObject({ x: rest.x, y: rest.y });
      // Back to the idle animation's own pace: no more redraws at frame rate.
      const settled = scene.render(now + 1, viewport).nextChangeAt;
      expect(settled).not.toBeNull();
      expect(settled! - (now + 1)).toBeGreaterThan(1000 / 30);
    });

    it("a removed session's throw is dropped", () => {
      const scene = restingScene();
      scene.grab("a", { x: 440, y: 120 }, settledAt);
      scene.remove("a", settledAt);
      expect(scene.busy).toBe(false);
      scene.release(settledAt);
      expect(scene.render(settledAt, viewport).sprites.map((s) => s.id)).toEqual(["b"]);
    });
  });
});

describe("Scene redraws", () => {
  /** A seeded random, so the test is the same every run. */
  const seeded = (seed: number) => () => {
    seed = (seed * 1_103_515_245 + 12_345) % 2 ** 31;
    return seed / 2 ** 31;
  };
  const resting: BuddyState[] = ["idle", "working", "researching", "waiting", "sleeping"];

  it("a resting group wakes the loop at most 8 times a second, however many buddies", () => {
    for (const count of [1, 4, 10]) {
      const scene = new Scene(atlas, {
        random: seeded(count),
        strolls: { first: [1e9, 1e9], every: [1e9, 1e9], distance: [50, 50], pause: [0, 0], gap: [0, 0] },
      });
      // They turn up at odd times, in every resting state.
      const sessions: SessionView[] = [];
      for (let i = 0; i < count; i++) {
        sessions.push(view(`s${i}`, resting[i % resting.length]));
        scene.setSessions([...sessions], i * 1_337.7);
      }
      const from = count * 1_337.7 + 5_000;
      const wakeups: number[] = [];
      for (let t = scene.render(from, viewport).nextChangeAt!; t < from + 60_000; ) {
        wakeups.push(t);
        const next = scene.render(t, viewport).nextChangeAt!;
        // Always ahead of now, or the loop would draw the same frame again at once.
        expect(next).toBeGreaterThan(t);
        t = next;
      }
      // Every wakeup is a step of the shared 125 ms grid...
      expect(wakeups.filter((t) => Math.abs(t / 125 - Math.round(t / 125)) > 1e-6)).toEqual([]);
      // ...so there are at most 8 a second, the same for 1 buddy as for 10.
      expect(wakeups.length / 60).toBeLessThanOrEqual(8);
    }
  });
});

describe("Scene excursions", () => {
  const spawnMs = atlas.clips.get("spawn")!.duration;
  const calm: StrollTiming = {
    first: [1000, 1000],
    every: [60_000, 60_000],
    distance: [50, 50],
    pause: [1000, 1000],
    gap: [0, 0],
  };
  const frames = (name: string) => atlas.clips.get(name)!.frames;

  /** Renders every 50 ms from `from` to `until`; returns the frames. */
  function play(scene: Scene, from: number, until: number) {
    const out = [];
    for (let t = from; t <= until; t += 50) out.push({ t, frame: scene.render(t, viewport) });
    return out;
  }

  it("an idle buddy strolls away from the corner and back, one at a time", () => {
    const scene = new Scene(atlas, { strolls: calm, random: () => 0 });
    scene.setSessions([view("a", "idle"), view("b", "idle")], 0);
    const rest = scene.render(spawnMs, viewport).sprites.map((s) => s.x);
    expect(scene.roomNeeded()).toBeNull();
    // a goes at +1 s and is back after 3.5 s; b waits its turn (+4 s) and goes then.
    const seen = play(scene, spawnMs, spawnMs + 12_000);
    const out = seen.filter(({ frame }) => frame.sprites.some((s, i) => s.x !== rest[i]));
    expect(out.length).toBeGreaterThan(0);
    // Bottom-right: it walks left (smaller x), and only one buddy at a time.
    for (const { frame } of out) {
      const moved = frame.sprites.filter((s, i) => s.x !== rest[i]);
      expect(moved).toHaveLength(1);
      expect(moved[0].x).toBeLessThan(rest[frame.sprites.indexOf(moved[0])]);
    }
    // Back in place at the end, and the overlay may shrink again.
    expect(seen.at(-1)!.frame.sprites.map((s) => s.x)).toEqual(rest);
    expect(scene.roomNeeded()).toBeNull();
  });

  it("keeps a quiet time between strolls, whoever goes", () => {
    const scene = new Scene(atlas, { strolls: { ...calm, gap: [20_000, 20_000] }, random: () => 0 });
    scene.setSessions([view("a", "idle"), view("b", "idle")], 0);
    const rest = scene.render(spawnMs, viewport).sprites.map((s) => s.x);
    const moving = (t: number) => scene.render(t, viewport).sprites.some((s, i) => s.x !== rest[i]);
    // a goes at +1 s and is home by +4.5 s; b may not go before +24.5 s.
    const seen: { t: number; moving: boolean }[] = [];
    for (let t = spawnMs; t <= spawnMs + 30_000; t += 50) seen.push({ t, moving: moving(t) });
    const starts = seen.filter((s, i) => s.moving && !seen[i - 1]?.moving).map((s) => s.t - spawnMs);
    expect(starts).toHaveLength(2);
    expect(starts[1] - starts[0]).toBeGreaterThanOrEqual(3_500 + 20_000);
  });

  it("asks for a floor strip while someone strolls", () => {
    const scene = new Scene(atlas, { strolls: calm, random: () => 0 });
    scene.setSessions([view("a", "idle")], 0);
    play(scene, spawnMs, spawnMs + 1500);
    expect(scene.roomNeeded()).toBe("floor");
  });

  it("never strolls with reduce motion, while busy or under the pointer", () => {
    for (const setup of [
      (s: Scene) => s.setReduceMotion(true, 0),
      (s: Scene) => s.update(view("a", "working"), 0),
      (s: Scene) => s.setHovered("a"),
    ]) {
      const scene = new Scene(atlas, { strolls: calm, random: () => 0 });
      scene.setSessions([view("a", "idle")], 0);
      setup(scene);
      play(scene, spawnMs, 5000);
      expect(scene.roomNeeded()).toBeNull();
    }
  });

  it("a stroller called back to work walks back and sits down", () => {
    const scene = new Scene(atlas, { strolls: calm, random: () => 0 });
    scene.setSessions([view("a", "idle")], 0);
    const rest = scene.render(spawnMs, viewport).sprites[0].x;
    play(scene, spawnMs, 1600);
    scene.update(view("a", "working"), 1600);
    const seen = play(scene, 1650, 6000);
    const back = seen.find(({ frame }) => frame.sprites[0].x === rest)!;
    expect(back).toBeDefined();
    const after = seen.filter(({ t }) => t > back.t).map(({ frame }) => frame.sprites[0].frame);
    expect(after.some((f) => frames("sit_down").includes(f))).toBe(true);
  });

  it("a session messaging another walks its buddy over to talk", () => {
    const never = { ...calm, first: [1e9, 1e9] as [number, number] };
    const scene = new Scene(atlas, { strolls: never });
    scene.setSessions([view("a", "idle"), view("b", "idle")], 0);
    const rest = scene.render(spawnMs, viewport).sprites;
    scene.interact("b", "a", spawnMs);
    const seen = play(scene, spawnMs, spawnMs + 12_000);
    // b walks towards a (to the right in this corner), talks, and is back.
    const talking = seen.filter(({ frame }) => frames("talk").includes(frame.sprites[1].frame));
    expect(talking.length).toBeGreaterThan(0);
    expect(talking[0].frame.sprites[1].x).toBeGreaterThan(rest[1].x);
    // a answers with a bubble while b talks.
    expect(seen.some(({ frame }) => frame.effects.some((e) => e.id === "a"))).toBe(true);
    expect(seen.at(-1)!.frame.sprites[1].x).toBe(rest[1].x);
    expect(seen.at(-1)!.frame.effects).toHaveLength(0);
  });

  it("without a known recipient, the sender just shows a speech bubble", () => {
    const scene = new Scene(atlas);
    scene.setSessions([view("a", "working")], 0);
    scene.interact("a", null, spawnMs);
    const frame = scene.render(spawnMs + 100, viewport);
    expect(frame.effects.map((e) => e.id)).toEqual(["a"]);
    expect(frames("fx_talk")).toContain(frame.effects[0].frame);
    expect(scene.roomNeeded()).toBeNull();
    expect(scene.render(spawnMs + 3000, viewport).effects).toHaveLength(0);
  });
});
