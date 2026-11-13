import { describe, expect, it } from "vitest";

import type { SessionView } from "../types";
import {
  cardLines,
  clip,
  formatElapsed,
  HOVER_DELAY_MS,
  HoverState,
  MAX_LINE_CHARS,
  placeCard,
} from "./hoverCard";

const base: SessionView = {
  id: "a",
  project: "claude-buddy",
  cwd: "/p/claude-buddy",
  state: "working",
  since: 0,
  source: "hooks",
};

describe("clip", () => {
  it("leaves short text alone", () => {
    expect(clip("Edit · app.ts")).toBe("Edit · app.ts");
  });

  it("cuts long text to 80 characters with an ellipsis", () => {
    const cut = clip("x".repeat(200));
    expect(Array.from(cut)).toHaveLength(MAX_LINE_CHARS);
    expect(cut.endsWith("…")).toBe(true);
  });

  it("counts Unicode code points, never splitting a character", () => {
    const cut = clip("🦀".repeat(100));
    expect(Array.from(cut)).toHaveLength(MAX_LINE_CHARS);
    expect(cut.startsWith("🦀🦀")).toBe(true);
    expect(cut).not.toContain("�");
  });

  it("strips control characters", () => {
    expect(clip("line\nbreak\u001b[31m")).toBe("line break [31m");
  });
});

describe("formatElapsed", () => {
  it.each([
    [0, "0s"],
    [14_900, "14s"],
    [134_000, "2m 14s"],
    [3_900_000, "1h 5m"],
    [-5, "0s"],
  ])("%d ms → %s", (ms, text) => {
    expect(formatElapsed(ms)).toBe(text);
  });
});

describe("cardLines", () => {
  it("project, state and time, then the activity", () => {
    const lines = cardLines({ ...base, summary: "Edit · src/app.ts" }, 134_000);
    expect(lines).toEqual(["claude-buddy · Working · 2m 14s", "Edit · src/app.ts"]);
  });

  it("falls back to the tool name, and to nothing", () => {
    expect(cardLines({ ...base, tool: "Bash" }, 0)[1]).toBe("Bash");
    expect(cardLines(base, 0)).toHaveLength(1);
  });

  it("labels waiting for the user plainly", () => {
    expect(cardLines({ ...base, state: "waiting" }, 0)[0]).toContain("Waiting for you");
  });

  it("marks transcript-only sessions as approximate", () => {
    const lines = cardLines({ ...base, source: "transcript", summary: "Read · a.ts" }, 0);
    expect(lines).toHaveLength(3);
    expect(lines[2]).toMatch(/^Approximate/);
  });

  it("never exceeds 3 lines of 80 characters", () => {
    const long = { ...base, project: "p".repeat(200), summary: "s".repeat(200), source: "transcript" as const };
    const lines = cardLines(long, 0);
    expect(lines.length).toBeLessThanOrEqual(3);
    for (const line of lines) expect(Array.from(line).length).toBeLessThanOrEqual(80);
  });

  it("an empty project name still reads well", () => {
    expect(cardLines({ ...base, project: "" }, 0)[0]).toBe("session · Working · 0s");
  });
});

describe("HoverState", () => {
  it("opens only after the pointer rests 300 ms on one buddy", () => {
    const hover = new HoverState();
    hover.pointer("a", 1000);
    expect(hover.openId(1000 + HOVER_DELAY_MS - 1)).toBeNull();
    expect(hover.nextChangeAt(1000, null)).toBe(1000 + HOVER_DELAY_MS);
    expect(hover.openId(1000 + HOVER_DELAY_MS)).toBe("a");
  });

  it("moving to another buddy restarts the delay", () => {
    const hover = new HoverState();
    hover.pointer("a", 0);
    hover.pointer("b", 200);
    expect(hover.openId(400)).toBeNull();
    expect(hover.openId(500)).toBe("b");
  });

  it("stays open while the pointer is on the card, closes when it leaves both", () => {
    const hover = new HoverState();
    hover.pointer("a", 0);
    expect(hover.openId(300)).toBe("a");
    hover.pointer("card", 400);
    expect(hover.openId(5000)).toBe("a");
    hover.pointer(null, 5100);
    expect(hover.openId(5100)).toBeNull();
    expect(hover.nextChangeAt(5100, null)).toBeNull();
  });

  it("the card itself does not open anything", () => {
    const hover = new HoverState();
    hover.pointer("card", 0);
    expect(hover.openId(1000)).toBeNull();
  });

  it("ticks once per second while open, on the elapsed-time boundary", () => {
    const hover = new HoverState();
    hover.pointer("a", 0);
    hover.openId(300);
    // State started at t=50: the label changes at 1050, 2050, ...
    expect(hover.nextChangeAt(300, 50)).toBe(1050);
    expect(hover.nextChangeAt(1050, 50)).toBe(2050);
  });
});

describe("placeCard", () => {
  const canvas = { width: 800, height: 300 };
  const card = { width: 300, height: 60 };
  const gap = 8;

  it("sits above the buddy and grows inwards in a bottom-right corner", () => {
    const buddy = { x: 600, y: 236, width: 64, height: 64 };
    expect(placeCard(buddy, card, canvas, "bottom-right", gap)).toEqual({
      x: 664 - 300,
      y: 236 - 8 - 60,
      width: 300,
      height: 60,
    });
  });

  it("sits below the buddy in a top-left corner", () => {
    const buddy = { x: 72, y: 0, width: 64, height: 64 };
    expect(placeCard(buddy, card, canvas, "top-left", gap)).toMatchObject({ x: 72, y: 72 });
  });

  it("flips back inside near an edge", () => {
    const nearRight = { x: 700, y: 236, width: 64, height: 64 };
    expect(placeCard(nearRight, card, canvas, "bottom-left", gap).x).toBe(800 - 300);
    const nearLeft = { x: 10, y: 236, width: 64, height: 64 };
    expect(placeCard(nearLeft, card, canvas, "bottom-right", gap).x).toBe(0);
  });

  it("never leaves the canvas vertically", () => {
    const buddy = { x: 600, y: 20, width: 64, height: 64 };
    expect(placeCard(buddy, card, canvas, "bottom-right", gap).y).toBe(0);
  });
});
