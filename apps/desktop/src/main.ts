import { startMockFeed } from "./mock/feed";

// Rendering lands with the sprite pipeline (#18). Until then the overlay is an
// empty transparent canvas; in mock mode the feed is logged for inspection.
if (import.meta.env.MODE === "mock") {
  startMockFeed((sessions) => console.debug("[mock] sessions", sessions));
}
