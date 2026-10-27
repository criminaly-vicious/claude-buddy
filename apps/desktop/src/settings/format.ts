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
