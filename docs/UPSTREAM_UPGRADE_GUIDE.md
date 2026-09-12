# UPSTREAM_UPGRADE_GUIDE.md

# Purpose
This document describes how to upgrade the Direct-IP RustDesk fork to a newer upstream RustDesk release while preserving the fork behavior.

**In-progress work**: `docs/PLAN-install-separator.md` — Windows (Rust runtime `APP_NAME`, the
Local-mode server/IPC removal, and the MSI installer split) are implemented as of 2026-09-11,
each with its own hook point below ("App Identity", "No Server/IPC for Local Mode", "App Identity
(MSI)"). **None of it has been CI-verified or manually tested on a real machine yet.** Linux,
macOS, and Android packaging identity separation (Phases 3-5 of that plan) remain not started.
Check that plan's current status before assuming any part of it is done.

## Current Baseline
- RustDesk Version: 1.4.9
- Commit: 6c578292e

## Upgrade Workflow
1. Create branch: upgrade/rustdesk-<version>
2. Import or merge new upstream.
3. Build unmodified upstream.
4. Run fork verification checklist.
5. Reapply any required fork-specific patches.
6. Re-verify `docs/FEATURE_ENFORCEMENT_MATRIX.md` — the authoritative record of which enforcement layer (UI/config/remote/upstream) backs every fork feature. Every "Yes" cell cites a specific source location; confirm each one still holds in the new upstream version before trusting the matrix for release acceptance.
7. Execute automated regression tests.

## Configuration Format
The fork's own configuration file is TOML (confirmed 2026-08-28) — reuses the `toml`/`confy` crates already present via `hbb_common`, no new dependency. Any YAML-fenced example elsewhere in the project's documentation is illustrative only.

## Critical Hook Points
### Role Enforcement
Verify:
- is_incoming_only()
- is_outgoing_only()
- HARD_SETTINGS["conn-type"]

### Authentication Mapping
Verify:
- approve-mode
- click
- password
- both/default

Mappings (`approve-mode`):
- ask -> click
- password -> password
- ask_and_password -> both/default (empty string, falls through to upstream's own default)

**Second, independent mapping (added in `28518a326`, not to be confused with the above)**:
`auth-mode` also drives upstream's `verification-method` option — this controls whether a
temporary password is *generated/displayed* at all, separately from `approve-mode` controlling
*how* a connection is approved. Without this second mapping, the temporary-password display
reflects whatever upstream's leftover/default value happens to be, regardless of `auth-mode` —
this is exactly what caused a real reported bug ("ask mode still shows a password"). Mappings:
- ask -> `use-permanent-password` (approval is a manual click; no password is ever checked, so hide it)
- password -> `use-temporary-password` (approval requires the temporary password; show it)
- ask_and_password -> empty string (keep upstream's own default, temporary password shown)

Both mappings live together in `fork_config.rs::apply()`; a future upstream change to either
`approve-mode` or `verification-method`'s semantics (`libs/hbb_common/src/password_security.rs`)
must be re-verified against **both** mappings, not just one.

### Local Client
Verify outbound-only behavior still works.

### Remote Client
Verify inbound-only behavior still works.

### Connection Workflow (revised 2026-08-28, second revision — formerly "Session Startup")
Verify:
- Desktop button launches a standard `DEFAULT_CONN` session only (all upstream capabilities intact, no camera, no voice call), hidden entirely when `desktop_share_enabled = false`.
- Support button always launches `VIEW_CAMERA` + a Voice Call on it, additionally `DEFAULT_CONN` when `desktop_share_enabled = true`; hidden entirely when `support_enabled = false`.
- A config with both `support_enabled` and `desktop_share_enabled` false is rejected by `src/fork_config.rs`'s validation.
- `ConnType::VIEW_CAMERA` and `ConnType::DEFAULT_CONN` can still run concurrently to the same peer (independent `SessionID`s).
- `VoiceCallRequest`/`VoiceCallResponse`/`AudioFrame`/`AudioFormat` remain whitelisted for view-camera-scoped messages (`src/server/connection.rs:5508-5546`) — this is the single fact the entire Support design depends on; if a future upstream release narrows this whitelist, Voice Call on `VIEW_CAMERA` breaks.
- `enable-camera` (`OPTION_ENABLE_CAMERA`) still gates `VIEW_CAMERA` login acceptance at `src/server/connection.rs:2544-2551` — this is what `support_enabled`'s remote-side enforcement depends on.
- No server-side audio/media code was touched by this fork — confirm that remains true after the upgrade (see `docs/HOOK_POINTS.md` "Connection Workflow" section; all withdrawn rows should stay withdrawn unless a future investigation proves them necessary again).
- **Known gap, re-verify it's still a gap:** confirm no existing upstream permission has been added to reject `DEFAULT_CONN` outright — if one has, it may be worth revisiting whether `desktop_share_enabled` can now be enforced remotely too (currently local-UI-only, documented in `docs/FORK_PROFILE_SPEC.md`).

### Minimal UI (implemented 2026-08-29; revised 2026-09-11 — see below)
Verify:
- `flutter/lib/desktop/pages/connection_page.dart` still has no peer list, autocomplete, ID-lookup, or public-server messaging after merging a new upstream release — this file was fully rewritten, so a naive merge/patch is the most likely thing to silently resurrect removed UI.
- `DesktopSettingPage.tabKeys` (`flutter/lib/desktop/pages/desktop_setting_page.dart`) still conditionally excludes `account` based on `is_disable_account()`, and `HARD_SETTINGS`/`BUILTIN_SETTINGS` are still plain `pub static` maps `fork_config.rs::apply()` can write directly.
  - **Revised 2026-09-11**: the Network tab is **no longer excluded**. Only two rows *within* it are hidden — `BUILTIN_SETTINGS["hide-server-settings"]`/`["hide-websocket-settings"]`, set unconditionally in `fork_config.rs::apply()` — because Network also holds Proxy/TLS/UDP options this fork still uses. `kOptionHideNetworkSetting`/`hide-network-settings` (whole-tab hide) is no longer used anywhere in this fork. If a future upstream release renames/restructures `desktop_setting_page.dart`'s `network()` builder or the `hide-server-settings`/`hide-websocket-settings` `kOption*` constants, re-verify these two rows are still the ones hidden, not the whole tab.
  - **Added 2026-09-11**: the Printer tab is now also hidden unconditionally via
    `BUILTIN_SETTINGS["hide-remote-printer-settings"]` (`fork_config.rs::apply()`) — a
    Windows-only remote-printer-driver management screen, not applicable to this fork's supported
    use cases. Unlike Network, this tab has no relevant content to preserve, so it's a full-tab
    hide (matching Account), not a row-level trim.
- `flutter/lib/desktop/pages/desktop_home_page.dart`'s remote status pane still has no ID board, still shows password management (`buildPasswordBoard2`) and connection status (`_ConnectionStatusWidget`).
  - **Revised 2026-09-11**: the Settings gear icon and the "Change Password" pencil icon are each now additionally gated on `mainGetBoolOptionSync("show-setup-ui")` (in addition to `!bind.isDisableSettings()` for the pencil icon) — added because `DesktopSettingPage.switch2page()` already gated the *action* on `show-setup-ui`, but nothing previously gated the *visibility* of the two icons that call it, so they stayed clickable-but-broken when `show-setup-ui = "N"`. A future upstream change to either icon's surrounding widget must preserve this visibility gate, not just the click-through gate in `switch2page()`.
- `server_page.dart`'s `ConnectionManager`/`_CmHeader`/`_PrivilegeBoard` (connection manager, Voice Call accept/reject) remain untouched — this phase deliberately did not modify them.

### Advance Setup Gate (implemented 2026-09-12)
Verify:
- `src/core_main.rs` still filters `--advance-setup` out of the general args vector (in the same
  arg-parsing loop as `--elevate`/`--no-server`/etc.) — it must **not** end up in `args`, since a
  non-empty `args` changes which startup branch runs (would silently break the role-gated
  server-thread spawn from the "No Server/IPC for Local Mode" hook point below). It sets
  `BUILTIN_SETTINGS["advance-setup"]` to `"Y"`/`"N"` — **in-memory only, not persisted to
  `config.toml`** — so it must be passed on every launch that wants Safety/Display visible; it
  does not stick across restarts by design.
- `flutter/lib/consts.dart`'s `kOptionAdvanceSetup = "advance-setup"` still matches the Rust-side
  key string exactly (no shared constant between the two languages — a future rename on either
  side silently breaks this if not mirrored).
- `DesktopSettingPage.tabKeys` (`flutter/lib/desktop/pages/desktop_setting_page.dart`) still ANDs
  `bind.mainGetBuildinOption(key: kOptionAdvanceSetup) == 'Y'` onto both the Safety tab's existing
  role-based condition (`!isOutgoingOnly()`) and the Display tab's (`!isIncomingOnly()`) — the
  role-based gating from "Minimal UI" above is unchanged and still applies; `--advance-setup` is
  an *additional* requirement, not a replacement for it. Without the flag, Safety/Display stay
  hidden regardless of role, same as before this change for a plain launch.
- **Not extended to Network, Account, or Printer** — those keep their existing gating
  (`hide-server-settings`/`hide-websocket-settings` row-level trim, full-tab hide, full-tab hide
  respectively), unaffected by `--advance-setup`.

### App Identity (implemented 2026-09-10/11, see `docs/DECISIONS.md` "App Identity")
Verify:
- `src/core_main.rs::core_main()` still sets `hbb_common::config::APP_NAME` to a fork-distinct
  value (currently `"RustDesk-DirectIP-RemoteSupport"`) as the *first* thing it does — before
  `load_custom_client()`, before `hbb_common::init_log()`, before `fork_config::config_exists()`/
  `load_and_apply()`. This one value drives the Windows IPC pipe name
  (`\\.\pipe\{APP_NAME}\query`), the `%APPDATA%\{APP_NAME}\` storage directory (peers/options/
  logs), the window title/taskbar text, and (via the in-app "Install" flow's existing
  `rename_exe_cmd()`/`get_default_install_path()`/`get_subkey()` helpers, all already keyed off
  `crate::get_app_name()`) the install path, Windows Service name, and renamed installed exe when
  a user clicks "Install" from the portable build.
- **Why this exists**: without it, this fork's `APP_NAME` defaulted to the literal string
  `"RustDesk"` — identical to a real RustDesk install. A machine with real RustDesk already
  installed (its background `--service`/`--server` process running) caused this fork's GUI to
  connect, over the shared-by-name IPC pipe, to that *other* process instead of its own,
  silently serving stale/foreign option values and making `config.toml` changes appear to have
  no effect. See `docs/DECISIONS.md` for the full incident.
- **Gap closed 2026-09-11 for the MSI installer specifically** — see the new "App Identity (MSI)"
  hook point below. The separately-built Windows **MSI installer** (`res/msi/`) never read this
  Rust constant; it now gets an equivalent, independently-set identity via CI build parameters.
- A future upstream change to `ui_interface.rs`'s `OPTIONS` cache/`ipc::connect()` pipe-path
  construction, or to `hbb_common::config::Config::path()`/`ipc_path()`'s use of `APP_NAME`, should
  be re-checked against this hook — the fix depends on `APP_NAME` still being the single source of
  truth for both.

### App Identity (MSI) (implemented 2026-09-11, `docs/PLAN-install-separator.md` Phase 1)
Verify:
- `.github/workflows/flutter-build.yml`'s "Build msi" step still loops over two variants (Local,
  Remote) rather than building a single MSI, still resets `res/msi/Package` via
  `git checkout` between passes (required — `preprocess.py` edits `Includes.wxi`/`RustDesk.wxs` in
  place, a second un-reset pass duplicates every file component and breaks the WiX build), still
  renames the dist-dir exe to `$env:MSI_APP_NAME.exe` (currently
  `RustDesk-DirectIP-RemoteSupport.exe` — must match `src/core_main.rs`'s runtime `APP_NAME`) and
  passes `--app-name $env:MSI_APP_NAME` to `preprocess.py`, and still passes `--conn-type outgoing`
  for the Local variant only (Remote gets no `--conn-type` flag — today's default).
- `--app-name` changes, consistently, everything derived from WiX's one `$(var.Product)` variable:
  ProductName, install folder (`C:\Program Files\<AppName>\`), `UpgradeCode` (this is the piece
  that actually matters — WiX's `<MajorUpgrade>` triggers off `UpgradeCode` family match, not
  install folder, so this is what stops a real RustDesk MSI being silently uninstalled/upgraded
  over), registry root, Windows Service name, uninstall entry, DisplayIcon path, shortcuts, and the
  file/URL-association entries in `Regs.wxs`. If a future upstream release restructures any of
  these WiX templates to derive from a *different* variable, or removes `$(var.Product)`
  entirely, this hook breaks silently (the build may still succeed, producing an MSI that's
  identical to real RustDesk's again).
- `--conn-type outgoing` relies on pre-existing, unmodified upstream WiX conditions
  (`CC_CONNECTION_TYPE="outgoing"`, [RustDesk.wxs:47,48,59,78,128](../res/msi/Package/Components/RustDesk.wxs))
  that gate service creation/start, tray auto-launch, and a SAS-generation registry tweak. A
  future upstream change to any of these conditions needs re-verifying that the Local variant
  still installs with no service.
- The bundled `config.toml` (`configs/local.toml` / `configs/remote.toml`, copied into the dist
  dir before each pass) means `fork_config::config_exists()` is already true on first launch of
  an MSI-installed copy — the first-run Local/Remote picker dialog should never appear for either
  variant. A future upstream change to `handle_first_run_setup()`'s trigger condition
  (`fork_config::config_exists()`) should be re-checked against this.
- **Not yet done**: `res/msi/CustomActions/CustomActions.cpp` was deliberately left untouched
  (this Phase renames the exe to match the product identity instead of trying to keep it as
  `rustdesk.exe` while the identity differs, which would have required native C++ changes to the
  uninstall/upgrade service-teardown path — see `docs/PLAN-install-separator.md` for why that
  alternative was rejected as too risky to implement without a build/test environment).
- **Not yet verified on a real machine** (no WiX/MSBuild/Windows install environment available
  during implementation) — see the manual test checklist in `docs/PLAN-install-separator.md`
  Phase 1 before assuming install/upgrade/uninstall/service behavior is correct, beyond "the CI
  build produces two .msi files without error."

### No Server/IPC for Local Mode (implemented 2026-09-11, `docs/PLAN-install-separator.md` Phase 2)
Verify:
- `core_main.rs`'s `std::thread::spawn(move || crate::start_server(false, no_server))` (in the
  no-args GUI-launch branch) is still gated behind `!config::is_outgoing_only()` — a `role=local`
  instance should never spawn the background "server" thread at all.
- `flutter_ffi.rs::main_check_connect_status()` still skips calling `start_option_status_sync()`
  when `config::is_outgoing_only()` — this is the function `main.dart` calls unconditionally at
  startup (`bind.mainCheckConnectStatus()`) that would otherwise force the `SENDER` lazy-static
  (and therefore the whole GUI↔server IPC polling loop) to initialize even with nothing on the
  other end.
- `tray.rs`'s `start_query_session_count` spawn is still gated the same way.
- **Why this exists**: this is not just cosmetic — it's what makes the App Identity incident above
  structurally impossible to repeat for Local deployments, rather than merely fixed for the one
  pipe-name collision already found. A `role=local` instance now has no local IPC channel to be
  hijacked by an unrelated process at all, regardless of pipe naming.
- **Upgrade check**: if a future upstream release changes `ui_interface.rs`'s `get_option`/
  `set_option`/`set_options` to depend on IPC succeeding (they currently write through to
  `Config`/the in-process `OPTIONS` cache directly, with IPC as a best-effort side channel — see
  `ipc.rs:1767`), re-verify Settings changes still persist correctly in Local mode with the server
  thread never started. This hook was deliberately implemented *without* adding a direct-`Config`-
  access branch (unlike the Android/iOS pattern) specifically because that write-through already
  existed — confirm it still does after any upstream change to that code path.
- Not yet extended to `SENDER`'s other touchpoints (e.g. `check_mouse_time()`) — confirmed
  unreachable for `role=local` in practice (inbound-session-only call paths); re-verify this
  assumption if a future upstream release starts calling those from an outgoing-only code path.

### Fork Peer Marker — NOT IMPLEMENTED, blocked (see `docs/DECISIONS.md`)
No hook point exists in the code today; nothing to verify. Recorded here only so a future upgrade
doesn't rediscover the same blocker from scratch: adding a protocol-level "is this actually a fork
peer" marker to `LoginRequest` requires editing `libs/hbb_common/protos/message.proto`, which
lives inside the `libs/hbb_common` git submodule — the official upstream `rustdesk/hbb_common`
repo, not something this fork owns. See `docs/DECISIONS.md` "Fork Peer Marker" for the full
writeup and the options under consideration before this can be implemented.

### Direct-IP Enforcement (implemented 2026-08-29, ADR-0003)
Verify:
- `src/rendezvous_mediator.rs::start_all()` still has both `--- BEGIN/END DIRECT-IP FORK ---` blocks: the `hbbs_http::sync::start()` call removed, and the registration loop replaced with `loop { sleep(1.).await; }`.
- No path outside this function calls `RendezvousMediator::start()`/`start_udp()`/`start_tcp()`/`register_pk()`/`register_peer()` directly (re-run `grep -rn "RendezvousMediator::start\(" src/` and confirm the only match is inside `start_all()` itself, now unreachable).
- `direct_server(...)` and LAN listening are still spawned as independent tasks *before* the removed loop, and both still start successfully for `role=remote`.
- `Config::set_option("enable-lan-discovery", "N")` is still present in `fork_config.rs::apply()`, and `src/lan.rs`'s ping-response handler still gates the ID-bearing `pong` on that exact option.
- A `role=remote` instance, monitored at the network level, sends **no** outbound UDP/TCP traffic to any rendezvous server address, and does not respond to a LAN-broadcast discovery ping with its ID.
- `RendezvousMediator::restart()`'s call sites (`flutter_ffi.rs`, `ipc.rs`, `ui_interface.rs`) still compile — the function itself is intentionally unmodified even though its effect is now inert.

## Newly Discovered Upgrade Risks (found during Phase 3 implementation)

- **Startup call-order dependency (revised 2026-09-11 — line numbers below now current).** Inside
  `pub fn core_main()` (`src/core_main.rs:191`), the order is now: `global_init()` →
  `hbb_common::config::APP_NAME` set (`src/core_main.rs:207`, see "App Identity" hook point above)
  → `load_custom_client()` (`:208`) → `hbb_common::init_log()` (`:214`, moved here specifically so
  `fork_config`'s own `log::info!`/`log::warn!` calls are actually captured — see next bullet) →
  `fork_config::config_exists()` (`:220`) → `fork_config::load_and_apply()` (`:230`). All of this
  relies on running before argument parsing and before the inbound-listener/outbound-connect
  decision. If a future upstream release reorders `core_main()` — e.g. moves argument parsing or
  server-spawn logic earlier — the fork's role/auth mapping could apply too late (after the
  listener already started, or after an outbound connect was already permitted), and/or the
  `APP_NAME` fix could end up set too late to prevent the IPC-pipe collision it exists to fix.
  **Upgrade check:** confirm all five of the above still run, in this relative order, before any
  branching in `core_main()`, and re-anchor the fork hooks to the same relative position rather
  than trusting the line numbers above (they will drift on every upstream merge).
- **`init_log()`/`fork_config` logging order (added 2026-09-11).** The `log` crate is a no-op sink
  until a logger backend is installed, and `hbb_common::init_log()`'s internal `static INIT: Once`
  guard means only the *first* call per process takes effect. Before this fix, `init_log()` was
  called later in `core_main()` (after the arg-parsing loop, for per-process log-file naming —
  `--server`/`--tray`/`--elevate` etc. each get their own log file), which meant every
  `log::info!`/`log::warn!` call inside `fork_config::load_and_apply()` — including its one
  diagnostic summary line (`fork_config: applied role=... auth_mode=... ...`) — was silently
  discarded. Fixed by extracting the per-process log-name computation into a standalone
  `early_log_name()` helper (`src/core_main.rs:162`) that runs, and calls `init_log()`, before
  `fork_config::load_and_apply()`. **Upgrade check:** if a future upstream release changes how
  `init_log()`'s `Once` guard works, or restructures the per-process log-naming logic this helper
  duplicates, re-verify `fork_config:`-prefixed log lines still appear in the log file for every
  process kind (`--server`, `--tray`, plain GUI launch, etc.), not just the ones that happen to
  call `init_log()` a second time harmlessly.
- **Temporary diagnostic logging in `main_get_option_sync` (added 2026-09-10, not yet removed).**
  `src/flutter_ffi.rs`'s `main_get_option_sync()` currently logs every GUI read of
  `desktop-share-enabled`, `show-setup-ui`, and `enable-camera` (`fork_config: GUI read option
  '{key}' = '{v}'`), added to diagnose the IPC/`OPTIONS`-cache collision described in the "App
  Identity" hook point above. The code comment marks it "Remove once confirmed." **Upgrade check /
  cleanup reminder:** decide whether to remove this before treating the App Identity fix as fully
  closed out — if kept, a future upstream change to `main_get_option_sync`'s signature or the
  `get_option()` it wraps needs to preserve this logging, not silently drop it on merge.
- **Mobile entry path not covered.** `core_main()` is `#[cfg(not(any(target_os = "android", target_os = "ios")))]` (`src/core_main.rs:30`) — the fork's hook does not run on Android/iOS. Not a regression today (desktop-only scope), but if a future upstream upgrade is paired with adding mobile support to this fork, a second hook point in the mobile entry path (not yet identified) would be required.
- **`set_option` persists, not just overrides in-memory.** `Config::set_option` (`libs/hbb_common/src/config.rs:1259-1274`) writes through to `config2.toml` via `CONFIG2.write()...store()`. A future upstream change to `is_option_can_save`/`OVERWRITE_SETTINGS`/`DEFAULT_SETTINGS` semantics (`config.rs` — the gating logic around line 1260) could silently turn the fork's `set_option("approve-mode", ...)` call into a no-op if `approve-mode` becomes a hard-overwritten setting upstream. **Upgrade check:** verify a fork-set `approve-mode` value actually persists and is read back after restart, not just accepted without error.
- **`toml` crate version must track `hbb_common`'s.** The fork's `Cargo.toml` pins `toml = "0.7"` to match `libs/hbb_common/Cargo.toml:43` exactly (reusing the version already resolved in the workspace, no new dependency). If a future upstream release bumps `hbb_common`'s `toml` version, the fork's `Cargo.toml` must be bumped to match, or Cargo will resolve two versions in the lockfile.
- **`HARD_SETTINGS` has no schema/versioning of its own.** It's a bare `HashMap<String, String>` (`config.rs:82`) populated by whichever code runs first — both `load_custom_client()` and the fork's own loader write into it. If a future upstream release starts using the `"conn-type"` key for something else, or introduces its own conflicting writer, the fork's role enforcement would silently break (no compile-time or type-level safety). **Upgrade check:** grep for `"conn-type"` and `HARD_SETTINGS` after every upgrade to confirm nothing new writes to that key before the fork's hook runs.

## Known Build Environment Issue (discovered 2026-08-28, unrelated to fork code)

A clean `cargo build`/`cargo test` of the full `rustdesk` binary on this Windows dev machine is currently blocked by a pre-existing, environment-level issue in the vendored `aom` (AV1) vcpkg port — **not caused by any fork change**:

- `vcpkg install` (manifest mode, triplet `x64-windows-static`) succeeds for every dependency except `aom`, which fails during CMake configure: `Unsupported nasm: multipass optimization not supported` (`aom_optimization.cmake:219`). This is a known compatibility gap between this repo's overlay `aom` port (`res/vcpkg/aom`) and the NASM version vcpkg downloads for itself (3.01) — unrelated to the system NASM installed separately for this environment.
- Separately, `vcpkg install` in manifest mode installs to `<repo>/vcpkg_installed/<triplet>`, but the `vcpkg-rs`-based build scripts in `libs/scrap/build.rs` and `magnum-opus` look for `$VCPKG_ROOT/installed/<triplet>` (classic-mode layout). Worked around locally with a directory junction (`New-Item -ItemType Junction`) linking the two; a real fix would set `VCPKG_ROOT` to the project-local install or pass `VCPKGRS_TRIPLET`/equivalent so the build scripts resolve the manifest location directly.
- Net effect: `libvpx`, `libyuv`, `opus`, and `libjpeg-turbo` build and link successfully; only `aom` (AV1 support) is unavailable, which blocks `scrap`'s build script (it unconditionally generates AV1 FFI bindings, `libs/scrap/build.rs:249`, consumed unconditionally by `libs/scrap/src/common/aom.rs:7`/`mod.rs:51` — there is no feature flag to skip it).
- **Verification workaround used for Phase 3:** `src/fork_config.rs` was verified in an isolated scratch crate (real module + real tests, against a stub `hbb_common` matching the exact signatures of `Config::get_option`/`set_option`, `HARD_SETTINGS`, and `is_incoming_only()`/`is_outgoing_only()` read from the actual source) — all 12 tests pass, `cargo fmt`/`clippy` clean. This is a legitimate proxy for the module's own correctness but does **not** substitute for linking the real binary.
- **Recommended follow-up (separate task, not part of Phase 3):** either patch/pin a compatible NASM version for the `aom` port (or update the overlay port's baseline to one with a compatible check), and fix the manifest/classic vcpkg path mismatch properly (rather than the junction workaround) so `cargo build`/`cargo test` succeed end-to-end on this machine.

## Regression Checklist
- Local cannot accept sessions.
- Remote cannot initiate sessions.
- Direct-IP connect using hostname works.
- Direct-IP connect using IP works.
- ask mode works.
- password mode works.
- ask_and_password mode works.
- Desktop button: standard `DEFAULT_CONN` session only, all upstream capabilities (keyboard, mouse, clipboard, file transfer, audio) work unmodified; no camera, no voice call.
- Support button: `VIEW_CAMERA` establishes and the Voice Call connects (after host-side accept), with no `DEFAULT_CONN` present when `desktop_share_enabled = false`; `DEFAULT_CONN` additionally establishes when `desktop_share_enabled = true`.
- Support button does not render when `support_enabled = false`; Desktop button does not render when `desktop_share_enabled = false`.
- A config with both flags false is rejected at load time.
- Remote rejects `VIEW_CAMERA`/Voice Call when `support_enabled = false` (via `enable-camera`).
- Local connect screen shows only a hostname/IP field and the applicable Support/Desktop button(s) — no peer list, no ID field, no public-server prompt.
- Remote status pane shows no RustDesk ID, but does show the one-time password board and connection status.
- Settings page has no Account tab, on both local and remote builds. **Revised 2026-09-10**: the
  Network tab is intentionally *visible* (not removed) — verify instead that only its "ID/Relay
  Server" and "Use WebSocket" rows are hidden, while Proxy/TLS-fallback/Disable-UDP remain.
- Settings gear icon and "Change Password" pencil icon (`desktop_home_page.dart`) are hidden
  entirely, not just non-functional, when `show-setup-ui = "N"`.
- Connection-manager accept/reject dialogs (including Voice Call's) still appear and function normally.
- Local mode: no background server thread/IPC polling loop starts (nothing to observe directly in
  the UI, but Settings changes still persist across restart, outgoing connect still works, tray
  icon behaves normally, About tab fingerprint field is blank — accepted, not a bug).
- Remote mode: unaffected by the above — server thread and IPC still start normally.
- Launching normally (no `--advance-setup`): Safety and Display tabs are hidden regardless of
  role, even for the role each would normally be shown for.
- Launching with `--advance-setup`: Safety shows for `role=remote`, Display shows for
  `role=local` — same as pre-this-change behavior, but only with the flag present. Relaunching
  without the flag hides them again immediately (no persistence).

## Build Environment Verification (added 2026-08-29)

**Before attempting `cargo build`:**

1. **Check for known vcpkg/dependency blockers** (see `docs/BUILD_BLOCKER_ANALYSIS.md`).
   - aom version: Confirm the upstream version's `res/vcpkg/aom/vcpkg.json` and expected NASM compatibility.
   - If aom 3.12.1+ is required and you hit NASM multipass errors → apply Strategy 1 (downgrade to 3.9.1) or your chosen remediation from the BUILD_BLOCKER_ANALYSIS.

2. **Run vcpkg dependency resolution:**
   ```bash
   vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static libjpeg-turbo:x64-windows-static
   ```
   - Expected: All packages resolve without error.
   - If any fail: document the new blocker in `docs/BUILD_BLOCKER_ANALYSIS.md`.

3. **Attempt a clean Rust build:**
   ```bash
   cargo build --release
   ```
   - Expected: `target/release/rustdesk.exe` (or equivalent) produced; no critical errors.
   - Time budget: 30–60 minutes (cold start) or 5–15 minutes (incremental).
   - If blocker: stop; resolve before proceeding to packaging or release phases.

4. **Run fork-specific test suite:**
   ```bash
   cargo test -- --test-threads=1
   ```
   - Focus on `src/fork_config.rs` tests (role mapping, authentication mode mapping, button visibility).
   - Expected: All tests pass.

5. **Check Flutter builds for the target platform(s):**
   ```bash
   cd flutter
   flutter pub get
   flutter build windows --release  # (or macos/linux)
   ```
   - Expected: `flutter/build/[windows|macos|linux]/...` directory produced with all assets.
   - Time budget: 20–30 minutes (cold start) or 5–10 minutes (incremental).

## Packaging Verification (added 2026-08-29)

**After successful builds, prepare release artifacts:**

1. **Ensure the `configs/*.toml` samples are up-to-date** (see `docs/PACKAGING_PLAN.md`).
   - Verify the schema key set (`role`, `auth-mode`, `support-enabled`, `desktop-share-enabled`,
     `listen-address`, `listen-port`, `video-quality`, `audio-quality`, `log-level`,
     `show-setup-ui`, `config-version`) matches `src/fork_config.rs`'s `keys` module and
     `ForkConfig` struct. **Revised 2026-09-10**: these keys no longer carry a `direct-ip-`
     prefix — it was removed from every fork-owned schema key across `src/fork_config.rs`,
     `configs/*.toml`, and related Dart comments. The two keys that look similar but are
     deliberately *not* part of this schema and were *not* renamed — `direct-server` /
     `direct-access-port` — are genuine, unrelated upstream RustDesk option keys (upstream's own
     "Enable direct IP access" feature); do not confuse the two or rename those if a future
     upgrade revisits this area.
   - Provide both local and remote examples (`configs/local.toml`/`remote.toml`).

2. **Build platform-specific installers/packages** (see `docs/PACKAGING_PLAN.md` for detailed steps):
   - Windows: NSIS or MSI installer (e.g., rustdesk-local-[version]-x64.exe)
   - macOS: .dmg or .app bundle
   - Linux: .deb or .rpm packages

3. **Generate checksums** for all artifacts:
   ```bash
   sha256sum rustdesk-*.exe rustdesk-*.dmg rustdesk-*.deb > checksums.txt
   ```

4. **Sign packages** (optional, recommended for Windows/macOS).

## Release Validation (added 2026-08-29)

**Before shipping, complete the full release checklist** (see `docs/RELEASE_CHECKLIST.md`):

1. **Build Verification:** All Rust, Flutter, and packaging steps complete without errors.
2. **Functional Verification:** Complete all tests in RELEASE_CHECKLIST.md (Support mode, Desktop mode, Voice Call, authentication modes, role enforcement).
3. **Direct-IP Enforcement Verification:**
   - [ ] No rendezvous registration (monitor network traffic; see ADR-0003).
   - [ ] No relay participation (both instances on different networks; relay should not be attempted).
   - [ ] No LAN discovery ID exposure (send broadcast ping; remote should not respond with ID).
4. **Regression Testing:** Verify upstream features (keyboard, mouse, clipboard, file transfer, audio) still work, especially on `DEFAULT_CONN` (Desktop mode).
5. **Documentation review:** Confirm release notes reference `docs/DECISIONS.md`, `docs/architecture.md`, and the relevant ADRs (e.g., ADR-0003 for Direct-IP Enforcement).

**Gate:** All items must pass before release is approved.

## Build Blocker Tracking (added 2026-08-29)

Maintain `docs/BUILD_BLOCKER_ANALYSIS.md` as the authoritative record of:
- Current blockers (if any)
- Root-cause classification (environment/RustDesk-design/vcpkg/external)
- Remediation strategy chosen
- Workarounds in use

**Trigger an update to this file when:**
- A new blocker is discovered during an upgrade or build attempt
- A blocker is resolved
- A workaround is replaced with a permanent fix

## Release Acceptance
Upgrade is accepted only if all checks pass:
1. **Build Readiness:** `docs/BUILD_BLOCKER_ANALYSIS.md` shows no unresolved blockers; `cargo build --release` succeeds.
2. **Packaging Readiness:** `docs/PACKAGING_PLAN.md` build steps complete; all platform-specific artifacts generated.
3. **Functional Readiness:** `docs/RELEASE_CHECKLIST.md` items all pass.
4. **Documentation Current:** `docs/FEATURE_ENFORCEMENT_MATRIX.md` has been re-verified against the new upstream version (not just left as-is from the prior baseline).
