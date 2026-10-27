# User guide

Claude Buddy puts a small animated buddy in the corner of your screen for every Claude Code CLI session running on your Windows PC. This guide covers installing it, connecting it to Claude Code, and what the buddies do.

> Claude Buddy is pre-release. Features that are not built yet are marked **(coming soon)**.

<!-- TODO(#32): add GIFs of each pose, drag & throw and the hover card once the final character art lands (#19). -->

## Contents

- [Requirements](#requirements)
- [Install](#install)
- [First run](#first-run)
- [Connect Claude Code (install hooks)](#connect-claude-code-install-hooks)
- [What the buddies do](#what-the-buddies-do)
- [Interacting with buddies](#interacting-with-buddies)
- [The tray menu](#the-tray-menu)
- [Settings](#settings)
- [Troubleshooting](#troubleshooting)
- [FAQ](#faq)

## Requirements

- Windows 10 (21H2 or later) or Windows 11, x64.
- WebView2 runtime. It comes with Windows 11; on Windows 10 it is installed with Microsoft Edge.
- [Claude Code](https://code.claude.com/docs) **CLI** sessions, in any terminal (Windows Terminal, PowerShell, cmd, the VS Code terminal). The Claude desktop app is not supported.

## Install

**From a release (coming soon).** Once v0.1.0 is published, download `Claude Buddy_<version>_x64-setup.exe` from the [Releases page](https://github.com/criminaly-vicious/claude-buddy/releases) and run it. It installs for your user only (under `%LOCALAPPDATA%`) and needs no admin rights.

**From source (now).** Follow the setup in [CONTRIBUTING.md](../CONTRIBUTING.md#setup), then:

```bash
pnpm install
pnpm bundle
```

The installer is written to `target/release/bundle/nsis/`. Run it, or use `pnpm tauri dev` to try the app without installing.

The installer is not code-signed yet, so Windows may warn you the first time. See [Windows SmartScreen warning](#windows-smartscreen-warning).

## First run

1. Start **Claude Buddy** from the Start menu. There is no main window: the app lives in the system tray (the area by the clock; it may be under the `^` arrow).
2. Click the tray icon to open its menu.
3. Choose **Install / repair hooks…** once (next section) so buddies show exact states.
4. Start, or restart, a Claude Code session. A buddy pops in at the corner of your screen.

Claude Buddy runs one copy at a time. Launching it again while it runs just shows the buddies of the copy already running.

## Connect Claude Code (install hooks)

Claude Code can run small programs, called *hooks*, when something happens in a session: a prompt is sent, a tool starts, Claude waits for you. Claude Buddy adds a hook that tells it what each session is doing. Without hooks, buddies still appear, but their states are only approximate (see [below](#approximate-states)).

To install:

1. Tray menu → **Install / repair hooks…**
2. Claude Buddy first works out the change without writing anything, then shows a summary: which file it will edit (normally `C:\Users\<you>\.claude\settings.json`, or `%CLAUDE_CONFIG_DIR%\settings.json` if you set that variable), how many Claude Code events get the hook, and how many lines change. It never shows what is in the file, because it can hold secrets.
3. Confirm. Before writing, Claude Buddy saves a copy of your file next to it as `settings.json.claude-buddy-backup-<timestamp>` (it keeps the last 5). Your other settings and hooks are left as they are.
4. It then checks that sessions can reach the app. You should see **"Bridge check: OK, sessions can reach Claude Buddy."**

Sessions started after this show a buddy with exact states. Sessions that were already running may need a restart.

Running **Install / repair hooks…** again is safe: it says "already installed and up to date", or repairs the entries if something changed them (for example, after you moved the app).

To remove the hooks, use **Uninstall hooks…** (see [Uninstall](#how-do-i-uninstall)).

## What the buddies do

Each buddy acts out what its session is doing:

| Session | What the buddy does |
| --- | --- |
| Just started | Pops in with a little hop |
| Working: editing files, running commands, running subagents | Sits down, opens a laptop and types |
| Researching: reading files, searching, browsing the web | Sits down and reads a sheet of paper |
| Waiting for you: a permission prompt or a question | Stands up and waves, with a `!` bubble. It also steps forward a little so you notice it |
| Idle: Claude finished its turn | Stands around, now and then looks around, stretches or scratches |
| Idle for a while (5 minutes by default, [adjustable](#settings)) | Curls up and sleeps (`z` bubbles). Wakes up on the next activity |
| Session ended | Waves goodbye and walks off |

Buddies line up side by side in a corner of the screen (bottom right unless you [change it](#settings)), oldest closest to the corner. Each one keeps its place until its session ends; then the others walk up to close the gap. Up to 10 are shown. With more sessions, the extra buddies collapse into a `+N` chip; hover over it to see everyone.

A buddy also leaves if its session's process disappears for 15 seconds, for example when you close the terminal window without exiting Claude Code.

Full details: [animation spec](animation-spec.md).

## Interacting with buddies

The overlay is click-through: clicks anywhere except on a buddy (or its hover card) go to the window underneath, so the buddies never get in your way.

### Hover card

Rest the pointer on a buddy for about a third of a second to see a small card:

```
claude-buddy · Working · 2m 14s
Edit · overlay.ts
```

- Line 1: the project (the session's folder name), what it is doing, and for how long.
- Line 2: the current or last tool and a short target, such as a file name or the first words of a search.
- For sessions without hooks, a third line says the state is approximate.

The card never shows prompts, file contents or full commands, and every line is cut at 80 characters.

### Click to focus the terminal

Click a buddy to bring its session's terminal window to the front. When it works, the buddy does a small happy hop.

This is best effort:
- Windows Terminal shows many tabs in one window. Claude Buddy brings that window forward, but cannot pick the tab yet.
- If no window is found for the session, nothing happens and the buddy does not hop.

### Drag and throw

Grab a buddy and drag it: it dangles and follows your pointer with a little lag. Let go while moving and it flies, bounces off the screen edges and lands. Throw it hard and it lands with a splat and is dizzy for a moment. Then it walks back to its place and carries on with what its session is doing. You can catch it again in mid-air.

A press that barely moves counts as a click, not a drag.

**(coming soon)** Double-click for a silly animation, and a right-click menu (focus, copy session id, hide this buddy).

## The tray menu

| Item | What it does |
| --- | --- |
| **Hide buddies** / **Show buddies** | Hides or shows all buddies. Sessions keep being tracked while hidden |
| **Install / repair hooks…** | Connects Claude Code to Claude Buddy ([details](#connect-claude-code-install-hooks)) |
| **Uninstall hooks…** | Removes Claude Buddy's hooks from your Claude Code settings |
| **Settings…** | Opens the [settings window](#settings) |
| **Quit Claude Buddy** | Closes the app. Claude Code sessions are not affected |

**(coming soon)** Start with Windows and a global hotkey to show or hide the buddies (default `Ctrl+Alt+B`).

## Settings

Tray menu → **Settings…** opens the settings window. Changes apply right away, with no Save button, and are kept for next time.

**Buddies**

| Setting | Options | Default |
| --- | --- | --- |
| Corner | Bottom right, bottom left, top right, top left | Bottom right |
| Monitor | Any connected monitor, listed by number, name and resolution | Primary monitor |
| Size | 1×, 2×, 3× | 2× |
| Fall asleep after | 1 to 120 minutes idle | 5 minutes |
| Reduce motion | Turns off the random idle animations | Off |

**Startup:** start with Windows and a show/hide hotkey **(coming soon, [#29](https://github.com/criminaly-vicious/claude-buddy/issues/29))**.

**Claude Code hooks:** a status panel that says whether the hooks are installed, when the last hook event arrived, and which Claude Code settings file is used. Press **Refresh** to check again. Installing, repairing and removing the hooks is done from the tray menu.

Settings are stored in `%APPDATA%\claude-buddy\config.json`. If that file gets damaged, Claude Buddy starts with the defaults and keeps the broken file as `config.json.invalid` instead of overwriting it.

## Troubleshooting

### No buddies appear

1. **Is Claude Buddy running?** Look for its icon in the tray (check under the `^` arrow). If it is not there, start it from the Start menu.
2. **Are the buddies hidden?** Open the tray menu. If it says **Show buddies**, click it.
3. **Is it a Claude Code CLI session?** Sessions in the Claude desktop app are not shown.
4. **Are the hooks installed?** Open **Settings…** and look at the *Claude Code hooks* panel, or run **Install / repair hooks…**. If it reports "Bridge check failed", quit Claude Buddy, start it again and repeat.
5. **Was the session started before the hooks were installed?** Restart that session. Sessions started before the app or the hooks may still appear through the transcript fallback, but only if they wrote to their transcript in the last 30 minutes.
6. **Did you set `CLAUDE_CONFIG_DIR`?** Claude Buddy reads the same variable as Claude Code. Make sure it is set the same way for both (set it in Windows' environment variables, then restart both).

### Approximate states

When a session has no hooks, Claude Buddy reads the session's transcript file instead (Claude Code keeps it under `.claude\projects\`). That is enough to see that a session exists and roughly what it does, but it is slower and less precise. In particular it cannot reliably tell that Claude is waiting for your permission. The hover card shows "Approximate: install hooks for exact states" for these sessions, and their buddies leave after 30 minutes without activity.

Fix: run **Install / repair hooks…** and restart the session.

### Clicking a buddy does not focus the terminal

- With Windows Terminal, the right window comes forward but not always the right tab (a known limitation).
- Windows sometimes refuses to bring a window forward while you are typing in another app. Click the buddy again.
- If the buddy does not hop, Claude Buddy found no window for that session. This can happen with some terminal hosts; [open an issue](https://github.com/criminaly-vicious/claude-buddy/issues/new/choose) naming your terminal.

### A hook error shows up in Claude Code

Claude Buddy's hook is built to finish instantly and silently, even when the app is closed. If Claude Code ever reports a hook error that mentions `buddy-hook`, run **Install / repair hooks…** (the hook path may be stale, for example after reinstalling to another folder), or **Uninstall hooks…** to remove it, and [open an issue](https://github.com/criminaly-vicious/claude-buddy/issues/new/choose).

### Windows SmartScreen warning

The installer is not code-signed yet, so Windows Defender SmartScreen may show "Windows protected your PC" when you run it. If you downloaded it from this project's [Releases page](https://github.com/criminaly-vicious/claude-buddy/releases) (or built it yourself), click **More info**, then **Run anyway**. Do not do this for a copy from anywhere else.

## FAQ

### What does Claude Buddy read? Does it see my prompts or code?

It only needs to know *what kind of thing* each session is doing:

- From the hook: the session id, its folder, the event (prompt sent, tool started, waiting, finished), the tool name, and a short target such as a file name. The hook drops prompts, file contents, tool output and full commands before anything leaves it.
- From transcripts (only for sessions without hooks): the type of each entry and the tool name. It does not keep or show any text.

Everything stays on your computer. The hook talks to the app over `127.0.0.1` with a random token that only your user account can read. There is no telemetry, no account and no network access beyond that.

### Does it change how Claude Code works?

No. It only observes. It never sends input to a session or approves anything for you. The hook runs in the background and adds no delay.

### What does it change on my computer?

- Your Claude Code `settings.json`, only when you choose **Install / repair hooks…**, with a backup kept next to it.
- A small file at `%APPDATA%\claude-buddy\bridge.json` that tells the hook where to reach the app while it runs.
- Your Claude Buddy settings, in `%APPDATA%\claude-buddy\config.json`.

### How do I uninstall?

1. Uninstall it from **Settings → Apps → Installed apps** (Windows 11) or **Apps & features** (Windows 10).
2. The uninstaller asks whether to also remove Claude Buddy's hooks from your Claude Code settings. Answer **Yes**: it removes only Claude Buddy's entries and keeps a backup. If you keep them, Claude Code reports a hook error in every session, because the hook program is gone. (You can also remove them beforehand with tray menu → **Uninstall hooks…**.)
3. Tick **Delete the application data** to also remove your Claude Buddy settings (`%APPDATA%\claude-buddy\`).
4. Optional: delete the `settings.json.claude-buddy-backup-*` files next to your Claude Code settings.

### How heavy is it?

It is designed to use under 1% CPU and under 80 MB of RAM with five buddies on screen. It only redraws when something changes.

### Is this made by Anthropic?

No. Claude Buddy is an independent open-source project, not affiliated with or endorsed by Anthropic. The buddy is an original character inspired by Claude Code's playful orange mascot; it does not use Anthropic's logo or artwork ([ADR-0005](../.context/decisions/0005-original-character-art.md)).

### Where do I report a bug or ask for a feature?

[Open an issue](https://github.com/criminaly-vicious/claude-buddy/issues/new/choose). For security problems, follow [SECURITY.md](../SECURITY.md) instead.
