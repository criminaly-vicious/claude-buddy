# 0001. Tauri 2 as desktop shell

- **Status:** Accepted
- **Date:** 2026-10-27

## Context
We need a background tray app with a transparent, frameless, always-on-top overlay on Windows, low RAM/CPU, a small installer, and native Win32 access. Future macOS/Linux ports are desirable.

## Decision
Use **Tauri 2** (Rust core + system WebView2) with a TypeScript frontend.

## Alternatives considered
- **Electron:** easiest transparent windows, but 100+ MB RAM and large installers break the "light" principle.
- **Native Rust (winit + wgpu/softbuffer):** lightest, but tray, settings UI and text rendering cost much more effort.
- **C#/WPF:** great on Windows, but blocks future ports and splits the codebase from the Rust hook binary.

## Consequences
- Small installer, shared Rust code with `buddy-hook`.
- WebView2 transparency and click-through need validation (spike B-06). If the cost is too high we fall back to a native render window for the overlay only, keeping Tauri for tray/settings (new ADR).
