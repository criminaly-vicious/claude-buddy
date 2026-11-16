import type { BuddyState, SessionView, SubagentView } from "../types";

/** Order the mock feed cycles through so every state can be seen without real sessions. */
export const MOCK_STATE_CYCLE: readonly BuddyState[] = [
  "spawning",
  "idle",
  "working",
  "researching",
  "waiting",
  "sleeping",
];

/** What busy mock sessions are doing, for the hover balloon. */
const MOCK_TOOLS: Partial<Record<BuddyState, [string, string][]>> = {
  working: [
    ["Edit", "main.ts"],
    ["Bash", "pnpm"],
    ["Write", "README.md"],
  ],
  researching: [
    ["Read", "scene.ts"],
    ["Grep", "fn main"],
    ["WebSearch", "tauri transparent window"],
  ],
};

const MOCK_AGENTS = ["Explore", "general-purpose", "Plan"];
const MOCK_AGENT_TOOLS: [string, string][] = [
  ["Read", "scene.ts"],
  ["Grep", "subagent"],
  ["Bash", "cargo"],
];

/**
 * The subagents of busy mock session `i` at `tick`: between 1 and `max`,
 * changing every few ticks, so minis pop in and out.
 */
function mockSubagents(i: number, tick: number, max: number, now: number): SubagentView[] {
  if (max <= 0) return [];
  const n = 1 + ((i + Math.floor(tick / 2)) % max);
  return Array.from({ length: n }, (_, k) => {
    const [tool, target] = MOCK_AGENT_TOOLS[(k + tick) % MOCK_AGENT_TOOLS.length];
    return {
      id: `mock-${i}-agent-${k}`,
      kind: MOCK_AGENTS[(i + k) % MOCK_AGENTS.length],
      tool,
      summary: `${tool} · ${target}`,
      since: now,
    };
  });
}

/** Builds `count` fake sessions, each offset in the state cycle by `tick`. */
export function mockSessions(count: number, tick: number, now = Date.now(), subagents = 0): SessionView[] {
  return Array.from({ length: count }, (_, i) => {
    const state = MOCK_STATE_CYCLE[(tick + i) % MOCK_STATE_CYCLE.length];
    const tools = MOCK_TOOLS[state];
    const [tool, target] = tools ? tools[(tick + i) % tools.length] : [undefined, undefined];
    const busy = state === "working" || state === "researching";
    const helpers = busy ? mockSubagents(i, tick, subagents, now) : [];
    return {
      id: `mock-${i}`,
      project: `project-${i + 1}`,
      cwd: `C:\\dev\\project-${i + 1}`,
      state,
      since: now,
      source: "hooks",
      ...(tool ? { tool, summary: `${tool} · ${target}` } : {}),
      ...(helpers.length > 0 ? { subagents: helpers } : {}),
    };
  });
}

export interface MockFeedOptions {
  count?: number;
  /** How often states move on; 0 keeps every session in its first state. */
  intervalMs?: number;
  /** Called now and then with two sessions talking to each other. */
  onInteraction?: (from: string, to: string | null) => void;
  interactEveryMs?: number;
  /** Busy sessions run up to this many subagents (mini Clawds). */
  subagents?: number;
}

/** Emits a new mock snapshot every `intervalMs`. Returns a function that stops the feed. */
export function startMockFeed(
  onSnapshot: (sessions: SessionView[]) => void,
  { count = 5, intervalMs = 3000, onInteraction, interactEveryMs = 9000, subagents = 0 }: MockFeedOptions = {},
): () => void {
  let tick = 0;
  onSnapshot(mockSessions(count, tick, Date.now(), subagents));
  const timers: ReturnType<typeof setInterval>[] = [];
  if (intervalMs > 0) {
    timers.push(setInterval(() => onSnapshot(mockSessions(count, ++tick, Date.now(), subagents)), intervalMs));
  }
  if (onInteraction && count > 1) {
    timers.push(
      setInterval(() => {
        const from = Math.floor(Math.random() * count);
        const to = (from + 1 + Math.floor(Math.random() * (count - 1))) % count;
        onInteraction(`mock-${from}`, `mock-${to}`);
      }, interactEveryMs),
    );
  }
  return () => timers.forEach((timer) => clearInterval(timer));
}
