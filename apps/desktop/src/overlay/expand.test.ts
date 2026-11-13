import { describe, expect, it } from "vitest";

import { flightArea, homeArea } from "./expand";

const layout = { width: 1920, height: 1032, home: { x: 1432, y: 864, width: 480, height: 160 } };

describe("expanded overlay", () => {
  it("draws the group in the home area once the window has grown", () => {
    expect(homeArea({ width: 1920, height: 1032 }, layout)).toEqual(layout.home);
    expect(homeArea({ width: 1921, height: 1031 }, layout)).toEqual(layout.home);
  });

  it("uses the whole canvas before the window grows and after it shrinks", () => {
    expect(homeArea({ width: 480, height: 160 }, layout)).toEqual({ x: 0, y: 0, width: 480, height: 160 });
    expect(homeArea({ width: 480, height: 160 }, null)).toEqual({ x: 0, y: 0, width: 480, height: 160 });
  });

  it("flies in the work area, relative to home", () => {
    expect(flightArea({ width: 480, height: 160 }, layout)).toEqual({
      left: -1432,
      top: -864,
      right: 488,
      bottom: 168,
    });
    // Browser mock: no backend, the page is the work area.
    expect(flightArea({ width: 800, height: 600 }, null)).toEqual({ left: 0, top: 0, right: 800, bottom: 600 });
  });
});
