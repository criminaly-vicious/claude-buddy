import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...args: unknown[]) => invoke(...args) }));

const native = await import("./native");

describe("overlay native commands", () => {
  beforeEach(() => invoke.mockReset().mockResolvedValue(undefined));

  it("sends hit regions under the argument name the Rust command expects", async () => {
    const regions = [{ x: 1, y: 2, width: 32, height: 32 }];
    await native.setHitRegions(regions);
    expect(invoke).toHaveBeenCalledWith("set_hit_regions", { regions });
  });

  it("sends the anchor in the Rust shape", async () => {
    await native.setOverlayAnchor({ corner: "top-left", monitor: null });
    expect(invoke).toHaveBeenCalledWith("set_overlay_anchor", {
      anchor: { corner: "top-left", monitor: null },
    });
  });

  it("asks the backend to focus a session by id", async () => {
    invoke.mockResolvedValue(true);
    await expect(native.focusSession("abc")).resolves.toBe(true);
    expect(invoke).toHaveBeenCalledWith("focus_session", { id: "abc" });
  });

  it("maps element bounds to a region", () => {
    const element = {
      getBoundingClientRect: () => ({ left: 4, top: 5, width: 6, height: 7 }),
    } as Element;
    expect(native.regionOf(element)).toEqual({ x: 4, y: 5, width: 6, height: 7 });
  });
});
