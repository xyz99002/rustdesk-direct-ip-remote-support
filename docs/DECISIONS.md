# Final Product Decisions

## Connectivity

- Direct-IP only
- No public IDs
- No relay UI
- No rendezvous UI
- No account system UI
- IP/Hostname entry only

## Authentication

Preserve upstream RustDesk authentication.

Supported modes:

- ask
- password
- ask_and_password

Configured via configuration file.

No mandatory first-run password creation.
No password complexity policy.
No authentication redesign.

## Connection Screen (supersedes "Session Startup" below — 2026-08-28)

Two buttons, not one:

```
[ Hostname / IP ]

[ Support ]
[ Desktop ]
```

- **Desktop** — standard upstream `DEFAULT_CONN`. No camera. Every upstream capability (keyboard, mouse, clipboard, file transfer, audio) works exactly as it does in stock RustDesk. Always shown.
- **Support** — `DEFAULT_CONN` + `VIEW_CAMERA` opened together (two sessions, existing session mechanisms, no new session type). Audio rides on the `DEFAULT_CONN` half exactly as upstream already does it — no camera-audio combination is attempted. Visibility gated by `support_enabled` in config; the button must not be rendered at all when disabled (not just greyed out).

This is a product-goal change (customer-support-focused derivative): keep as close to upstream as possible, minimize fork maintenance, avoid transport/authentication/encryption/protocol/media-path modifications unless conclusively proven necessary.

### Session Startup (superseded, kept for history)

~~One Start Session button. Always starts: camera, audio. Also starts: desktop, when desktop_enabled=true.~~ Replaced by the two-button Support/Desktop model above — camera+audio-combined-in-one-action is no longer the design; audio-on-camera specifically was investigated and found unnecessary once Support opens a real `DEFAULT_CONN` alongside `VIEW_CAMERA` (see `docs/session-orchestration-analysis.md` §7-8).

## Local Client

- Outbound only
- Cannot accept inbound connections

## Remote Client

- Inbound only
- Cannot initiate outbound sessions

## Upstream Base

RustDesk 1.4.9
Commit 6c578292e