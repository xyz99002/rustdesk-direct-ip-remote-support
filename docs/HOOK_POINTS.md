# HOOK_POINTS.md — Consolidated Upstream Hook Point Registry

Every upstream RustDesk mechanism this fork reads from, writes to, or calls into, in one place — so an upstream upgrade can be checked against a single list rather than re-discovered. Each entry: what it is, where it lives, what the fork does with it, and what phase introduced the dependency. See `docs/UPSTREAM_UPGRADE_GUIDE.md` for the upgrade-time verification checklist built from this list, and `docs/FORK_AUTOMATION.md` for the narrative/pipeline framing of the same facts.

## Configuration & role (Phase 3)

| Hook | Location | Fork usage | Modification |
|---|---|---|---|
| `HARD_SETTINGS` | `libs/hbb_common/src/config.rs:82` — `pub static ref HARD_SETTINGS: RwLock<HashMap<String, String>>` | Fork writes `"conn-type"` = `"outgoing"`/`"incoming"` here at startup | None — direct write to an existing public static |
| `is_outgoing_only()` | `libs/hbb_common/src/config.rs:2784-2790` | Read by upstream's own `rendezvous_mediator.rs:118-122` to skip the inbound listener | None — read-only dependency |
| `is_incoming_only()` | `libs/hbb_common/src/config.rs:2774-2781` | Read by upstream's own `src/client.rs:255-257` to refuse outbound connects | None — read-only dependency |
| `core_main()` startup order | `src/core_main.rs:31-40` | Fork's `fork_config::load_and_apply()` called at line 36 (was line 35 before this insertion), immediately after `crate::load_custom_client()`, before any listener/connect decision | 1-line insertion — see `docs/UPSTREAM_UPGRADE_GUIDE.md`'s "startup call-order dependency" risk |
| `approve-mode` config option | read by `libs/hbb_common/src/password_security.rs:77-86` (`approve_mode()`) | Fork writes it via `Config::set_option` from `authentication.mode` | None — read-only dependency on the read side |
| `Config::get_option`/`set_option` | `libs/hbb_common/src/config.rs:1245-1274` | Fork's only write path for `approve-mode` | None — calls existing public API |
| `Config::path`/`load_path`/`store_path` | `libs/hbb_common/src/config.rs:558-591,783-804` | Confirms `toml`/`confy` already resolved in the workspace; fork's own loader (`src/fork_config.rs`) does not call these directly (needs precise error distinction they don't provide) but depends on the `toml` crate version they pin | Fork's `Cargo.toml` pins `toml = "0.7"` to match |

## Connection Workflow (revised 2026-08-28 — formerly "Session orchestration"; server-side media rows below are withdrawn)

The Support/Desktop redesign (see `docs/architecture.md` "Connection screen") eliminates the need for any server-side audio/camera plumbing change. The two rows below (`add_camera_connection()`, `try_sub_camera_displays()`) that were planned in the prior "Session orchestration" design are **no longer needed** — kept here struck through for history, since the investigation that justified them (`docs/session-orchestration-analysis.md` §6) is superseded by §7-8's finding that `DEFAULT_CONN` already carries audio with zero changes.

| Hook | Location | Fork usage (planned) | Modification (planned) |
|---|---|---|---|
| `ConnType::VIEW_CAMERA` / `ConnType::DEFAULT_CONN` | Rust enum, used throughout `src/client.rs`, `src/client/io_loop.rs`, `src/server/connection.rs` | Support button opens both a `DEFAULT_CONN` and a `VIEW_CAMERA` session together; Desktop button opens only `DEFAULT_CONN` | None — reuses both existing connection types as-is |
| `connect()` (Dart) | `flutter/lib/common.dart:2580-2637` | Support handler calls this twice (once default for `DEFAULT_CONN`, once `isViewCamera: true`) for the same target; Desktop handler calls it once (default) | New call sites; `connect()` itself unmodified |
| `FFI.start()` | `flutter/lib/models/model.dart:3746-3849` | Downstream of `connect()`; sets `ConnType` and drives `sessionAddSync`/`sessionStart` | None — reused as-is |
| `sessionAddSync`/`sessionStart` (FFI bridge) | `src/flutter_ffi.rs:137-193` | Rust-side entry from Dart | None — reused as-is |
| Session bookkeeping by `SessionID` | `src/flutter.rs` (`sessions::insert_peer_session_id`, `flutter.rs:1283`) | Confirms multiple concurrent sessions to one peer are already supported — no changes needed here | None |
| `Client::_start` | `src/client.rs:238-257` | The single outbound choke point both sessions pass through (already houses the `is_incoming_only()` role check from Phase 3) | None |
| Connection screen widget | `flutter/lib/desktop/pages/connection_page.dart` (`_ConnectionPageState`, `onConnect()` at lines 330-339, button at 521-526) | **Planned change:** replace the single Connect button with Desktop + conditionally-rendered Support buttons, wiring each to the calls above | UI-only change; no Rust code touched by this row |
| ~~`add_camera_connection()`~~ | ~~`src/server.rs:373-382`~~ | ~~Add an `include_audio` parameter~~ | **Withdrawn** — not needed; Support's audio comes from the `DEFAULT_CONN` half, not from `VIEW_CAMERA` |
| ~~`try_sub_camera_displays()` call site~~ | ~~`src/server/connection.rs:1963-1970`~~ | ~~Pass `self.audio_enabled()` into `add_camera_connection`~~ | **Withdrawn** — same reason |

## Not yet mapped (future phases)

- Minimal UI suppression points (hiding ID/relay/rendezvous/account controls) — Direct-IP transport / minimal-UI phases.
- `listen_address`/`listen_port` wiring — Direct-IP transport phase.
- `video_quality`/`audio_quality`/`log_level` wiring — Media / minimal-UI phases.

## How to use this during an upstream upgrade

For each row above: confirm the referenced symbol/line still exists and still has the same meaning in the new upstream version. A rename, signature change, or behavioral change in any row is a required fork-code update, not optional. Cross-check against `docs/UPSTREAM_UPGRADE_GUIDE.md`'s regression checklist after updating.
