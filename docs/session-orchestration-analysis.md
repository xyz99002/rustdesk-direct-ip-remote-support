# Session Orchestration — Analysis (pre-implementation)

> **Superseded 2026-08-28 by the Support/Desktop button redesign** (see `docs/architecture.md` "Connection screen" and `docs/DECISIONS.md`). §§1-6 below were written against a single "Start Session" button (camera+audio always, desktop optional) that no longer reflects the product design — kept for their factual findings (launch paths, session bookkeeping) which remain accurate and load-bearing. §6's proposed server-side change (extending `add_camera_connection()` to carry audio) is **withdrawn** — see §7-8, which found it unnecessary once Support opens a real `DEFAULT_CONN` (which already has audio) alongside `VIEW_CAMERA`, rather than trying to put audio on `VIEW_CAMERA` itself.

Original scope (superseded): the next phase must make a single **Start Session** action on the local client launch a camera+audio session always, and additionally a desktop-control session when `desktop_enabled = true` (per `docs/DECISIONS.md`, `docs/FORK_PROFILE_SPEC.md`, `CLAUDE_MASTER_PROMPT.md`). This document is research only — no media or UI source has been modified. See `docs/HOOK_POINTS.md` for the consolidated hook-point registry this analysis feeds.

## 1. Camera launch path (today)

Entry points are peer-card context-menu items, not a dedicated button:

- `flutter/lib/common/widgets/peer_card.dart:596-602` — `_viewCameraAction()`, wired into several peer-card menus (lines 972, 1037, 1097, 1156, 1313).
- Calls `_connectCommonAction(..., isViewCamera: true)` → `connectInPeerTab(..., isViewCamera: true, ...)` (`peer_card.dart:1541-1580`) → the shared top-level `connect(context, peer.id, isViewCamera: true, ...)` in `flutter/lib/common.dart:2580`.
- `connect()` (`common.dart:2580-2637`) → `connectMainDesktop(id, isViewCamera: true, ...)` (`common.dart:2533-2573`) → `rustDeskWinManager.newViewCamera(id, ...)` (`common.dart:2550`; manager method `flutter/lib/utils/multi_window_manager.dart:308-327`) — opens a `WindowType.ViewCamera` window building `ViewCameraPage` (`flutter/lib/desktop/pages/view_camera_page.dart`).
- `ViewCameraPage.initState()` (lines 101-121) → `_ffi.start(widget.id, isViewCamera: true, ...)` (line 111-121).
- `FFI.start()` (`flutter/lib/models/model.dart:3746-3849`): with `isViewCamera: true`, sets `connType = ConnType.viewCamera` (line 3775), calls `bind.sessionAddSync(..., isViewCamera: true, ...)` (lines 3793-3806) then `bind.sessionStart(...)` (line 3843) — the flutter_rust_bridge calls into `src/flutter_ffi.rs:137-193`, mapping to Rust `ConnType::VIEW_CAMERA`.

Mobile has an equivalent, simpler path (`flutter/lib/mobile/pages/home_page.dart:207-250` → same `connect()` → `flutter/lib/mobile/pages/view_camera_page.dart:90` → `gFFI.start(..., isViewCamera: true)`), not this fork's target UI but confirming the same underlying mechanism.

## 2. Audio launch path (today) — the key constraint

**Audio is not requested by the connecting (local) client at all.** It is a host-side (remote) permission decision, and — critically — it is only ever wired up for `ConnType::DEFAULT_CONN` (desktop) sessions, never for `ConnType::VIEW_CAMERA`:

- `src/server/connection.rs:522` — `audio: Self::permission(keys::OPTION_ENABLE_AUDIO, &control_permissions)`, set from the host's own config when the `Connection` is constructed. Nothing the client sends influences this.
- `audio_enabled()` (`connection.rs:2078-2080`) — `self.audio && !self.disable_audio`.
- `audio_enabled()` is only consulted inside `try_sub_monitor_services()` (`connection.rs:1980-2004`), and that function is **only called when `is_remote()` is true** (`is_remote()` defined `connection.rs:1972-1978` — true only for `ConnType::DEFAULT_CONN`, false for file-transfer, port-forward, terminal, and **view-camera**). If `!audio_enabled()`, `audio_service::NAME` is excluded from the subscribed services (lines 2002-2004) — i.e. audio simply follows the desktop video stream automatically for `DEFAULT_CONN`, with no separate client toggle.
- Client-side, `src/client/io_loop.rs:1812-1814` only *reacts* to a `Permission::Audio` push from the host — it never requests audio.
- A distinct, unrelated feature exists: `handle_voice_call`/`close_voice_call` (`connection.rs:4363-4389`), gated on `is_authed_view_camera_conn()` — an explicit "voice call" toggle layered onto camera sessions. This is a separate on/off feature the user must invoke, not automatic audio-with-video, and is a different mechanism from `try_sub_monitor_services`'s auto-audio-with-video behavior for `DEFAULT_CONN`.

**Consequence:** as of upstream 1.4.9, there is no existing code path that gives a `VIEW_CAMERA` session automatic two-way audio the way a `DEFAULT_CONN` session gets it. "Camera + audio always" as specified cannot be satisfied purely by orchestrating existing UI calls — it requires a small, targeted server-side change (see §6).

## 3. Desktop launch path (today)

- Main "Connect" button: `flutter/lib/desktop/pages/connection_page.dart`, class `_ConnectionPageState` (line 199). The `ElevatedButton` (lines 521-526, `onPressed: onConnect`) and the ID field's `onSubmitted` (line 441) both call `onConnect()` (lines 330-339), with no `isViewCamera`/`isFileTransfer`/etc. flags — i.e. a default desktop-control connection.
- `onConnect()` → same `connect()` (`common.dart:2580`) → `connectMainDesktop()`'s `else` branch → `rustDeskWinManager.newRemoteDesktop(id, ...)` (`common.dart:2568`; manager `multi_window_manager.dart:270-287`) — opens `WindowType.RemoteDesktop` building `RemotePage` (`flutter/lib/desktop/pages/remote_page.dart`).
- `RemotePage.initState()` (lines 122-143) → `_ffi.start(widget.id, ...)` (line 134), no `isViewCamera` → `FFI.start()` sets `connType = ConnType.defaultConn` (`model.dart:3782`) → same `sessionAddSync`/`sessionStart` chain → Rust `ConnType::DEFAULT_CONN`.

## 4. Session initialization path (Dart → Rust), confirmed end-to-end

```
Dart button/menu
  -> connect()                          common.dart:2580
  -> connectMainDesktop()                common.dart:2533
  -> rustDeskWinManager.new{RemoteDesktop,ViewCamera}()   multi_window_manager.dart:270-343
       (spawns/reuses a Flutter sub-window/tab)
  -> page widget initState()            remote_page.dart:134 / view_camera_page.dart:111
  -> FFI.start()                        model.dart:3746
  -> bind.sessionAddSync() + bind.sessionStart()   (flutter_rust_bridge -> src/flutter_ffi.rs:137-193)
  -> Rust session bookkeeping           src/flutter.rs (sessions::insert_peer_session_id, :1283)
  -> src/client/io_loop.rs:172
  -> Client::_start()                   src/client.rs:238
```

Each Dart `FFI` instance gets its own fresh `SessionID` (`model.dart:3705`), and Rust indexes sessions by `SessionID`, not by peer id (`sessions::insert_peer_session_id` keys on `(peer_id, conn_type, session_id, ...)`). **This confirms multiple independent, concurrent sessions to the same peer are already supported** — a camera session and a desktop session to the same host can coexist, each with its own `SessionID`/`FFI` instance and its own `Client::_start` call. This is the load-bearing fact that makes "one button, two sessions underneath" implementable without any session-model changes.

## 5. Connect button handler (today's wiring point)

The concrete top-level Dart entry a future "Start Session" button parallels: `onConnect()` in `flutter/lib/desktop/pages/connection_page.dart:330-339`, which wraps the shared `connect()` (`common.dart:2580-2637`). `connect()` already accepts `isViewCamera` as a flag and dispatches accordingly, so a new handler can call it twice — `connect(context, id, isViewCamera: true)` and, conditionally, `connect(context, id)` — for the same `id`, producing two independent sessions per §4. `connectInPeerTab()` (`peer_card.dart:1541`) is the equivalent for peer-list-initiated connections.

## 6. The camera+audio constraint — options considered

Given §2's finding, several options were considered for how "camera + audio always" can actually be satisfied. Two service-registration mechanisms exist server-side, discovered by tracing exactly how each connection type gets its services:

- **`add_connection(conn, noperms)`** (`src/server.rs:384-401`, used by `DEFAULT_CONN` via `try_sub_monitor_services`, `connection.rs:1980-2011`) — subscribes `conn` to *every* registered service except those named in `noperms`. This is a broad, generic mechanism: it also covers clipboard, cursor position, and window-focus services, gated by their own permission checks (`connection.rs:1986-2001`).
- **`add_camera_connection(conn)`** (`src/server.rs:373-382`, used by `VIEW_CAMERA` via `try_sub_camera_displays`, `connection.rs:1963-1970`) — a completely separate, narrow mechanism that subscribes `conn` to *only* the primary camera video service. It never touches `self.services` generically and has no audio (or any other service) subscription at all, by construction.

Options:

- **(a) Widen `is_remote()`/`try_sub_monitor_services`'s condition to also cover `VIEW_CAMERA`.** Initially considered, then rejected on closer inspection: `try_sub_monitor_services` runs through `add_connection`'s broad per-service loop, which would also make camera sessions eligible for clipboard, cursor-position, and window-focus subscriptions (each still individually gated by its own permission check, but this widens the surface unnecessarily and mixes desktop-control-oriented logic into the camera path — not the smallest change).
- **(b) Make the camera stream ride on a `DEFAULT_CONN` session instead of `VIEW_CAMERA`.** Rejected: `DEFAULT_CONN`'s video is the desktop screen, not the camera device — this doesn't produce a camera stream at all, it's a different capture source entirely.
- **(c) Always open a second, hidden `DEFAULT_CONN` session purely to carry audio, alongside a `VIEW_CAMERA` session for video.** Rejected: a `DEFAULT_CONN` session inherently captures and streams the desktop screen — there is no way to open one for "audio only" without also always capturing the screen, which directly undermines "desktop launches only when `desktop_enabled = true`" (the remote's screen would always be captured/transmitted regardless of that setting — a privacy regression and wasted bandwidth/CPU).
- **(d) Extend `add_camera_connection` itself to optionally also subscribe the audio service.** **Recommended.** Two small, isolated changes:
  1. `src/server.rs:373-382` — change `add_camera_connection(&mut self, conn: ConnInner)` to `add_camera_connection(&mut self, conn: ConnInner, include_audio: bool)`; when `include_audio` is true, look up the audio service (by `audio_service::NAME`, the same constant `try_sub_monitor_services` already uses at `connection.rs:2003`) and call `on_subscribe` on it, mirroring the one-service-at-a-time pattern already used for the camera service in the same function (lines 375-379).
  2. `src/server/connection.rs:1968` (inside `try_sub_camera_displays`) — change the call to pass `self.audio_enabled()` (the same method `try_sub_monitor_services` already calls at line 2002) as the new argument.

  This touches only the camera-connection path — nothing about `DEFAULT_CONN`, `is_remote()`, clipboard, cursor, window-focus, authentication, transport, or encryption changes. Camera sessions gain exactly one new capability (audio), using the same `audio_enabled()` permission check upstream already applies to desktop sessions.

## 7. Verification (2026-08-28) — audio-on-camera evidence, and desktop-video permission evidence

Requested by the user before any implementation: source-traced verification of whether upstream already provides audio-on-camera, camera-only, audio-without-desktop, and desktop enable/disable via existing permissions. Evidence only — no fix proposed here; this **supersedes §6's premise** that camera sessions structurally cannot carry audio (see the revised conclusion at the end of this section).

### Question 1 — Audio on VIEW_CAMERA sessions

**A. Does the login/permission message contain an audio capability flag?**
Yes. `LoginRequest` (`libs/hbb_common/protos/message.proto:72-91`) has an `OptionMessage option = 6` field, present on every login regardless of connection type (it's outside the `oneof union` that carries `file_transfer`/`port_forward`/`view_camera`/`terminal`). `OptionMessage` (`message.proto:672-699`) has `BoolOption disable_audio = 7`.

**B. Is that flag transmitted for VIEW_CAMERA sessions?**
Yes, structurally — but in practice, almost never with a meaningful value. Client-side, `Client::get_option_message()` (`src/client.rs:2237-2309`) builds this field for every conn type except `PORT_FORWARD`/`RDP`/`FILE_TRANSFER` (line 2238-2243) — `VIEW_CAMERA` is included. But the only line that touches it is `if self.get_toggle_option("disable-audio") { msg.disable_audio = BoolOption::Yes.into(); }` (lines 2298-2300) — there is **no corresponding `else` branch setting `BoolOption::No`**. So: if the user has never explicitly disabled audio for that peer, `disable_audio` is left at its protobuf zero-value, `NotSet` — not "enabled", just absent.

**C. Is that flag accepted by the remote side?**
Yes, explicitly and deliberately. `Connection::scoped_view_camera_option()` (`src/server/connection.rs:5554-5577`) is the whitelist filter applied to login-time options for `AuthConnType::ViewCamera` (dispatched from `scoped_login_option()`, `connection.rs:5358-5371`, called via `update_scoped_login_options()`, `connection.rs:5334-5356`). It explicitly copies `disable_audio` through (`connection.rs:5569-5571`: `if let Ok(value) = option.disable_audio.enum_value() { scoped.disable_audio = value.into(); }`), and `disable_audio` is conspicuously **absent** from `option_has_non_view_camera_login_field()`'s violation list (`connection.rs:5579-5592`, which does list `disable_clipboard`, `disable_keyboard`, etc.) — i.e. it was deliberately classified as camera-compatible, not filtered out.

**D. What code path consumes that permission?**
`Connection::update_options()` (`src/server/connection.rs:4405-4496` for the relevant branch). At lines 4475-4496: `if let Ok(q) = o.disable_audio.enum_value() { if q != BoolOption::NotSet { self.disable_audio = q == BoolOption::Yes; if let Some(s) = self.server.upgrade() { if self.is_authed_view_camera_conn() { if self.voice_calling || !self.audio_enabled() { s.write().unwrap().subscribe(super::audio_service::NAME, self.inner.clone(), self.audio_enabled()); } } else { s.write().unwrap().subscribe(super::audio_service::NAME, self.inner.clone(), self.audio_enabled()); } } } }`. This function is reached two ways: (1) at login, via `update_scoped_login_options()` (§C above), and (2) mid-session, via the `Misc::Option` message handler (`connection.rs:3467-3473`, itself scoped through `scoped_update_option_message()` → `scoped_login_option()` for non-`Remote` conn types, `connection.rs:5283-5290`). **Critically, this whole block is gated on `q != BoolOption::NotSet`** — it only runs at all if the client sent an explicit Yes or No.

**E. Does VIEW_CAMERA ultimately subscribe to `audio_service`, directly or indirectly?**
Two separate paths exist, with different outcomes:
- **Initial connection setup** (`try_sub_camera_displays()`, `connection.rs:1963-1970`, calling `Server::add_camera_connection()`, `src/server.rs:373-382`): **No.** This path only ever subscribes the primary camera video service (`server.rs:375-379`); it has no audio logic at all, and no `noperms`-style parameter to add any.
- **`update_options()`** (§D): **Yes, conditionally** — if it ever runs with a non-`NotSet` `disable_audio` value, it calls `subscribe(audio_service::NAME, conn, self.audio_enabled())` for view-camera connections too, gated on `self.voice_calling || !self.audio_enabled()` (i.e. it (re)subscribes when turning audio on, or when a voice call is active).
- `audio_enabled()` itself (`connection.rs:2078-2080`: `self.audio && !self.disable_audio`) is identical logic to the `DEFAULT_CONN` case — `self.audio` comes from the host's `enable-audio` permission (`Self::permission(keys::OPTION_ENABLE_AUDIO, ...)`, set once at connection construction, `connection.rs:522`), unrelated to conn type.

**F. If audio does not function by default, what's the limitation?**
None of the four listed categories fits cleanly on its own — the evidence points to a **default-value / unreached-code-path gap**, not a hard block:
- Not (1) disabled permission — `enable-audio` is a normal host permission, evaluated identically for both conn types.
- Not (2) disabled capability flag in the strict sense — the flag isn't "disabled", it's simply never *set* by the client for camera sessions in the normal flow, because `get_option_message()` has no `else` branch to send an explicit `No`.
- Not (3) missing service subscription in the sense of "doesn't exist" — the subscription call exists and works (§D/E); it's just never invoked, because the `NotSet` guard at `connection.rs:4476` never opens.
- Not (4) protocol limitation — the wire format and the server-side scoping logic explicitly support this exact case (§B/C).
- **Additional client-UI finding:** `flutter/lib/common/widgets/toolbar.dart:886-917` (`toolbarDisplayToggle`) is the only place that ever sends an explicit `disable-audio` toggle (`bind.sessionToggleOption(sessionId, 'disable-audio')`, which reaches `Client::toggle_option()`, `src/client.rs:2077-2111`, which *does* send an explicit `BoolOption::Yes`/`BoolOption::No` — line 2104-2111). This "Mute" menu entry is gated by `final isDefaultConn = ffi.connType == ConnType.defaultConn;` and `if (isDefaultConn && perms['audio'] != false)` (`toolbar.dart:893,906`) — **it is never shown for `ConnType.viewCamera`**. So even the one client-side path capable of sending the explicit value that would activate §D's code is deliberately excluded from the camera UI.

**Net finding:** the server-side machinery to give a `VIEW_CAMERA` session audio already exists and is exercised by real, non-dead code (§D/E) — it is reachable today only via a mid-session `Misc::Option{disable_audio}` message, which nothing in the current Flutter UI ever sends for a camera session, and which the initial `LoginRequest` never sends either (it only ever sends `Yes`, never `No`). This is a materially different, and smaller, gap than §6 assumed.

### Question 2 — Desktop video enable/disable

**A. Is there a flag in the login/permission message that enables/disables desktop video independently?**
No such field exists. `OptionMessage` (`message.proto:672-699`) has `disable_clipboard`, `disable_keyboard`, `disable_camera`, `disable_audio`, `block_input`, `privacy_mode`, `show_remote_cursor`, etc. — no `disable_video`/`disable_screen`/`disable_desktop` field. `PermissionInfo.Permission` (`message.proto:630-639`) enumerates `Keyboard, Clipboard, Audio, File, Restart, Recording, BlockInput, PrivacyMode` — no `Video`/`Screen` value. The host-side permission-key constants mirror this: `libs/hbb_common/src/config.rs:2899-2910` defines `OPTION_ENABLE_KEYBOARD`, `OPTION_ENABLE_CLIPBOARD`, `OPTION_ENABLE_FILE_TRANSFER`, `OPTION_ENABLE_CAMERA`, `OPTION_ENABLE_TERMINAL`, `OPTION_ENABLE_AUDIO`, `OPTION_ENABLE_TUNNEL`, `OPTION_ENABLE_REMOTE_RESTART`, `OPTION_ENABLE_RECORD_SESSION`, `OPTION_ENABLE_BLOCK_INPUT`, `OPTION_ENABLE_PRIVACY_MODE` — again, nothing for desktop video/screen.

**B. Can a client establish a `DEFAULT_CONN` session while denying desktop video?**
No. `try_sub_monitor_services()` (`connection.rs:1980-2011`) calls `s.try_add_primay_video_service()` (line 2009) **unconditionally** whenever `is_remote()` is true and services haven't been subscribed yet — there is no permission check, `noperms` entry, or `OptionMessage` field gating it, unlike clipboard/cursor/window-focus/audio (lines 1986-2004), which are all conditionally excluded from `noperms` based on a permission/config check. Primary video is structurally mandatory for `DEFAULT_CONN`.

**C. Can desktop visibility be turned on/off purely by permissions, without changing connection type?**
No — see B. The closest adjacent mechanism is `PrivacyMode` (`toggle_privacy_mode`, `connection.rs:4301-4310`; `PermissionInfo::Permission::PrivacyMode`), which blanks the **host's own physical display** while still streaming video to the controller — it does not stop the connecting client from receiving a video stream, so it does not answer "can the client deny receiving video." `OPTION_ENABLE_CAMERA` (`config.rs:2902`, consumed at `connection.rs:2400,2545`) is the nearest thing to a per-capability enable/disable flag, but it gates the separate `VIEW_CAMERA` connection type (whether camera connections are allowed *at all*), not desktop video within a `DEFAULT_CONN` session.

**D. Is desktop access negotiated through the same permission framework as keyboard/mouse/file-transfer?**
No. Keyboard, clipboard, audio, and file-transfer are all **sub-capability permissions layered on top of an already-established `DEFAULT_CONN` (or, for audio, potentially `VIEW_CAMERA`, per Q1) session** — each independently toggleable via `OptionMessage`/`PermissionInfo` while the session runs. Desktop video access itself is gated one level higher, **at connection-type selection** (choosing to establish a `DEFAULT_CONN` at all, itself gated by host-side approval/auth, not by an in-session permission bit). There is no permission bit for "give me a `DEFAULT_CONN` session but without its video."

## Revised conclusion (evidence-driven, no fix proposed)

Per the user's stated preference to reuse existing upstream capability rather than add fork-specific behavior: **(1) camera-only** already works exactly as upstream ships it (§1 camera launch path, unmodified). **(2) camera + audio** does *not* require the server-side `add_camera_connection()` change floated in §6 — the server-side plumbing already exists and already works (§1.D/E above); what's actually missing is a client-side value (an explicit non-`NotSet` `disable_audio`) that nothing today sends for a camera session. **(3) audio without desktop** falls out of the same finding as (2) — once camera sessions carry audio, camera-only already *is* "audio without desktop video." **(4) desktop enable/disable via existing permissions** does **not** exist upstream (§2) — enabling/disabling desktop screen sharing is not a permission-framework capability today; it can only be expressed as "establish a `DEFAULT_CONN` session or don't," which is exactly the fork's planned `desktop_enabled` config flag deciding whether to open that second session at all (§6's already-adopted approach), not something achievable by toggling an existing permission bit.

## 9. Voice Call — existing feature, verified to work standalone on VIEW_CAMERA (2026-08-28)

Investigated whether RustDesk's existing "Voice Call" feature (two-way microphone audio, distinct from the one-way system-audio-follows-`DEFAULT_CONN` behavior in §1-2) can be initiated programmatically and function with only a `VIEW_CAMERA` session — no `DEFAULT_CONN` anywhere.

**Source paths (all pre-existing, none modified):**

| Layer | File:line |
|---|---|
| Protocol messages | `libs/hbb_common/protos/message.proto:865-875` (`VoiceCallRequest{req_timestamp, is_connect}`, `VoiceCallResponse{accepted, req_timestamp, ack_timestamp}`), `:974-975` (`Message.union` fields 23/24) |
| Client send | `src/ui_session_interface.rs:1597-1606` (`request_voice_call()`/`close_voice_call()`) → `src/client.rs:3817-3818` (`Data::NewVoiceCall`/`CloseVoiceCall`) → `src/client/io_loop.rs:981-997` (builds/sends the message) |
| Server receive/accept | `src/server/connection.rs:3633-3644` (incoming request → notifies connection manager, unconditional on conn type), `:4363-4403` (`handle_voice_call()`/`close_voice_call()`) |
| View-camera message whitelist | `src/server/connection.rs:5508-5522` — `is_view_camera_scoped_message()` explicitly permits `VoiceCallRequest`, `VoiceCallResponse`, `AudioFrame`; `is_view_camera_scoped_misc()` (`:5524-5546`) permits `AudioFormat` |
| Connection-manager (host accept) | `src/ui_cm_interface.rs:656,944-951`, `src/ipc.rs:376` |
| FFI | `src/flutter_ffi.rs:1703-1728` — `session_request_voice_call`/`session_close_voice_call` (controller), `cm_handle_incoming_voice_call`/`cm_close_voice_call` (host) |
| Dart entry points | `flutter/lib/desktop/widgets/remote_toolbar.dart:2651` (desktop `DEFAULT_CONN`), `flutter/lib/mobile/pages/remote_page.dart:814` (mobile `DEFAULT_CONN`), `flutter/lib/mobile/pages/view_camera_page.dart:496` (mobile **`VIEW_CAMERA`** — direct proof it's already wired for camera sessions). Desktop's `view_camera_page.dart` has no such button today (UI gap only). |

**Session type:** not a new/separate `ConnType`. A message-level feature layered on an already-established session — confirmed identical for `DEFAULT_CONN` and `VIEW_CAMERA`.

**Permission:** no dedicated "enable-voice-call" key exists. Every incoming request unconditionally surfaces an accept/reject prompt at the host (no auto-accept path found in Rust or Dart) — a human must click Accept. Once accepted, the host's `enable-audio` permission additionally gates whether audio actually flows.

## 10. Scenario verification — VIEW_CAMERA-only voice call, no DEFAULT_CONN anywhere

Traced precisely whether any part of the Voice Call pipeline requires a `DEFAULT_CONN` session to exist. **Conclusion: no — every piece of state and every method involved belongs to the single `Connection` struct instance representing the `VIEW_CAMERA` session itself.**

- **Session ownership:** `voice_call_request_timestamp`, `voice_calling`, `disable_audio`, `audio_sender`, `authed_conn_id` are all fields of *that one* `Connection` instance (`src/server/connection.rs:343,348,558`). `is_authed_view_camera_conn()` (`connection.rs:5265-5270`) checks only `self.authed_conn_id` — never looks up or requires a sibling connection.
- **Audio-service subscription path (host mic → controller):** `handle_voice_call()` (`connection.rs:4363-4389`, accepted branch) calls `audio_service::set_voice_call_input_device(...)` then `s.write().unwrap().subscribe(audio_service::NAME, self.inner.clone(), ...)`. `Server::subscribe()` (`src/server.rs:432-444`) registers *this connection's own* `ConnInner`. `audio_service` (`src/server/audio_service.rs`) is a single, **process-wide** capture+broadcast service (not per-connection) — `get_audio_input()` (`audio_service.rs:61-68`) returns the microphone when `VOICE_CALL_INPUT_DEVICE` is set, else the normal system-audio device, and broadcasts to every subscribed `ConnInner` regardless of conn type. Subscribing the `VIEW_CAMERA` connection's `ConnInner` is sufficient by itself.
- **Reverse direction (controller mic → host):** `Misc::Union::AudioFormat` (`connection.rs:3526-3535`) creates `self.audio_sender` (again, a field of the `VIEW_CAMERA` connection's own struct) via `start_audio_thread()`; subsequent `AudioFrame` messages (`connection.rs:3622-3631`) feed into that same sender. Both message types are on the view-camera whitelist (§9 table).
- **Assumption/side-effect worth carrying forward:** `audio_service` being process-wide means that if a `DEFAULT_CONN` session to the same host is *also* active and *also* subscribed when a `VIEW_CAMERA` voice call starts, that `DEFAULT_CONN` session's audio would momentarily switch from system audio to the microphone too, for as long as the call lasts. Not a defect for the VIEW_CAMERA-only scenario, but relevant when Support opens both session types together (`desktop_share_enabled = true` case) — noted, not fixed (no media-service changes authorized).

## Next step (superseded twice)

~~With (d) selected, the smallest-change implementation plan...~~ then ~~Support = DEFAULT_CONN + VIEW_CAMERA always, audio via DEFAULT_CONN...~~ — both superseded 2026-08-28 by §9-10's finding: Voice Call already works standalone on `VIEW_CAMERA`, so Support needs `VIEW_CAMERA` + Voice Call only, with `DEFAULT_CONN` now an independent, separately-flagged addition (`desktop_share_enabled`) rather than a permanent companion. See the chat response for the current Connection Workflow implementation plan.
