/** Mirrors the `SessionView` contract in docs/architecture.md#data-model. */
export type BuddyState =
  | "spawning"
  | "idle"
  | "working"
  | "researching"
  | "waiting"
  | "sleeping"
  | "leaving";

export interface SessionView {
  id: string;
  project: string;
  cwd: string;
  state: BuddyState;
  tool?: string;
  summary?: string;
  since: number;
  source: "hooks" | "transcript";
  pid?: number;
}
