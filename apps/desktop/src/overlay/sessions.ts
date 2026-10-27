import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { startMockFeed } from "../mock/feed";
import type { SessionView } from "../types";

export interface SessionHandlers {
  snapshot(views: SessionView[]): void;
  update(view: SessionView): void;
  remove(id: string): void;
}

/**
 * Feeds session changes to `handlers`: the fake feed in `pnpm dev:mock`,
 * otherwise the backend's current sessions followed by its
 * `sessions:snapshot` / `sessions:update` / `sessions:remove` events.
 * Returns a function that stops the subscription.
 */
export async function subscribeSessions(handlers: SessionHandlers): Promise<() => void> {
  if (import.meta.env.MODE === "mock") {
    return startMockFeed(handlers.snapshot);
  }
  const unlisten = await Promise.all([
    listen<SessionView[]>("sessions:snapshot", (e) => handlers.snapshot(e.payload)),
    listen<SessionView>("sessions:update", (e) => handlers.update(e.payload)),
    listen<string>("sessions:remove", (e) => handlers.remove(e.payload)),
  ]);
  // The startup snapshot event fires before this page listens; ask for it.
  handlers.snapshot(await invoke<SessionView[]>("get_sessions"));
  return () => unlisten.forEach((stop) => stop());
}
