# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

First release: an animated buddy in the corner of your screen for every Claude Code CLI session running on Windows.

### Added

- **A buddy per session.** Every running Claude Code CLI session gets its own buddy, which appears when the session starts and waves goodbye when it ends or its terminal is closed (#41, #45).
- **Exact states through Claude Code hooks.** A tiny hook reports what each session is doing, instantly and without slowing Claude Code down. Nothing leaves your computer, and prompts, file contents and full commands are dropped by the hook before anything reaches the app (#44, #53).
- **Works without hooks too.** Sessions without hooks still get a buddy, with approximate states read from Claude Code's transcripts (#46).
- **Poses for every state.** Buddies type on a laptop while working, read a sheet of paper while researching, wave with a `!` when a session waits for you, idle around, and fall asleep after a while (#50, #51).
- **Tidy group.** Buddies stand side by side in a corner and keep their place. A buddy that waits for you steps forward, and with more than 10 sessions the extras collapse into a `+N` chip that expands on hover (#49, #52).
- **Hover card.** Hover a buddy to see its project, what it is doing, the current tool and for how long (#55).
- **Click to focus.** Click a buddy to bring its session's terminal window to the front; it hops when that worked (#56).
- **Drag and throw.** Grab a buddy and throw it: it bounces off the screen edges, lands (sometimes dizzy) and walks back to its place (#57).
- **Tray menu.** Show or hide the buddies, install, repair or remove the Claude Code hooks, open Settings, and quit. Installing the hooks shows a summary first, keeps a backup of your Claude Code settings, and checks the connection afterwards. Starting Claude Buddy again just shows the copy already running (#48, #59).
- **Settings.** Choose the corner and monitor, the buddy size (1×, 2×, 3×), when idle buddies fall asleep, and reduce motion, and see whether the hooks are working. Changes apply right away (#60).
- **Start with Windows and a hotkey.** Optionally start Claude Buddy when you sign in, and show or hide the buddies from anywhere with `Ctrl+Alt+B` or a hotkey of your choice (#62).
- **Installer.** A small per-user installer that needs no admin rights. Uninstalling offers to remove the hooks from your Claude Code settings and removes the start-with-Windows entry (#63).
- **User guide** covering installation, the hooks, every pose, troubleshooting and privacy (#61).

### Known limitations

- The character art is a placeholder until the final pixel art lands (#19).
- Clicking a buddy brings its Windows Terminal window to the front, but not the exact tab (#66).
- The installer is not code-signed yet, so Windows SmartScreen may warn the first time you run it.
