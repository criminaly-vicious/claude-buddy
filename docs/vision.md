# Vision

## Problem

Power users run several Claude Code sessions at once (five terminals is normal). Knowing which one is busy, which one finished and which one is blocked on a permission prompt means alt-tabbing through every window. Sessions sit blocked for minutes because nobody noticed.

## Idea

Turn each session into a tiny living character on the desktop. Status becomes **ambient**: you see at a glance, from the corner of your eye, who is working, who is reading, and who is waving at you for attention. It should feel like having a small team on your desk, not like a dashboard.

## Users

- **Primary:** a developer on Windows running 2 to 10 Claude Code CLI sessions in parallel (Windows Terminal, PowerShell, cmd, VS Code terminal).
- **Secondary (later):** macOS/Linux users, people who want it just for fun.

## Principles

1. **Ambient first.** The buddies answer "what is going on?" without a click. Anything that needs reading goes in the hover card.
2. **Light.** Background app. Near-zero CPU when nothing changes, no polling loops that burn battery, small install.
3. **Charming, not noisy.** Animations are short and readable. No sounds by default. Never steals focus.
4. **Local and private.** Everything stays on the machine. No telemetry. We read only the metadata needed to show status.
5. **Never in the way.** The overlay is click-through except on the buddies themselves; it can be hidden from the tray or a hotkey.
6. **Read-only towards Claude Code.** We observe sessions and focus their windows. We never type into them or change their behaviour.

## Success looks like

- A blocked session is noticed in seconds instead of minutes.
- The user keeps the app running all day and forgets it is there until a buddy waves.
- People share GIFs of their buddies being thrown around.
