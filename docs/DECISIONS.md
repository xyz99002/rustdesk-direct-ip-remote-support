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

## App Identity (Distinct From Upstream RustDesk)

**Status: implemented 2026-09-11.** `src/core_main.rs::core_main()` sets `hbb_common::config::APP_NAME`
to `"RustDesk-DirectIP-RemoteSupport"` unconditionally, before anything that derives a path or IPC
endpoint from it. Left at upstream's default (`"RustDesk"`), this fork is indistinguishable — on
Windows in particular — from a genuinely installed RustDesk on the same machine: both would use the
identical `\\.\pipe\RustDesk\query` IPC pipe name and the identical `%APPDATA%\RustDesk\` storage
directory (`RustDesk2.toml`, peers, logs).

This caused a real incident: with a real RustDesk already installed (its background `--service`/
`--server` process running), this fork's GUI process connected over IPC to *that* process instead of
its own — the GUI's option cache (`ui_interface::OPTIONS`) got silently overwritten with the other
process's stale/foreign values, making `desktop-share-enabled`/`show-setup-ui`/etc. appear to have no
effect despite `fork_config.rs` correctly applying them. Uninstalling the real RustDesk made the
symptom disappear, which is what pointed at the shared identity as the cause.

Giving the fork its own `APP_NAME` fixes the IPC pipe collision and the config-directory collision as
one change, since both are derived from it. It also causes upstream's own translation layer
(`src/lang.rs`'s `is_rustdesk()` check) to substitute the fork's name for "RustDesk" in every
translated UI string, and changes the window title/taskbar text to match — an accepted, visible
side effect, not a bug.

## Fork Peer Marker

**Status: BLOCKED 2026-09-11, not implemented.** Goal: a `role=local` instance dialing an IP:port
should get a clear, immediate rejection if what answers isn't this fork's `role=remote` (e.g. a
real RustDesk instance happening to listen there), instead of falling through to normal password
authentication and producing a generic, confusing failure — a different concern from the
app-identity/install-separation work (`docs/PLAN-install-separator.md`), which is about OS-level
process/installer identity *before* any connection exists; this is about the connection itself.

The natural design — add a `fork_marker` field to `LoginRequest`, set by every connection this
fork initiates, checked as the first thing done with an incoming `LoginRequest` before any other
login processing, with a mismatch as a hard rejection — was drafted and then reverted after
discovering a structural blocker: **`LoginRequest`, and every other protocol message, is defined
in `libs/hbb_common/protos/message.proto`, which lives inside `libs/hbb_common` — a git submodule
pointing at the official upstream `rustdesk/hbb_common` repository, not something this fork owns
or has push access to.** Adding a field there means patching a separate, upstream-owned repo —
exactly the situation this project has consistently avoided elsewhere (it's the same reason
`config.toml` lives next to the executable rather than patching `hbb_common`'s `Config::path()`,
back at the start of this fork's work). Neither `LoginRequest` nor its `OptionMessage` companion
has any free-form/extensible field that could carry a new marker without a schema change — every
field is fixed, named, and already meaningfully used (checked: `avatar`, `hwid`, `version` are all
real, in-use fields, not safe to repurpose without risking breaking their actual function or
leaking marker data into user-visible UI, e.g. a connection-manager approval dialog).

Options going forward, none yet decided:
1. **Fork `libs/hbb_common` too** (a new repo under this project's control, `.gitmodules` pointed
   at it) — enables a clean protocol extension, but commits to maintaining a second forked repo
   alongside this one, a materially bigger ongoing commitment than anything else in this fork.
2. **Drop this specific safeguard.** The peer-mismatch scenario it targets is already narrowed by
   the app-identity work: a `role=local` instance now has its own distinct IPC/config identity, so
   the *local* version of this problem (this fork's own GUI talking to the wrong local process) is
   already fixed by `docs/PLAN-install-separator.md`'s Phase 1/2. What remains uncovered is
   specifically dialing a *remote* IP:port that happens to be a real RustDesk instance rather than
   this fork's `role=remote` — password authentication still applies as the real access control in
   that case; the gap is only "a confusing generic error" rather than "a clear one."
3. Some other repurposing of an existing field, accepting a documented risk to that field's real
   function — not recommended without a specific proposal and sign-off, given the risks above.

## Upstream Base

RustDesk 1.4.9
Commit 6c578292e