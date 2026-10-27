import { describe, expect, it, vi } from "vitest";

import { parseAtlas } from "../sprites/atlas";
import placeholder from "../sprites/placeholder/atlas.json";
import type { BuddyState, SessionView } from "../types";
import { HitRegionSync } from "./hitSync";
import { HOP_MS, Scene, SPRITE_SIZE, Viewport } from "./scene";

const atlas = parseAtlas(placeholder);
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

  it("reports hit regions in CSS pixels", () => {
    const scene = new Scene(atlas);
    scene.setSessions([view("a", "idle")], 0);
    const { regions } = scene.render(0, viewport);
    expect(regions).toEqual([{ x: 480 - 64, y: 160 - 64, width: 64, height: 64 }]);
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
    expect(mid.nextChangeAt).toBeLessThanOrEqual(settledAt + 200 + 34);

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
    expect(landed.nextChangeAt).toBe(rest.nextChangeAt);
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
      expect(big.regions[0]).toEqual({ x: 1000 + 480 - 64, y: 500 + 160 - 64, width: 64, height: 64 });
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
      expect(frame.regions[0].x).toBe(a.x / 2);
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
      // Back to the idle animation's own pace, as if it had never been thrown.
      const untouched = restingScene();
      expect(scene.render(now + 1, viewport).nextChangeAt).toBe(
        untouched.render(now + 1, viewport).nextChangeAt,
      );
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
