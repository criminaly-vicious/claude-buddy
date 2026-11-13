import { describe, expect, it } from "vitest";
import { MOCK_STATE_CYCLE, mockSessions } from "./feed";

describe("mockSessions", () => {
  it("creates the requested number of sessions with unique ids", () => {
    const sessions = mockSessions(5, 0, 0);
    expect(sessions).toHaveLength(5);
    expect(new Set(sessions.map((s) => s.id)).size).toBe(5);
  });

  it("advances every session one step in the state cycle per tick", () => {
    const [first] = mockSessions(1, 0, 0);
    const [next] = mockSessions(1, 1, 0);
    expect(first.state).toBe(MOCK_STATE_CYCLE[0]);
    expect(next.state).toBe(MOCK_STATE_CYCLE[1]);
  });

  it("wraps around the cycle", () => {
    const [session] = mockSessions(1, MOCK_STATE_CYCLE.length, 0);
    expect(session.state).toBe(MOCK_STATE_CYCLE[0]);
  });
});
