import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** A rectangle in CSS pixels relative to the overlay's top-left corner. */
export interface HitRegion {
  x: number;
  y: number;
  width: number;
  height: number;
}

export type Corner = "top-left" | "top-right" | "bottom-left" | "bottom-right";

export interface Anchor {
  corner: Corner;
  /** Index into the monitor list; `null` means the primary monitor. */
  monitor: number | null;
}

/**
 * Registers where buddies and the hover card are. The overlay is
 * click-through everywhere else. Send the full list whenever it changes.
 */
export function setHitRegions(regions: HitRegion[]): Promise<void> {
  return invoke("set_hit_regions", { regions });
}

/** Grows the overlay to its whole monitor, e.g. while a buddy is thrown. */
export function expandOverlay(): Promise<void> {
  return invoke("overlay_expand");
}

/** Back to the anchored corner. */
export function restoreOverlay(): Promise<void> {
  return invoke("overlay_restore");
}

/**
 * Tells the backend how much room the buddies need (CSS pixels), so the
 * overlay window grows or shrinks around the group from its corner.
 */
export function setOverlayContentSize(width: number, height: number): Promise<void> {
  return invoke("set_overlay_content_size", { width, height });
}

export function getOverlayAnchor(): Promise<Anchor> {
  return invoke("get_overlay_anchor");
}

export function setOverlayAnchor(anchor: Anchor): Promise<void> {
  return invoke("set_overlay_anchor", { anchor });
}

/** Bounds of an element as a hit region (CSS pixels, overlay-relative). */
export function regionOf(element: Element): HitRegion {
  const rect = element.getBoundingClientRect();
  return { x: rect.left, y: rect.top, width: rect.width, height: rect.height };
}

/** Calls `handler` whenever the overlay's anchor changes (settings, #28). */
export function onOverlayAnchor(handler: (anchor: Anchor) => void): Promise<() => void> {
  return listen<Anchor>("overlay:anchor", (event) => handler(event.payload));
}
