import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { startMockFeed } from "../mock/feed";
import type { SessionView } from "../types";

export interface SessionHandlers {
  snapshot(views: SessionView[]): void;
  update(view: SessionView): void;
  remove(id: string): void;
  /** Session `from` messaged session `to` (`null`: the app could not tell whom). */
  interaction(from: string, to: string | null): void;
}

/**
 * Feeds session changes to `handlers`: the fake feed in `pnpm dev:mock`,
 * otherwise the backend's current sessions followed by its
 * `sessions:snapshot` / `sessions:update` / `sessions:remove` /
 * `sessions:interaction` events. Returns a function that stops the subscription.
 */
export async function subscribeSessions(handlers: SessionHandlers): Promise<() => void> {
  if (import.meta.env.MODE === "mock") {
    // `?count=12` tries the overflow chip (default 5); `?cycle=0` keeps the
    // states still (default: next state every 3 s); `?chat=4000` makes two
    // sessions talk every 4 s (default 9 s).
    const params = new URLSearchParams(location.search);
    const count = Number(params.get("count")) || 5;
    const cycle = params.get("cycle");
    return startMockFeed(handlers.snapshot, {
      count,
      intervalMs: cycle === null ? 3000 : Number(cycle),
      onInteraction: handlers.interaction,
      interactEveryMs: Number(params.get("chat")) || 9000,
    });
  }
  const unlisten = await Promise.all([
    listen<SessionView[]>("sessions:snapshot", (e) => handlers.snapshot(e.payload)),
    listen<SessionView>("sessions:update", (e) => handlers.update(e.payload)),
    listen<string>("sessions:remove", (e) => handlers.remove(e.payload)),
    listen<{ from: string; to: string | null }>("sessions:interaction", (e) =>
      handlers.interaction(e.payload.from, e.payload.to),
    ),
  ]);
  // The startup snapshot event fires before this page listens; ask for it.
  handlers.snapshot(await invoke<SessionView[]>("get_sessions"));
  return () => unlisten.forEach((stop) => stop());
}
