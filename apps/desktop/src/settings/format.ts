/** Text for the settings window. Pure, so it is tested without a DOM. */

import type { HookStatus, MonitorInfo } from "../config";

/** "never since Claude Buddy started", "just now", "42 s ago", "3 min ago", "2 h ago". */
export function lastEventText(at: number | null, now: number): string {
  if (at === null) return "none since Claude Buddy started";
  const s = Math.max(0, Math.floor((now - at) / 1000));
  if (s < 10) return "just now";
  if (s < 60) return `${s} s ago`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m} min ago`;
  return `${Math.floor(m / 60)} h ago`;
}

/** "2: DELL U2720Q, 2560×1440 (primary)". Numbered from 1 for people. */
export function monitorLabel(monitor: MonitorInfo): string {
  const name = monitor.name?.replace(/^\\\\\.\\/, "") || "Monitor";
  const primary = monitor.primary ? " (primary)" : "";
  return `${monitor.index + 1}: ${name}, ${monitor.width}×${monitor.height}${primary}`;
}

/** The key part of a `KeyboardEvent`, as the hotkey text expects it. */
export interface KeyPress {
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
  /** `KeyboardEvent.code`: layout-independent, e.g. "KeyB", "Digit7", "F9". */
  code: string;
}

/**
 * The hotkey text for a key press ("Ctrl+Alt+B"), or `null` while only
 * modifiers are down or for a key the backend does not accept.
 */
export function hotkeyFromPress(press: KeyPress): string | null {
  const key =
    /^Key([A-Z])$/.exec(press.code)?.[1] ??
    /^Digit([0-9])$/.exec(press.code)?.[1] ??
    (/^F([1-9]|1[0-9]|2[0-4])$/.test(press.code) ? press.code : null) ??
    (press.code === "Space" ? "Space" : null);
  if (!key) return null;
  const parts = [
    press.ctrlKey && "Ctrl",
    press.altKey && "Alt",
    press.shiftKey && "Shift",
    press.metaKey && "Win",
  ].filter((p): p is string => Boolean(p));
  return [...parts, key].join("+");
}

/** The hook status line and whether it needs attention. */
export function hookStatusText(status: HookStatus): { text: string; ok: boolean } {
  if (!status.bridgeRunning) {
    return { text: "Claude Buddy's bridge is not running: restart the app.", ok: false };
  }
  if (status.installed === null) {
    return { text: "Could not read Claude Code's settings.", ok: false };
  }
  if (!status.installed) {
    return {
      text: "Hooks are not installed. Buddies are approximate until you install them from the tray menu.",
      ok: false,
    };
  }
  return { text: "Hooks are installed.", ok: true };
}
