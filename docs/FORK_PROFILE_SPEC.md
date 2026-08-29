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

UI (revised 2026-08-28 — supersedes the single "Start Session" button below):

```
[ Hostname / IP ]

[ Support ]
[ Desktop ]
```

`Support` visibility is controlled by `support_enabled` (config); it must not render when disabled. `Desktop` is always shown. See "Session Profile" below for what each button launches.

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

# Session Profile (revised 2026-08-28 — supersedes the single Start Session model below)

Two independent actions, each already an existing upstream session mechanism — no new session type is introduced:

- **Desktop button** -> one `DEFAULT_CONN` session. Standard upstream behavior: keyboard, mouse, clipboard, file transfer, audio, and any other upstream-supported capability all work unmodified. No camera.
- **Support button** -> one `DEFAULT_CONN` session + one `VIEW_CAMERA` session, opened together. Purpose: customer desktop support plus customer camera support. Audio is carried entirely by the `DEFAULT_CONN` half, using standard upstream audio routing — no audio-path or `VIEW_CAMERA`-audio changes are made or required (see `docs/session-orchestration-analysis.md` §7-8 for the investigation that ruled this unnecessary).
- Support button is rendered only when `support_enabled = true`; otherwise it must not appear in the UI at all.

### Superseded: single Start Session model

~~Start Session action launches camera + two-way audio always, desktop optionally when `desktop_enabled = true`, as a single user action.~~ This required combining audio onto a `VIEW_CAMERA` session, which upstream doesn't do by default. The Support/Desktop model avoids that entirely by using `DEFAULT_CONN` for anything that needs audio.

---

# Configuration Profile

Actual configuration format: TOML (see `docs/architecture.md` for the concrete schema and file location; any `key = value` shown in this document is TOML, not YAML — examples elsewhere in the doc set are illustrative unless explicitly marked as TOML).

Required configuration keys (revised 2026-08-28):

version
role
support_enabled
authentication.mode
listen_address
listen_port
video_quality
audio_quality
log_level

**`support_enabled`** (new) — gates whether the Support button is rendered. `camera_enabled`, `audio_enabled`, and `desktop_enabled` from the prior schema are **removed as an assumption of this revision, pending confirmation**: under the Support/Desktop model neither has a remaining referent — Desktop is unconditionally available (no gate needed) and Support unconditionally launches both `DEFAULT_CONN` and `VIEW_CAMERA` when enabled (no independent camera/audio toggle). `listen_address`, `listen_port`, `video_quality`, `audio_quality`, `log_level` are unaffected by this revision and remain reserved for their respective future phases (Direct-IP transport, minimal UI).

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
- Desktop button launches a standard upstream `DEFAULT_CONN` session with all upstream capabilities (keyboard, mouse, clipboard, file transfer, audio) intact.
- Support button launches `DEFAULT_CONN` + `VIEW_CAMERA` together, only when `support_enabled = true`; the button itself must not render when `support_enabled = false`.
- Public-ID workflow absent from user experience.
- Relay workflow absent from user experience.
- Rendezvous workflow absent from user experience.
- Configuration fully controls supported behavior.

---

# Automation Intent

This profile is the authoritative behavioral specification.

Automation tools, Claude Code workflows, upgrade scripts, and validation pipelines should use this profile as the source of truth when generating future Direct-IP fork releases.
