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

### Direct-IP Listener Enforcement (implemented 2026-09-30, found via a real user connection test)
Verify, on any upstream merge that touches `src/rendezvous_mediator.rs`'s `direct_server()`/
`get_direct_port()`, or `libs/hbb_common/src/config.rs`'s `OPTION_DIRECT_SERVER`/
`OPTION_DIRECT_ACCESS_PORT`:
- **Critical finding**: this fork has no rendezvous/relay accept path at all
  (`docs/ADR-0003-DIRECT-IP-ENFORCEMENT.md`) — `direct_server()` is the *only* code path that ever
  binds a listening socket, and it explicitly does nothing when upstream's own `direct-server`
  option is off. `configs/remote.toml` shipped `direct-server = "N"` (upstream's own default),
  meaning **every "remote"-role instance, as shipped, never accepted a single inbound connection**
  — confirmed by a real two-machine test (local role on one machine, remote role on another;
  connection only succeeded once the remote side was switched to a plain upstream RustDesk build).
  This was not caught by any of this fork's own test suite because `fork_config.rs`'s existing
  tests check `is_incoming_only()`/`HARD_SETTINGS["conn-type"]`, not whether anything is actually
  listening — those are necessary but not sufficient for "remote role actually works."
- **Fixed** in `fork_config.rs::apply()`: `direct-server` is now unconditionally forced to `"Y"`
  and `direct-access-port` is forced from this module's own `listen_port` schema field — both
  written into `hbb_common::config::OVERWRITE_SETTINGS`, not just `Config::set_option`, so
  `ui_interface::is_option_fixed()` also reports them as fixed, which the existing Settings UI
  (`desktop_setting_page.dart`'s "Enable direct IP access" checkbox, `settings_page.dart`'s mobile
  equivalent) already checks to grey out a control that must never actually be changeable — there
  is deliberately no user-facing toggle for something that can only ever be one value, not just a
  pre-ticked default.
- **Also fixed, a smaller but real correctness issue found in the same pass**: `config.toml`'s
  schema had *two* port-shaped keys — this module's own `listen-port` (validated since day one,
  per its doc comment, but never actually wired to anything) and the plain mirrored upstream
  `direct-access-port` (the one that actually did something, via `mirror_upstream_options()`).
  `listen-port` is now the single authoritative source — `configs/local.toml`/`remote.toml` no
  longer list `direct-server`/`direct-access-port` directly, since they're now fully overridden
  regardless of what's in the file.
- **Upgrade check**: if a future upstream release changes `direct_server()`'s gating condition,
  renames `OPTION_DIRECT_SERVER`/`OPTION_DIRECT_ACCESS_PORT`, or changes how `is_option_fixed()`
  resolves `OVERWRITE_SETTINGS`, re-verify a remote-role instance still actually binds a listener
  end-to-end — this hook point exists specifically because that gap was invisible to unit tests
  and only surfaced via a real network connection attempt.

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
  `bind.mainGetBuildinOption(key: kOptionAdvanceSetup) == 'Y'` onto the Safety tab's existing
  role-based condition (`!isOutgoingOnly()`), the Display tab's (`!isIncomingOnly()`), **and**
  the Network tab's existing `hide-network-settings`/`hide-server-settings`/
  `hide-websocket-settings` row-level condition — all pre-existing gating is unchanged and still
  applies; `--advance-setup` is an *additional* requirement on all three, not a replacement for
  any of it. Without the flag, Safety/Display/Network all stay hidden regardless of role or
  `fork_config.rs`'s row-level settings.
- **Not extended to Account** — it keeps its existing unconditional full-tab hide, unaffected by
  either flag below (100% irrelevant to this fork regardless of role, no "show with a flag" case).
- **Printer got its own separate flag, `--printer-setup`, added 2026-09-12** — deliberately not
  folded into `--advance-setup`, since Printer's irrelevance (a Windows remote-printer-driver
  management screen) is unrelated to Safety/Display/Network's role-based relevance. Same
  in-memory-only, filtered-from-`args` treatment. `flutter/lib/consts.dart`'s
  `kOptionPrinterSetup = "printer-setup"` must match the Rust-side key exactly, same caveat as
  `kOptionAdvanceSetup` above. `DesktopSettingPage.tabKeys`'s Printer condition is now
  `isWindows && (hide-remote-printer-settings != 'Y' || printer-setup == 'Y')` — the `||` matters:
  `fork_config.rs::apply()` unconditionally sets `hide-remote-printer-settings = "Y"`, so
  `--printer-setup` must be able to **override** that hide, not just avoid conflicting with it. A
  future upstream change to how `hide-remote-printer-settings` is read/combined should preserve
  this override relationship.
  - **Revised 2026-09-12**: unlike Account, Printer turned out to have genuinely mixed content
    like Network — `__PrinterState.build()`'s `outgoing(context)` section (install/manage a local
    printer driver so a remote session's print job redirects to a printer on *this* machine) only
    matters when this instance initiates connections; its `incoming(context)` section ("Incoming
    Print Jobs": dismiss/default-printer/selected-printer, auto-print) only matters when this
    instance is being controlled. So even with `--printer-setup` shown, the tab now hides
    `outgoing` for `role=remote` and `incoming` for `role=local` (`if (!bind.isIncomingOnly())
    outgoing(context)`, `if (!bind.isOutgoingOnly()) incoming(context)`) — same
    only-irrelevant-content-removed treatment as Network's rows, not a whole-tab decision. A
    future upstream change to `_Printer`'s section names/structure should preserve this per-role
    split.

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
- **Bug found and fixed 2026-09-12**: `get_valid_subkey()` (`src/platform/windows.rs`) checked a
  fixed, hardcoded product-code GUID (`IS1`, a legacy Inno-Setup-style identifier shared by every
  RustDesk-family build regardless of app name) *before* falling back to the `APP_NAME`-derived
  subkey. On a machine with a real RustDesk already installed, this meant the in-app "Install"
  dialog's pre-filled path (`bind.installInstallPath()` → `ui_interface::install_path()` →
  `get_install_info()` → `get_valid_subkey()`) silently reused the *real RustDesk's* registered
  `InstallLocation` (`C:\Program Files\RustDesk`) instead of computing this fork's own
  app-name-based default — reported by a user screenshot showing the Install dialog defaulting to
  `C:\Program Files\RustDesk`. Fixed by removing the `IS1` checks entirely; `get_valid_subkey()`
  now only ever looks up this fork's own `APP_NAME`-derived subkey. **Upgrade check**: if a future
  upstream release reintroduces a similar "detect any existing RustDesk-family install via a fixed
  identifier" mechanism anywhere in `platform/windows.rs`, re-verify it doesn't reintroduce this
  same collision — any lookup keyed by something other than `crate::get_app_name()` is suspect.
- **Second bug found and fixed 2026-09-12, same incident**: with the `IS1` bug fixed, installing
  via the portable exe's "Install" button correctly targeted
  `C:\Program Files\RustDesk-DirectIP-RemoteSupport\`, but produced a Start Menu shortcut pointing
  at a file that was never created — a "Missing Shortcut" error reported via screenshot.
  Root cause: `install_me()` (`src/platform/windows.rs`) calls `copy_exe_cmd()` to copy the source
  exe into the install folder verbatim, but never called the existing `rename_exe_cmd()` helper —
  every shortcut/registry entry `install_me()` generates assumes the installed exe is named
  `{APP_NAME}.exe` (via `get_install_info()`'s `exe` field), but since this fork deliberately keeps
  the *portable* exe named `rustdesk.exe` (distinct from `APP_NAME`), the copied file never
  actually got renamed to match. `rename_exe_cmd()` already existed and already handled exactly
  this (used elsewhere, in an unrelated update/refresh code path) — it just wasn't wired into
  `install_me()` itself. Fixed by adding it to `install_me()`'s generated command sequence, right
  after the copy. Also fixed a latent case-sensitivity inconsistency in `rename_exe_cmd()` itself
  (it lower-cased the rename target's filename; harmless on Windows's case-insensitive filesystem,
  but needlessly inconsistent with the properly-cased name every shortcut/registry entry expects).
  **Upgrade check**: if a future upstream release adds another install/copy code path, verify it
  also calls `rename_exe_cmd()` when the source exe's filename doesn't match `get_app_name()`.
- **Gap closed 2026-09-11 for the MSI installer specifically** — see the new "App Identity (MSI)"
  hook point below. The separately-built Windows **MSI installer** (`res/msi/`) never read this
  Rust constant; it now gets an equivalent, independently-set identity via CI build parameters.
- A future upstream change to `ui_interface.rs`'s `OPTIONS` cache/`ipc::connect()` pipe-path
  construction, or to `hbb_common::config::Config::path()`/`ipc_path()`'s use of `APP_NAME`, should
  be re-checked against this hook — the fix depends on `APP_NAME` still being the single source of
  truth for both.
- **Third bug found and fixed 2026-09-30, same class of incident, found via user report**: the
  Windows **portable exe wrapper** (`libs/portable/`, the self-extracting `rustdesk_portable.exe`
  built by `libs/portable/generate.py` — distinct from both the MSI and the plain flutter-built
  exe) was completely missed by the original App Identity fix, because it's a separate, standalone
  Rust crate (its own `Cargo.toml`) that does not link against `hbb_common` or `src/`, so it never
  even sees `APP_NAME`. Its `main.rs` hardcoded `const APP_PREFIX: &str = "rustdesk";`, used as
  `dirs::data_local_dir().join(APP_PREFIX)` — i.e. the wrapper silently extracts the real
  application (including `config.toml`, if bundled) to `%LOCALAPPDATA%\rustdesk\` and launches
  *that* copy, every single time, regardless of this fork's `APP_NAME`. This is the exact same
  class of collision the original App Identity fix exists to prevent — a real RustDesk's own
  portable exe would extract to and share that exact same directory — it just went unnoticed until
  a user reported being unable to find `config.toml` "next to" the portable exe they ran (it isn't
  there; it needs to go in the extraction directory, which this bug also meant wasn't distinct).
  Fixed by changing `APP_PREFIX` to the literal `"RustDesk-DirectIP-RemoteSupport"` (matching
  `core_main.rs`'s `APP_NAME` value; the two can't share a real source of truth since they're
  separate crates — same situation as the MSI build's own independent `MSI_APP_NAME` CI variable).
  The portable exe now extracts to `%LOCALAPPDATA%\RustDesk-DirectIP-RemoteSupport\` — `config.toml`
  needs to be placed there (or via `rustdesk --setup-local`/`--setup-remote` run from that extracted
  copy) to affect a portable-exe launch, not next to the original downloaded `.exe`.
  **Not changed** (out of scope for this fix, flagged for awareness): `libs/portable/src/main.rs`'s
  `WIN_TOPMOST_INJECTED_PROCESS_EXE`/`win::copy_runtime_broker()` (privacy-mode magnifier helper
  process name, `"RuntimeBroker_rustdesk.exe"`) is a *different* hardcoded literal that must stay
  byte-for-byte identical to the canonical copy in
  `src/privacy_mode/win_topmost_window.rs::WIN_TOPMOST_INJECTED_PROCESS_EXE` for the privacy-mode
  magnifier trick to keep working — renaming one without the other breaks that feature. Left alone
  because it wasn't part of what was reported and privacy mode can't be tested in this environment.
  **Upgrade check**: if a future upstream release changes how `libs/portable` resolves its
  extraction directory (e.g. reads an env var, a build-time metadata file), re-verify this literal
  still tracks `APP_NAME` — this crate has no compile-time link to the runtime constant, so nothing
  will fail loudly if they drift apart again.

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
- **Extended 2026-09-28 to the Linux/macOS systemd/launchd entry point**: the original 2026-09-11
  fix only covered the plain interactive GUI launch (`args.is_empty()`); it did **not** cover
  `--service`/`--server`, the entry points systemd (`res/rustdesk.service`, unconditionally
  enabled/started by `res/DEBIAN/postinst` regardless of role — see "Linux Install/Role
  Separation" below) and launchd invoke. `core_main.rs`'s `--service` and `--server` arg branches
  now both check `config::is_outgoing_only()` first and exit cleanly (logging why) instead of
  calling `start_os_service()`/`start_server(true, false)` for `role=local` — confirmed the
  systemd unit has no `Restart=` directive (defaults to `Restart=no`), so this clean exit does not
  cause a restart loop. **Upgrade check**: if a future upstream release adds a `Restart=` directive
  to `res/rustdesk.service` (or the macOS launchd plists gain retry behavior), re-verify this gate
  still results in a single clean exit, not a crash-restart loop, for `role=local`.

### Linux Install/Role Separation (implemented 2026-09-28, `docs/PLAN-install-separator.md` Phase 3)
Verify, on any upstream merge that touches `build.py`, `res/DEBIAN/*`, `res/rustdesk*.desktop`,
`res/rustdesk.service`, `res/PKGBUILD`, `res/pacman_install`, `appimage/AppImageBuilder-*.yml`, or
`flatpak/rustdesk.json`:
- **Package/app identity, not just filenames.** A prior CI fix (2026-09-27) already renames the
  *output artifact filenames* (e.g. `rustdesk-direct-ip-*.zst`/`.AppImage`) post-build. That is
  cosmetic only — it does not change what the package manager itself thinks the package is. The
  identifiers that actually matter for avoiding a collision/replace with a real RustDesk install on
  the same machine are: dpkg `Package:` field (`build.py::generate_control_file()`, now
  `rustdesk-direct-ip-remote-support`), the PKGBUILD `pkgname` (`res/PKGBUILD`, same value), and the
  flatpak `id` (`flatpak/rustdesk.json`, `com.rustdesk.DirectIPRemoteSupport`). Re-verify these
  after any upstream change to the packaging scripts — it's easy to fix the CI rename step and miss
  that the package's own internal identity is unchanged underneath.
- **Install directory renamed**: `/usr/share/rustdesk` → `/usr/share/rustdesk-direct-ip-remote-support`
  throughout `build.py` (`build_flutter_deb()`/`build_deb_from_folder()`), `res/PKGBUILD`,
  `res/pacman_install`, and the appimage recipes. The **binary command name itself is deliberately
  left as `rustdesk`** (matching the Windows precedent of keeping `rustdesk.exe` unchanged) — only
  the directory it lives in and the package-level names around it are distinct. This means the
  `/usr/bin/rustdesk` symlink target itself is still a shared global path; two packages (this fork's
  `.deb`/PKGBUILD and a real RustDesk's) each installing that same symlink is a known, accepted
  residual collision risk, deliberately not solved here (would require renaming the actual `rustdesk`
  command, out of scope per explicit decision) — dpkg/pacman will flag a file conflict if both are
  installed at once. Not an issue for flatpak/AppImage, which sandbox their own prefix.
- **systemd unit renamed**: `res/rustdesk.service`'s `Description=` and the unit's *installed*
  filename (`rustdesk-direct-ip-remote-support.service`, set in `res/DEBIAN/postinst`/`prerm` and
  `res/pacman_install`, not in the `.service` file's own name in `res/`) both changed. `ExecStart=
  /usr/bin/rustdesk --service` is unchanged (see binary-name note above).
- **`.desktop` entries renamed**: `res/rustdesk.desktop`'s `Name=`/`Icon=` and
  `res/rustdesk-link.desktop`'s `Name=`/`Icon=`/`MimeType=` all changed; their *installed* filenames
  (`rustdesk-direct-ip-remote-support[.desktop|-link.desktop]`) are set at copy time in `build.py`/
  `res/pacman_install`/the flatpak `rename-desktop-file` field, not in the `res/` source filenames
  themselves (left as `rustdesk.desktop`/`rustdesk-link.desktop` on disk to minimize diff noise).
  `Exec=`/`TryExec=rustdesk` unchanged (binary name). `StartupWMClass=rustdesk` unchanged — GTK's
  default WM_CLASS derives from the process's argv[0]/prgname, which is still `rustdesk`, not from
  this file.
- **Icon filenames renamed** in the shared `/usr/share/icons/hicolor/{size}/apps/` namespace:
  `rustdesk.png`/`rustdesk.svg` → `rustdesk-direct-ip-remote-support.{png,svg}`, everywhere they're
  copied (`build.py`, `res/PKGBUILD`, appimage recipes). `flutter/linux/my_application.cc`'s
  `gtk_icon_theme_load_icon()` call was updated to look up the new icon name — this is a real C++
  code change (not just packaging), needed because the in-app window-icon lookup is by icon-theme
  name, not path.
- **Fixed a latent, previously-undiscovered bug found during this work**: `res/rustdesk-link.desktop`'s
  `MimeType=x-scheme-handler/rustdesk;` was already stale *before* any of the above renaming — it
  never matched the runtime `get_uri_prefix()` value (`format!("{}://",
  get_app_name().to_lowercase())`), which has computed `rustdesk-directip-remotesupport://` on every
  platform (Linux/macOS included) ever since the cross-platform `APP_NAME` fix, because that fix is
  guarded only by `#[cfg(not(any(target_os = "android", target_os = "ios")))]` at the top of
  `core_main()` — not Windows-only as the packaging files assumed. Now fixed to
  `x-scheme-handler/rustdesk-directip-remotesupport;`. **Upgrade check**: if `APP_NAME` or
  `get_uri_prefix()`'s derivation ever changes, re-derive this exact string and re-check it against
  the `.desktop` file — a silent mismatch here means `rustdesk://`-style deep links resolve to
  nothing instead of erroring loudly.

### Linux Install-Time Role Gating (implemented 2026-09-29, `docs/PLAN-install-separator.md` Phase 2 follow-up)
Verify, on any upstream merge that touches `res/DEBIAN/postinst`, `res/pacman_install`,
`res/PKGBUILD`, `res/rpm-flutter{,-suse}.spec`, or `build.py`:
- **Superseded same day**: this hook point originally documented a *single-package* design (one
  `.deb`, role read from `config.toml` at install time, falling back to "not started" if no config
  existed yet — requiring a manual `rustdesk --setup-local`/`--setup-remote` CLI step on a fresh
  install before a Remote install would actually accept connections). **This was wrong** — it
  reintroduced exactly the manual-CLI-step requirement the Windows Local/Remote MSI split exists to
  avoid. Corrected later the same day to a proper **two-package-variant split**, matching Windows
  exactly: see "Linux Two-Variant Package Split" below. This entry is kept for history; do not
  implement anything described in it.

### Linux Two-Variant Package Split (implemented 2026-09-29, corrects the single-package design above)
Verify, on any upstream merge that touches `build.py`, `res/DEBIAN/postinst`, `res/pacman_install`,
`res/PKGBUILD`, `res/rpm-flutter.spec`, `res/rpm-flutter-suse.spec`, or the Linux jobs in
`.github/workflows/flutter-build.yml` (`build rustdesk linux`, `Build appimage`, `Build flatpak`):
- **Every Linux package format now builds two variants** — Local and Remote — each with its role
  pre-baked into `config.toml` at build time, exactly mirroring the Windows MSI split
  (`res/msi/preprocess.py`'s `--conn-type`/pre-baked `configs/{local,remote}.toml` approach).
  **Neither variant ever requires a manual `rustdesk --setup-local`/`--setup-remote` step** — that
  was the whole point of the Windows split in the first place, and the single-package design above
  had silently dropped that guarantee for Linux.
  - **`.deb`** (`build.py`): `build_flutter_deb()` now calls a new `package_deb_variant(version,
    variant)` twice (`"local"`, `"remote"`), each copying `configs/{variant}.toml` directly as
    `config.toml` into the package (not as a same-named sample file), producing
    `rustdesk-{version}-local.deb` / `rustdesk-{version}-remote.deb`. `res/DEBIAN/postinst`'s
    existing role-check logic (reading `config.toml`'s `role =` line) needed no logic changes —
    it now simply always finds a config to read, so it auto-`enable`s/`start`s the service for the
    Remote variant, and never for Local, with zero manual steps in either case. `.github/workflows/
    flutter-build.yml`'s `build rustdesk linux` job's "Upload deb" step's `path` was widened to a
    glob matching both variants.
  - **Archlinux (`res/PKGBUILD`/`res/pacman_install`)**: `PKGBUILD`'s `package()` now reads a `ROLE`
    environment variable (`local`/`remote`) and bakes the matching `configs/$ROLE.toml` as
    `config.toml`; falls back to bundling both as loose samples if `ROLE` is unset (e.g. a manual
    `makepkg -f` outside CI). `pacman_install`'s `_enable_and_start_if_remote()` needed no logic
    change, same reasoning as `postinst`. The CI job now runs `makepkg -f` twice with
    `ROLE=local`/`ROLE=remote`, renaming the (identically-named, since `pkgname`/`pkgver` don't vary
    by role) output between passes before the next pass overwrites it.
  - **AppImage** (`appimage/AppImageBuilder-{x86_64,aarch64}.yml`, unchanged): each variant AppImage
    is now built by extracting the matching variant `.deb` (which already has `config.toml` baked
    in) — the recipe's own `mv ./usr ./AppDir/usr` step carries that file through automatically, no
    recipe changes needed. The CI job now loops both variants, extracting each `.deb` into a fresh
    `AppDir` and renaming the output AppImage before the next pass overwrites it.
  - **Flatpak** (`flatpak/rustdesk.json`, unchanged): same principle — the `run-on-arch-action`
    script now branches on which staged `.deb` file(s) are present (a single `rustdesk.deb` for the
    unrelated legacy sciter build, vs. `rustdesk-local.deb`/`rustdesk-remote.deb` for the two real
    variants) and loops `flatpak build-bundle` per variant.
  - **RPM (`res/rpm-flutter.spec`, `res/rpm-flutter-suse.spec`)** — **a separate, more serious bug
    was found and fixed here**: these two `.spec` files are genuinely invoked by CI
    (`flutter-build.yml`'s `build rustdesk linux` job runs `rpmbuild` on both, unlike the truly-dead
    `res/rpm.spec`/`res/rpm-suse.spec`, which only the legacy Sciter path in `build.py` touches) —
    but they were **completely missed by the original Phase 3 identity-separation pass**. Before
    this fix, every Fedora/openSUSE `.rpm` build still used the literal upstream `Name: rustdesk`,
    installed to `/usr/share/rustdesk`, and used the bare `rustdesk.service`/`rustdesk.desktop`
    identifiers — meaning **none of Phase 3's collision-avoidance ever applied to RPM installs at
    all**, contradicting what Phase 3's own changelog claimed. Fixed now, alongside adding the same
    `ROLE`-driven two-variant config-bake and role-conditional `%post` as the other formats. `%pre`/
    `%post`/`%preun`/`%postun` scriptlets updated to the renamed service/desktop/icon names
    throughout. **Not added**: a PAM config file for RPM — upstream never shipped one for this
    format (the `Requires: pam` dependency exists but nothing installs a corresponding
    `/etc/pam.d/*` file), so `pam_get_service_name()` still falls back to `gdm` on RPM installs; this
    is a pre-existing upstream gap, not something introduced or worsened here, and adding a new file
    format's worth of PAM config was judged out of scope for this pass.
  - **Upgrade check for all of the above**: if a future upstream release changes `fork_config.rs`'s
    `[options]` table format (e.g. quoting style, key renamed away from `role`), the `grep`/`sed`
    line in `postinst`/`pacman_install`/both `.spec` files that extracts the role value needs to
    change to match — it's deliberately a plain-text regex, not a real TOML parser (none available
    in a bare shell scriptlet), so a format change would silently break the extraction rather than
    erroring loudly. This only matters for the "config missing" fallback path now, though, since the
    config is pre-baked and always present in the normal case.
- **PAM service name: IMPLEMENTED 2026-09-29 — and turned out to be a real, previously-undiscovered
  functional bug, not just a collision-avoidance nice-to-have.** `src/platform/linux_desktop_manager.rs`'s
  `pam_get_service_name()` already dynamically computes the *expected* PAM service filename as
  `/etc/pam.d/{get_app_name().to_lowercase()}` and **silently falls back to the `"gdm"` PAM stack**
  if that file doesn't exist. Since `APP_NAME` has been `"RustDesk-DirectIP-RemoteSupport"` since the
  original cross-platform identity fix (well before any of this Linux packaging work), and the `.deb`
  was still installing the PAM config at the literal old path `/etc/pam.d/rustdesk`, this check has
  been failing and silently using `gdm`'s PAM stack instead of a fork-specific one on every install,
  the whole time — a real bug, not a hypothetical. Fixed by renaming the installed file (in
  `build.py::build_flutter_deb()`) to `/etc/pam.d/rustdesk-direct-ip-remote-support`, matching what
  `pam_get_service_name()` already looked for. **Upgrade check**: if `pam_get_service_name()`'s
  derivation ever changes (e.g. a different fallback name, or a different case transform), re-verify
  the installed PAM file's name still matches exactly, or this silent-fallback bug returns.
  Archlinux (`res/PKGBUILD`) does not install a PAM config file at all — confirmed pre-existing (not
  a regression from this fix), so `pam_get_service_name()` always falls back to `gdm` there;
  flagged, not fixed, since it would mean introducing a new file/behavior this plan didn't
  previously ship on that platform.
- **Polkit action ID: investigated, found not to be a real gap.** No `.policy` action-definition
  file, D-Bus polkit authority check, or `pkexec`/`polkit` API call exists anywhere in this
  codebase — `build.py` only `mkdir -p`s an (empty) `usr/share/polkit-1/actions/` directory and
  drops an unrelated placeholder script at `.../files/polkit` (a no-op shell script containing only
  a shebang, not installed into that actions directory). There is no actual polkit action ID to
  rename. The item in the original audit was speculative/generic, not based on something present in
  this repo — corrected here rather than fixing something that doesn't exist.
- **`/etc/rustdesk/` config directory: IMPLEMENTED 2026-09-29** — renamed to
  `/etc/rustdesk-direct-ip-remote-support/` in `build.py::build_flutter_deb()` (the only place that
  installs into it; `res/PKGBUILD` doesn't install these files at all). Confirmed nothing in `src/`
  or `libs/hbb_common` reads this path by name — it only holds `startwm.sh`/`xorg.conf`, reference
  files for an xrdp-style X session setup that an admin would point a separate, externally-managed
  X session manager config at manually; the rename is packaging-hygiene/collision-avoidance only,
  not a functional fix like the PAM one above.

### macOS App Identity (implemented 2026-09-29, `docs/PLAN-install-separator.md` Phase 4)
Verify, on any upstream merge that touches `flutter/macos/Runner/Configs/AppInfo.xcconfig`,
`flutter/macos/Runner.xcodeproj/project.pbxproj`, `flutter/macos/Runner/Info.plist`, or
`src/platform/macos.rs`:
- **Key finding that narrowed this phase's actual scope**: `docs/PLAN-install-separator.md`'s
  original Phase 4 write-up assumed `src/platform/privileges_scripts/daemon.plist`/`agent.plist`/
  `install.scpt`/`uninstall.scpt`/`update.scpt` would each need their own explicit identity-string
  edits. Investigation found this is **already handled generically**: `macos.rs::correct_app_name()`
  (called on every one of those template files before use — see its call sites around
  `install_service()`/`update_service()`/`uninstall_service()`) does three blind string
  replacements — `"com.carriez.rustdesk"` → the app's *actual running* `CFBundleIdentifier` (read
  via `NSBundle.mainBundle.bundleIdentifier` in `get_bundle_id()`), then `"rustdesk"` →
  `get_app_name().to_lowercase()`, then `"RustDesk"` → `get_app_name()`. Since `APP_NAME` is already
  set fork-wide in `core_main()` (see "App Identity" above), these templates were **already**
  producing a distinct launchd `Label`/`AssociatedBundleIdentifiers`/`/Applications/<app>.app/...`
  path/preferences-file path before this phase touched anything — confirmed by manually tracing
  `correct_app_name()`'s three replacements against each template file's literal content. **None of
  these five files needed editing.** This mirrors the pattern already seen in "Linux Install/Role
  Separation" (Phase 3) and is exactly why `docs/PLAN-install-separator.md` was revised again here.
- **What actually needed changing** — the *static*, build-time-baked identity that
  `correct_app_name()` cannot reach because it isn't inside a bundled template file:
  - `PRODUCT_BUNDLE_IDENTIFIER` (`com.carriez.rustdesk` → `com.rustdesk.DirectIPRemoteSupport`) in
    `project.pbxproj`'s three Runner-target build configs (Debug/Release/Profile), and the matching
    (previously already-inconsistent, previously-inert-but-now-fixed-for-clarity) line in
    `AppInfo.xcconfig`. This is the value `get_bundle_id()` above actually reads at runtime, and
    what macOS Launch Services/the sandboxed `~/Library/Containers/<bundle-id>/` path key off.
  - `PRODUCT_NAME` (`RustDesk` → `RustDesk-DirectIP-RemoteSupport`) in `AppInfo.xcconfig` — changes
    the built app bundle's filename to `RustDesk-DirectIP-RemoteSupport.app`. **Upgrade check**:
    every hardcoded `.../Release/RustDesk.app` path in `build.py::build_flutter_dmg()`,
    `.github/workflows/flutter-build.yml`'s macOS job, `.github/workflows/playground.yml`, and
    `res/osx-dist.sh` had to be updated to match this renamed bundle filename in the same commit —
    a future upstream change to `PRODUCT_NAME` needs the same sweep across all four.
  - `CFBundleURLSchemes`/`CFBundleURLName` in `Info.plist` — **fixed the same class of stale-URI-
    scheme bug already found and fixed on Linux** (`res/rustdesk-link.desktop`'s `MimeType`): this
    was still the literal `rustdesk`/`com.carriez.rustdesk` even though the Rust-side
    `get_uri_prefix()` has computed `rustdesk-directip-remotesupport://` since the original
    cross-platform `APP_NAME` fix (macOS is not excluded by that fix's `#[cfg(...)]` guard — see
    "App Identity" above). Fixed to `rustdesk-directip-remotesupport` and `$(PRODUCT_BUNDLE_IDENTIFIER)`
    respectively.
  - The 3 `RustDesk.app` `PBXFileReference` path literals in `project.pbxproj` (Xcode project
    navigator bookkeeping, not itself load-bearing for the build, but updated to avoid a confusing
    mismatch with the real build output name).
- **Confirmed NOT a concern, contrary to the original plan's speculation**: `DebugProfile.entitlements`/
  `Release.entitlements` contain no bundle-id- or keychain-access-group-scoped entries (app sandbox
  is disabled entirely for this target), so the bundle-ID rename has no entitlements/code-signing
  side effects beyond needing a distinct signing identity (unrelated to this fork's changes).
- **`build.py`'s legacy Sciter-based macOS build path** (`main()`'s non-`--flutter`, `osx`-guarded
  branch, hardcoding `target/release/bundle/osx/RustDesk.app`) **left untouched** — confirmed dead,
  same as `res/rpm.spec` and the Linux Sciter `.deb` path (Phase 3): CI's macOS job always passes
  `--flutter`.
- **Risk carried over from the original plan, still true**: no macOS build/signing/notarization
  environment available to verify any of this directly — build success in CI (which now runs the
  actual `.pbxproj`/`Info.plist` through `xcodebuild`) is the only automatic check here; a real Mac
  is needed to confirm Launch Services / `rustdesk-directip-remotesupport://` deep links / daemon
  install actually behave correctly end to end.

### Android App Identity (implemented 2026-09-29, `docs/PLAN-install-separator.md` Phase 5)
Verify, on any upstream merge that touches `flutter/android/app/build.gradle`:
- **`applicationId` changed** from `com.carriez.flutter_hbb` to `com.rustdesk.directipremotesupport`
  — this is the only change made. Android has always supported `applicationId` differing from the
  Java/Kotlin package namespace (used purely for class references, `R` class generation, and JNI
  symbol names), so this needed no source-file renaming, no `AndroidManifest.xml` `package=`
  attribute change, and no changes to any of the Kotlin files under
  `flutter/android/app/src/main/kotlin/com/carriez/flutter_hbb/` (left as-is, deliberately, exactly
  like the Windows/Linux/macOS "keep the underlying binary/executable name unchanged" precedent).
- **Confirmed narrower risk profile than Windows/Linux/macOS, and explained why**: Android already
  sandboxes every app's data directory, IPC, and permissions per-`applicationId` at the OS level —
  the entire class of bug this whole effort exists to prevent (a shared named pipe / shared install
  directory / shared systemd unit silently serving a different app's already-running instance)
  **cannot happen on Android regardless of this fork's own branding choices**, because the OS itself
  already isolates apps by package name. The only real risk `applicationId` collision creates is an
  **install-time conflict** (Android's package manager refuses to install a second APK claiming an
  already-installed `applicationId` signed with a different key) if this fork's `applicationId`
  happens to match the real RustDesk Android app's own (plausible, since it was never changed from
  upstream's default) — fixed by this rename.
- **Deliberately NOT changed, and why**: `AndroidManifest.xml`'s `<data android:scheme="rustdesk" />`
  deep-link intent-filter. Unlike every other platform, **Android's mobile UI never runs
  `core_main()`** (`src/core_main.rs::core_main()` is `#[cfg(not(any(target_os = "android", target_os
  = "ios")))]` — it doesn't exist on these targets at all), so `APP_NAME`/`get_uri_prefix()` are
  never customized on Android/iOS in the first place; the runtime still expects (and, per
  `flutter/lib/common.dart`'s deep-link handler, does not even inspect `uri.scheme` when routing —
  only `uri.authority`/`uri.path`) exactly the literal `rustdesk://` scheme upstream ships. Renaming
  the manifest's registered scheme without a corresponding Rust/Dart-side change would have made the
  app stop responding to its own still-unchanged expected deep-link scheme — a regression, not a
  fix. **Upgrade check**: if `core_main()`'s target exclusion list ever changes to include Android,
  or if `APP_NAME`/`get_uri_prefix()` customization is ever extended to mobile, re-visit this
  decision and update the manifest scheme to match at that point.
- **Confirmed out of scope, not touched**: no `google-services.json`/Firebase configuration exists
  anywhere under `flutter/android/`, so the Firebase/push-notification dependency the original plan
  worried about does not apply. (iOS *did* have a real `GoogleService-Info.plist` — see "iOS App
  Identity" below, a follow-on phase added 2026-09-29 after this Android phase landed.)

### iOS App Identity (implemented 2026-09-29, follow-on to `docs/PLAN-install-separator.md` Phase 5 —
never one of the plan's original five phases; added after being flagged as a discovered gap)
Verify, on any upstream merge that touches `flutter/ios/Runner.xcodeproj/project.pbxproj`,
`flutter/ios/Runner/Info.plist`, or `flutter/ios/exportOptions.plist`:
- **Changed**: `PRODUCT_BUNDLE_IDENTIFIER` (3 occurrences in `project.pbxproj`) `com.carriez.flutterHbb`
  → `com.rustdesk.DirectIPRemoteSupport`; `Info.plist`'s `CFBundleDisplayName`/`CFBundleName`
  (`RustDesk` → `RustDesk-DirectIP-RemoteSupport`) and `CFBundleURLName` (→
  `$(PRODUCT_BUNDLE_IDENTIFIER)`, matching the macOS fix); `exportOptions.plist`'s
  `provisioningProfiles` dictionary key (unused by CI — this file isn't referenced by any workflow —
  but kept in sync for anyone doing a manual signed App Store export later).
- **This carried a real risk the Android phase didn't**: iOS actually has `aps-environment` (push
  notifications) in `Runner.entitlements` and a real `GoogleService-Info.plist` referencing
  upstream's own Firebase project (`PROJECT_ID: rustdesk`, `BUNDLE_ID: com.carriez.flutterHbb`) — a
  bundle-ID rename could plausibly break push notifications or Firebase initialization if either
  were actually wired up. **Verified before proceeding, not assumed**: `flutter/pubspec.yaml`'s only
  Firebase dependency (`firebase_analytics`) is commented out; no `firebase_core`/Firebase
  initialization call exists anywhere in `flutter/lib/` or `flutter/ios/Runner/AppDelegate.swift`;
  and `GoogleService-Info.plist` was not referenced anywhere in `project.pbxproj` (not a build
  resource, not bundled into the IPA) — fully orphaned. No code path anywhere registers for push
  notifications either. This confirmed the rename was safe to make.
- **Deleted `flutter/ios/Runner/GoogleService-Info.plist`** rather than trying to keep it in sync —
  it was already dead (see above), and leaving it in place would mean it permanently references a
  stale bundle ID that no longer matches anything, for no functional benefit. **Upgrade check**: if
  a future upstream release actually wires up Firebase (adds `firebase_core`, calls
  `Firebase.initializeApp()`/`FirebaseApp.configure()`, or starts registering for remote
  notifications), this whole assessment needs redoing — at that point a fork-owned Firebase project
  would be needed before changing `PRODUCT_BUNDLE_IDENTIFIER` further, since Firebase apps are
  registered per bundle ID against a specific project this fork doesn't control.
- **Deliberately left unchanged, same reasoning as Android**: `CFBundleURLSchemes` (`rustdesk`) —
  iOS is excluded from `core_main()` by the same `#[cfg(...)]` as Android, so `get_uri_prefix()` is
  never customized here either; the app still expects literal `rustdesk://`.
- **`PRODUCT_NAME` (`"$(TARGET_NAME)"`, resolving to the Xcode target's own name "Runner") was left
  unchanged** — unlike macOS, iOS has no `AppInfo.xcconfig`-style override, and no CI step or script
  hardcodes an expected `RustDesk.app`/`.ipa` product name for iOS (the `flutter build ipa` /
  publish steps in `flutter-build.yml` are either unaffected or already commented out), so there was
  nothing to keep in sync by renaming it.

### File Copy/Paste Default (implemented 2026-09-12)
Verify:
- `fork_config.rs::apply()` still unconditionally sets
  `UserDefaultConfig::load().set(OPTION_ENABLE_FILE_COPY_PASTE, "N")` on every startup, forcing
  the Display tab's "Enable file copy and paste" default to off.
- **This is a `UserDefaultConfig` value, not a plain `Config` option** — a completely separate
  storage mechanism (`hbb_common::config::UserDefaultConfig`, its own `_default` file) from
  everything else this module writes via `Config::set_option`/`mirror_upstream_options`. Putting
  `enable-file-copy-paste = "N"` in `config.toml`'s `[options]` table would have **no effect** —
  `mirror_upstream_options()` only ever calls `Config::set_option`, which this key is never read
  from (upstream's own hardcoded default for it, `"Y"`, lives in
  `UserDefaultConfig::get()`'s match arm, `libs/hbb_common/src/config.rs:2380`). A future upstream
  change to which options are `UserDefaultConfig`-backed vs. plain `Config` options should be
  re-checked against this — using the wrong write path is a silent no-op, not an error.
- **Design choice, not a bug**: this re-applies unconditionally on every launch, same as
  `enable-lan-discovery = "N"` elsewhere in this function — if a user manually re-enables the
  checkbox in Settings, it reverts to off on the next restart. This was an explicit choice made
  to match this fork's existing pattern of permanent, non-persisted product defaults; revisit if
  a genuinely user-adjustable-and-sticky default is wanted instead.

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
- Launching normally (no `--advance-setup`): Safety, Display, and Network tabs are all hidden,
  regardless of role or `fork_config.rs`'s row-level settings.
- Launching with `--advance-setup`: Safety shows for `role=remote`, Display shows for
  `role=local`, Network shows (with its usual two rows hidden) — same as pre-this-change
  behavior, but only with the flag present. Relaunching
  without the flag hides them again immediately (no persistence).
- Launching with `--printer-setup` (independent of `--advance-setup`): Printer tab shows on
  Windows, overriding `fork_config.rs`'s unconditional hide. Without it, hidden as before.

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

## CI Job Hygiene (added 2026-09-12)

The `i686-pc-windows-msvc` job under `build-for-windows-sciter` (`.github/workflows/
flutter-build.yml`) is disabled (`if: false`), not just flaky. It pinned a Rust nightly from
2023-10-13 to build a 32-bit Sciter (pre-Flutter UI) fallback binary this fork doesn't ship or
use — confirmed the toolchain itself is still reachable on rust-lang's dist servers, so the
recurring failure wasn't "the toolchain vanished," but upstream had already flagged this exact job
for disabling in a comment ("Temporarily disable this action due to additional test is needed")
and left the `if: false` commented out, so it silently kept running and failing on every CI run
going back before this fork's own changes began. **Upgrade check**: if a future upstream release
re-enables or restructures this job, re-evaluate whether it's still needed (unlikely, given this
fork ships no Sciter/32-bit artifacts) before assuming a CI failure there is worth chasing.

**Follow-on regression, fixed same day**: disabling `build-for-windows-sciter` initially broke
`publish_unsigned` (the job that bundles macOS + Windows x86_64 outputs into one combined
`*-unsigned.tar.gz` release asset) — its `needs:` list included `build-for-windows-sciter`, and a
disabled job (`if: false`) never reports `success`, so `publish_unsigned` silently skipped on
*every* run via the `needs` chain, permanently, not just when Sciter happened to fail. Fixed by
removing `build-for-windows-sciter` from `publish_unsigned`'s `needs:` and removing its
now-impossible `windows-x86` artifact download/combine step. **Upgrade check**: whenever disabling
or removing any job, grep for it in every other job's `needs:` list first — a `needs`-chain skip is
silent (no error, just an absent release asset) and easy to miss.

**Unrelated third bug, fixed 2026-09-16**: `build rustdesk linux x86_64-unknown-linux-gnu`'s
"Rename archlinux release files (direct-ip)" step was failing (exit 1) on every nightly build,
right after the preceding "Build archlinux package" step succeeded. Root cause: that preceding
step runs via `rustdesk-org/arch-makepkg-action`, a Docker-based composite action (the only
containerized *step* in this job — everything else, including the `.deb`/`.rpm` packaging earlier
in the same job, runs natively on the runner) — files it writes into the bind-mounted `res/`
directory come out owned by root on the host, and the following plain-bash step (running as the
normal, non-root runner user) then failed outright trying to `cp` them. Confirmed by reproducing
the rename script's logic locally against representative inputs — it succeeds fine on its own, so
the failure had to be an ownership/permissions issue specific to the Docker-container step, not a
script bug. Fixed by adding `sudo chown -R "$(id -u):$(id -g)" res/` at the start of the rename
step, plus switching the glob from a bare `for f in ...*.zst` (which silently did nothing under
`nullglob` if no file matched, with no log trace either way) to an explicit array with an
`::warning::` if nothing matched and an echo per rename, so a future failure here is diagnosable
from the log alone rather than requiring log-download access to investigate. **Upgrade check**: if
a future upstream release changes `arch-makepkg-action` or moves the archlinux build to run
natively instead of in a container, this `chown` becomes unnecessary (harmless either way, but
worth removing for clarity) — re-verify which steps in this job are containerized before assuming
this fix still applies.

**Fourth bug, same class, fixed 2026-09-22**: after the archlinux fix above actually shipped and
was verified (that job went fully green), the nightly build kept failing anyway — this time in
both `Build appimage x86_64-unknown-linux-gnu` and `Build appimage aarch64-unknown-linux-gnu`, at
their own "Rename release files (direct-ip)" step. Identical root cause, different trigger: the
preceding "Build appimage package" step runs `appimage-builder` via `sudo` directly (not a Docker
container this time, but same effect) — files it writes into `./appimage/` come out root-owned,
and the following non-sudo rename step failed the same way. **This means the "one Docker step"
framing above was too narrow** — the actual rule is "any step that produces output via `sudo` or a
container, anywhere upstream of a plain-user file operation, is suspect," not just the specific
archlinux case. Fixed with the identical pattern: `sudo chown -R "$(id -u):$(id -g)" ./appimage/`
before the rename, plus the same array/warning/echo diagnostics. **Upgrade check**: before trusting
that "the nightly build is fixed," grep every job in this file for `sudo ` followed later by a
`cp`/`mv`/rename step in the same job without an intervening ownership fix — this pattern has now
recurred twice and may still exist elsewhere undetected until it's actually exercised by a run.

## Release Acceptance
Upgrade is accepted only if all checks pass:
1. **Build Readiness:** `docs/BUILD_BLOCKER_ANALYSIS.md` shows no unresolved blockers; `cargo build --release` succeeds.
2. **Packaging Readiness:** `docs/PACKAGING_PLAN.md` build steps complete; all platform-specific artifacts generated.
3. **Functional Readiness:** `docs/RELEASE_CHECKLIST.md` items all pass.
4. **Documentation Current:** `docs/FEATURE_ENFORCEMENT_MATRIX.md` has been re-verified against the new upstream version (not just left as-is from the prior baseline).
