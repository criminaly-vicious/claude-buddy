# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Subagents as mini Clawds.** Each subagent a session runs shows as a mini Clawd standing on its buddy's head, stacked in the order they started. Hover one to see what it is doing. Throw the buddy and the minis scatter, walk back and climb on again, always in the same order (#83, #84).

### Changed

- **Walking in three-quarter view.** Clawd turns towards where he walks, half front and half side, when he walks back after a throw, closes a gap, strolls or visits (#85).
- **Hooks follow subagents.** Claude Buddy now listens to two more Claude Code events, `SubagentStart` and `SubagentStop`. With the classic hooks, run **Install / repair hooks…** from the tray menu once; the settings window says when they are out of date. The mod updates with the plugin (#83).
- A subagent's tools now show on its mini Clawd; its session's buddy keeps showing that it is delegating (#83).

## [1.0.0] - 2026-11-13

First release: an animated buddy in the corner of your screen for every Claude Code CLI session running on Windows.

### Added

- **A buddy per session.** Every running Claude Code CLI session gets its own buddy, which appears when the session starts and waves goodbye when it ends or its terminal is closed (#41, #45).
- **Exact states through Claude Code hooks.** A tiny hook reports what each session is doing, instantly and without slowing Claude Code down. Nothing leaves your computer, and prompts, file contents and full commands are dropped by the hook before anything reaches the app (#44, #53).
- **Works without hooks too.** Sessions without hooks still get a buddy, with approximate states read from Claude Code's transcripts (#46).
- **Poses for every state.** Buddies type on a laptop while working, read a sheet of paper while researching, wave with a `!` when a session waits for you, idle around, and fall asleep after a while (#50, #51).
- **Tidy group.** Buddies stand side by side in a corner and keep their place. A buddy that waits for you steps forward, and with more than 10 sessions the extras collapse into a `+N` chip that expands on hover (#49, #52).
- **Hover balloon.** Hover a buddy for a pixel-art speech balloon that says what its session is doing in plain words ("Editing app.ts", "Running pnpm", "Waiting for you"), with the project and for how long (#55, #76).
- **Click to focus.** Click a buddy to bring its session's terminal window to the front; it hops when that worked (#56).
- **Drag and throw.** Grab a buddy and throw it: it bounces off the screen edges, lands (sometimes dizzy) and walks back to its place (#57).
- **Buddies go for walks.** Now and then an idle buddy strolls along the bottom of the screen, looks around and walks back to its place: one at a time, with a quiet minute or so between strolls. It hurries back when its session gets work (#77).
- **Sessions chat.** When one Claude Code session messages another, its buddy walks over to the other one and they chat with speech bubbles before it walks back. Claude Buddy only looks at who a message is for, never at what it says (#78).
- **Tray menu.** Show or hide the buddies, install, repair or remove the Claude Code hooks, open Settings, and quit. Installing the hooks shows a summary first, keeps a backup of your Claude Code settings, and checks the connection afterwards. Starting Claude Buddy again just shows the copy already running (#48, #59).
- **Settings.** Choose the corner and monitor, the buddy size (1×, 2×, 3×), when idle buddies fall asleep, and reduce motion, and see whether the hooks are working. Changes apply right away (#60).
- **Start with Windows and a hotkey.** Optionally start Claude Buddy when you sign in, and show or hide the buddies from anywhere with `Ctrl+Alt+B` or a hotkey of your choice (#62).
- **Claude Code mod.** Connect Claude Code with one line, `/plugin install buddy-bridge --marketplace criminaly-vicious/claude-buddy`, without touching your Claude Code settings. The classic hooks from the tray remain as an alternative, and the two never report the same session twice (#70).
- **Installer.** A small per-user installer that needs no admin rights. Uninstalling offers to remove the hooks from your Claude Code settings and removes the start-with-Windows entry (#63).
- **Clawd.** The buddy is Clawd, drawn pixel by pixel, with an animation for every state and interaction: popping in, idling, typing, reading, waving, sleeping, walking, being thrown, landing, getting dizzy and waving goodbye. He breathes and blinks out of step with the others, and pops in and out in a puff. The app and installer icons show him too (#19, #79).
- **Light.** Resting buddies share one animation clock and redraw only when their picture changes, so a whole group redraws at most 8 times a second however many buddies there are. Walking and flying redraw faster, and only while they last (#30, #79).
- **User guide** covering installation, the hooks, every pose, troubleshooting and privacy (#61).

### Known limitations

- Clicking a buddy brings its Windows Terminal window to the front, but not the exact tab (#66).
- The installer is not code-signed yet, so Windows SmartScreen may warn the first time you run it.
