# FORK_PROFILE_SPEC.md

# RustDesk Direct-IP Fork Profile Specification

## Purpose

This document defines the product profile that transforms an upstream RustDesk release into the Direct-IP product.

The profile describes product behavior rather than implementation details.

Future RustDesk versions should be adapted by applying this profile and validating the documented hook points.

---

# Profile Metadata

profile_name = "rustdesk-direct-ip"
profile_version = "1.0"
upstream_baseline = "RustDesk 1.4.9"
upstream_commit = "6c578292e"

---

# Product Model

Two executable variants are produced:

## Local Client

Responsibilities:

- Initiate direct-IP sessions
- Never accept inbound sessions
- Never expose listener controls

UI (revised 2026-08-28 — both buttons now independently config-gated):

```
[ Hostname / IP ]

[ Support ]   (support_enabled = true)
[ Desktop ]   (desktop_share_enabled = true)
```

Each button's visibility is controlled by its own flag; it must not render at all when its flag is false (not greyed out — absent). At least one of the two flags must be true. See "Session Profile" below for what each button launches.

---

## Remote Client

Responsibilities:

- Accept inbound direct-IP sessions
- Never initiate outbound sessions

Expose:

- Authentication mode
- Password management
- Permission status
- Connection status

---

# Connectivity Profile

connection_mode = "direct_ip"

Users connect only by:

- IPv4 address
- IPv6 address
- DNS hostname

The following must not appear in the user experience:

- Public IDs
- Relay configuration
- Rendezvous configuration
- Discovery configuration
- Account workflows
- Cloud workflows

---

# Authentication Profile

Preserve upstream RustDesk authentication behavior.

Supported modes:

- ask
- password
- ask_and_password

Mapping:

ask -> click
password -> password
ask_and_password -> both/default

No custom authentication implementation.

---

# Role Profile

role = "local"

Behavior:

- outbound only

role = "remote"

Behavior:

- inbound only

Implementation should reuse upstream role enforcement wherever available.

---

# Session Profile (revised 2026-08-28 — Support no longer requires DEFAULT_CONN)

Independent actions, each already an existing upstream session/message mechanism — no new session type or protocol message is introduced:

- **Desktop button** -> one `DEFAULT_CONN` session only. Standard upstream behavior: keyboard, mouse, clipboard, file transfer, audio, and any other upstream-supported capability all work unmodified. No camera, no voice call. Shown only when `desktop_share_enabled = true`.
- **Support button** -> always one `VIEW_CAMERA` session + a Voice Call on it (`session_request_voice_call()`, existing `VoiceCallRequest`/`VoiceCallResponse` messages — confirmed to work standalone on `VIEW_CAMERA` with no `DEFAULT_CONN` required, `docs/session-orchestration-analysis.md` §9-10). Additionally opens a `DEFAULT_CONN` session when `desktop_share_enabled = true`. Voice Call remains subject to the existing upstream accept/reject workflow on the remote side. Shown only when `support_enabled = true`.
- Each button renders only when its own flag is true. At least one flag must be true (config with both false is rejected).

### Superseded models (kept for history)

~~Start Session action launches camera + two-way audio always, desktop optionally when `desktop_enabled = true`, as a single user action.~~ Superseded because it required combining audio onto a `VIEW_CAMERA` session, which upstream doesn't do by default.

~~Support = `DEFAULT_CONN` + `VIEW_CAMERA` always (single `support_enabled` flag), audio via `DEFAULT_CONN`; Desktop unconditional.~~ Superseded once Voice Call was confirmed to work standalone on `VIEW_CAMERA` — `DEFAULT_CONN` is no longer required for Support at all, and Desktop is no longer unconditional.

---

# Configuration Profile

Actual configuration format: TOML (see `docs/architecture.md` for the concrete schema and file location; any `key = value` shown in this document is TOML, not YAML — examples elsewhere in the doc set are illustrative unless explicitly marked as TOML).

Required configuration keys (revised 2026-08-28 — `desktop_share_enabled` added):

version
role
support_enabled
desktop_share_enabled
authentication.mode
listen_address
listen_port
video_quality
audio_quality
log_level

**`support_enabled`** — gates the Support button (local UI) and, on the remote side, whether `VIEW_CAMERA` (and therefore Voice Call) connections are accepted at all, by reusing the existing upstream `enable-camera` permission (`libs/hbb_common/src/config.rs:2902`, enforced at `src/server/connection.rs:2544-2551`) — no new authentication code. **`desktop_share_enabled`** — gates the Desktop button (local UI). **Validation:** at least one of the two must be true; a config with both false is rejected.

**Known gap, documented not silently worked around:** unlike `support_enabled`/`enable-camera`, there is **no existing upstream permission that rejects a `DEFAULT_CONN` login outright** (`DEFAULT_CONN`'s video is unconditional once accepted — see `docs/session-orchestration-analysis.md` §2 desktop-video-permission findings). So `desktop_share_enabled = false` is enforced **only at the local UI** (Desktop button hidden) — it does not, and today cannot without a new authentication-path check, prevent a `DEFAULT_CONN` login attempt at the remote side. Flagged for a decision rather than silently adding new authentication code or silently leaving it unenforced.

`camera_enabled`, `audio_enabled`, and `desktop_enabled` from the prior schema remain removed — no referent under this model. `listen_address`, `listen_port`, `video_quality`, `audio_quality`, `log_level` are unaffected and remain reserved for their respective future phases.

Version changes must be backward compatible or provide migration guidance. This revision keeps `version = 1` (pre-release, no deployed configs depend on backward compatibility yet) but is recorded here and in `CHANGELOG_IMPLEMENTATION.md` for traceability.

---

# Upgrade Rules

When upgrading upstream RustDesk:

1. Verify role hook points still exist.
2. Verify authentication mapping still exists.
3. Verify the Desktop button's `DEFAULT_CONN` path and the Support button's `DEFAULT_CONN` + `VIEW_CAMERA` pairing still work as two independent, unmodified upstream session mechanisms.
4. Verify UI suppression points still exist (Support button hidden when `support_enabled = false`; no public ID/relay/rendezvous/account UI).
5. Execute regression tests.

---

# Acceptance Criteria

- Local client initiates sessions only.
- Remote client accepts sessions only.
- IP/Hostname is the only connection method.
- ask mode works.
- password mode works.
- ask_and_password mode works.
- Desktop button launches a standard upstream `DEFAULT_CONN` session with all upstream capabilities (keyboard, mouse, clipboard, file transfer, audio) intact; shown only when `desktop_share_enabled = true`.
- Support button launches `VIEW_CAMERA` + Voice Call always, plus `DEFAULT_CONN` when `desktop_share_enabled = true`; shown only when `support_enabled = true`.
- A configuration with both flags false is rejected.
- Remote side rejects `VIEW_CAMERA`/Voice Call when `support_enabled = false` (via `enable-camera`).
- Public-ID workflow absent from user experience.
- Relay workflow absent from user experience.
- Rendezvous workflow absent from user experience.
- Configuration fully controls supported behavior.

---

# Automation Intent

This profile is the authoritative behavioral specification.

Automation tools, Claude Code workflows, upgrade scripts, and validation pipelines should use this profile as the source of truth when generating future Direct-IP fork releases.
