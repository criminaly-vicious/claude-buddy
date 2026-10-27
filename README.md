# Claude Buddy

> Little orange desk-mates that live in the corner of your screen, one for every Claude Code session you have running.

Claude Buddy is a lightweight background app for **Windows** (other platforms later). It connects to the **Claude Code CLI** sessions open on your machine and gives each one a small animated pixel-art buddy. Buddies sit together in a corner of the screen and act out what their session is doing:

| Session state | What the buddy does |
| --- | --- |
| Working (editing files, running commands) | Sits down, pulls out a laptop and types |
| Researching (web search, reading files) | Pulls out a sheet of paper and reads it |
| Waiting for you (permission prompt, question) | Stands up, waves, shows a `!` bubble |
| Idle (turn finished) | Stretches, looks around, naps after a while |
| Session ended | Waves goodbye and walks off |

Hover a buddy to see a short summary of what its session is doing (project, current tool, last action). Click it to bring that session's terminal to the front. Grab it and throw it: it flies, bounces and lands with a goofy animation.

Like Wispr Flow, it runs from the system tray: open it, hide it, quit it. It must stay light (target: under 1% CPU at idle and under 80 MB RAM).

> **Status:** planning. No code yet. See the [roadmap](docs/roadmap.md) and the issue tracker.

## Documentation

| Doc | What it covers |
| --- | --- |
| [Vision](docs/vision.md) | Why this exists, principles, users |
| [Scope](docs/scope.md) | MVP in/out, non-goals, success criteria |
| [Architecture](docs/architecture.md) | Components, data flow, tech stack |
| [Session detection](docs/session-detection.md) | How we learn what each Claude Code session is doing |
| [Character & animation spec](docs/animation-spec.md) | Buddy design, states, animations, interactions |
| [Roadmap](docs/roadmap.md) | Milestones and their issues |
| [Backlog](docs/backlog.md) | Every planned issue, by milestone |
| [Contributing](CONTRIBUTING.md) | Conventional Commits, Git Flow, PR rules |
| [Agent context](.context/README.md) | Briefs for AI agents, decision records (ADRs) |

## Note on the character

The buddy is an **original** character inspired by the playful orange pixel mascot of Claude Code. We do not ship Anthropic's logo or official artwork, and this project is not affiliated with or endorsed by Anthropic. See [ADR-0005](.context/decisions/0005-original-character-art.md).

## License

[Apache-2.0](LICENSE)
