/**
 * Settings window (#28). Every change is saved and applied at once; the
 * backend sends back the stored values (clamped into range), which the form
 * then shows. In `pnpm dev` without Tauri it edits an in-memory copy.
 */

import {
  AppConfig,
  DEFAULT_CONFIG,
  getConfig,
  hookStatus,
  HookStatus,
  integrationStatus,
  IntegrationStatus,
  listMonitors,
  MonitorInfo,
  setConfig,
} from "../config";
import type { Corner } from "../overlay/native";
import { hookStatusText, hotkeyFromPress, lastEventText, monitorLabel } from "./format";

const inTauri = "__TAURI_INTERNALS__" in window;

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const corner = $<HTMLSelectElement>("corner");
const monitor = $<HTMLSelectElement>("monitor");
const idle = $<HTMLInputElement>("idle");
const reduceMotion = $<HTMLInputElement>("reduce-motion");
const autostart = $<HTMLInputElement>("autostart");
const hotkey = $<HTMLInputElement>("hotkey");
const scales = [...document.querySelectorAll<HTMLInputElement>('input[name="scale"]')];
const error = $<HTMLParagraphElement>("error");

let current: AppConfig = DEFAULT_CONFIG;

function show(config: AppConfig): void {
  current = config;
  corner.value = config.corner;
  monitor.value = config.monitor === null ? "" : String(config.monitor);
  for (const radio of scales) radio.checked = Number(radio.value) === config.scale;
  idle.value = String(config.idleSleepMinutes);
  reduceMotion.checked = config.reduceMotion;
  autostart.checked = config.startWithWindows;
  hotkey.value = config.hotkey ?? "";
}

function fromForm(): AppConfig {
  const scale = scales.find((r) => r.checked)?.value ?? String(current.scale);
  return {
    ...current,
    corner: corner.value as Corner,
    monitor: monitor.value === "" ? null : Number(monitor.value),
    scale: Number(scale),
    idleSleepMinutes: Math.round(Number(idle.value)) || current.idleSleepMinutes,
    reduceMotion: reduceMotion.checked,
    startWithWindows: autostart.checked,
    hotkey: hotkey.value === "" ? null : hotkey.value,
  };
}

/** Shows `text` in the error line `id`, or hides it. */
function problem(id: string, text: string | null): void {
  const line = $<HTMLParagraphElement>(id);
  line.textContent = text ?? "";
  line.hidden = text === null;
}

function showIntegrations(status: IntegrationStatus): void {
  autostart.disabled = status.autostart === null;
  if (status.autostart !== null) autostart.checked = status.autostart;
  problem("autostart-error", status.autostartError);
  problem("hotkey-error", status.hotkeyError);
}

async function refreshIntegrations(): Promise<void> {
  if (inTauri) showIntegrations(await integrationStatus());
}

async function save(): Promise<void> {
  const wanted = fromForm();
  try {
    show(inTauri ? await setConfig(wanted) : wanted);
    error.hidden = true;
    await refreshIntegrations();
  } catch (e) {
    error.textContent = String(e);
    error.hidden = false;
    show(current);
  }
}

function showMonitors(monitors: MonitorInfo[]): void {
  for (const m of monitors) {
    const option = document.createElement("option");
    option.value = String(m.index);
    option.textContent = monitorLabel(m);
    monitor.append(option);
  }
}

function showHookStatus(status: HookStatus): void {
  const line = $<HTMLParagraphElement>("hook-status");
  const { text, ok } = hookStatusText(status);
  line.textContent = text;
  line.className = `status ${ok ? "ok" : "bad"}`;
  $<HTMLSpanElement>("last-event").textContent = lastEventText(status.lastEventAt, Date.now());
  $<HTMLParagraphElement>("settings-path").textContent = status.settingsPath
    ? `Settings file: ${status.settingsPath}`
    : "";
}

async function refreshHooks(): Promise<void> {
  if (!inTauri) {
    showHookStatus({ installed: false, settingsPath: null, lastEventAt: null, bridgeRunning: true });
    return;
  }
  showHookStatus(await hookStatus());
}

async function start(): Promise<void> {
  if (inTauri) {
    showMonitors(await listMonitors().catch(() => []));
    show(await getConfig());
  } else {
    show(DEFAULT_CONFIG);
  }
  for (const input of [corner, monitor, reduceMotion, autostart, ...scales]) {
    input.addEventListener("change", () => void save());
  }
  idle.addEventListener("change", () => void save());
  // The hotkey box records the next key press instead of typing.
  hotkey.addEventListener("keydown", (event) => {
    if (event.key === "Tab") return;
    event.preventDefault();
    const text = hotkeyFromPress(event);
    if (text && text !== hotkey.value) {
      hotkey.value = text;
      void save();
    }
  });
  $<HTMLButtonElement>("hotkey-clear").addEventListener("click", () => {
    hotkey.value = "";
    void save();
  });
  $<HTMLButtonElement>("refresh").addEventListener("click", () => void refreshHooks());
  // Coming back to the window is the natural moment to look again; no timer.
  window.addEventListener("focus", () => {
    void refreshHooks();
    void refreshIntegrations(); // Task Manager may have changed autostart.
  });
  await Promise.all([refreshHooks(), refreshIntegrations()]);
}

start().catch((e) => {
  error.textContent = `Settings could not load: ${String(e)}`;
  error.hidden = false;
});
