# Architecture (Final — requirements frozen)

**Status:** Frozen 2026-08-28, per direct user instruction plus `docs/DECISIONS.md`. This document is now authoritative and **supersedes** the earlier `docs/phase1-architecture-proposal.md` draft it replaces (that content is gone; see git history if the earlier proposal is needed for reference). Read `docs/upstream-analysis.md` alongside this for the source citations the decisions below are checked against.

## Authentication — no redesign, expose existing upstream modes as-is

RustDesk 1.4.9 already implements exactly three approval modes via the `approve-mode` config option (`libs/hbb_common/src/password_security.rs:77-86`):

| Fork-facing name | Config value | Upstream enum | Behavior |
|---|---|---|---|
| `ask` | `click` | `ApproveMode::Click` | remote user manually approves each incoming session; no password required |
| `password` | `password` | `ApproveMode::Password` | session requires a valid password (temporary and/or permanent, per `verification-method`) |
| `ask_and_password` | *(default/empty)* | `ApproveMode::Both` | either a manual click-approve or a valid password is sufficient |

Decisions (frozen):

- **No authentication redesign.** No changes to `src/server/connection.rs`'s login/approval logic, password hashing/storage, or lockout/brute-force handling. Upstream behavior is preserved exactly.
- **No mandatory first-run password creation.** The app does not force a password to be set before the listener works.
- **No password complexity/policy enforcement.** Whatever upstream accepts today (including an empty password under `ask`/`ask_and_password`) continues to be accepted.
- The only in-scope work is making the existing `approve-mode` (and `verification-method`) options selectable through this fork's own configuration surface, using `ask` / `password` / `ask_and_password` as the fork's vocabulary for the three upstream values above. This is configuration wiring, not new authentication logic.

This reverses two items that were previously planned: the "mandatory first-run password gate" and "harden default-password lockout" work is now explicitly out of scope.

## Connectivity — direct-IP only, enforced at the UI/config layer, not the protocol layer

- **No transport redesign.** `src/rendezvous_mediator.rs`, `src/client.rs`'s connection paths, and all relay/rendezvous logic are preserved unchanged. Upstream transport behavior — including the rendezvous discovery and relay fallback code paths — is not removed, rewritten, or gated behind a new "direct-IP-only" enforcement flag.
- The direct-IP dial path already exists in upstream (`is_ip_str` / `is_domain_port_str` handling in `src/client.rs`, confirmed in `docs/upstream-analysis.md` §1) and is what this fork's UI exclusively drives.
- "Direct-IP only" is achieved by **not building UI or configuration surface** for anything else, per `docs/DECISIONS.md`:
  - No public ID display or ID-based connect.
  - No relay configuration UI.
  - No rendezvous-server configuration UI.
  - No account-system UI.
  - The only connect input is a hostname/IP field.
- **Local client:** outbound-only in its UI — it only ever initiates a direct-IP connection; there is no UI path for it to accept inbound sessions.
- **Remote client:** inbound-only in its UI — it only listens for and accepts incoming direct-IP sessions; there is no UI path for it to initiate an outbound session.
- Net effect: the upstream engine is capable of more than this (rendezvous, relay, bidirectional roles), but this fork's UI/config never exposes or exercises any of that — direct-IP-only is a product-level restriction, not a protocol change.

## Connection screen — two independently-flagged buttons (revised 2026-08-28, second revision)

**Supersedes both prior models below.** Per `docs/DECISIONS.md` and `docs/FORK_PROFILE_SPEC.md`, and informed by `docs/session-orchestration-analysis.md` §7-10 (audio-on-camera investigation, then Voice Call confirmed to work standalone on `VIEW_CAMERA`):

- **Desktop button** → one standard upstream `DEFAULT_CONN` session, **only**. No camera, no voice call. Every upstream capability (keyboard, mouse, clipboard, file transfer, audio) works exactly as it does in stock RustDesk — no modifications to transport, authentication, permission negotiation, audio routing, encryption, or protocol messages. Rendered only when `desktop_share_enabled = true`.
- **Support button** → always one `VIEW_CAMERA` session + a Voice Call on it (`session_request_voice_call()` — existing `VoiceCallRequest`/`VoiceCallResponse` messages, confirmed in §9-10 to work with no `DEFAULT_CONN` present anywhere). Additionally opens a `DEFAULT_CONN` session when `desktop_share_enabled = true` (independent flag, not tied to Support). Voice Call remains subject to the existing upstream host-side accept/reject workflow — no bypass. Rendered only when `support_enabled = true`.
- Each button renders only when its own flag is true — never greyed out, absent entirely. **Validation:** at least one of `support_enabled`/`desktop_share_enabled` must be true; both false is rejected.
- **Remote-side enforcement:** `support_enabled` reuses the existing upstream `enable-camera` permission (`libs/hbb_common/src/config.rs:2902`, enforced at `src/server/connection.rs:2544-2551`) to reject `VIEW_CAMERA` (and therefore Voice Call, which rides on it) when disabled. **No equivalent existing permission was found to reject `DEFAULT_CONN`** — `desktop_share_enabled` is therefore enforced locally only (button hidden); see `docs/FORK_PROFILE_SPEC.md`'s Configuration Profile for this documented gap.
- No new `ConnType`, no new protocol messages, no audio-service changes, no changes to `is_remote()`/`try_sub_monitor_services()`/`add_camera_connection()` — every session and every message reused exactly as upstream already provides it.

### Prior models (superseded, kept for history)

~~A single Start Session action always starts camera+audio, and additionally desktop when `desktop_enabled=true`, requiring camera and audio to combine on one `VIEW_CAMERA` connection.~~ Required a server-side audio change (§6, withdrawn).

~~Support = `DEFAULT_CONN` + `VIEW_CAMERA` always (single `support_enabled` flag), audio via `DEFAULT_CONN`; Desktop unconditional.~~ Superseded once Voice Call was confirmed to work standalone on `VIEW_CAMERA` (§9-10) — `DEFAULT_CONN` is no longer required for Support at all, and Desktop is no longer unconditional.

## Configuration — TOML, versioned

The fork's own configuration is a TOML file (confirmed 2026-08-28: reuses the `toml`/`confy` crates already in the dependency graph via `hbb_common`; no new dependency — see `docs/FORK_AUTOMATION.md`). Any YAML-fenced example elsewhere in the doc set (including in `CLAUDE_MASTER_PROMPT.md`) is illustrative only, not the actual format.

Full schema (per `docs/FORK_PROFILE_SPEC.md`'s "Configuration Profile" and `CLAUDE_MASTER_PROMPT.md`'s "# Configuration" section):

```toml
# NOTE: [authentication] must stay LAST — in TOML, every key = value line after a
# [table] header belongs to that table, not the top level. This ordering was found to
# matter in practice: an earlier draft of this example had [authentication] first,
# which would have silently nested every key below it under authentication.* instead
# of the top level (caught by fork_config.rs's own test suite).
version = 1
role = "local"

support_enabled = true
desktop_share_enabled = true

listen_address = "0.0.0.0"
listen_port = 21118

video_quality = "medium"
audio_quality = "medium"

log_level = "info"

[authentication]
mode = "ask"
```

**Revised 2026-08-28 (second revision):** `desktop_share_enabled` added alongside `support_enabled` — Desktop is no longer unconditional; each button now has its own independent flag. **Validation:** reject a config where both `support_enabled` and `desktop_share_enabled` are false. `support_enabled` is additionally written to the existing upstream `enable-camera` option (`Config::set_option("enable-camera", ...)`) so the remote side enforces it too, not just the local UI. `camera_enabled`/`audio_enabled`/`desktop_enabled` from the original schema remain removed. `listen_address`, `listen_port`, `video_quality`, `audio_quality`, `log_level` are unaffected and remain reserved for the Direct-IP transport and minimal-UI phases.

Phase 3 (Configuration and Role Restriction, accepted) implements loading and validation of a config schema and wires `version`, `role`, and `authentication.mode` to actual behavior. This revision (Connection Workflow) adds `support_enabled` + `desktop_share_enabled`, wired to both button visibility (local) and, for `support_enabled`, remote-side enforcement via `enable-camera`.

## Upstream baseline

- RustDesk `1.4.9`, commit `6c578292e8ebbbec708b76986ba8c4bc7c509747` (already merged into `main`; work happens on `feature/direct-ip-fork`).

## Traceability note — conflict with `CLAUDE_MASTER_PROMPT.md` (now resolved)

`CLAUDE_MASTER_PROMPT.md`'s *original* acceptance criteria ("remote accepts authenticated direct-IP only," "no relay/rendezvous," "mandatory first-run password") read, taken literally, as a transport/auth *code* change, conflicting with the frozen decision above. `CLAUDE_MASTER_PROMPT.md` has since been rewritten (2026-08-28) and now explicitly states the same position as this document: no mandatory first-run password, no custom authentication mechanisms, and direct-IP-only achieved through "UI restrictions, Product workflow restrictions, Configuration restrictions... NOT through transport rewrites, custom protocols, networking redesign." The conflict noted here is historical — kept for the record, not because it's still live.
