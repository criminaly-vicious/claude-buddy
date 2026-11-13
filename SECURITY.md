# Security Policy

## Design principles

- **Local only.** The hook server binds to `127.0.0.1` and never listens on other interfaces.
- **Random token.** Each time the app starts it generates a new random token, readable only by your user account; hook requests without it are rejected.
- **No telemetry.** Claude Buddy sends nothing off your machine.
- **Minimal data.** It reads only what is needed to derive session state and a short status summary. It does not store or transmit prompt contents.

## Supported versions

Until 1.0, only the latest release receives security fixes.

## Reporting a vulnerability

Please do not open a public issue. Use GitHub's private advisory form: **Security** tab > **Report a vulnerability** (private vulnerability reporting) on this repository.

Include affected version, reproduction steps and impact. We aim to acknowledge reports within 7 days and will coordinate disclosure with you.
