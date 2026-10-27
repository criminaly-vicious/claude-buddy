import { describe, expect, it, vi } from "vitest";

import { parseAtlas } from "../sprites/atlas";
import placeholder from "../sprites/placeholder/atlas.json";
import type { BuddyState, SessionView } from "../types";
import { HitRegionSync } from "./hitSync";
import { Scene, SPRITE_SIZE, Viewport } from "./scene";

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
    scene.remove("b");
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
