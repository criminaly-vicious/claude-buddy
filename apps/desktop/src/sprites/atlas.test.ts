import { describe, expect, it } from "vitest";

import placeholder from "./placeholder/atlas.json";
import { AtlasError, frameAt, parseAtlas, requireClips } from "./atlas";

/** Every animation name in docs/animation-spec.md. */
const SPEC_ANIMATIONS = [
  "spawn",
  "idle",
  "sit_down",
  "work_type",
  "research_read",
  "stand_up",
  "wait_wave",
  "sleep",
  "walk",
  "grabbed",
  "fly",
  "land",
  "dizzy",
  "leave",
];

const frame = (x: number, duration?: number) => ({ frame: { x, y: 0, w: 32, h: 32 }, duration });

describe("parseAtlas", () => {
  it("reads array exports with tags and durations", () => {
    const atlas = parseAtlas({
      frames: [frame(0, 100), frame(32, 200), frame(64, 50)],
      meta: { image: "a.png", frameTags: [{ name: "walk", from: 0, to: 1 }] },
    });
    expect(atlas.image).toBe("a.png");
    const walk = atlas.clips.get("walk")!;
    expect(walk.frames.map((f) => f.x)).toEqual([0, 32]);
    expect(walk.duration).toBe(300);
  });

  it("reads hash exports in key order", () => {
    const atlas = parseAtlas({
      frames: { "b 0": frame(0), "b 1": frame(32) },
      meta: { image: "b.png", frameTags: [{ name: "b", from: 0, to: 1 }] },
    });
    expect(atlas.clips.get("b")!.frames.map((f) => f.x)).toEqual([0, 32]);
    expect(atlas.clips.get("b")!.duration).toBe(200);
  });

  it("applies reverse and pingpong directions", () => {
    const frames = [frame(0), frame(32), frame(64)];
    const atlas = parseAtlas({
      frames,
      meta: {
        image: "c.png",
        frameTags: [
          { name: "back", from: 0, to: 2, direction: "reverse" },
          { name: "bounce", from: 0, to: 2, direction: "pingpong" },
        ],
      },
    });
    expect(atlas.clips.get("back")!.frames.map((f) => f.x)).toEqual([64, 32, 0]);
    expect(atlas.clips.get("bounce")!.frames.map((f) => f.x)).toEqual([0, 32, 64, 32]);
  });

  it("rejects broken atlases", () => {
    expect(() => parseAtlas({})).toThrow(AtlasError);
    const meta = (frameTags: unknown) => ({ frames: [frame(0)], meta: { image: "x", frameTags } });
    expect(() => parseAtlas(meta([{ name: "a", from: 0, to: 3 }]))).toThrow(/out of range/);
    expect(() =>
      parseAtlas(meta([{ name: "a", from: 0, to: 0 }, { name: "a", from: 0, to: 0 }])),
    ).toThrow(/twice/);
  });

  it("the placeholder atlas has every animation in the spec", () => {
    const atlas = parseAtlas(placeholder);
    expect(() => requireClips(atlas, SPEC_ANIMATIONS)).not.toThrow();
    expect(() => requireClips(atlas, ["nope"])).toThrow(/nope/);
  });
});

describe("frameAt", () => {
  const clip = parseAtlas({
    frames: [frame(0, 100), frame(32, 200)],
    meta: { image: "x", frameTags: [{ name: "c", from: 0, to: 1 }] },
  }).clips.get("c")!;

  it("picks the frame and the next change time", () => {
    expect(frameAt(clip, 0, true)).toEqual({ frame: clip.frames[0], nextChangeAt: 100 });
    expect(frameAt(clip, 150, true)).toEqual({ frame: clip.frames[1], nextChangeAt: 300 });
  });

  it("loops", () => {
    expect(frameAt(clip, 350, true)).toEqual({ frame: clip.frames[0], nextChangeAt: 400 });
  });

  it("holds the last frame of a one-shot clip and stops", () => {
    expect(frameAt(clip, 150, false).nextChangeAt).toBe(300);
    expect(frameAt(clip, 999, false)).toEqual({ frame: clip.frames[1], nextChangeAt: null });
  });

  it("a single-frame loop never needs a redraw", () => {
    const still = parseAtlas({
      frames: [frame(0, 100)],
      meta: { image: "x", frameTags: [{ name: "s", from: 0, to: 0 }] },
    }).clips.get("s")!;
    expect(frameAt(still, 5_000, true).nextChangeAt).toBeNull();
  });
});
