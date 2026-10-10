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

### First-Run Setup Skipped the Server Thread Spawn (fixed 2026-10-03, found via real testing)
Verify, on any upstream merge that touches `core_main.rs`'s first-run `config_exists()` check or
`handle_first_run_setup()`:
- **Problem found**: `core_main()`'s first-run branch did `return handle_first_run_setup();` —
  that function only ever returns `Some(vec![])` (setup succeeded, `config.toml` now exists) or
  `None` (cancelled/failed), but returning its `Some(vec![])` result *directly* short-circuited
  the rest of `core_main()`, skipping the background "server" thread spawn further down
  (`std::thread::spawn(move || crate::start_server(false, no_server))`, gated on
  `!config::is_outgoing_only()` — see "No Server/IPC for Local Mode" above) that the GUI's
  "Ready"/"Not ready" status and one-time-password generation actually depend on. Confirmed via a
  real test: the app got stuck forever on "Not ready. Please check your connection" with the
  one-time password stuck on "Generating..." specifically on the very first launch (before
  `config.toml` exists) — a second launch (config now present, this whole branch skipped,
  reaching the normal spawn) worked correctly every time, including without admin elevation
  (elevation was a red herring from an earlier, incorrect diagnosis of this same symptom — it
  isn't a UAC/elevation issue at all).
- **Fixed**: the first-run branch now only returns early on actual failure/cancellation
  (`handle_first_run_setup()` returning `None`); on success it falls through to the rest of
  `core_main()`'s normal startup (which calls `fork_config::load_and_apply()` again harmlessly —
  idempotent, just re-reads the config file `handle_first_run_setup()` just wrote — then proceeds
  through the same arg-parsing/server-spawn path a second launch would take).
- **Upgrade check**: if `handle_first_run_setup()`'s return contract ever changes (e.g. starts
  returning a non-empty `Vec<String>` for some new reason), re-verify this fall-through logic
  still only treats `None` as a hard stop.
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

### Fork Peer Marker (implemented 2026-09-30, see `docs/DECISIONS.md` for the full rationale)
Verify, on any upstream merge that touches `libs/hbb_common::get_version_number()`, `src/client.rs`'s
`LoginRequest` construction, or `src/server/connection.rs::on_message()`:
- `hbb_common::get_version_number()` still reads only the first two `-`-separated segments of its
  input and ignores everything after the second `-` — this is the exact property that makes it
  safe to embed `crate::fork_config::FORK_MARKER` as a third segment of `LoginRequest.version` without affecting
  any of the many numeric version-gate checks throughout `src/server/connection.rs`/`src/client.rs`
  that call this function. If a future upstream release starts reading a third segment for some
  new purpose, this marker would either collide with that or stop being silently ignored —
  re-verify before assuming it's still inert.
- `src/client.rs`'s single `LoginRequest { ... }` construction site still sets `version:
  format!("{}-0-{}", crate::VERSION, crate::fork_config::FORK_MARKER)` rather than plain `crate::VERSION`.
- `src/server/connection.rs::on_message()` still checks `crate::fork_config::is_fork_peer_version(&lr.version)`
  as the first thing done with a freshly received, not-yet-authorized `LoginRequest`, rejecting
  immediately (clear login error, no password/approval processing reached) if it doesn't match.
- **Policy change (2026-10-02, superseding the note below as originally written)**: this is no
  longer one-directional. Dialing out to a non-fork RustDesk instance from `role=local` is now
  **also** rejected, client-side, with a clear message — see "Symmetric Fork Peer Check" below.
  The scenario the line below used to call "deliberately-supported" is no longer supported by
  design; if a future need re-opens connecting to genuine stock RustDesk remotes, that's a
  deliberate reversal of this entry, not a bug.

### Symmetric Fork Peer Check (implemented 2026-10-02, found via real testing)
Verify, on any upstream merge that touches `src/server/connection.rs`'s `PeerInfo` construction or
`src/client/io_loop.rs`'s `login_response::Union::PeerInfo` handling:
- **Problem found**: the original Fork Peer Marker (above) only protects our own remote from
  accepting outside callers - it does nothing to stop *our* local from completing a connection to
  someone else's stock RustDesk remote, since stock RustDesk has no concept of our marker and
  just accepts our (marked) `LoginRequest` normally. Confirmed via a real test: our fork's local
  build successfully connected to a plain stock RustDesk remote.
- **Fixed** by embedding `FORK_MARKER` the other direction too: `src/server/connection.rs`'s
  `PeerInfo { version: ..., ... }` construction (sent back to the connecting side on successful
  login) now sets `version: format!("{}-0-{}", VERSION, crate::fork_config::FORK_MARKER)` instead
  of plain `VERSION` - the exact same safe, schema-free reuse of a free-form `version` string
  field as the original marker (see `get_version_number()`'s two-segments-only parsing, above).
  `src/client/io_loop.rs` adds `check_fork_peer_support(peer_version)` (mirroring the existing
  `check_view_camera_support`/`check_terminal_support` pattern exactly), called first thing in
  the `Some(login_response::Union::PeerInfo(pi))` arm of `handle_msg_from_peer`: if the peer's
  `version` doesn't carry our marker, shows an error msgbox ("This remote is not a Direct-IP
  RemoteSupport peer. Refusing to connect.") and aborts the connection (`return false`) before any
  session/video/control setup proceeds - a stock RustDesk remote's version string never contains
  our marker, so this never false-positives against a real stock peer.
- **Deliberately not done**: `src/port_forward.rs` has its own separate PeerInfo-handling loop
  (not `io_loop.rs`'s `handle_msg_from_peer`) and does not get this check - port forwarding isn't
  exposed by this fork's minimal UI (no peer list, Desktop/Support buttons only), so this is a
  low-priority gap, not an oversight to silently ignore if that ever changes.
- **Known limitation, deliberately not addressed yet (2026-10-03)**: this check only runs once
  login has fully completed (`PeerInfo` is the last message of a successful login) - meaning a
  non-fork remote's operator can briefly see a normal accept prompt (in click-to-accept mode)
  before we disconnect right after. A genuinely earlier rejection point exists and was discussed
  but deliberately deferred: `connection.rs::on_open()` sends a `Hash { salt, challenge }`
  message (`connection.rs:1423`) immediately on TCP connect, *before* any `LoginRequest` is even
  processed - if `FORK_MARKER` were appended to `challenge` (the secondary replay-protection
  hash input, not the primary password-derivation `salt` - lower risk of the two, though both are
  used as opaque bytes for hashing on both ends so either would likely work safely) and checked
  on receipt, a non-fork remote could be rejected before the remote ever shows an accept prompt
  at all. Not implemented because it touches actual authentication-hash input rather than a
  purely informational field like `version`, and the current login-complete-time check was judged
  sufficient for now. If this becomes worth doing later: append the marker to `Hash.challenge` in
  `on_open()`, check for it in the client's `Hash` message handler (io_loop.rs), and disconnect
  immediately before ever sending a `LoginRequest` if absent.
- **Upgrade check**: if upstream adds another path that processes `login_response::Union::PeerInfo`
  outside `io_loop.rs::handle_msg_from_peer` (besides the already-known `port_forward.rs` gap),
  route it through `check_fork_peer_support`-equivalent logic too.

### Support/Desktop Button Independence (fixed 2026-09-30, found via real two-machine testing)
`flutter/lib/desktop/pages/connection_page.dart`'s `onSupport()` used to open a VIEW_CAMERA session
*and*, if `desktop-share-enabled` was also true, a second plain DEFAULT_CONN session at the same
time — two independent sessions dialing out together, each producing its own accept/approval
prompt on the remote side, found to cause real synchronization/double-prompt issues once someone
actually tested with both buttons enabled. Fixed: `onSupport()` now opens only the VIEW_CAMERA
session; the Desktop button (unchanged) opens only a plain DEFAULT_CONN session. The two are fully
independent — neither triggers the other. **Upgrade check**: if a future upstream release changes
how Support-style camera sessions are initiated, make sure this fork's `onSupport()` doesn't
regain an implicit second connect call.

**Transfer file button (added 2026-10-09)**: upstream's connect panel offers "Transfer file"
(plus "View camera"/"Terminal") in a dropdown next to its Connect button; this fork's panel
replaced that dropdown with the Support/Desktop buttons and the option was lost. A third,
always-shown `OutlinedButton` ("Transfer file") now calls `onConnect(isFileTransfer: true)`,
i.e. upstream's own `connect()` → `rustDeskWinManager.newFileTransfer()` path, unchanged. Gated
like the other two buttons by a fork config key, `file-transfer-enabled` (optional, absent =
`"Y"`; `fork_config.rs` maps it onto upstream's own `enable-file-transfer` permission exactly as
`support-enabled` → `enable-camera`, overwriting any plain `enable-file-transfer` in the file —
`docs/CONFIG_REFERENCE.md` §4.14). That permission is what the remote's login handler already
checks ("No permission of file transfer"), so `"N"` on the remote rejects file-transfer
sessions outright; on the local it hides the button (`_fileTransferEnabled` reads
`enable-file-transfer`). The button row became a `Wrap` so a third button can't overflow the
320 px panel. Terminal/port-forward are still not offered — not requested.

**One click = one new session window (changed 2026-10-09)**: upstream's connect panel never
opens a second Desktop/camera/file session to the *same* peer id — a second click resolves to
the existing tab or window (`DesktopTabController.add` with an existing key, or the
`kWindowEventActiveSession` probe in `RustDeskMultiWindowManager.newSession`). Product
decision for this fork: every click on Support, Desktop or Transfer file starts a brand-new
session in its own window, also for a host that is already open (e.g. one Desktop window per
monitor). Implemented as a `forceNewWindow` flag threaded `connection_page.onConnect()` →
`connect()` → `connectMainDesktop()` (and the `kWindowConnect` relay in
`desktop_home_page.dart`) → `newRemoteDesktop()/newViewCamera()/newFileTransfer()` →
`newSession()`. **Corrected 2026-10-10 (found via real testing: the first version opened a bare
new window, which showed nothing and produced no accept prompt on the remote).** Upstream's
Rust session table (`sessions::insert_session` in `src/flutter.rs`) keys core connections by
`(peer id, conn type)` and `session_start_` only starts a connection for a peer's *first* UI
session — a second window for a host that is already connected can never be a second
connection; the Rust side silently attaches it to the existing one. Upstream's own "open
monitor in new window" (`openMonitorSession`) handles that by passing `tab_window_id` +
`display`/`displays`, so the new window calls `session_add_existed` and shows that display on
the existing connection. `newSession(forceNewWindow: true)` now does the same: if
`peerGetSessionsCount` says the host is connected (Desktop or camera), it opens a new window
attached to the existing connection showing display 0 (the toolbar's monitor menu then picks
any other monitor — one connection, one row on the remote's CM, "Monitor 1, Monitor 2" once
both are captured); if the host is not connected, `_newSession(openInTabs: false)` starts a
genuinely new connection in a new window; Transfer file for a connected host falls through to
upstream's activate-existing-window behaviour (no multi-window file transfer upstream).
Consequences: no second accept prompt for a host already connected (same connection); the
Support auto-dial does not fire for the attached window (it only fires on a live PeerInfo,
and the call already runs on the first window); the "Open new connections in tabs" setting
does not apply to these buttons. **Upgrade check**: if upstream changes `session_add_existed`,
`insert_peer_session_id` or the `display`/`displays`/`tab_window_id` window parameters, re-test
two Desktop windows to the same host.

### Support Voice Call Dials on Session Acceptance, Not on the First Video Frame; Camera/Desktop Window Usable Without Video (changed 2026-10-08)
Verify, on any upstream merge that touches `FfiModel.handlePeerInfo()` (`flutter/lib/models/model.dart`)
or `desktop/pages/view_camera_page.dart`'s `initState()`:
- **Why**: Support mode's automatic voice call used to be sent from
  `ImageModel.addCallbackOnFirstImage`, i.e. only once the camera had delivered a frame. A voice
  call needs only the accepted session (it is a message on the same connection); it never
  depended on video. Hanging it off the first frame meant **no call at all** whenever video
  never arrives — a failing/refused camera (see "Camera Capture Must Not Kill the Remote"), or a
  deployment where a proxy/policy blocks desktop or camera video but allows audio. Product
  decision: the call must come up regardless; a blank camera window (still needed for the
  toolbar) with a working call is the intended outcome.
- **Change**: `FfiModel` gained `callbacksOnPeerInfo` / `addCallbackOnPeerInfo()`, invoked at
  the end of `handlePeerInfo()` for every **non-cached** PeerInfo (the point where the remote
  has accepted the session and the transport is up — also after a reconnect, which is wanted
  since the previous call died with the previous connection). `view_camera_page.dart` dials
  `sessionRequestVoiceCall` from that callback; the first-image callback keeps only the
  keyboard-layout and recording-status work it had upstream. `ChatModel.voiceCallAutoDialed`
  is still set `true` immediately before the request (see "Duplicate Voice Call Prevention").
- **Camera and desktop windows must stay usable without video** (same change): upstream's
  modal "Connected, waiting for image..." dialog (`FfiModel.showConnectedWaitingForImage`,
  triggered by the `success` msgbox `ui_session_interface.rs` sends right after PeerInfo)
  stays up until the first frame and its only button closes the session. On a camera-less
  remote (PeerInfo with zero displays — the remote accepts the camera session anyway) or when
  video is blocked, that would block the toolbar for ever while the call is already running
  (camera) or could be started from the toolbar (desktop). `FfiModel` now has
  `videoUnavailable` / `noVideoTimer` / `markVideoUnavailable(reason)`: for a live camera- or
  desktop-session PeerInfo, a 10 s timer (`noVideoTimeout`) marks video unavailable if no
  frame has arrived; a camera session with no displays is marked immediately (a desktop with
  no displays stays upstream's "No Displays" case). Marking suppresses/dismisses the dialog
  (the `videoUnavailable` check in `showConnectedWaitingForImage`, plus
  `clearWaitingForImage`), shows a 6 s toast ("The remote has no camera" / "No video is being
  received from the remote" + "The voice call is not affected") and leaves `waitForFirstImage`
  untouched so a frame arriving later still runs the normal first-image initialisation in
  `FFI.onEvent2UIRgba` (which also cancels the timer). The toolbar never depended on the first
  frame — only the dialog did — so nothing in `view_camera_page.dart`/`remote_page.dart`
  changed.
- **Audio permission vs. voice call** (checked 2026-10-08, no change): the per-connection
  "Enable audio" permission (`Connection.audio`, CM toggle / `enable-audio` default) only gates
  the remote → local audio subscription (`audio_enabled()` in `handle_voice_call` and the
  `SwitchPermission` handler). It does not gate `VoiceCallRequest`, the CM prompt, or the
  local → remote mic path (`AudioFormat`/`AudioFrame` are gated only by the local's own
  `disable_audio` mute). With the permission off a call still connects and the remote hears
  the caller; the caller just hears nothing. It is therefore not a voice-call on/off control;
  that is the `voice-call-enabled` key in the next section.
- **Upgrade check**: if upstream restructures `handlePeerInfo()` (e.g. splits the cached vs
  live paths), keep the callback loop and the camera no-video arming on the live path only,
  after `_pi.isSet` is set. If the remote side ever starts rejecting `VoiceCallRequest` before
  its own subscription setup completes, the dial may need a retry — today `connection.rs`
  accepts it as soon as the connection is authorized. If upstream changes when/how the
  "waiting for image" dialog is shown, re-check the `videoUnavailable` guard still sits
  in front of it.

### Voice Call On/Off Control (implemented 2026-10-08)
Verify, on any upstream merge that touches `src/fork_config.rs`, the `VoiceCallRequest` arm
in `src/server/connection.rs`, `handleMsgBox` in `flutter/lib/models/model.dart`, the chat
menu in `flutter/lib/desktop/widgets/remote_toolbar.dart` or the mobile `showChatOptions`
menus:
- **Why**: product decision — voice calls must be switchable off per machine, on both ends,
  and the upstream `enable-audio` permission does not do that (previous section).
- **Config**: one optional key, `voice-call-enabled` (`"Y"`/`"N"`, absent = `"Y"`), parsed by
  `fork_config.rs` (`keys::VOICE_CALL_ENABLED`, `ForkConfig::voice_call_enabled`, listed in
  `OWN_KEYS`) and applied as `Config::set_option("voice-call-enabled", ...)`. Same key name on
  both roles; documented in `configs/local.toml`, `configs/remote.toml`,
  `configs/all-options-reference.toml` and `docs/CONFIG_REFERENCE.md` §4.13. Unit tests:
  `voice_call_enabled_defaults_to_true_when_absent`, `apply_maps_voice_call_enabled_to_option`
  (the test `GlobalStateGuard` snapshots/restores the option).
- **Remote enforcement** (`connection.rs`, `VoiceCallRequest` with `is_connect`): checked
  first, before the duplicate-call reservation and before `Data::VoiceCallIncoming` reaches the
  CM. Rejection = `new_voice_call_response(ts, false)` plus a `MessageBox{msgtype:
  "voice-call-disabled", title: "Voice Call", text: "Voice calls are disabled on the remote
  computer."}`, then `return true` (same shape as the duplicate-call refusal). No protocol
  change.
- **Local gating** (courtesy only; the remote enforces regardless): `isVoiceCallEnabledLocally()`
  in `common.dart` reads the option via `mainGetOptionSync`. Used by the desktop
  `_ChatMenu` (`remote_toolbar.dart`, the "Voice call" entry is omitted), both mobile
  `showChatOptions` menus (`mobile/pages/remote_page.dart`, `mobile/pages/view_camera_page.dart`;
  "End voice call" still shows while a call is active), and the Support auto-dial in
  `desktop/pages/view_camera_page.dart` (returns before `sessionRequestVoiceCall`).
- **Caller feedback**: `handleMsgBox` shows msgtype `voice-call-disabled` as a
  `custom-nocancel-info` box unconditionally (also for the automatic Support call — unlike
  `voice-call-duplicate`, which is suppressed for auto-dials), because otherwise the call UI
  would just silently reset. Must stay a `custom` type (see Duplicate Voice Call Prevention).
- **Upgrade check**: if upstream adds its own voice-call permission, decide whether to map
  this key onto it (as `support-enabled` → `enable-camera` does) and drop the fork-side
  rejection; until then keep the `connection.rs` check in front of upstream's handling.

### Connection Manager List View, Chat Sender Attribution, Video Sources and Camera Preview (implemented 2026-10-07)
Verify, on any upstream merge that touches `flutter/lib/desktop/pages/server_page.dart`,
`flutter/lib/models/server_model.dart`, `flutter/lib/models/chat_model.dart`,
`flutter/lib/common/widgets/chat_page.dart`, `flutter/lib/main.dart`,
`flutter/lib/utils/multi_window_manager.dart`, `src/ipc.rs` (`Data`), `src/ui_cm_interface.rs`
(`Client`, `InvokeUiCM`), `src/server/connection.rs` (subscription paths), `src/server.rs`
(`is_video_service_name`/`get_subbed_displays_count`) or `src/server/video_service.rs`'s capture
loop:
- **Why**: with several locals connected to one remote, upstream's CM (one tab per connection,
  a 300 px card with a permission grid) hid everything but the selected connection, gave no way
  to tell two locals with the same display name apart in chat, never said which monitor/camera
  a local was watching, and offered no way to see what a camera was sending (monitors are
  visible on the remote's own screen; a camera feed is not). This change addresses all four
  (product decisions: pop-up preview for cameras only; no fixed CM height; desktop only).
- **CM list view** (`server_page.dart`, rewritten): `ConnectionManagerState.build()` renders a
  `ListView` of `_GroupHeader` (avatar + `Client.displayName` = `"Name (id)"`, one per peer id,
  groups with a pending request sorted first) and `_ClientRow` (one per connection: type icon
  and label — "Desktop"/"Camera"/"File Transfer"/"Terminal"/"Port Forward" — followed by
  ` · <source names>`, a state line of chips — Pending / Connected hh:mm:ss / Voice call /
  Incoming call / Privacy mode / Disconnected — and inline actions: "Accept <type>" /
  "Reject <type>" when pending (e.g. "Accept Camera", "Reject File Transfer" — the button names
  what it accepts, as "Accept call"/"Decline call" do for a voice call; the row's type label is
  enlarged and orange while pending; changed 2026-10-10 from a "Requesting: <type>" chip with
  bare Accept/Reject buttons, which the user found unclear), plus Accept and Elevate when
  elevation applies; Accept call/Decline call, Audio input + Stop voice call, Switch Sides,
  Elevate, Chat (with unread badge) or File Transfer, a "⋯" permissions menu
  (`_buildPermissionsMenu`, upstream's `_PrivilegeBoard` as `CheckedPopupMenuItem`s), and
  Disconnect; Close when disconnected). Every action calls the same `bind.cm*` function the
  upstream card called — no backend behavior changed. Upstream's `_CmHeader`, `_PrivilegeBoard`,
  `_CmControlPanel` and `buildConnectionCard()` are gone.
  - **`DesktopTabController` is still the selection state**: `ServerModel._addTab()` still
    registers a (never rendered, `page: Offstage()`) tab per client so `tabController.onSelected`
    keeps driving the chat key, window title and `cmFileModel.updateCurrentClientId`; rows call
    `tabController.jumpTo(index)` on tap and `jumpToByKey` once after they are first built (what
    upstream's `_CmHeader.initState` did). `ServerModel` is otherwise unchanged in how it adds/
    removes clients and tabs; `chat_model.dart`'s unread/`selectedTabInfo` logic is untouched.
  - **Window size**: `kConnectionManagerWindowSizeClosedChat`/`OpenChat` (`consts.dart`) are now
    560×360 / 940×360 (were 300×490 / 700×490) and are only the initial/minimum size; the window
    is resizable (`setResizable(true)` in `runConnectionManagerScreen()` and `showCmWindow()`,
    where upstream calls `setResizable(false)`) and `_fitWindowHeight()` grows it with the row
    count up to `kConnectionManagerWindowMaxAutoHeight` (760), after which the list scrolls. It
    never shrinks the window on its own. `toggleCMSidePage()` now keeps the current height and
    changes only the width (upstream resets both to the fixed sizes).
  - **Chat/file side panel = upstream's `toggleCMSidePage()` again.** The fork's 2026-10-01
    floating-overlay chat (see the superseded section below) existed only because the tab layout
    could hide a pending accept prompt; the list cannot, so `showChatPage()`/`toggleCMChatPage()`
    are upstream's code again, `BlockableOverlay`/`applyFfi` were removed from the CM, and
    `buildSidePage()` gained a header ("Chat · Name (id)" or "File Transfer · Name (id)") with a
    close button. `DraggableChatWindow`/`toggleChatOverlay()`'s optional `type:` parameter from
    that change is kept (harmless).
- **Chat sender attribution**: `ChatModel.receive()` and `changeCurrentKey()` (CM branch) set the
  incoming `ChatUser.firstName` to `Client.displayName` ("Name (id)") instead of `client.name`,
  and `ChatPage`'s `messageTextBuilder` prints that author line above every incoming bubble when
  `type == ChatPageType.desktopCM`. Client-side (local) chat is unchanged. No protocol change —
  `ChatMessage` still carries only text; the identity comes from the connection's `LoginRequest`
  (`my_id`/`my_name`) the CM already has.
- **Video sources per connection** (what each local is watching):
  - `src/ipc.rs`: new `VideoSourceInfo { kind: "monitor"|"camera", index, name }` and
    `Data::VideoSources(Vec<VideoSourceInfo>)` (server → CM, full set, re-sent on change).
  - `src/server.rs`: `Server::get_subbed_video_sources(conn_id)` lists the `monitor{N}`/
    `camera{N}` services a connection is subscribed to (same name scheme as
    `get_subbed_displays_count`).
  - `src/server/connection.rs`: `report_video_sources()` maps those to names
    (`display_service::get_display_info(idx).name` / `camera::Cameras::get_sync_cameras()[idx].name`,
    falling back to "Monitor N"/"Camera N") and sends them; called after every subscription
    change: `try_sub_monitor_services()`, `try_sub_camera_displays()`, `switch_display_to()` and
    `capture_displays()` (each after dropping the server write lock — `report_video_sources`
    takes a read lock). **If upstream adds another path that subscribes/unsubscribes a video
    service, add the call there too**, otherwise the row goes stale.
  - `src/ui_cm_interface.rs`: `Client.sources` (serialized in `get_clients_state()` too, so the
    CM's periodic resync picks it up), `video_sources_changed()`, new `InvokeUiCM` methods
    `update_video_sources`/`camera_preview_frame` (Flutter: `push_event("update_video_sources")`;
    Sciter `src/ui/cm.rs`: no-ops). Flutter: `ServerModel.updateVideoSources()` via
    `model.dart`'s `update_video_sources` event; `Client.sources: List<VideoSourceInfo>`.
- **Camera preview pop-up** (cameras only — the operator can already see their monitors):
  - `src/ipc.rs`: `Data::CameraPreview { index, enable }` (CM → server) and
    `Data::CameraPreviewFrame { index, width, height, jpeg: base64 }` (server → CM).
  - `src/server/video_service.rs`: `CAMERA_PREVIEWS` (camera index → CM pipe + last-sent time),
    `set_camera_preview()`, and `maybe_send_camera_preview()` called from the capture loop's
    `frame.valid()` branch **only for `VideoSource::Camera`**. It taps the frame the service is
    already encoding — **no second camera open** (many drivers refuse shared access, and the
    virtual VF-WiFi-Camera already crashed once; see the camera-capture section). Throttled to
    one frame per `CAMERA_PREVIEW_INTERVAL` (250 ms), nearest-neighbour downscaled to
    ≤ `CAMERA_PREVIEW_MAX_WIDTH` (480 px) RGB on the capture thread (sub-millisecond), JPEG-
    encoded (`image` crate, quality 60) and base64'd on a spawned thread, sent over the CM pipe
    of whichever connection relayed the request; a failed send removes the entry. Costs nothing
    while no preview is open. Requires `image` — a desktop-only dependency in `Cargo.toml`, hence
    the `#[cfg(not(any(android, ios)))]` on the tap.
  - `src/server/connection.rs`: handles `Data::CameraPreview` from the CM pipe by calling
    `video_service::set_camera_preview(index, enable, conn.tx_to_cm.clone())`.
  - `src/ui_cm_interface.rs` `set_camera_preview(index, enable)` relays to **every** live
    connection (the server keeps the last pipe); `src/flutter_ffi.rs` `cm_set_camera_preview`
    (new bridge function — regenerated by CI's `flutter_rust_bridge_codegen`, the generated Dart
    is not committed).
  - Flutter: `WindowType.CameraPreview` (appended, index 6) and `DesktopType.cameraPreview`;
    `rustDeskWinManager.newCameraPreview(index, name)` creates the window; `main.dart` routes it
    to `DesktopCameraPreviewScreen` (`flutter/lib/desktop/screen/desktop_camera_preview_screen.dart`)
    with `showTitleBar(true)` on non-macOS so it has a native, minimizable frame; no custom tab
    bar, no `restoreWindowPosition`. `ServerModel.openCameraPreview()` (eye icon next to a camera
    source in the row; one window per camera index, shared by all viewers of it), `onCameraPreviewFrame()`
    forwards each `camera_preview_frame` event to the window as a
    `kWindowEventCameraPreviewFrame` method call (`DesktopMultiWindow.invokeMethod`), the window
    renders it with `Image.memory(gaplessPlayback: true)` and flags "No frames received recently"
    after 3 s; closing the window sends `kWindowEventCameraPreviewClosed` to window 0 (the CM
    main window, handled in `_DesktopServerPageState.initState`'s method handler) which calls
    `onCameraPreviewClosed()` → `cm_set_camera_preview(enable: false)`. `_refreshCameraPreviews()`
    re-issues `enable` for every open preview whenever a client is added/removed, so a preview
    survives the relaying connection closing. Multi-window from the CM process works because
    `desktop_multi_window` is registered for every engine in a process; the preview window runs
    `initEnv(kAppTypeDesktopCameraPreview)` like any sub-window.
- **Deliberately not done**: monitor previews (visible on the remote already); a single
  CM+main window; chat conferencing/relay between locals (noted as a later item); mobile CM
  changes (`flutter/lib/mobile/pages/server_page.dart` untouched; `Client.fromJson` tolerates a
  missing `sources`).
- **Upgrade checks**: (1) if upstream reshapes `Client` JSON or `InvokeUiCM`, keep `sources`
  and the two new methods; (2) if upstream changes how the CM selects a connection (replacing
  `DesktopTabController`), rewire `_ClientRow`'s `jumpTo`/`jumpToByKey` and `onSelected`; (3) if
  upstream's capture loop in `video_service.rs::run` is restructured, re-add the
  `maybe_send_camera_preview` tap before the screenshot block; (4) `WindowType` is positional
  (`Index` extension) — keep `CameraPreview` last before `Unknown`; (5) a Flutter upgrade may
  change `PopupMenuButton`/`CheckedPopupMenuItem` or `WindowController.showTitleBar` semantics —
  re-verify the "⋯" menu and the preview window's native frame.

### Connection Manager Chat: Floating Window Instead of Side Panel (fixed 2026-10-01; SUPERSEDED 2026-10-07 — kept for history)
**Superseded** by the list-view section above: CM chat is upstream's side panel again, and the
`BlockableOverlay` wrapping (and its `Consumer<ServerModel>` follow-up fix) no longer exists in
`server_page.dart`. Only the optional `type: ChatPageType?` parameter on
`DraggableChatWindow`/`showChatWindowOverlay()`/`toggleChatOverlay()` remains. The text below
describes the state between 2026-10-01 and 2026-10-07.

Verify, on any upstream merge that touches `flutter/lib/models/chat_model.dart`'s
`toggleCMSidePage()`/`toggleCMChatPage()`/`showChatPage()`, or
`flutter/lib/desktop/pages/server_page.dart`'s `ConnectionManagerState`/`buildSidePage()`:
- **Problem found**: upstream's own connection-manager chat (`toggleCMSidePage()`) widens the CM
  window and shows chat as a side panel next to whichever client tab is currently selected
  (`buildSidePage()`'s `Row` layout in `server_page.dart`). On a CM window with multiple connected
  clients, switching tabs while chat was open for one client could end up effectively hiding a
  still-pending accept/permission prompt for another — a real, reported problem, not hypothetical.
- **Fixed** by routing `role=remote`'s CM-side chat through the *same floating, draggable overlay
  window* (`toggleChatOverlay()`/`DraggableChatWindow`, `common/widgets/overlay.dart`) that a
  regular remote session already uses for chat, instead of the side-panel/window-resize approach:
  - `ConnectionManagerState` (`server_page.dart`) now owns a `BlockableOverlayState`, wires it to
    `gFFI` via `applyFfi()` in `initState()`, and wraps its entire `build()` return value in a
    `BlockableOverlay` — infrastructure that didn't exist on the CM side before this fix (it did
    already exist for regular remote-session pages).
  - `chat_model.dart`'s `toggleCMChatPage()` now calls `toggleChatOverlay()` instead of
    `toggleCMSidePage()` (same method name/call sites, different implementation) — also now
    explicitly clears `client.unreadChatMessageCount` itself, since `changeCurrentKey()`'s own
    unread-clear (`mobileClearClientUnread`) is a no-op on desktop and the removed
    `toggleCMSidePage()` call used to be what cleared it on this path.
  - `buildSidePage()` in `server_page.dart` is now file-transfer only — its chat branch is
    unreachable dead code (kept as `Offstage()` rather than removed/asserted, in case of a future
    caller), since nothing calls `toggleCMSidePage()` for chat anymore.
  - **File transfer's side panel is deliberately unaffected** — it still uses
    `toggleCMFilePage()`/`toggleCMSidePage()`/the window-resize approach exactly as before; only
    chat changed.
  - As a side effect of reusing this existing mechanism, CM chat now also has a visible close
    button (`DraggableChatWindow`'s app bar), which the side-panel version never had — this was a
    second complaint the same fix resolves, not a separate change.
- **Deliberately not done**: auto-closing the floating chat window when the connection ends. Per
  explicit product decision, not needed — the window simply stays open, same as the old side panel.
  The old side panel's read-only-after-disconnect behavior (`ChatPageType.desktopCM`'s `readOnly`
  check in `chat_page.dart`) *is* preserved: `DraggableChatWindow`/`showChatWindowOverlay()`/
  `toggleChatOverlay()` all now take an optional `type: ChatPageType?` parameter, and both CM call
  sites (`showChatPage`'s CM branch, `toggleCMChatPage()`) pass `type: ChatPageType.desktopCM`
  through to `ChatPage`, so CM chat still goes read-only once the client disconnects.
- **Upgrade check**: if a future upstream release changes `BlockableOverlayState`/
  `DraggableChatWindow`/`toggleChatOverlay()`'s behavior or requirements (e.g. requires something
  from a per-page `FFI` instance that CM's shared `gFFI` doesn't provide), re-verify the CM window
  still correctly renders the floating chat window — this fix depends on CM's `gFFI` being a
  sufficiently complete stand-in for the per-session `FFI` instances this mechanism was originally
  built for.
- **Follow-up bug found and fixed (2026-10-02, found via real two-machine testing)**: wrapping
  the CM window's entire `build()` return value in `BlockableOverlay` (to give the floating chat
  window somewhere to render) broke the CM's own client-list display. `BlockableOverlay.build()`
  constructs `Overlay(key: state.key, initialEntries: [OverlayEntry(builder: (_) => underlying),
  ...])` — but `Overlay`'s `initialEntries` is a Flutter quirk: it's only consulted once, when
  its `OverlayState` is first created. `server_page.dart` computed `underlying` as
  `serverModel.clients.isEmpty ? <"Waiting" placeholder> : <client DesktopTab>` directly in
  `build()`, then passed it into `BlockableOverlay` — on every subsequent rebuild (e.g. a client
  actually connecting), a *new* `underlying` value was computed and handed to a *new*
  `BlockableOverlay`/`Overlay` widget, but since `state.key` keeps the same `OverlayState` alive
  across rebuilds, that new `initialEntries` list is silently ignored. The result: the CM window
  stayed frozen on whichever branch was true the very first time it was built — in practice the
  empty "Waiting" placeholder, since that's always true before any client has connected — and
  never updated even after a client successfully connected (confirmed via a live two-machine
  test: the viewer saw the remote's desktop just fine, but the remote's own CM window still
  showed "Waiting" instead of the connected client's tab/accept controls).
  - **Fixed** by moving the reactive `serverModel.clients.isEmpty` branch *inside* a
    `Consumer<ServerModel>` nested within the one-time-captured `underlying` tree, instead of
    switching on it in the outer `build()` before handing the result to `BlockableOverlay`.
    `Consumer` subscribes to `ServerModel`'s ambient `Provider` directly through the element
    tree's own dependency mechanism, so it keeps rebuilding correctly even though the `Overlay`
    around it never re-reads `initialEntries` again.
  - **Not a regression anywhere else**: the other `BlockableOverlay` call sites
    (`remote_page.dart`/`view_camera_page.dart`, desktop and mobile) don't hit this, because their
    `bodyWidget()`s already route all dynamic behavior through nested `Obx`/`Consumer` widgets
    *inside* a structurally stable top-level tree — the same pattern this fix now also follows in
    `server_page.dart`. Checked at the time of this fix; re-verify this stays true if those files'
    `bodyWidget()`s are restructured.
  - **Upgrade check**: if upstream ever changes how `ConnectionManagerState.build()` switches
    between its empty and client-list states, make sure that switch stays inside a
    `Consumer`/`Obx`-style reactive wrapper nested within whatever gets handed to
    `BlockableOverlay`, not computed as a plain conditional in the outer `build()`.

### Duplicate Voice Call Prevention (implemented 2026-10-01, reworked server-side 2026-10-03)
Verify, on any upstream merge that touches `VoiceCallRequest`/`handle_voice_call`/
`close_voice_call`/`on_close` in `src/server/connection.rs`:
- **Problem found**: audio on the remote side is a single shared capture resource — the
  remote's audio service is a singleton (`src/server/audio_service.rs`), and a voice call
  additionally reroutes the remote's mic input away from normal PC-audio broadcast and toward
  whichever connection is in a call. If a local machine opens a Desktop session to a remote and
  starts a voice call, then also opens a Support session to the *same* remote (or just clicks
  "Voice call" again), a second, fully independent `VoiceCallRequest` was sent with nothing
  stopping it — resource contention on the remote's one audio stream, not a crash but a real
  functional conflict.
- **First attempt (2026-10-01) was client-side only** (a static, peer-id-keyed registry in
  `flutter/lib/models/chat_model.dart`, checked before dialing) — **reverted 2026-10-03, found
  via real testing to be unreliable**: the registry was only ever cleared by an explicit "voice
  call closed" event, so an abrupt disconnect (window closed, connection dropped) left a stale
  entry behind, causing a false "already in progress" that persisted until the app restarted —
  confirmed with zero actual calls in progress. Phase 2's audio-sharing work (next section) also
  removed the *implicit* protection the old resource-contention failure used to provide, which is
  what exposed that the client-side check was never reliably blocking the actual duplicate
  request in the first place — once a second connection's audio started actually working, the
  duplicate became obviously visible instead of silently broken.
- **Fixed server-side instead** (`src/server/connection.rs`): a `VOICE_CALL_BY_PEER: Mutex<HashMap<String, i32>>`
  maps each connecting peer's own id (`LoginRequest.my_id`) to the `conn_id` currently holding a
  pending-or-active call for it. `try_reserve_voice_call()`/`release_voice_call_reservation()`
  are the only two entry points:
  - A `VoiceCallRequest` with `is_connect = true` calls `try_reserve_voice_call` *before* ever
    setting `voice_call_request_timestamp` or notifying the CM — if a *different* connection
    already holds the reservation for that `my_id`, the request is rejected immediately (a
    normal `VoiceCallResponse { accepted: false }`, which the client already handles identically
    to an operator rejection — no client-side change needed) and the CM operator is never even
    shown a prompt for it.
  - The reservation is released in exactly three places: `handle_voice_call`'s rejected branch
    (operator declines), `close_voice_call()` (call ends normally), and — critically —
    `on_close()`'s connection teardown, **unconditionally**, regardless of *how* the connection
    ends. This last one is what makes it self-correcting: it depends only on this connection's
    own teardown running, never on the connecting client's own disconnect/lifecycle handling,
    which is exactly the class of bug that made the client-side attempt unreliable.
- **User feedback on refusal (2026-10-05)**: `VoiceCallResponse` has no text field, and the
  client shows nothing for `accepted = false` (true of a plain operator decline too), so a
  refused duplicate just made the call UI quietly reset. The remote now also sends a `MessageBox`
  (existing generic mid-session notice, no schema change) with the fork-specific
  `msgtype = "voice-call-duplicate"`. The remote cannot tell a manual "Voice call" click from the
  Support button's automatic call attempt (identical requests), and a popup is only wanted for
  the former — so the *client* decides: `handleMsgBox` (`flutter/lib/models/model.dart`) shows
  it only if `ChatModel.voiceCallAutoDialed` is false — and shows it as a `custom-nocancel-info`
  `msgBox`. The `custom` part is load-bearing: `msgBox()`'s OK button calls `closeConnection()`
  for every type that doesn't contain "custom" (that's the connection-error dialog), so showing
  this as `error` dismissed the popup by killing the whole session it appeared in (found via
  real testing, 2026-10-05). That per-session flag is set `true` by
  the one automatic site (`desktop/pages/view_camera_page.dart`'s PeerInfo auto-dial, see
  "Support Voice Call Dials on Session Acceptance" below) and
  `false` by every manual site (`remote_toolbar.dart`'s `_ChatMenu.voiceCall()`, the two mobile
  `onPressVoiceCall`s) immediately before each request. Any new request site must set it.
- **Phase 2 (implemented 2026-10-02): audio conferencing.** See the next section. Unaffected by
  this rework — this reservation only governs whether a *second* request from the *same peer* is
  even accepted; it says nothing about two *different* peers calling concurrently, which is
  exactly phase 2's scenario.
- **Upgrade check**: if upstream adds a new path that can produce a `VoiceCallRequest` outside
  this one message handler, route it through `try_reserve_voice_call`/
  `release_voice_call_reservation` too — do not reintroduce a client-side-only check as the
  primary guard.

### Audio Conferencing for Concurrent Voice Calls (phase 2, implemented 2026-10-02, reworked 2026-10-05)
Verify, on any upstream merge that touches `handle_voice_call`/`close_voice_call`/`on_close`/the
`AudioFormat`/`AudioFrame` receive handlers in `src/server/connection.rs`, or the
capture/encode path in `src/server/audio_service.rs`:
- **Problem**: when *different* local machines each have an active voice call with the same
  remote, nothing combined their audio. The remote's own mic already broadcast to every
  subscriber (direction 1, already worked via `audio_service`'s singleton `GenericService`), but
  no local's mic ever reached any *other* local — only the remote itself heard each caller
  independently, via its own per-connection decode/output pipeline in `src/client.rs`'s
  `start_audio_thread` (unaffected by this change). Separately, the remote's singleton
  mic-capture device toggle (`audio_service::set_voice_call_input_device`) was reset
  unconditionally by `close_voice_call()`/connection teardown regardless of whether *other*
  connections were still mid-call — a bug upstream's own code already flagged in a comment at
  `on_close()` as a known, accepted limitation ("We can add a (Vec<conn_id>, input device) to
  avoid this. But it's not necessary now...").
- **First implementation (2026-10-02) was reworked on 2026-10-05** after real two-machine
  testing showed it broke a working call the moment a second caller joined (remote's mic vanished
  for everyone, one caller got nothing back, the other heard only the second caller). Root causes,
  recorded so they aren't repeated: it mixed on a free-running 10ms `std::thread::sleep` ticker
  (~15ms granularity on Windows) that re-read each source's *latest* frame instead of consuming
  frames, so frames were duplicated/skipped and 10ms Opus frames were produced at a ~15ms
  cadence; it resampled the mix to a separately-tracked format before encoding and silently
  dropped any encode error (one failing member just went silent); it *unsubscribed* in-call
  members from the plain broadcast once there were two, so when the mixer misbehaved there was
  no fallback; and it only ever registered VIEW_CAMERA connections (Desktop-session calls were
  never members). Its premise that the remote's speaker plays N callers via N concurrent
  per-connection output streams was also never verified.
- **Current design** — entirely server-side (`src/server/voice_conference.rs`, no
  `libs/hbb_common` changes, no Flutter changes), driven by the real capture clock:
  - A registry of "connections currently in a voice call" (`voice_conference::STATE.members`),
    populated by `register()` in `handle_voice_call`'s accepted branch — **for every connection
    type**, outside the `is_authed_view_camera_conn()` block — and `unregister()` in
    `close_voice_call()` and the `on_close()` teardown path. `unregister()` returns whether it
    removed the *last* member — only then is `set_voice_call_input_device(None, true)` called,
    fixing the device-reset bug above.
  - `on_local_format`/`on_local_frame` (from `connection.rs`'s `AudioFormat`/`AudioFrame`
    handlers) decode a member's mic, convert it once to the remote's *capture* format, and queue
    it in a bounded per-member ring buffer (≤200ms, oldest dropped) that absorbs network jitter —
    the same idea as `client.rs`'s `AudioBuffer`. Both return `true` for a member: the
    `AudioFormat` handler then does **not** open upstream's per-connection `audio_sender`
    playback for it at all (even an idle extra output stream would be a second concurrent
    stream on the device), and the `AudioFrame` handler does **not** forward the frame to one
    (it would be heard twice).
  - `audio_service::send_f32` calls `on_capture_frame(data, sample_rate, channels, sp)` with
    every capture frame, **before** the zero gate (callers must keep hearing each other while
    the remote's mic is silent). Per 10ms frame it: drains exactly one frame's worth from every
    member's ring (zero-padded on underrun); plays the *sum of all members* on the remote's
    speaker through **one** shared `start_audio_thread()` playback (so this never depends on N
    concurrent output streams coexisting on the device); and sends each member `remote mic +
    every other member` — never its own — encoded with that member's own Opus encoder at the
    capture's exact format, delivered via `ServiceTmpl::send_to`. Frame sizes are therefore
    always ones the encoder accepts (the capture encoder already accepts them); there is no
    timer thread and no output resampling.
  - It returns how `send_f32` must route its plain capture-encoded frame: `None` (no call) →
    broadcast to everyone, upstream's system-sound sharing; `Some(ids)` (a call is active) →
    only to those members, via the new `ServiceTmpl::send_only` (`src/server/service.rs`), and
    if `ids` is empty the plain frame isn't even encoded. **Nobody is ever
    unsubscribed/resubscribed** — routing is decided per frame.
  - **Privacy decision (2026-10-05)**: while any call is active the capture *is* the microphone,
    and nobody outside the call receives audio at all until the last call ends. Stock RustDesk
    keeps broadcasting the (now microphone) stream to any non-call Desktop viewer for the
    duration of a call — verified on a stock build. Deliberately not kept: a viewer outside the
    call must not hear the mic, and the same agent's Desktop window shouldn't play the remote
    twice. System-sound sharing to Desktop viewers resumes when the last call ends.
  - With no members, `on_capture_frame` returns immediately and `send_f32` broadcasts exactly
    as upstream. With **one** member there is no per-member encode at all: its mix would be the
    remote's mic alone, which is exactly what the plain capture frame already is, so that frame
    is sent to it (and only to it). Encode count per 10ms is therefore 0 with nobody subscribed,
    1 for system-sound sharing with no call, 1 for a lone caller, and exactly N for N ≥ 2
    callers. Personalized streams start only at two members — at that moment a listener's incoming stream switches from the capture
    encoder to its per-member encoder, a few-ms decoder discontinuity comparable to one lost
    packet, accepted. The remote's speaker playback of callers is fed decoded PCM directly
    (`MediaData::AudioPcm`, a fork-added variant handled by `start_audio_thread`'s loop via the
    new `AudioHandler::handle_pcm` in `src/client.rs`) — no encode/decode round trip. So a lone
    caller costs nothing extra over upstream on the encode side; the only difference is that its
    mic plays through the conference's shared playback rather than upstream's per-connection
    `audio_sender` thread.
  - If the capture (re)starts in a new format (device change via `restart()`), rings, member
    encoders and the shared speaker are reset to the new format; clients keep receiving
    `create_format_msg` as usual since they stay subscribed.
- **Investigated, found to be a non-issue**: whether the CM (connection-manager) UI could
  visually confuse two overlapping incoming calls from different clients. It cannot — the CM's
  multi-client accept/reject UI (`flutter/lib/desktop/pages/server_page.dart`'s client list) is
  already driven by **per-client** `Client.inVoiceCall`/`incomingVoiceCall` fields (populated by
  `ServerModel.updateVoiceCallState`), not the shared `ChatModel.voiceCallStatus` Rx value —
  that shared value is only read by the single-session caller-side UI
  (`remote_toolbar.dart`/mobile pages), which is correctly scoped to its own session already.
- **Known caveats, deliberately accepted**: a member that has `disable_audio` set is no longer
  an `audio_service` subscriber, so `send_to` for it is a no-op — it simply hears nothing, which
  matches what disabling audio means (its own mic still reaches the remote and the other
  callers). Mixing is a plain sum with hard clamping to [-1, 1]; with several simultaneous loud
  talkers this clips rather than ducking — acceptable for a support-call scenario, revisit with
  per-source gain if it ever matters. The remote's speaker plays callers through one shared
  PCM-fed playback even for a single caller (see above) — if a solo-call regression is ever
  suspected on the remote's speaker, this path (not upstream's per-connection `audio_sender`) is
  where to look; what a solo caller *hears* is upstream's unchanged broadcast path.
- **Upgrade check**: if upstream changes `magnum_opus`'s `Decoder`/`Encoder` API, or the
  `AudioFrame`/`AudioFormat` message shapes, re-verify `voice_conference.rs` against
  `src/client.rs`'s own `AudioHandler::handle_format`/`handle_frame` (the reference usage this
  module's decode calls were modeled on).

### Voice Call Captured the Speaker Loopback Instead of the Mic (fixed 2026-10-05, found via real testing)
Verify, on any upstream merge that touches `audio_service.rs`'s `get_device()`/
`get_audio_input()`/`get_voice_call_input_device()`, or `ipc.rs`'s `voice-call-input` handling:
- **Problem found (upstream, not fork-introduced)**: during a voice call the remote's
  `audio_service` capture is meant to switch from the speaker loopback (normal PC audio) to a
  microphone via `set_voice_call_input_device(Some(<default input name>))`. But every
  `get_device()` variant only took the microphone path when the resolved input *name* was
  non-empty — and two upstream paths can set the voice-call device to `Some("")` ("default"):
  the option-sync in `ipc.rs` (`if self.voice_call_input != Config::get_option("voice-call-input")
  { set_voice_call_input_device(Some(<option>), true) }`) and the `voice-call-input` config
  setter used by the CM's device picker. `Some("")` fell straight through to the loopback branch,
  so the remote captured and re-broadcast **what its own speaker was playing** — i.e. the callers'
  voices — instead of its mic. Observed symptoms, all now explained: in a 1:1 call the caller hears
  their own voice back; with two callers (tested on a build *without* the fork's conferencing
  code, and reproducible on stock RustDesk) the remote's mic reaches nobody while each caller
  hears the other. This also fully corrupted the fork's conferencing, whose "remote mic" input
  is this same capture.
- **Fixed** in `get_device()` (Windows and screencapturekit variants): take the microphone path
  whenever a voice call is active (`get_voice_call_input_device().is_some()`), not only when the
  name is non-empty — `get_audio_input("")` already resolves to the default input device. The
  non-Windows/non-SCK variant always used `get_audio_input()` and needed no change.
  `handle_voice_call` now also logs a warning if `get_default_sound_input()` is `None` (no
  input device at all — the capture then genuinely cannot switch to a mic).
- **How to confirm on a machine**: right after a call is accepted, the remote's log (the
  `log` folder next to its config directory) shows `Input device: <name>` for the microphone
  path versus `Default output device: <name>` for the loopback path.
- **Not fixed here**: the Linux/PulseAudio path sends the (possibly empty) name to the `_pa`
  helper as `audio-input`; whether an empty name there selects the default *source* (mic) or a
  *monitor* (loopback) was not checked - verify with the same log test before relying on voice
  calls from a Linux remote.
- **Upgrade check**: if upstream reworks `get_device()`, keep the invariant "voice call active ⇒
  never the loopback branch", and keep `get_audio_input("")` resolving to a real input device.

### Camera Capture Must Not Kill the Remote (fixed 2026-10-07, found via real testing)
Verify, on any upstream merge that touches `libs/scrap/src/common/camera.rs` (`create_camera`,
`CameraCapturer::frame`) or `src/server/video_service.rs::get_capturer_camera`:
- **Problem found**: the remote process died silently — three times in a row, no error in the
  log — within one second of a client selecting one particular camera, a *virtual* "WiFi camera"
  driver (phone-as-webcam app). Physical cameras on the same machine, under the same voice calls
  and connections, streamed fine. The last log line each time was the camera service finishing
  setup for that device and pulling its first frame. Nothing in the fork touches camera capture;
  this is upstream behavior: the release profile has `panic = 'abort'` (`Cargo.toml`), so a panic
  inside the capture library (`nokhwa`, RustDesk's fork on the tellingly-named
  `fix_from_raw_parts` branch) when a driver hands back a frame that doesn't match what it
  advertised ends the process with the panic text going to stderr — never to the log.
- **Fixed, two halves, both in fork-owned code (no change to the `nokhwa` dependency)**:
  - **Validate before decoding** — `CameraCapturer::frame` now checks the raw frame against the
    format and resolution the driver *itself* declared (`validate_buffer`: non-empty; for the
    fixed-size uncompressed formats YUYV/NV12, at least `w*h*2` / `w*h*3/2` bytes; MJPEG and
    anything else size-checked for emptiness only) before `decode_image`. A mismatch becomes an
    ordinary, logged capture error (the video service already handles those: it logs and restarts
    or falls back) instead of reaching the library's unsafe decode path. Stream-open, frame and
    decode errors are all routed through `Cameras::note_capture_failure`.
  - **Fall back to the driver's default format** — `CAMERA_SAFE_FORMAT`/`CAMERA_UNUSABLE`
    (per camera index, process lifetime): after `CAMERA_FAILURES_BEFORE_FALLBACK` (3)
    *consecutive* capture failures the camera is marked so `create_camera` reopens it with
    `RequestedFormatType::None` (the driver's own default mode — what it's most likely to deliver
    correctly) instead of `AbsoluteHighestResolution`; 3 more consecutive failures in the default
    format mark it unusable, and `create_camera` then refuses it with a clear error rather than
    retrying forever. A single transient error on a healthy camera changes nothing: the counter
    (`CAMERA_FAILURES`) is reset by the first good frame of each capturer — once per capturer, so
    the streaming loop itself takes no lock. Every failure and both transitions are logged
    (`camera{N}: …`), so the log now says *why* a camera stopped. Performance: per frame this adds
    three field reads and an integer compare — nothing measurable next to decode + encode. Because a camera reopened in a different mode may have a
    different size, `CameraCapturer::new` corrects that camera's cached `DisplayInfo`
    (`SYNC_CAMERA_DISPLAYS`) and `get_capturer_camera` re-reads it **after** creating the capturer,
    so the encoder is sized to the real frames (the RGBA→YUV conversion already rejects
    oversized frames and pads undersized ones, so even a stale size can't crash — it would just
    show the image in a corner).
- **What this does and doesn't cover**: it fully covers a bad frame reaching the decode step,
  and any failure the library *reports* as an error. It cannot catch a panic or native fault
  that happens *inside* the library's stream-open or sample-read before our code sees the frame —
  with `panic = 'abort'` nothing in-process can. If the crash persists on that camera after this
  change, the next step is to confirm the exact site: run the remote from a console with
  `set RUST_BACKTRACE=1` and `rustdesk.exe 2> %TEMP%\rustdesk-stderr.txt`, select the camera,
  read the `panicked at <file:line>` line; or Event Viewer → Application → `Faulting module name`
  (`librustdesk.dll` ⇒ a panic/our side; a Media Foundation or camera DLL ⇒ the driver).
- **Upgrade check**: if upstream changes `nokhwa` (branch/version) or its `FrameFormat`/`Buffer`
  API, re-check `validate_buffer`'s format arms and `buffer.buffer()`/`source_frame_format()`;
  if upstream reworks `get_capturer_camera`, keep "create the capturer, *then* read the size".
- **Round 2 (2026-10-09, found via real testing of round 1)**: kills still happened, less
  often, and once the camera list showed the *same physical camera twice*, standing in for the
  broken one. Three further changes, all fork-owned:
  - **Panic hook** (`src/core_main.rs`, right after `init_log`): `std::panic::set_hook` logs
    `PANIC (process will abort): <message @ file:line>` plus a backtrace and flushes the
    logger (WriteMode::Direct) before the abort. The hook runs even with `panic = 'abort'`;
    recovery still isn't possible, but the next kill will name its exact site in the log —
    previously it left nothing. **This is what to look for first in the remote's log.**
  - **Open cameras by stable id, not enumeration index** (`camera.rs` `CAMERA_IDS`,
    `create_camera(idx)` → `open_camera(idx, CameraIndex)`): nokhwa opens
    `CameraIndex::Index(i)` by re-enumerating the devices and taking the i-th; a flaky virtual
    camera driver that drops out of / re-enters the enumeration between the login listing and
    the open shifts the indices, so index i can open a *different* camera — the "same camera
    twice" symptom. `all_info()` now records each device's Media Foundation symbolic link
    (`CameraInfo::misc()`) per display index and the capturer opens `CameraIndex::String(id)`,
    which nokhwa resolves by that link; a vanished device is an open error, never another
    camera. Linux keeps index-based open (its ids are not the same thing).
  - **Login listing hardened** (`all_info()` Windows branch): duplicates (same id listed twice
    by the system) are skipped; a camera that cannot be opened is listed as `online: false`
    with a placeholder size and counted via `note_capture_failure` instead of `?` failing the
    whole list (one broken device no longer hides the working ones, and no longer yields an
    empty camera list); resolutions are cached per device id (`CAMERA_RESOLUTIONS`) so a camera
    is opened for its size once, not on every login — every login used to touch the broken
    driver again. `get_camera_resolution` reads the listing instead of opening the camera a
    second time; `exists()` uses the de-duplicated list once it exists; `CameraCapturer::new`
    refreshes the cache and sets `online` when the camera really opens.
  - **Still outstanding at that point**: the crash site itself — resolved in round 3 below.
- **Round 3 (2026-10-09, the actual kill, found from this machine's own remote-role logs)**:
  every kill ends the log right after a camera service (re)starts ("new video service:
  cameraN … gdi: true") with **no error and no panic** — a native crash — and every one happens
  while **several camera services start/stop concurrently** (a local viewing all cameras, each
  new viewer making the others restart with "SWITCH", the login listing opening/dropping every
  camera). Reading nokhwa's Windows backend (`nokhwa-bindings-windows/src/lib.rs`): it keeps a
  (non-atomic) count of open camera objects and, when it drops to zero,
  `MediaFoundationDevice::drop` calls `de_initialize_mf()` = `MFShutdown()` +
  `CoUninitialize()` from whichever thread dropped the last camera — while another thread may
  be inside `ReadSample()`/`ActivateObject()`. Shutting the platform down under a live reader
  is an access violation inside Media Foundation, hence no log line and nothing for the panic
  hook. The pre-fix restart storm ("Hardware MFT failed to start streaming (0xC00D3704)" every
  second, 255 restarts in five minutes, a new camera + hardware encoder each time) made the
  race near-certain; round 1's six-attempt cap made it rarer — the "less frequent" observed.
  - **Fix** (`camera.rs` `mf_pin`, Windows only, fork-owned, no library change): call
    `MFStartup` once ourselves (`#[link(name = "mfplat")] extern "system"`, `MF_VERSION`
    0x00020070, `MFSTARTUP_NOSOCKET`) before every camera use (`all_info`, `exists`,
    `open_camera`) and never match it with `MFShutdown`. Windows reference-counts the platform
    startup, so the library's shutdown only decrements and Media Foundation stays alive for
    the process lifetime. Logged once ("Media Foundation pinned …"); a failed MFStartup is
    logged as a warning and changes nothing else.
  - **Upgrade check**: if upstream moves to a nokhwa version whose drop no longer calls
    `MFShutdown`, the pin becomes redundant but harmless; if the camera backend changes to
    something other than Media Foundation, remove it.
  - **Tried and reverted (2026-10-09)**: `panic = 'unwind'` in the release profile plus
    `catch_unwind` around the service loops in `src/server/service.rs` (commit `908617af9`,
    reverted by the next commit). It built and ran on every shipped target, but the armv7
    Linux Sciter job (legacy 32-bit UI under QEMU, not shipped by this fork) failed its LTO link
    with `LLVM ERROR: out of memory` because of the unwind tables. Product decision: keep
    upstream's `'abort'` rather than carry a profile divergence or exempt one job. The panic
    hook stays, so a Rust panic is at least named in the log before the abort.

### Direct-IP Enforcement (implemented 2026-08-29, ADR-0003)
Verify:
- `src/rendezvous_mediator.rs::start_all()` still has both `--- BEGIN/END DIRECT-IP FORK ---` blocks: the `hbbs_http::sync::start()` call removed, and the registration loop replaced with `loop { sleep(1.).await; }`.
- No path outside this function calls `RendezvousMediator::start()`/`start_udp()`/`start_tcp()`/`register_pk()`/`register_peer()` directly (re-run `grep -rn "RendezvousMediator::start\(" src/` and confirm the only match is inside `start_all()` itself, now unreachable).
- `direct_server(...)` and LAN listening are still spawned as independent tasks *before* the removed loop, and both still start successfully for `role=remote`.
- `Config::set_option("enable-lan-discovery", "N")` is still present in `fork_config.rs::apply()`, and `src/lan.rs`'s ping-response handler still gates the ID-bearing `pong` on that exact option.
- A `role=remote` instance, monitored at the network level, sends **no** outbound UDP/TCP traffic to any rendezvous server address, and does not respond to a LAN-broadcast discovery ping with its ID.
- `RendezvousMediator::restart()`'s call sites (`flutter_ffi.rs`, `ipc.rs`, `ui_interface.rs`) still compile — the function itself is intentionally unmodified even though its effect is now inert.

### Direct-IP Enforcement — GUI status side effect (found and fixed 2026-09-30, via a real user test)
A downstream consequence of the registration-loop removal above that wasn't caught at the time:
`Config::update_latency()` (`libs/hbb_common/src/config.rs`) — the only thing that ever populates
the `ONLINE` map `get_online_state()` reads — is called exclusively from the registration loop
that no longer runs. This meant `get_online_state()` always returned `0`, so every "online status"
consumer stayed permanently in its initial "connecting" state:
- `src/ipc.rs`'s `Data::OnlineStatus` handler (desktop, polled by the GUI over IPC from the running
  server process) — visibly, the desktop home page showed **"Connecting to the {app} network..."
  forever** for a `role=remote` instance, which a user reasonably read as "this thing is trying to
  call home," even though it's cosmetic only (no bytes are actually sent anywhere) and the issue is
  specific to a stale status *label*, not an actual network attempt. Confirmed this affects
  `role=remote` only, in practice — `role=local` never starts the IPC server at all ("No Server/IPC
  for Local Mode" above), so it never reaches this handler; it correctly shows "Service is not
  running" instead.
- `src/flutter_ffi.rs::main_get_connect_status()`'s `#[cfg(any(target_os = "android", target_os =
  "ios"))]` branch — same underlying value, read directly (no IPC involved on mobile), same stuck
  state.

Both are fixed to report ready (`status_num = 1`) immediately rather than reading
`get_online_state()` at all — there is nothing to wait for or register with in this fork, so a
perpetual "connecting" label was actively misleading, not just imprecise. **Upgrade check**: if a
future upstream release adds a *new* meaning to `get_online_state()`/`ONLINE` beyond rendezvous-
registration latency tracking, this fix would need revisiting — re-verify nothing else depends on
this value being genuinely wired before assuming the hardcoded `1` is still correct.

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
