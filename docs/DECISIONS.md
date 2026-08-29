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

## Connection Screen (revised 2026-08-28 — two independent config flags, not one)

Two independently-configured buttons:

```
[ Hostname / IP ]

[ Support ]   (shown when support_enabled = true)
[ Desktop ]   (shown when desktop_share_enabled = true)
```

- **Desktop** — standard upstream `DEFAULT_CONN` only. No camera, no voice call. Every upstream capability (keyboard, mouse, clipboard, file transfer, audio) works exactly as it does in stock RustDesk.
- **Support** — always `VIEW_CAMERA` + Voice Call (via the existing `session_request_voice_call()`/`VoiceCallRequest`/`VoiceCallResponse` mechanism, confirmed to work standalone on a `VIEW_CAMERA` session with no `DEFAULT_CONN` required — see `docs/session-orchestration-analysis.md` §9-10). Additionally opens `DEFAULT_CONN` when `desktop_share_enabled = true`. Voice Call remains subject to the existing upstream accept/reject workflow on the remote side — no bypass.
- Each button is rendered only when its own flag is true — neither is shown "greyed out," it's absent entirely. At least one of `support_enabled`/`desktop_share_enabled` must be true; a config with both false is rejected.
- **Remote-side enforcement:** `support_enabled` also controls whether the remote/host accepts `VIEW_CAMERA` (and therefore Voice Call, which rides on it) at all, reusing the existing upstream `enable-camera` permission — see `docs/architecture.md` for the mechanism and an open gap (`desktop_share_enabled` has no equivalent existing upstream permission to reject `DEFAULT_CONN`).

This is a product-goal change (customer-support-focused derivative): keep as close to upstream as possible, minimize fork maintenance, avoid transport/authentication/encryption/protocol/media-path modifications unless conclusively proven necessary.

### Prior Connection Screen model (superseded, kept for history)

~~Support = `DEFAULT_CONN` + `VIEW_CAMERA` always, audio via `DEFAULT_CONN`, gated by a single `support_enabled` flag; Desktop always shown.~~ Replaced by the two-independent-flags model above once Voice Call was confirmed to work standalone on `VIEW_CAMERA` — Support no longer needs `DEFAULT_CONN` at all unless `desktop_share_enabled` is also true, and Desktop is no longer unconditional.

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