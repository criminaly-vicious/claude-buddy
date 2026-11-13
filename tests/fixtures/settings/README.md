# Settings fixtures

Anonymised `~/.claude/settings.json` files for the hook installer tests (#16). Paths and commands are made up.

| File | Case |
| --- | --- |
| `no-hooks.json` | Typical settings without a `hooks` key |
| `third-party-hooks.json` | Hooks from other tools, other keys after `hooks` |
| `old-buddy.json` | An older Claude Buddy install: a different path, no `async`, an event we no longer register, sharing a group with someone else's hook |
| `invalid.json` | Not JSON (a comment); the installer must refuse it |

The JSON files use two-space indentation, as Claude Code writes them, so an install followed by an uninstall can be compared byte for byte.

Pre-existing empty hook containers (`"hooks": {}`, `"Stop": []`) are tested inline in `installer/tests.rs`, because their round trip is semantically identical rather than byte for byte.
