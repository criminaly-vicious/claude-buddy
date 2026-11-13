# User guide

Claude Buddy puts a small animated buddy in the corner of your screen for every Claude Code CLI session running on your Windows PC. This guide covers installing it, connecting it to Claude Code, and what the buddies do.

> Claude Buddy is pre-release. Features that are not built yet are marked **(coming soon)**.

<!-- TODO(#32): add GIFs of each pose, drag & throw and the hover card once the final character art lands (#19). -->

## Contents

- [Requirements](#requirements)
- [Install](#install)
- [First run](#first-run)
- [Connect Claude Code](#connect-claude-code)
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

**From a release (coming soon).** Once v1.0.0 is published, download `Claude Buddy_<version>_x64-setup.exe` from the [Releases page](https://github.com/criminaly-vicious/claude-buddy/releases) and run it. It installs for your user only (under `%LOCALAPPDATA%`) and needs no admin rights.

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
3. Connect Claude Code once (next section) so buddies show exact states: install the Claude Code mod, or choose **Install / repair hooks…** in the tray.
4. Start, or restart, a Claude Code session. A buddy pops in at the corner of your screen.

Claude Buddy runs one copy at a time. Launching it again while it runs just shows the buddies of the copy already running.

## Connect Claude Code

Claude Code tells Claude Buddy what each session is doing through *hooks*: small pieces of code it runs when something happens in a session (a prompt is sent, a tool starts, Claude waits for you). There are two ways to set them up. Use one of them: without either, buddies still appear, but their states are only approximate (see [below](#approximate-states)).

### Recommended: the Claude Code mod

Claude Buddy's mod, `buddy-bridge`, is a Claude Code plugin. Type this once at the prompt of a Claude Code session:

```
/plugin install buddy-bridge --marketplace criminaly-vicious/claude-buddy
```

Answer `y` to add the marketplace, then press Enter to install it for your user. Every Claude Code session you start from then on reports to Claude Buddy. Nothing is written to your `settings.json`, and the mod never slows a session down: when Claude Buddy is closed it simply sends nothing.

- To remove it: `/plugin uninstall buddy-bridge`.
- If you also have the classic hooks below installed, the mod stays quiet and the hooks keep reporting, so nothing is counted twice. Remove them with **Uninstall hooks…** in the tray to switch to the mod.
- Mods are new in Claude Code. If your version does not load them, use the classic hooks.

### Alternative: classic hooks from the tray

The tray can add the hooks to your Claude Code settings instead. To install:

1. Tray menu → **Install / repair hooks…**
2. Claude Buddy first works out the change without writing anything, then shows a summary: which file it will edit (normally `C:\Users\<you>\.claude\settings.json`, or `%CLAUDE_CONFIG_DIR%\settings.json` if you set that variable), how many Claude Code events get the hook, and how many lines change. It never shows what is in the file, because it can hold secrets.
3. Confirm. Before writing, Claude Buddy saves a copy of your file next to it as `settings.json.claude-buddy-backup-<timestamp>` (it keeps the last 5). Your other settings and hooks are left as they are.
4. It then checks that sessions can reach the app. You should see **"Bridge check: OK, sessions can reach Claude Buddy."**

Sessions started after this show a buddy with exact states. Sessions that were already running may need a restart.

Running **Install / repair hooks…** again is safe: it says "already installed and up to date", or repairs the entries if something changed them (for example, after you moved the app).

To remove these hooks, use **Uninstall hooks…** (see [Uninstall](#how-do-i-uninstall)).

## What the buddies do

Each buddy acts out what its session is doing:

| Session | What the buddy does |
| --- | --- |
| Just started | Pops in with a little hop |
| Working: editing files, running commands, running subagents | Sits down, opens a laptop and types |
| Researching: reading files, searching, browsing the web | Sits down and reads a sheet of paper |
| Waiting for you: a permission prompt or a question | Stands up and waves, with a `!` bubble. It also steps forward a little so you notice it |
| Idle: Claude finished its turn | Stands around, now and then looks around, stretches or scratches, and every few minutes takes a [short stroll](#strolls) |
| Idle for a while (5 minutes by default, [adjustable](#settings)) | Curls up and sleeps (`z` bubbles). Wakes up on the next activity |
| Session ended | Waves goodbye and walks off |

Buddies line up side by side in a corner of the screen (bottom right unless you [change it](#settings)), oldest closest to the corner. Each one keeps its place until its session ends; then the others walk up to close the gap. Up to 10 are shown. With more sessions, the extra buddies collapse into a `+N` chip; hover over it to see everyone.

A buddy also leaves if its session's process disappears for 15 seconds, for example when you close the terminal window without exiting Claude Code.

### Strolls

Now and then an idle buddy wanders off along the bottom of the screen, looks around and walks back to its place. Only one buddy goes at a time, and the group then stays home for a minute or so. If its session gets work meanwhile, it turns around at once and goes back to its desk. Hovering a buddy keeps it home. **Reduce motion** in [Settings](#settings) turns strolls off.

### Sessions talking to each other

When one session sends a message to another (for example with Claude Code's `SendMessage` tool, or a tool that messages another session), the sender's buddy gets up, walks over to the other buddy and they chat for a moment: a `…` bubble over the sender and a heart in reply. Then it walks back and carries on.

- Claude Buddy only looks at who a message is for, never at what it says.
- If it cannot tell who the message is for, or the other buddy is not on screen, the sender just shows a `…` bubble where it stands.
- With **Reduce motion** on, nobody walks: the bubbles show where the buddies stand.

Full details: [animation spec](animation-spec.md).

## Interacting with buddies

The overlay is click-through: clicks anywhere except on a buddy (or its hover balloon) go to the window underneath, so the buddies never get in your way.

### Hover balloon

Rest the pointer on a buddy for about a third of a second and a speech balloon pops up above it, saying what it is doing:

```
● Editing overlay.ts
claude-buddy · Working 2m 14s
```

- Line 1: what the session is doing, in plain words: "Editing overlay.ts", "Running pnpm", "Reading README.md", "Searching the web for …", "Messaging researcher", or, between tools, "Thinking", "Waiting for you", "Done, waiting for a prompt", "Sleeping". The dot's colour shows the state.
- Line 2: the project (the session's folder name), the state and for how long.
- For sessions without hooks, a third line says the state is approximate.

The balloon stays open while the pointer is on the buddy or on the balloon, and closes a moment after it leaves both. It never shows prompts, file contents or full commands, and every line is cut at 80 characters.

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
| **Install / repair hooks…** | Connects Claude Code to Claude Buddy with the classic hooks ([details](#alternative-classic-hooks-from-the-tray)) |
| **Uninstall hooks…** | Removes Claude Buddy's hooks from your Claude Code settings |
| **Settings…** | Opens the [settings window](#settings) |
| **Quit Claude Buddy** | Closes the app. Claude Code sessions are not affected |

Press `Ctrl+Alt+B` (or [your hotkey](#settings)) to show or hide the buddies without opening the menu. To start Claude Buddy when you sign in, turn on **Start with Windows** in [Settings](#settings).

## Settings

Tray menu → **Settings…** opens the settings window. Changes apply right away, with no Save button, and are kept for next time.

**Buddies**

| Setting | Options | Default |
| --- | --- | --- |
| Corner | Bottom right, bottom left, top right, top left | Bottom right |
| Monitor | Any connected monitor, listed by number, name and resolution | Primary monitor |
| Size | 1×, 2×, 3× | 2× |
| Fall asleep after | 1 to 120 minutes idle | 5 minutes |
| Reduce motion | Turns off the random idle animations and the [strolls](#strolls); [sessions talking](#sessions-talking-to-each-other) show as bubbles, without walking over | Off |

**Startup**

| Setting | What it does | Default |
| --- | --- | --- |
| Start with Windows | Starts Claude Buddy when you sign in. Per user, no admin needed. It is the same switch as Claude Buddy in Task Manager → **Startup apps**: turning it off there shows here, and turning it on here enables it there again | Off |
| Show / hide hotkey | Shows or hides the buddies from anywhere. Click the box and press the keys: at least one of Ctrl, Alt or Win (Shift can be added), plus a letter, a digit, F1–F24 or Space. **None** turns it off. If another app or Windows already uses the combination, a message says so: pick another | `Ctrl+Alt+B` |

**Claude Code hooks:** a status panel that says whether the hooks are installed, when the last hook event arrived, and which Claude Code settings file is used. Press **Refresh** to check again. Installing, repairing and removing the hooks is done from the tray menu.

Settings are stored in `%APPDATA%\claude-buddy\config.json`. If that file gets damaged, Claude Buddy starts with the defaults and keeps the broken file as `config.json.invalid` instead of overwriting it.

## Troubleshooting

### No buddies appear

1. **Is Claude Buddy running?** Look for its icon in the tray (check under the `^` arrow). If it is not there, start it from the Start menu.
2. **Are the buddies hidden?** Open the tray menu. If it says **Show buddies**, click it.
3. **Is it a Claude Code CLI session?** Sessions in the Claude desktop app are not shown.
4. **Is Claude Code connected?** Check that the mod is installed (`/plugin` in Claude Code lists `buddy-bridge`), or open **Settings…** and look at the *Claude Code hooks* panel, or run **Install / repair hooks…**. If it reports "Bridge check failed", quit Claude Buddy, start it again and repeat.
5. **Was the session started before the hooks were installed?** Restart that session. Sessions started before the app or the hooks may still appear through the transcript fallback, but only if they wrote to their transcript in the last 30 minutes.
6. **Did you set `CLAUDE_CONFIG_DIR`?** Claude Buddy reads the same variable as Claude Code. Make sure it is set the same way for both (set it in Windows' environment variables, then restart both).

### Approximate states

When a session has no hooks, Claude Buddy reads the session's transcript file instead (Claude Code keeps it under `.claude\projects\`). That is enough to see that a session exists and roughly what it does, but it is slower and less precise. In particular it cannot reliably tell that Claude is waiting for your permission. The hover balloon shows "Approximate: connect Claude Code for exact states" for these sessions, and their buddies leave after 30 minutes without activity.

Fix: [connect Claude Code](#connect-claude-code) (the mod, or **Install / repair hooks…**) and restart the session.

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

- From the mod or the hook: the session id, its folder, the event (prompt sent, tool started, waiting, finished), the tool name, and a short target such as a file name. For a message to another session, the target is only who it is for. Both drop prompts, message text, file contents, tool output and full commands before anything leaves Claude Code.
- From transcripts (only for sessions without hooks): the type of each entry and the tool name. It does not keep or show any text.

Everything stays on your computer. The mod and the hook talk to the app over `127.0.0.1` with a random token that only your user account can read. There is no telemetry, no account and no network access beyond that.

### Does it change how Claude Code works?

No. It only observes. It never sends input to a session or approves anything for you. The mod and the hook report in the background and add no delay. Once per session the mod starts one short helper process to learn Claude Code's process id, so Claude Buddy can tell when a session's terminal was closed.

### What does it change on my computer?

- With the mod: nothing outside Claude Code's own plugins folder, where `/plugin install` puts it.
- With the classic hooks: your Claude Code `settings.json`, only when you choose **Install / repair hooks…**, with a backup kept next to it.
- A small file at `%APPDATA%\claude-buddy\bridge.json` that tells the hook where to reach the app while it runs.
- Your Claude Buddy settings, in `%APPDATA%\claude-buddy\config.json`.

### How do I uninstall?

1. If you use the mod, remove it in Claude Code: `/plugin uninstall buddy-bridge`.
2. Uninstall Claude Buddy from **Settings → Apps → Installed apps** (Windows 11) or **Apps & features** (Windows 10).
3. The uninstaller asks whether to also remove Claude Buddy's hooks from your Claude Code settings. Answer **Yes**: it removes only Claude Buddy's entries and keeps a backup. If you keep them, Claude Code reports a hook error in every session, because the hook program is gone. (You can also remove them beforehand with tray menu → **Uninstall hooks…**.)
4. Tick **Delete the application data** to also remove your Claude Buddy settings (`%APPDATA%\claude-buddy\`). The uninstaller always removes the **Start with Windows** entry.
5. Optional: delete the `settings.json.claude-buddy-backup-*` files next to your Claude Code settings.

### How heavy is it?

It is designed to use under 1% CPU and under 80 MB of RAM with five buddies on screen. It only redraws when a buddy's picture changes: while the buddies rest, all of them share one animation clock, so a whole group redraws a few times a second at most, however many there are. It draws more often only while a buddy walks, strolls or flies, and only for as long as that lasts.

### Is this made by Anthropic?

No. Claude Buddy is an independent open-source project, not affiliated with or endorsed by Anthropic. The buddy is fan art of Clawd, the Claude Code mascot, which is Anthropic's character. It is drawn pixel by pixel in code, and the app ships no Anthropic logos or official files ([ADR-0013](../.context/decisions/0013-clawd-character.md)).

### Where do I report a bug or ask for a feature?

[Open an issue](https://github.com/criminaly-vicious/claude-buddy/issues/new/choose). For security problems, follow [SECURITY.md](../SECURITY.md) instead.
