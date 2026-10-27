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

/** Builds `count` fake sessions, each offset in the state cycle by `tick`. */
export function mockSessions(count: number, tick: number, now = Date.now()): SessionView[] {
  return Array.from({ length: count }, (_, i) => {
    const state = MOCK_STATE_CYCLE[(tick + i) % MOCK_STATE_CYCLE.length];
    return {
      id: `mock-${i}`,
      project: `project-${i + 1}`,
      cwd: `C:\\dev\\project-${i + 1}`,
      state,
      since: now,
      source: "hooks",
    };
  });
}

/** Emits a new mock snapshot every `intervalMs`. Returns a function that stops the feed. */
export function startMockFeed(
  onSnapshot: (sessions: SessionView[]) => void,
  { count = 5, intervalMs = 3000 } = {},
): () => void {
  let tick = 0;
  onSnapshot(mockSessions(count, tick));
  const timer = setInterval(() => onSnapshot(mockSessions(count, ++tick)), intervalMs);
  return () => clearInterval(timer);
}
