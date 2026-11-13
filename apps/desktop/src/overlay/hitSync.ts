import type { HitRegion } from "./native";

/**
 * Sends hit regions to the backend only when they actually change, so a
 * redraw that moves nothing costs no IPC.
 */
export class HitRegionSync {
  private last: string | null = null;

  constructor(private readonly send: (regions: HitRegion[]) => Promise<void>) {}

  update(regions: HitRegion[]): void {
    // Rounded to 1/100 px: sub-pixel noise is not a change.
    const key = JSON.stringify(
      regions.map((r) => [r.x, r.y, r.width, r.height].map((v) => Math.round(v * 100))),
    );
    if (key === this.last) return;
    this.last = key;
    this.send(regions).catch(() => {
      // Retry on the next change rather than hold a stale "sent" state.
      this.last = null;
    });
  }
}
