# Claude Buddy

[![ci](https://github.com/criminaly-vicious/claude-buddy/actions/workflows/ci.yml/badge.svg?branch=develop)](https://github.com/criminaly-vicious/claude-buddy/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
![Platform: Windows 10/11](https://img.shields.io/badge/platform-Windows%2010%20%7C%2011-0078d4.svg)

> Little orange desk-mates that live in the corner of your screen, one for every Claude Code session you have running.

Claude Buddy is a lightweight Windows tray app that gives each running **Claude Code CLI** session its own animated pixel-art buddy, so you can see at a glance which sessions are working, which are reading, and which are waiting for you.

<!-- TODO(#19): add a screenshot or GIF of the buddies here once the final character art lands. -->

## Features

| Session | What its buddy does |
| --- | --- |
| Working (editing files, running commands) | Sits down, opens a laptop and types |
| Researching (reading, searching the web) | Reads a sheet of paper |
| Waiting for you (permission prompt, question) | Stands up and waves, with a `!` bubble |
| Idle | Looks around, stretches, falls asleep after a while |
| Ended | Waves goodbye and walks off |

- **Hover** a buddy for its project, current tool and elapsed time.
- **Click** it to bring its terminal window to the front.
- **Grab and throw** it: it bounces off the screen edges and walks back to its place.
- **Tray app** with settings (corner, monitor, size, sleep time, reduce motion), start with Windows and a show/hide hotkey (`Ctrl+Alt+B`).
- **Light:** designed for under 1% CPU at idle and under 80 MB of RAM.

Full list in the [changelog](CHANGELOG.md).

## Install

**Release (coming soon).** v0.1.0 will be published on the [Releases page](https://github.com/criminaly-vicious/claude-buddy/releases) as a small per-user installer that needs no admin rights.

**From source (now).** With the [prerequisites](CONTRIBUTING.md#setup) installed:

```bash
pnpm install
pnpm bundle   # installer in target/release/bundle/nsis/
```

## Quick start

1. Start **Claude Buddy**. It lives in the system tray, by the clock.
2. Tray menu → **Install / repair hooks…** and confirm. Claude Buddy shows what it will change in your Claude Code settings and keeps a backup.
3. Start (or restart) a Claude Code session. Its buddy pops in at the corner of your screen.

Everything else (settings, troubleshooting, uninstalling) is in the [user guide](docs/user-guide.md).

## Privacy

- **Local only.** Claude Buddy talks to its hook over `127.0.0.1` with a random token. No telemetry, no account, no cloud.
- **No content.** It knows what *kind* of work each session does (the tool and a short target such as a file name), never your prompts, file contents or full commands.
- **Read-only.** It never sends input to Claude Code or approves anything for you; it only observes and focuses windows.

Details: [user guide FAQ](docs/user-guide.md#what-does-claude-buddy-read-does-it-see-my-prompts-or-code) and [security policy](SECURITY.md).

## Contributing and docs

Contributions are welcome. Start with [CONTRIBUTING.md](CONTRIBUTING.md) (Git Flow, Conventional Commits, branch naming, releases) and the [Code of Conduct](CODE_OF_CONDUCT.md). Report security issues privately as described in [SECURITY.md](SECURITY.md).

| Doc | What it covers |
| --- | --- |
| [User guide](docs/user-guide.md) | Install, connect Claude Code, what buddies do, troubleshooting, FAQ |
| [Vision](docs/vision.md) | Why this exists, principles, users |
| [Scope](docs/scope.md) | MVP in/out, non-goals, success criteria |
| [Architecture](docs/architecture.md) | Components, data flow, tech stack, bridge protocol |
| [Session detection](docs/session-detection.md) | How Claude Buddy learns what each Claude Code session is doing |
| [Character & animation spec](docs/animation-spec.md) | Buddy design, states, animations, interactions |
| [Roadmap](docs/roadmap.md) and [backlog](docs/backlog.md) | Milestones and planned issues |
| [Agent context](.context/README.md) | Briefs for AI agents and decision records (ADRs) |

## Not affiliated with Anthropic

Claude Buddy is an independent open-source project. It is not affiliated with or endorsed by Anthropic. The buddy is an **original** character inspired by the playful orange mascot of Claude Code; the project does not ship Anthropic's logo or artwork ([ADR-0005](.context/decisions/0005-original-character-art.md)).

## License

[Apache-2.0](LICENSE)
