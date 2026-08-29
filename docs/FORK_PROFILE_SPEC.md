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

UI:

[ Hostname / IP ] [ Start Session ]

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

# Session Profile

Start Session action launches:

Required:

- camera
- two-way audio

Optional:

- desktop

Desktop launches only when:

desktop_enabled = true

The user experiences a single action even if multiple upstream session types are created.

---

# Configuration Profile

Actual configuration format: TOML (see `docs/architecture.md` for the concrete schema and file location; any `key = value` shown in this document is TOML, not YAML — examples elsewhere in the doc set are illustrative unless explicitly marked as TOML).

Required configuration keys:

version
role
authentication.mode
camera_enabled
audio_enabled
desktop_enabled
listen_address
listen_port
video_quality
audio_quality
log_level

Version changes must be backward compatible or provide migration guidance.

---

# Upgrade Rules

When upgrading upstream RustDesk:

1. Verify role hook points still exist.
2. Verify authentication mapping still exists.
3. Verify session startup still supports camera/audio/desktop.
4. Verify UI suppression points still exist.
5. Execute regression tests.

---

# Acceptance Criteria

- Local client initiates sessions only.
- Remote client accepts sessions only.
- IP/Hostname is the only connection method.
- ask mode works.
- password mode works.
- ask_and_password mode works.
- Camera launches successfully.
- Audio launches successfully.
- Desktop launches only when enabled.
- Public-ID workflow absent from user experience.
- Relay workflow absent from user experience.
- Rendezvous workflow absent from user experience.
- Configuration fully controls supported behavior.

---

# Automation Intent

This profile is the authoritative behavioral specification.

Automation tools, Claude Code workflows, upgrade scripts, and validation pipelines should use this profile as the source of truth when generating future Direct-IP fork releases.
