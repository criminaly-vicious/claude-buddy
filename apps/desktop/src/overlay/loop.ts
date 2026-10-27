/**
 * Render scheduler that sleeps whenever nothing will change. `draw` returns
 * when the picture next changes (an absolute time in ms, e.g. the next
 * sprite frame) or `null` if it is static. The loop then waits with a timer
 * until that time, never faster than `maxFps`, and draws inside an animation
 * frame. With nothing animating, no timer and no frame callback is pending.
 */

export interface LoopHost {
  now(): number;
  setTimer(callback: () => void, delayMs: number): unknown;
  clearTimer(handle: unknown): void;
  requestFrame(callback: () => void): void;
}

export const browserHost: LoopHost = {
  now: () => performance.now(),
  setTimer: (callback, delayMs) => setTimeout(callback, delayMs),
  clearTimer: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
  requestFrame: (callback) => requestAnimationFrame(() => callback()),
};

export class RenderLoop {
  private timer: unknown = null;
  /** When the pending timer fires, or `null` if none is pending. */
  private scheduledAt: number | null = null;
  private lastDraw = -Infinity;
  private stopped = false;
  private readonly minInterval: number;

  constructor(
    private readonly draw: (now: number) => number | null,
    private readonly host: LoopHost = browserHost,
    maxFps = 30,
  ) {
    this.minInterval = 1000 / maxFps;
  }

  /** Something changed (new session, state, resize): redraw soon. */
  invalidate(): void {
    this.schedule(this.host.now());
  }

  /** Whether a draw is pending. False means the loop is asleep. */
  get awake(): boolean {
    return this.scheduledAt !== null;
  }

  stop(): void {
    this.stopped = true;
    if (this.timer !== null) this.host.clearTimer(this.timer);
    this.timer = null;
    this.scheduledAt = null;
  }

  private schedule(at: number): void {
    if (this.stopped) return;
    const when = Math.max(at, this.lastDraw + this.minInterval);
    if (this.scheduledAt !== null && this.scheduledAt <= when) return;
    if (this.timer !== null) this.host.clearTimer(this.timer);
    this.scheduledAt = when;
    const delay = Math.max(0, when - this.host.now());
    this.timer = this.host.setTimer(() => {
      this.timer = null;
      this.host.requestFrame(() => this.frame());
    }, delay);
  }

  private frame(): void {
    if (this.stopped) return;
    const now = this.host.now();
    this.scheduledAt = null;
    this.lastDraw = now;
    const next = this.draw(now);
    if (next !== null) this.schedule(next);
  }
}
