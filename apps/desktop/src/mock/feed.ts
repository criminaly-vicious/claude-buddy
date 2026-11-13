import type { BuddyState, SessionView } from "../types";

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

/** Builds `count` fake sessions, each offset in the state cycle by `tick`. */
export function mockSessions(count: number, tick: number, now = Date.now()): SessionView[] {
  return Array.from({ length: count }, (_, i) => {
    const state = MOCK_STATE_CYCLE[(tick + i) % MOCK_STATE_CYCLE.length];
    const tools = MOCK_TOOLS[state];
    const [tool, target] = tools ? tools[(tick + i) % tools.length] : [undefined, undefined];
    return {
      id: `mock-${i}`,
      project: `project-${i + 1}`,
      cwd: `C:\\dev\\project-${i + 1}`,
      state,
      since: now,
      source: "hooks",
      ...(tool ? { tool, summary: `${tool} · ${target}` } : {}),
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
}

/** Emits a new mock snapshot every `intervalMs`. Returns a function that stops the feed. */
export function startMockFeed(
  onSnapshot: (sessions: SessionView[]) => void,
  { count = 5, intervalMs = 3000, onInteraction, interactEveryMs = 9000 }: MockFeedOptions = {},
): () => void {
  let tick = 0;
  onSnapshot(mockSessions(count, tick));
  const timers: ReturnType<typeof setInterval>[] = [];
  if (intervalMs > 0) {
    timers.push(setInterval(() => onSnapshot(mockSessions(count, ++tick)), intervalMs));
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
