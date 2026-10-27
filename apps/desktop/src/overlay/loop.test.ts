import { describe, expect, it } from "vitest";

import { LoopHost, RenderLoop } from "./loop";

/** Deterministic host: a manual clock, a timer queue, immediate frames. */
class FakeHost implements LoopHost {
  time = 0;
  private timers = new Map<number, { at: number; callback: () => void }>();
  private nextId = 1;

  now = () => this.time;
  setTimer = (callback: () => void, delayMs: number) => {
    const id = this.nextId++;
    this.timers.set(id, { at: this.time + delayMs, callback });
    return id;
  };
  clearTimer = (handle: unknown) => {
    this.timers.delete(handle as number);
  };
  requestFrame = (callback: () => void) => callback();

  get pending() {
    return this.timers.size;
  }

  /** Advances the clock, firing due timers in order. */
  advance(ms: number) {
    const end = this.time + ms;
    for (;;) {
      const due = [...this.timers.entries()]
        .filter(([, t]) => t.at <= end)
        .sort((a, b) => a[1].at - b[1].at)[0];
      if (!due) break;
      this.timers.delete(due[0]);
      this.time = due[1].at;
      due[1].callback();
    }
    this.time = end;
  }
}

describe("RenderLoop", () => {
  it("sleeps when the picture is static", () => {
    const host = new FakeHost();
    const draws: number[] = [];
    const loop = new RenderLoop((now) => (draws.push(now), null), host);

    loop.invalidate();
    host.advance(1000);
    expect(draws).toEqual([0]);
    expect(loop.awake).toBe(false);
    expect(host.pending).toBe(0);
  });

  it("wakes only when the next frame is due", () => {
    const host = new FakeHost();
    const draws: number[] = [];
    // A clip whose frames change every 250 ms.
    const loop = new RenderLoop((now) => (draws.push(now), Math.floor(now / 250) * 250 + 250), host);

    loop.invalidate();
    host.advance(1000);
    expect(draws).toEqual([0, 250, 500, 750, 1000]);
  });

  it("never exceeds 30 fps", () => {
    const host = new FakeHost();
    const draws: number[] = [];
    // Asks to redraw immediately every time.
    const loop = new RenderLoop((now) => (draws.push(now), now), host);

    loop.invalidate();
    host.advance(1000);
    expect(draws.length).toBeLessThanOrEqual(31);
    for (let i = 1; i < draws.length; i++) {
      expect(draws[i] - draws[i - 1]).toBeGreaterThanOrEqual(1000 / 30 - 1e-9);
    }
  });

  it("coalesces repeated invalidations into one draw", () => {
    const host = new FakeHost();
    let draws = 0;
    const loop = new RenderLoop(() => (draws++, null), host);
    loop.invalidate();
    loop.invalidate();
    loop.invalidate();
    host.advance(100);
    expect(draws).toBe(1);
  });

  it("an earlier invalidation replaces a later scheduled frame", () => {
    const host = new FakeHost();
    const draws: number[] = [];
    let next: number | null = 10_000;
    const loop = new RenderLoop((now) => (draws.push(now), next), host);
    loop.invalidate(); // draws at 0, schedules 10 000
    host.advance(500);
    next = null;
    loop.invalidate(); // a change now must not wait until 10 000
    host.advance(100);
    expect(draws).toEqual([0, 500]);
    expect(loop.awake).toBe(false);
  });

  it("stop cancels everything", () => {
    const host = new FakeHost();
    let draws = 0;
    const loop = new RenderLoop(() => (draws++, 0), host);
    loop.invalidate();
    loop.stop();
    host.advance(1000);
    expect(draws).toBe(0);
    expect(host.pending).toBe(0);
  });
});
