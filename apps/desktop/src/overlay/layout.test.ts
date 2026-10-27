import { describe, expect, it } from "vitest";

import { layout, LayoutInput, MAX_VISIBLE, toCanvas } from "./layout";

const base: Omit<LayoutInput, "ids"> = {
  waiting: new Set(),
  size: 64,
  gap: 8,
  step: 8,
  expanded: false,
};
const ids = (n: number) => Array.from({ length: n }, (_, i) => `s${i}`);

describe("layout", () => {
  it("lines buddies up from the corner, oldest first, gap apart", () => {
    const { buddies, chip, extent } = layout({ ...base, ids: ids(3) });
    expect([...buddies.values()].map((p) => p.offset)).toEqual([0, 72, 144]);
    expect(chip).toBeNull();
    expect(extent).toEqual({ along: 3 * 64 + 2 * 8, across: 64 + 8 });
  });

  it("keeps everyone's order when a buddy in the middle leaves", () => {
    const before = layout({ ...base, ids: ["a", "b", "c", "d"] });
    const after = layout({ ...base, ids: ["a", "c", "d"] });
    expect(after.buddies.get("a")).toEqual(before.buddies.get("a"));
    // Those behind the gap move up one slot; nobody swaps places.
    expect(after.buddies.get("c")!.offset).toBe(before.buddies.get("b")!.offset);
    expect(after.buddies.get("d")!.offset).toBe(before.buddies.get("c")!.offset);
  });

  it("a newcomer takes the outer end without moving anyone", () => {
    const before = layout({ ...base, ids: ["a", "b"] });
    const after = layout({ ...base, ids: ["a", "b", "c"] });
    expect(after.buddies.get("a")).toEqual(before.buddies.get("a"));
    expect(after.buddies.get("b")).toEqual(before.buddies.get("b"));
  });

  it("shows ten and a +N chip for the rest, newest hidden", () => {
    const all = ids(13);
    const { buddies, chip, extent } = layout({ ...base, ids: all });
    expect(buddies.size).toBe(MAX_VISIBLE);
    expect(buddies.has("s9")).toBe(true);
    expect(buddies.has("s10")).toBe(false);
    expect(chip).toEqual({ offset: 10 * 72, lift: 0, hidden: 3 });
    expect(extent.along).toBe(11 * 64 + 10 * 8);
  });

  it("exactly ten needs no chip", () => {
    expect(layout({ ...base, ids: ids(10) }).chip).toBeNull();
  });

  it("expanded shows everyone and no chip", () => {
    const { buddies, chip } = layout({ ...base, ids: ids(13), expanded: true });
    expect(buddies.size).toBe(13);
    expect(chip).toBeNull();
  });

  it("a waiting buddy steps forward", () => {
    const { buddies } = layout({ ...base, ids: ["a", "b"], waiting: new Set(["b"]) });
    expect(buddies.get("a")!.lift).toBe(0);
    expect(buddies.get("b")!.lift).toBe(8);
  });

  it("nothing to show takes no space along the floor", () => {
    expect(layout({ ...base, ids: [] }).extent.along).toBe(0);
  });
});

describe("toCanvas (all four corners)", () => {
  const canvas = { width: 800, height: 200 };
  const pos = { offset: 72, lift: 8 };

  it.each([
    ["bottom-right", { x: 800 - 64 - 72, y: 200 - 64 - 8 }],
    ["bottom-left", { x: 72, y: 200 - 64 - 8 }],
    ["top-right", { x: 800 - 64 - 72, y: 8 }],
    ["top-left", { x: 72, y: 8 }],
  ] as const)("%s", (corner, expected) => {
    expect(toCanvas(pos, corner, canvas, 64)).toEqual(expected);
  });

  it("the oldest buddy touches the corner", () => {
    expect(toCanvas({ offset: 0, lift: 0 }, "bottom-right", canvas, 64)).toEqual({
      x: 736,
      y: 136,
    });
    expect(toCanvas({ offset: 0, lift: 0 }, "top-left", canvas, 64)).toEqual({ x: 0, y: 0 });
  });
});
