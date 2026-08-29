# Final Product Decisions

## Connectivity

- Direct-IP only
- No public IDs
- No relay UI
- No rendezvous UI
- No account system UI
- IP/Hostname entry only

**Status: implemented (Minimal UI phase, 2026-08-29).** `flutter/lib/desktop/pages/connection_page.dart` was rewritten to a plain hostname/IP field plus the Support/Desktop buttons — no peer list, autocomplete, or ID lookup. Account and Network (relay/rendezvous server address) settings tabs are hidden via existing upstream mechanisms (`HARD_SETTINGS["disable-account"]`, `BUILTIN_SETTINGS["hide-network-settings"]`, both set unconditionally in `src/fork_config.rs::apply()`). See `docs/FEATURE_ENFORCEMENT_MATRIX.md` for exactly which of these are UI-only vs. protocol-level.

**Status update (2026-08-29, ADR-0003): now also protocol-level, not just UI-level.** An investigation found that `role=remote` actively registered its public ID with RustDesk's default public rendezvous server and would participate in relay, regardless of the UI changes above — `Config::get_rendezvous_servers()` never returns an empty list (it falls back to a hardcoded public-server constant). `src/rendezvous_mediator.rs::start_all()` now permanently skips rendezvous registration and relay participation for this fork (relay has no independent activation path — it's entirely downstream of registration), and `enable-lan-discovery` is set to `"N"` to close the LAN-broadcast ID-exposure path too. `direct_server` (the direct-IP listener) is unaffected. See `docs/ADR-0003-DIRECT-IP-ENFORCEMENT.md` for the full decision record.

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