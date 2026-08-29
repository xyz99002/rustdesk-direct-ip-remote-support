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

## Connection Workflow (revised 2026-08-28, second revision — Support no longer needs DEFAULT_CONN or its audio)

The Support/Desktop redesign (see `docs/architecture.md` "Connection screen") eliminates the need for any server-side audio/camera plumbing change. Two earlier rounds of planned server-side changes have both been withdrawn — kept struck through for history:

| Hook | Location | Fork usage | Modification |
|---|---|---|---|
| `ConnType::VIEW_CAMERA` / `ConnType::DEFAULT_CONN` | Rust enum, used throughout `src/client.rs`, `src/client/io_loop.rs`, `src/server/connection.rs` | Support button always opens `VIEW_CAMERA`; additionally opens `DEFAULT_CONN` when `desktop_share_enabled=true`. Desktop button opens only `DEFAULT_CONN`, when `desktop_share_enabled=true` | None — reuses both existing connection types as-is |
| `connect()` (Dart) | `flutter/lib/common.dart:2580-2637` | Support handler calls `connect(context, id, isViewCamera: true)` always, plus `connect(context, id)` when `desktop_share_enabled`; Desktop handler calls it once (default), when `desktop_share_enabled` | New call sites; `connect()` itself unmodified |
| `FFI.start()` | `flutter/lib/models/model.dart:3746-3849` | Downstream of `connect()`; sets `ConnType` and drives `sessionAddSync`/`sessionStart` | None — reused as-is |
| `sessionAddSync`/`sessionStart` (FFI bridge) | `src/flutter_ffi.rs:137-193` | Rust-side entry from Dart | None — reused as-is |
| `session_request_voice_call(session_id)` | `src/flutter_ffi.rs:1703-1706` → `src/ui_session_interface.rs:1597-1601` | Called with the `VIEW_CAMERA` session's own `session_id` right after it connects, to start the Voice Call | None — reused as-is; new call site only |
| `VoiceCallRequest`/`VoiceCallResponse`, `is_view_camera_scoped_message` | `libs/hbb_common/protos/message.proto:865-875`; `src/server/connection.rs:5508-5522` | Confirmed already whitelisted for `VIEW_CAMERA` — see `docs/session-orchestration-analysis.md` §9-10 | None |
| Session bookkeeping by `SessionID` | `src/flutter.rs` (`sessions::insert_peer_session_id`, `flutter.rs:1283`) | Confirms multiple concurrent sessions to one peer are already supported — no changes needed here | None |
| `Client::_start` | `src/client.rs:238-257` | The single outbound choke point both sessions pass through (already houses the `is_incoming_only()` role check from Phase 3) | None |
| `enable-camera` permission | `libs/hbb_common/src/config.rs:2902` (`OPTION_ENABLE_CAMERA`), enforced at `src/server/connection.rs:2544-2551` | Fork writes it via `Config::set_option("enable-camera", "Y"/"N")` from `support_enabled`, so the remote rejects `VIEW_CAMERA`/Voice Call when disabled | None — reused exactly as `approve-mode` already is |
| Connection screen widget | `flutter/lib/desktop/pages/connection_page.dart` (`_ConnectionPageState`, `onConnect()`, `onSupport()`) | Implemented: independently-flagged Support/Desktop buttons | UI-only change; no Rust code touched by this row |
| `ViewCameraPage.initState()` | `flutter/lib/desktop/pages/view_camera_page.dart` (`addCallbackOnFirstImage`) | Implemented: calls `bind.sessionRequestVoiceCall(sessionId: _ffi.sessionId)` once the camera session's first frame arrives — safe in this fork because the page is only ever reached via the Support button (no peer list exists to reach it otherwise) | New call; page logic otherwise unmodified |
| ~~`add_camera_connection()` `include_audio` param~~ | ~~`src/server.rs:373-382`~~ | ~~Subscribe audio for view-camera~~ | **Withdrawn (round 1)** — not needed once Voice Call was found to work standalone |
| ~~`try_sub_camera_displays()` call site~~ | ~~`src/server/connection.rs:1963-1970`~~ | ~~Pass `self.audio_enabled()`~~ | **Withdrawn (round 1)** — same reason |
| ~~Rejecting `DEFAULT_CONN` when `desktop_share_enabled=false`~~ | ~~`src/server/connection.rs`'s login `_ =>` arm, `:2583-2587`~~ | ~~A new permission check~~ | **Investigated, not implemented** — no existing upstream permission rejects `DEFAULT_CONN` outright (its video is unconditional once accepted); adding one would be new authentication code, not reuse. Enforced locally only (button hidden). See `docs/FORK_PROFILE_SPEC.md`'s Configuration Profile. |

## Minimal UI (2026-08-29 — local connect screen stripped, remote ID/account/network UI hidden)

| Hook | Location | Fork usage | Modification |
|---|---|---|---|
| `DesktopSettingPage.tabKeys` | `flutter/lib/desktop/pages/desktop_setting_page.dart:66-84` | Pre-existing upstream conditionals: `network` tab hidden when `bind.mainGetBuildinOption(key: kOptionHideNetworkSetting) == 'Y'`; `account` tab hidden when `!bind.isDisableAccount()` is false (i.e. shown only when NOT disabled); `display`/`plugin` tabs already auto-hidden for `isIncomingOnly()`. Fork discovered and reused this — no Dart change needed at all. | None |
| `HARD_SETTINGS["disable-account"]` | `libs/hbb_common/src/config.rs:82`, read via `is_disable_account()` (`config.rs:2817-2819`, `is_some_hard_opton`) | Fork writes `"Y"` unconditionally in `apply()` — hides the Account settings tab (upstream's own custom-client mechanism) | None — direct write to the same map role/conn-type already use |
| `BUILTIN_SETTINGS["hide-network-settings"]` | `libs/hbb_common/src/config.rs:83`, read via `get_builtin_option()` (`src/common.rs:2273-2280`) | Fork writes `"Y"` unconditionally in `apply()` — hides the Network tab (relay/rendezvous server address configuration) | None — same pattern, different map |
| Local connect screen | `flutter/lib/desktop/pages/connection_page.dart` | Rewritten: peer list (`PeerTabPage`), autocomplete (`RawAutocomplete`/`AllPeersLoader`), the ID-specific `IDTextEditingController`/`IDTextInputFormatter`, public-server messaging (`OnlineStatusWidget`), and the ID-based workflow menu (file transfer/terminal/bare view-camera via peer context menu) all removed. Final UI: hostname/IP field + Support/Desktop buttons only. | Full-file rewrite; `ConnectionPage` class name and `onConnect`/`onSupport` API preserved so `desktop_home_page.dart`'s `buildRightPane()` needed no change |
| Remote status pane's ID board | `flutter/lib/desktop/pages/desktop_home_page.dart` (`buildIDBoard`/`buildPopupMenu`, removed) | ID display removed entirely. Its Settings-access popup menu is superseded by making the pre-existing bottom-left gear icon (previously `if (isOutgoingOnly)`-only) show for both roles | Removed 2 methods, widened 1 existing conditional |
| Remote status pane's connection status | `flutter/lib/desktop/pages/desktop_home_page.dart` (new `_ConnectionStatusWidget`, replacing the deleted `OnlineStatusWidget`) | Kept the status dot/text + "Start service" link (operational, needed for any inbound connection); dropped the public-server guide link (`onUsePublicServerGuide`, `setup_server_tip`) | New, smaller private widget scoped to this file |

## Not yet mapped (future phases)

- `listen_address`/`listen_port` wiring — Direct-IP transport phase.
- `video_quality`/`audio_quality`/`log_level` wiring — Media / minimal-UI polish phases.

## How to use this during an upstream upgrade

For each row above: confirm the referenced symbol/line still exists and still has the same meaning in the new upstream version. A rename, signature change, or behavioral change in any row is a required fork-code update, not optional. Cross-check against `docs/UPSTREAM_UPGRADE_GUIDE.md`'s regression checklist after updating.
