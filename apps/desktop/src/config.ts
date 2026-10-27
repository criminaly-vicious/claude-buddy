import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type { Corner } from "./overlay/native";

/** Mirrors `AppConfig` in src-tauri/src/config.rs (`config.json`). */
export interface AppConfig {
  version: number;
  corner: Corner;
  /** Monitor index; `null` means the primary monitor. */
  monitor: number | null;
  /** CSS pixels per sprite pixel: 1, 2 or 3. */
  scale: number;
  idleSleepMinutes: number;
  reduceMotion: boolean;
  /** Mirrors the real autostart entry in the registry. */
  startWithWindows: boolean;
  /** Global show/hide hotkey, e.g. "Ctrl+Alt+B"; `null` for none. */
  hotkey: string | null;
}

export const DEFAULT_CONFIG: AppConfig = {
  version: 1,
  corner: "bottom-right",
  monitor: null,
  scale: 2,
  idleSleepMinutes: 5,
  reduceMotion: false,
  startWithWindows: false,
  hotkey: "Ctrl+Alt+B",
};

export function getConfig(): Promise<AppConfig> {
  return invoke("get_config");
}

/** Saves and applies `config`; resolves the config as stored (values in range). */
export function setConfig(config: AppConfig): Promise<AppConfig> {
  return invoke("set_config", { config });
}

/** Calls `handler` with the new config whenever any window changes it. */
export function onConfigChanged(handler: (config: AppConfig) => void): Promise<() => void> {
  return listen<AppConfig>("config:changed", (event) => handler(event.payload));
}

export interface MonitorInfo {
  index: number;
  name: string | null;
  width: number;
  height: number;
  primary: boolean;
}

export function listMonitors(): Promise<MonitorInfo[]> {
  return invoke("list_monitors");
}

export interface HookStatus {
  /** `null` when it could not be checked. */
  installed: boolean | null;
  settingsPath: string | null;
  /** Epoch ms of the last hook event since Claude Buddy started. */
  lastEventAt: number | null;
  bridgeRunning: boolean;
}

export function hookStatus(): Promise<HookStatus> {
  return invoke("hook_status");
}

export interface IntegrationStatus {
  /** Whether Claude Buddy starts with Windows; `null` where unsupported. */
  autostart: boolean | null;
  autostartError: string | null;
  /** Why the hotkey is not active: bad text, or another app holds it. */
  hotkeyError: string | null;
}

export function integrationStatus(): Promise<IntegrationStatus> {
  return invoke("integration_status");
}
