/**
 * Hover balloon (docs/animation-spec.md#hover-card-content-rules): what a
 * session is doing, in a speech balloon over its buddy, 300 ms after the
 * pointer rests on it. At most 3 lines of 80 characters. The text comes from
 * the backend already sanitised; it is cut again here as a second barrier.
 *
 * Pure: content, timing and placement are computed from inputs, so they are
 * tested without a canvas.
 */

import type { BuddyState, SessionView, SubagentView } from "../types";
import type { Corner } from "./native";

export const HOVER_DELAY_MS = 300;
export const MAX_LINES = 3;
export const MAX_LINE_CHARS = 80;

export const STATE_LABELS: Record<BuddyState, string> = {
  spawning: "Starting",
  idle: "Idle",
  working: "Working",
  researching: "Researching",
  waiting: "Waiting for you",
  sleeping: "Sleeping",
  leaving: "Leaving",
};

/** What the balloon says about a state when no tool tells more. */
const STATE_SENTENCES: Record<BuddyState, string> = {
  spawning: "Starting up",
  idle: "Done, waiting for a prompt",
  working: "Thinking",
  researching: "Looking things up",
  waiting: "Waiting for you",
  sleeping: "Sleeping",
  leaving: "Saying goodbye",
};

const APPROXIMATE_HINT = "Approximate: connect Claude Code for exact states";

/** Cuts `text` to `max` characters (Unicode code points), ending with `…` when cut. */
export function clip(text: string, max = MAX_LINE_CHARS): string {
  // Control characters (newlines, escapes) never reach the balloon.
  const isControl = (ch: string) => ch.charCodeAt(0) < 0x20 || ch.charCodeAt(0) === 0x7f;
  const chars = Array.from(
    Array.from(text, (ch) => (isControl(ch) ? " " : ch))
      .join("")
      .trim(),
  );
  if (chars.length <= max) return chars.join("");
  return chars.slice(0, max - 1).join("") + "…";
}

/** `14s`, `2m 14s`, `1h 5m`. */
export function formatElapsed(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ${s % 60}s`;
  return `${Math.floor(m / 60)}h ${m % 60}m`;
}

/** The target in a backend summary (`"Edit · app.ts"` → `"app.ts"`), if any. */
function targetOf({ tool, summary }: { tool?: string; summary?: string }): string | undefined {
  if (!tool || !summary) return undefined;
  const prefix = `${tool} · `;
  return summary.startsWith(prefix) ? summary.slice(prefix.length) : undefined;
}

/** A tool call in plain words: `Edit` on `app.ts` → "Editing app.ts". */
export function describeTool(tool: string, target?: string): string {
  const on = (verb: string, fallback: string) => (target ? `${verb} ${target}` : fallback);
  if (/^SendMessage$/.test(tool) || /^mcp__.+__send_message$/i.test(tool)) {
    return on("Messaging", "Sending a message");
  }
  if (tool.startsWith("mcp__")) {
    const [server = "", name = ""] = tool.slice("mcp__".length).split("__");
    return name ? `Using ${server}: ${name.replace(/[_-]+/g, " ")}` : `Using ${server}`;
  }
  switch (tool) {
    case "Read":
      return on("Reading", "Reading files");
    case "Edit":
    case "MultiEdit":
    case "NotebookEdit":
      return on("Editing", "Editing files");
    case "Write":
      return on("Writing", "Writing a file");
    case "Grep":
      return on("Searching for", "Searching the code");
    case "Glob":
      return on("Looking for", "Looking for files");
    case "LS":
      return "Looking around the files";
    case "WebSearch":
      return on("Searching the web for", "Searching the web");
    case "WebFetch":
      return on("Reading", "Reading a web page");
    case "Bash":
    case "PowerShell":
      return on("Running", "Running a command");
    case "Task":
    case "Agent":
      return on("Delegating to", "Delegating to a helper");
    case "TodoWrite":
      return "Planning the next steps";
    default:
      return `Using ${tool}`;
  }
}

/** What the buddy is doing, in plain words. */
export function activity(view: SessionView): string {
  const busy = view.state === "working" || view.state === "researching";
  if (busy && view.tool) return describeTool(view.tool, targetOf(view));
  return STATE_SENTENCES[view.state];
}

/** The balloon's text for `view` at wall-clock time `now` (epoch ms). First line bold. */
export function balloonLines(view: SessionView, now: number): string[] {
  const lines = [
    activity(view),
    `${view.project || "session"} · ${STATE_LABELS[view.state]} ${formatElapsed(now - view.since)}`,
  ];
  if (view.source === "transcript") lines.push(APPROXIMATE_HINT);
  const helpers = view.subagents?.length ?? 0;
  if (helpers > 0) lines.push(`${helpers} ${helpers === 1 ? "subagent" : "subagents"} working`);
  return lines.slice(0, MAX_LINES).map((line) => clip(line));
}

/** The balloon over a mini Clawd: what its subagent does, its type and for how long. */
export function miniBalloonLines(subagent: SubagentView, parent: SessionView, now: number): string[] {
  const doing = subagent.tool ? describeTool(subagent.tool, targetOf(subagent)) : "Thinking";
  const who = subagent.kind ? `${subagent.kind} subagent` : "Subagent";
  return [doing, `${who} · ${parent.project || "session"} · ${formatElapsed(now - subagent.since)}`].map(
    (line) => clip(line),
  );
}

/**
 * Hover timing. The pointer is over a buddy (its id), over the balloon
 * itself, or nowhere. The balloon opens after the pointer rests on one buddy
 * for {@link HOVER_DELAY_MS}, stays open while the pointer is on the buddy
 * or the balloon, and closes as soon as it leaves both.
 */
export class HoverState {
  private target: string | null = null;
  private since = 0;
  private open: string | null = null;

  /** `over`: a buddy id, `"card"` (the balloon), or `null` for nothing. */
  pointer(over: string | null, now: number): void {
    if (over === "card") {
      if (this.open !== null) return; // Reading the balloon keeps it open.
      over = null;
    }
    if (over !== this.target) {
      this.target = over;
      this.since = now;
      if (this.open !== over) this.open = null;
    }
  }

  /** The buddy under the pointer, whether or not its balloon is open yet. */
  get hovered(): string | null {
    return this.target;
  }

  /** The session whose balloon shows at `now`, if any. */
  openId(now: number): string | null {
    if (this.open === null && this.target !== null && now - this.since >= HOVER_DELAY_MS) {
      this.open = this.target;
    }
    return this.open;
  }

  /**
   * When the balloon next needs a redraw: the end of the hover delay, or
   * the next whole second of the elapsed time while open. `null` when idle.
   */
  nextChangeAt(now: number, since: number | null): number | null {
    if (this.openId(now) !== null && since !== null) {
      const elapsed = now - since;
      return now + (1000 - (((elapsed % 1000) + 1000) % 1000));
    }
    if (this.target !== null && this.open === null) return this.since + HOVER_DELAY_MS;
    return null;
  }
}

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/**
 * Where the balloon goes, in canvas pixels: on the side of the buddy away
 * from the screen edge (above it in bottom corners, below in top ones),
 * centred on the buddy, and pushed back inside the canvas when it would
 * cross an edge. `gap` leaves room for the balloon's tail.
 */
export function placeCard(
  buddy: Rect,
  card: { width: number; height: number },
  canvas: { width: number; height: number },
  corner: Corner,
  gap: number,
): Rect {
  const bottom = corner.startsWith("bottom");
  let x = Math.round(buddy.x + buddy.width / 2 - card.width / 2);
  x = Math.min(Math.max(0, x), Math.max(0, canvas.width - card.width));
  let y = bottom ? buddy.y - gap - card.height : buddy.y + buddy.height + gap;
  y = Math.min(Math.max(0, y), Math.max(0, canvas.height - card.height));
  return { x, y, width: card.width, height: card.height };
}
