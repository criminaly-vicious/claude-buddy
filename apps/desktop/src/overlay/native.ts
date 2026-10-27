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

/** The expanded overlay (CSS pixels): its size and where the corner area sits in it. */
export interface ExpandedLayout {
  width: number;
  height: number;
  home: HitRegion;
}

/**
 * Grows the overlay to its monitor's work area while a buddy is dragged or
 * thrown. Resolves where the anchored corner area lies inside it, or `null`
 * when the backend knows no monitor.
 */
export function expandOverlay(): Promise<ExpandedLayout | null> {
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

/**
 * Brings the terminal window of session `id` to the front. Resolves `true`
 * only when it is now in the foreground (the buddy then hops).
 */
export function focusSession(id: string): Promise<boolean> {
  return invoke("focus_session", { id });
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

/**
 * Calls `handler(false)` when the pointer leaves every buddy, chip and card
 * region (the overlay turns click-through and stops seeing the mouse), and
 * `handler(true)` when it enters one.
 */
export function onOverlayPointer(handler: (inside: boolean) => void): Promise<() => void> {
  return listen<boolean>("overlay:pointer", (event) => handler(event.payload));
}

/** Calls `handler` when the buddies are hidden (`false`) or shown (`true`) from the tray. */
export function onOverlayVisible(handler: (visible: boolean) => void): Promise<() => void> {
  return listen<boolean>("overlay:visible", (event) => handler(event.payload));
}
