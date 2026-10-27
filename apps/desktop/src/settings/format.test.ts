import { describe, expect, it } from "vitest";

import { hookStatusText, lastEventText, monitorLabel } from "./format";

describe("settings text", () => {
  it("describes the last hook event", () => {
    const now = 1_000_000_000;
    expect(lastEventText(null, now)).toBe("none since Claude Buddy started");
    expect(lastEventText(now - 3_000, now)).toBe("just now");
    expect(lastEventText(now - 42_000, now)).toBe("42 s ago");
    expect(lastEventText(now - 3 * 60_000 - 5_000, now)).toBe("3 min ago");
    expect(lastEventText(now - 2 * 3_600_000, now)).toBe("2 h ago");
    expect(lastEventText(now + 5_000, now)).toBe("just now"); // clock skew
  });

  it("labels monitors from 1, without the device prefix", () => {
    expect(
      monitorLabel({ index: 0, name: "\\\\.\\DISPLAY1", width: 2560, height: 1440, primary: true }),
    ).toBe("1: DISPLAY1, 2560×1440 (primary)");
    expect(monitorLabel({ index: 1, name: null, width: 1920, height: 1080, primary: false })).toBe(
      "2: Monitor, 1920×1080",
    );
  });

  it("summarises the hook status, most urgent first", () => {
    const base = { installed: true, settingsPath: "/x", lastEventAt: null, bridgeRunning: true };
    expect(hookStatusText(base)).toEqual({ text: "Hooks are installed.", ok: true });
    expect(hookStatusText({ ...base, installed: false }).ok).toBe(false);
    expect(hookStatusText({ ...base, installed: null }).text).toContain("Could not read");
    expect(hookStatusText({ ...base, bridgeRunning: false }).text).toContain("not running");
  });
});
