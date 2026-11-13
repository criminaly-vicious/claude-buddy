# 0013. The buddy is Clawd, drawn in code

- **Status:** Accepted
- **Date:** 2026-11-13
- **Issue:** #19
- **Supersedes:** [ADR-0005](0005-original-character-art.md)

## Context
ADR-0005 asked for an original character, so that the project would not ship Anthropic's artwork. For the v1.0.0 release the owner looked at three original concepts (bean, sprout and puff, in #71) and chose Clawd instead: the orange mascot of Claude Code, which belongs to Anthropic.

This brings two risks, which the owner accepted:
- **Takedown.** Clawd is Anthropic's character. A public app that ships it can be taken down for copyright or trademark reasons, together with its releases.
- **Looking official.** Clawd and the name "Claude Buddy" together can make the app look like an Anthropic product.

## Decision
- The buddy is **Clawd**, as fan art drawn pixel by pixel in code (the art generator in `apps/desktop/scripts/`). The repository has no copied official files or images, and no Anthropic logos (the Claude starburst, wordmarks).
- The same goes for the app icons, which are generated from the sprite.
- The README, the user guide and the app say plainly that the project is unofficial fan art, not affiliated with or endorsed by Anthropic, and that Clawd is Anthropic's character.
- If Anthropic asks for the art or the name to be changed, the project complies right away. The original concepts in #71's history are the fallback.

## Alternatives considered
- **Keep an original character (ADR-0005):** no legal risk, but the owner wants the buddy to be Clawd.
- **Clawd only as a local skin:** the public repository would ship an original default and Clawd would stay on the owner's computer. This removes the risk, but the owner chose to ship Clawd.

## Consequences
- The buddy is instantly recognisable to Claude Code users.
- The release can be taken down at Anthropic's request. The disclaimer and the ready fallback keep a change quick.
- Contributors must never add Anthropic logos or copied official assets. The rule in `CLAUDE.md` points here.
