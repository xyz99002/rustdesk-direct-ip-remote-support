# Session Orchestration — Analysis (pre-implementation)

Scope: the next phase must make a single **Start Session** action on the local client launch a camera+audio session always, and additionally a desktop-control session when `desktop_enabled = true` (per `docs/DECISIONS.md`, `docs/FORK_PROFILE_SPEC.md`, `CLAUDE_MASTER_PROMPT.md`). This document is research only — no media or UI source has been modified. See `docs/HOOK_POINTS.md` for the consolidated hook-point registry this analysis feeds.

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

## Next step

With (d) selected, the smallest-change implementation plan (source files, diagram) is presented in the chat response alongside this document — implementation itself is deferred to the next turn, pending review.
