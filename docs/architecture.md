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

## Connection screen — two buttons, two independent upstream session mechanisms (revised 2026-08-28)

**Supersedes the single "Start Session" model below.** Per `docs/DECISIONS.md` and `docs/FORK_PROFILE_SPEC.md`, and informed by the audio-on-camera investigation in `docs/session-orchestration-analysis.md` §7-8:

- **Desktop button** → one standard upstream `DEFAULT_CONN` session. No camera. Every upstream capability (keyboard, mouse, clipboard, file transfer, audio) works exactly as it does in stock RustDesk — no modifications to transport, authentication, permission negotiation, audio routing, encryption, or protocol messages. Always shown.
- **Support button** → one `DEFAULT_CONN` session + one `VIEW_CAMERA` session, opened together as one user action, using the existing session-establishment mechanisms already documented in `docs/session-orchestration-analysis.md` §4 (each gets its own `SessionID`, both can run concurrently to the same peer — already supported, no session-model changes needed). Audio is carried entirely by the `DEFAULT_CONN` half, using standard upstream audio routing. Rendered only when `support_enabled = true`; must not render at all when disabled.
- **Why this avoids the camera+audio problem entirely:** the investigation in `docs/session-orchestration-analysis.md` §7 found that upstream's server-side machinery to give a `VIEW_CAMERA` session audio exists but is never triggered by the current client UI (a default-value gap, not a protocol block) — see §8 below for why the Support/Desktop design sidesteps needing to fix that at all: audio needs come from `DEFAULT_CONN`, which already has full audio support, so `VIEW_CAMERA`'s lack of automatic audio is simply irrelevant to this design.
- No new `ConnType`, no audio-service changes, no changes to `is_remote()`/`try_sub_monitor_services()`/`add_camera_connection()` — the two sessions are opened independently, each exactly as upstream already supports.

### Session Startup (superseded, kept for history)

~~A single Start Session action always starts camera+audio, and additionally desktop when `desktop_enabled=true`, requiring camera and audio to combine on one `VIEW_CAMERA` connection.~~ Replaced by the Support/Desktop model above specifically because combining audio onto `VIEW_CAMERA` would have required a server-side change (see the now-superseded §6 investigation in `docs/session-orchestration-analysis.md`), whereas Support/Desktop achieves the same product goal using `DEFAULT_CONN` for anything that needs audio — zero server-side media changes.

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

listen_address = "0.0.0.0"
listen_port = 21118

video_quality = "medium"
audio_quality = "medium"

log_level = "info"

[authentication]
mode = "ask"
```

**Revised 2026-08-28:** `support_enabled` replaces `camera_enabled`/`audio_enabled`/`desktop_enabled` from the schema Phase 3 originally implemented. Under the Support/Desktop model neither has a remaining referent: Desktop is unconditionally available (no gate needed), and Support unconditionally launches both `DEFAULT_CONN` and `VIEW_CAMERA` when enabled (no independent camera/audio toggle) — proposed as an assumption of this revision, pending confirmation (see `docs/FORK_PROFILE_SPEC.md`'s Configuration Profile). `listen_address`, `listen_port`, `video_quality`, `audio_quality`, `log_level` are unaffected and remain reserved for the Direct-IP transport and minimal-UI phases.

Phase 3 (Configuration and Role Restriction, accepted) implements loading and validation of a config schema and wires `version`, `role`, and `authentication.mode` to actual behavior — see the mapping tables above. This revision (Connection Workflow) adds `support_enabled`, wired to Support-button visibility, and drops the three now-unused keys from required validation.

## Upstream baseline

- RustDesk `1.4.9`, commit `6c578292e8ebbbec708b76986ba8c4bc7c509747` (already merged into `main`; work happens on `feature/direct-ip-fork`).

## Traceability note — conflict with `CLAUDE_MASTER_PROMPT.md` (now resolved)

`CLAUDE_MASTER_PROMPT.md`'s *original* acceptance criteria ("remote accepts authenticated direct-IP only," "no relay/rendezvous," "mandatory first-run password") read, taken literally, as a transport/auth *code* change, conflicting with the frozen decision above. `CLAUDE_MASTER_PROMPT.md` has since been rewritten (2026-08-28) and now explicitly states the same position as this document: no mandatory first-run password, no custom authentication mechanisms, and direct-IP-only achieved through "UI restrictions, Product workflow restrictions, Configuration restrictions... NOT through transport rewrites, custom protocols, networking redesign." The conflict noted here is historical — kept for the record, not because it's still live.
