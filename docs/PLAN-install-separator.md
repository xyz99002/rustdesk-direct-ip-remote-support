# Plan: Install Separator (Distinct Local/Remote MSI Installers From Real RustDesk)

**Status: DRAFT — for review before implementation. Nothing in this plan has been built yet.**

## Context / why

This fork ships a single portable exe today; the user picks "Local" or "Remote" role via
a first-run setup dialog, which writes `config.toml` next to the exe. That part already works
and is unaffected by this plan.

We hit a real incident: a machine with real RustDesk already installed (its background service
running) caused this fork's GUI to connect, over a local IPC named pipe, to that *other* process
instead of its own — silently serving stale/foreign option values. Root cause: the IPC pipe name,
`%APPDATA%` storage path, and window title were all derived from a single Rust runtime value,
`APP_NAME`, which defaulted to literally `"RustDesk"` — identical to a real install. That runtime
value is already fixed (`53c4fb71c`, shipped).

Investigating further surfaced that the **same kind of identity collision exists in every other
packaging mechanism this fork uses**, independently, because each one hardcodes its own
"RustDesk"/`com.carriez.*`/`com.rustdesk.*` identity rather than deriving it from one shared
source of truth:

- **Windows MSI installer** (`res/msi/`) — own ProductName/install-folder/`UpgradeCode`/service
  name, unrelated to the Rust `APP_NAME` fix.
- **Linux** — `.desktop` files, a systemd `.service` unit, `.deb`/`.rpm` package name and install
  paths, an AppImage `app_info: id`.
- **macOS** — Xcode bundle identifier, two launchd daemon/agent `.plist` files (with hardcoded
  `/Applications/RustDesk.app/...` paths).
- **Android** — `applicationId`, apparently never addressed even though it likely matches the real
  RustDesk Android app's package id.

Given the decision to pursue full separation for consistency and security, this plan now covers
all of the above, plus removing this fork's local-mode dependency on the local IPC/service channel
entirely, so the class of bug we hit cannot recur even in some future, unanticipated pipe-naming
edge case.

Decisions carried into this plan from prior discussion:
- Keep the **portable exe and self-extracted exe** named `rustdesk.exe` on Windows — no change,
  full parity with upstream. Only the **MSI-packaged copy** of the exe gets renamed, matching what
  the in-app "Install" button *already does today* via upstream's own `rename_exe_cmd()` helper
  ([platform/windows.rs:1463](../src/platform/windows.rs:1463)) — Phase 1 just brings the MSI path
  in line with that existing, accepted behavior, not introducing a new kind of rename.
- Ship **two separate MSI installers** (Local, Remote) instead of one universal MSI, since service
  creation is a build-time WiX property (`CC_CONNECTION_TYPE`), not something that can react to a
  role chosen after install. ("Install Separator" rather than "two installers" because, once Linux
  packaging is included, the same Local/Remote split applies there too — potentially more than two
  installable artifacts in total across platforms.)
- Each installer ships pre-baked with the matching `config.toml`, so the first-run Local/Remote
  picker dialog never appears for an installed build — the role is already decided by which
  installer you ran.
- Where possible, no changes to `libs/hbb_common` and no changes to Rust/C++ source. This holds
  for Phase 1 fully. Phase 2 (skip local-mode IPC) does require small, additive Rust changes in the
  main crate (not `hbb_common`) — called out explicitly below since it breaks the "no code changes"
  pattern of Phase 1.

## Phases

### Phase 1 — Windows: MSI installer identity + two variants

*(Content unchanged from the original draft of this plan; Windows-specific, already scoped in
detail against the actual `res/msi/` source.)*

#### 1a. Give each MSI its own distinct app identity

For **both** installer variants (Local and Remote), before running `preprocess.py`:

1. Rename the just-built `rustdesk.exe` (in the CI job's dist directory) to
   `<AppName>.exe`, where `<AppName>` is a fixed value distinct from `"RustDesk"` —
   proposed: `RustDesk-DirectIP-RemoteSupport` (matching the runtime `APP_NAME` already in use,
   for consistency, open to a different string on review).
2. Pass `--app-name <AppName>` to `preprocess.py`.

This changes, consistently (all derived from the one `$(var.Product)` WiX variable — confirmed no
risk of drift between them): ProductName, install folder name (`C:\Program Files\<AppName>\`),
`UpgradeCode`, registry root, Windows Service name, uninstall entry, DisplayIcon path, shortcuts,
and the file/URL-association registry entries in `Regs.wxs`.

**Effect**: our `UpgradeCode` no longer matches a real RustDesk MSI's, so `<MajorUpgrade>` will not
treat installing ours as an upgrade of the real product (confirmed: this is `UpgradeCode`-driven,
not install-folder-driven — picking a different folder alone would not have prevented this).

#### 1b. Two installer variants, gated by `--conn-type`

Confirmed pre-existing, unmodified WiX conditions all gate on `CC_CONNECTION_TYPE="outgoing"`
being the *only* case that skips them ([RustDesk.wxs:47,48,59,78,128](../res/msi/Package/Components/RustDesk.wxs)):
service creation, service auto-start, tray auto-launch on install, and a SAS-generation registry
tweak.

- **Local installer**: pass `--conn-type outgoing` to `preprocess.py`. No service, no tray
  auto-launch on install.
- **Remote installer**: no `--conn-type` flag (today's default, unchanged). Service created and
  started as today.

#### 1c. Pre-baked `config.toml` per variant

Before running `preprocess.py` for each variant, copy the matching sample config into the dist
directory as `config.toml`:

- Local installer: `configs/local.toml` → `<dist-dir>/config.toml`
- Remote installer: `configs/remote.toml` → `<dist-dir>/config.toml`

`preprocess.py`'s auto-component generator already globs every file in the dist directory into the
MSI ([preprocess.py:118](../res/msi/preprocess.py:118)), so this file gets bundled automatically —
no WiX template change needed. Once installed next to the exe, `fork_config::config_exists()`
finds it immediately, `load_and_apply()` applies it on first launch, and
`handle_first_run_setup()`'s Local/Remote picker dialog is skipped entirely (config already exists).

Open question to confirm on review: should the bundled `config.toml` be the sample files as they
exist today in `configs/`, or do we want a leaner "installer default" variant of each (e.g.
stripped comments, specific default password/auth mode chosen for a fresh install)? Currently
assumed: reuse `configs/local.toml` / `configs/remote.toml` verbatim.

#### 1d. CI workflow changes (`.github/workflows/flutter-build.yml`)

**Status: IMPLEMENTED 2026-09-11** (not yet CI-verified or manually tested — see Risk below).

The "Build msi" step now loops over two variants (Local, Remote) per architecture, with a reset of
the WiX template files between passes — **necessary detail found during investigation**:
`preprocess.py` edits `Package/Includes.wxi` and `Package/Components/RustDesk.wxs` **in place**,
inserting generated content after a marker comment without clearing prior content
([preprocess.py:430-439](../res/msi/preprocess.py:430)). Running it twice back-to-back without a
reset would duplicate every file component and break the WiX build. Each pass, driven by a
PowerShell `foreach` over a small variant array:

```
1. git checkout -- res/msi/Package        # reset WiX templates from any prior pass
2. Copy-Item configs/<local|remote>.toml ./rustdesk/config.toml
3. Rename-Item ./rustdesk/rustdesk.exe -> ./rustdesk/<AppName>.exe
4. pushd res/msi; python preprocess.py --arp -d ../../rustdesk -v <version> --app-name <AppName> [--conn-type outgoing]; ...; popd
5. msbuild msi.sln ...
6. move the produced .msi to ./SignOutput/rustdesk-<version>-<arch>-<local|remote>.msi
7. Rename-Item ./rustdesk/<AppName>.exe -> ./rustdesk/rustdesk.exe   # restore for the next pass
8. Remove-Item ./rustdesk/config.toml
```

`<AppName>` is `RustDesk-DirectIP-RemoteSupport`, matching the runtime `APP_NAME` value already
shipped, for consistency.

Output naming (as implemented): `rustdesk-<version>-<arch>-local.msi` /
`rustdesk-<version>-<arch>-remote.msi`, which the existing downstream "Rename release files
(direct-ip)" step's prefix-trimming logic already turns into
`rustdesk-direct-ip-<direct-ip-version>-windows-<arch>-local.msi` /
`...-<arch>-remote.msi` with **no changes needed to that step or the "Publish Release" step** —
both already glob `rustdesk-*.msi`/`rustdesk-direct-ip-*.msi`, which matches both variants
without modification.

This applies to both Windows architectures already built (`x86_64-pc-windows-msvc`,
`aarch64-pc-windows-msvc`), so 4 MSI files total per release instead of 2.

**Files touched**: `.github/workflows/flutter-build.yml` only. No `libs/hbb_common`, no Rust, no
C++, no WiX template edits (only build-time arguments to the existing, unmodified templates).

**Risk**: as originally assessed — no WiX/MSBuild/Windows install environment available here.
CI build success confirms both MSIs *package* correctly; actual install/upgrade/uninstall/service
behavior needs the manual test checklist below, on a real machine, before this is considered done.

---

### Phase 2 — Windows: no server/IPC communication in Local mode

Rationale (per discussion): the incident happened because the GUI process talks to a "server"
component over a local IPC named pipe even when running with zero installation — that component
runs as a background thread of the same process for a plain portable launch
([core_main.rs:387](../src/core_main.rs:387)), and the GUI still round-trips to it over IPC rather
than reading state directly, purely for code uniformity with the genuinely-separate-process case
(installed service). A `role=local` (outgoing-only) instance has no legitimate need for almost
everything that channel carries: it never accepts inbound connections, never needs the
connection-manager approval flow, never needs mouse-idle/controlled-session tracking, and — per
this fork's design — has no rendezvous/relay server to register an ID or be found through.
Removing this dependency for `role=local` means the *entire class* of "local IPC channel answered
by an unrelated process" bug becomes structurally impossible for Local deployments, not just fixed
for the one pipe-name collision we found.

Prior investigation (already completed, see conversation history) found:
- Config reads already work without IPC in practice — `OPTIONS` is initialized directly from
  `Config::get_options()` in-process; the IPC loop only *refreshes* it to catch an unrelated
  process's changes, which never applies when there's no such process.
- Config writes already don't depend on IPC succeeding — `ipc::set_options()` already falls
  through to a direct in-process `Config::set_options()` write regardless of IPC outcome
  ([ipc.rs:1767](../src/ipc.rs:1767)).
- Outgoing connection initiation itself never routes through this channel.
- Nothing hangs or panics if the IPC channel is simply never started — every call site already
  treats it as best-effort.
- "My ID" is not displayed anywhere in this fork's UI. "Fingerprint" (Settings > About) is the one
  UI element that currently reads over IPC; per discussion, an empty fingerprint field for
  `role=local` is accepted rather than adding a direct-read fallback for it (can be revisited).

#### Changes

**Status: IMPLEMENTED 2026-09-11** (not yet pushed/tested on a real machine — see Risk below).
Turned out simpler than originally scoped: re-reading `ui_interface.rs`'s `get_option`/
`set_option`/`set_options` while implementing showed they **already** write straight through to
the in-process `OPTIONS` cache and to `Config` regardless of IPC outcome
(`ipc::set_options()` already falls back to a direct `Config::set_options()` write even when IPC
fails — [ipc.rs:1767](../src/ipc.rs:1767)), and `OPTIONS`'s lazy-static initializer already reads
`Config::get_options()` in-process on first touch. So there was nothing stale to fix there once
nothing is left to *corrupt* it — no direct-`Config`-access branch needed after all, unlike the
android/ios pattern originally expected to mirror.

Actual changes:
- **`src/core_main.rs`**: the `std::thread::spawn(move || crate::start_server(false, no_server))`
  call is now gated behind `!config::is_outgoing_only()` — skipped entirely for `role=local`.
- **`src/flutter_ffi.rs`**: `main_check_connect_status()` (the function Flutter's `main.dart`
  calls unconditionally at startup via `bind.mainCheckConnectStatus()`, which is what actually
  forces the `SENDER` lazy-static — and therefore the whole IPC polling loop — to initialize) now
  skips calling `start_option_status_sync()` when `config::is_outgoing_only()`.
- **`src/tray.rs`**: `start_query_session_count` (inbound-session-count tooltip) is now gated
  behind `!hbb_common::config::is_outgoing_only()`.
- `SENDER`'s few other touchpoints (e.g. `check_mouse_time()`, used for inbound-session
  input-blocking checks) were deliberately left unchanged — confirmed unreachable in practice for
  `role=local` (they're inbound-session-only call paths), and even in a worst case would just be a
  harmless failed-connection retry (no listener exists for `role=local` to answer it at all,
  wrong-process or otherwise).
- No changes needed in `src/ui_interface.rs`, `src/client.rs`, `src/ipc.rs`, or any Flutter/Dart
  file — Dart continues calling `mainGetOptionSync`/etc. exactly as today.

**Files touched**: `src/core_main.rs`, `src/flutter_ffi.rs`, `src/tray.rs` — main crate only, no
`libs/hbb_common` changes, no `ui_interface.rs` changes (both were expected pre-implementation,
neither was needed).

**Risk**: low-to-moderate. Rust-only, logic fully readable/verifiable here, unlike the WiX/C++ work
avoided in Phase 1. **Not yet built or run** — no local Rust toolchain available in this
environment (see `docs/UPSTREAM_UPGRADE_GUIDE.md`'s "Known Build Environment Issue"); needs a CI
build + manual smoke test before being considered done (settings persist across restart in Local
mode with no server thread running, outgoing connect still works, tray icon fine, About tab
fingerprint field acceptably blank, Remote mode unaffected).

---

### Phase 3 — Linux: same separation as Windows

**Status: PARTIALLY IMPLEMENTED 2026-09-28** (identity/naming separation done; install-time role
gating deliberately deferred — see below and `docs/UPSTREAM_UPGRADE_GUIDE.md`'s "Linux Install/Role
Separation" hook point for the full detail). Not yet CI-verified or manually tested on a real
machine — see Risk below. `res/rpm.spec`/`res/rpm-flutter.spec` were found to be dead files (not
invoked by any CI job — CI only builds `.deb` via `build.py`, archlinux via `res/PKGBUILD`, and
AppImage/flatpak, all four of which now carry the renamed identity) and were left untouched rather
than updated speculatively.

Additionally: `role=local` no longer even *runs* a background service on Linux/macOS if one somehow
starts — this was implemented and shipped separately, ahead of the rest of Phase 3, as an extension
of Phase 2's runtime gate (`core_main.rs`'s `--service`/`--server` arg branches, commit `81f42bca8`;
see "No Server/IPC for Local Mode" in `docs/UPSTREAM_UPGRADE_GUIDE.md`). That is a *runtime*
behavioral gate, distinct from the *install-time* gate (whether `postinst` enables/starts the
service at all) described as deferred below.

From the prior cross-platform audit, all of the following independently hardcode "rustdesk"/
"RustDesk" and would collide with a real RustDesk install on the same Linux machine:

- **`.desktop` files** (`res/rustdesk.desktop`, `res/rustdesk-link.desktop`): `Name=RustDesk`,
  `Icon=rustdesk`, `Exec=rustdesk %u`, `StartupWMClass=rustdesk`, `x-scheme-handler/rustdesk` MIME
  handler. Installing both packages means the second one's desktop entry/icon/URI-handler silently
  overwrites the first's in `/usr/share/applications/` and `/usr/share/icons/`.
- **systemd service** (`res/rustdesk.service`): unit literally named `rustdesk.service`,
  `ExecStart=/usr/bin/rustdesk --service`, `PIDFile=/run/rustdesk.pid`. `res/DEBIAN/postinst`/
  `prerm` actively manage `/etc/systemd/system/rustdesk.service` and
  `/usr/lib/systemd/{system,user}/rustdesk.service` by that literal name on install/removal — same
  collision as the Windows service, at the systemd level.
- **`.deb`/`.rpm` package identity** (`build.py::generate_control_file()`, `res/rpm.spec`,
  `res/rpm-flutter.spec`): `Package`/`Name: rustdesk`, installing to `/usr/share/rustdesk/` and
  symlinking `/usr/bin/rustdesk`. This is the most severe Linux collision — package managers treat
  same-named packages as literally the same package, so installing this fork's `.deb`/`.rpm`
  **replaces** a real RustDesk install outright, not side-by-side.
- **AppImage**: built from the same `.deb`, so inherits the above; additionally
  `AppImageBuilder-{x86_64,aarch64}.yml` set `app_info: id: rustdesk`. Lower risk as a standalone
  file, but a desktop-integration tool reading the embedded identity would hit the same collision
  as the `.desktop` file case.

#### Changes actually made (as implemented 2026-09-28)

- **Identity renamed to `rustdesk-direct-ip-remote-support`** (lowercase/hyphenated, per Linux
  package-naming convention) across: `build.py` (`generate_control_file()`'s `Package:` field,
  `build_flutter_deb()`/`build_deb_from_folder()`'s install dir/icon/desktop/systemd-unit
  filenames), `res/rustdesk.desktop` and `res/rustdesk-link.desktop` (`Name=`/`Icon=`, plus the
  stale `MimeType=x-scheme-handler/rustdesk;` fixed to match the actual runtime
  `get_uri_prefix()` value), `res/rustdesk.service` (`Description=` only — see binary-name note),
  `res/DEBIAN/postinst`/`prerm` (systemd unit filename, `/usr/bin/rustdesk` symlink source path),
  `res/PKGBUILD` (`pkgname`, install dir, icon filenames), `res/pacman_install` (same, plus fixed
  the `.desktop` destination filenames which the original script left unrenamed — a real bug this
  work would have introduced otherwise), `appimage/AppImageBuilder-{x86_64,aarch64}.yml` (app
  id/name/icon, install-dir references, icon filenames), and `flatpak/rustdesk.json` +
  `flatpak/com.rustdesk.DirectIPRemoteSupport.metainfo.xml` (flatpak app id, `rename-desktop-file`/
  `rename-icon`, the `/app/share/rustdesk/...` symlink path that would otherwise have been broken
  by the install-dir rename). Also updated `flutter/linux/my_application.cc`'s
  `gtk_icon_theme_load_icon()` call — a real (small) C++ code change, not just packaging, since the
  window-icon lookup is by icon-theme name.
- **Deliberately kept unchanged**: the `rustdesk` binary/command name itself (matches the Windows
  precedent of keeping `rustdesk.exe`), and `StartupWMClass=rustdesk` in the `.desktop` files (GTK
  derives WM_CLASS from argv[0]/prgname, not from this file, so it already matches the unchanged
  binary name). This leaves the `/usr/bin/rustdesk` symlink target as a known, accepted residual
  collision point between this fork's package and a real RustDesk's on the same machine — see
  `docs/UPSTREAM_UPGRADE_GUIDE.md` for the detailed tradeoff writeup.
- **`res/rpm.spec`/`res/rpm-flutter.spec` left untouched** — confirmed dead (not referenced by any
  CI workflow step), so updating them would be speculative work with no way to verify it.
- **Install-time Local/Remote split: IMPLEMENTED 2026-09-29**, as a follow-up to the identity work
  above, in a single package rather than two build variants (see "Linux Install-Time Role Gating" in
  `docs/UPSTREAM_UPGRADE_GUIDE.md` for the full detail):
  - `build.py::build_flutter_deb()` now bundles `configs/local.toml`/`configs/remote.toml` into the
    package so `rustdesk --setup-local`/`--setup-remote` has something to read — this also fixed a
    real bug where a prior CI step tried to do this same bundling but placed the files after
    `build.py` had already packaged the `.deb` from a separate staging copy, so they never actually
    reached the installed package.
  - `res/DEBIAN/postinst` and `res/pacman_install` no longer unconditionally `systemctl enable`/
    `start` the service — they read `role` from an already-existing `config.toml` (present only
    after a prior `--setup-local`/`--setup-remote` run) and only enable+start for `role = "remote"`;
    a fresh install with no `config.toml` yet defaults to installed-but-not-started.
  - `res/PKGBUILD`/`res/pacman_install` got the equivalent config-bundling and role-check changes.
  - **Deliberately not done**: splitting into two `.deb`/PKGBUILD build variants (mirroring the
    two-MSI Windows approach) with a pre-baked `config.toml` per variant. Assessed as too large a
    surface change to make safely without a real Linux build/install environment to verify against,
    given it would also ripple into the AppImage/flatpak jobs (both built from the single `.deb`).
    The single-package-with-role-check approach above achieves the same practical outcome (no
    Local-role install ever auto-starts an inbound-accepting service) without that risk.
  - **PAM service name, polkit action ID, `/etc/rustdesk/` naming: IMPLEMENTED 2026-09-29** (was
    listed here as deferred; see "PAM service name"/"Polkit action ID"/"`/etc/rustdesk/` config
    directory" in `docs/UPSTREAM_UPGRADE_GUIDE.md` for full detail). The PAM rename turned out to fix
    a real, previously-undiscovered bug (`pam_get_service_name()` was silently falling back to the
    `gdm` PAM stack on every install, since it already looked for a file matching the fork's own
    `APP_NAME` that the packaging never actually installed under that name). The "polkit action ID"
    item turned out not to exist in this codebase at all — no `.policy` file or polkit API call
    anywhere — corrected rather than fixing something that isn't there. `/etc/rustdesk/` was renamed
    for packaging hygiene only (nothing reads it by path).

**Risk**: moderate — more files than Windows Phase 1, but all plain text/script changes (no
compiled native code except the one-line `my_application.cc` fix, no WiX), and no verification
environment (no Linux build/install target available in this sandbox) — build success in CI is the
only automatic check; install/uninstall behavior needs manual testing on a real Linux machine,
ideally one with a real RustDesk `.deb` already installed.

---

### Phase 4 — macOS: same separation

**Status: IMPLEMENTED 2026-09-29** — narrower in scope than originally estimated below; see
"macOS App Identity" in `docs/UPSTREAM_UPGRADE_GUIDE.md` for the full detail, summarized here:

- **The launchd daemon/agent concern below turned out to already be solved.** The original audit
  (below, preserved for history) assumed `daemon.plist`/`agent.plist`/`install.scpt`/`uninstall.scpt`/
  `update.scpt` would each need explicit identity-string edits. Investigation found
  `src/platform/macos.rs::correct_app_name()` already generically substitutes the real running
  bundle ID and `APP_NAME` into all of them before use, and has done so since the original
  cross-platform `APP_NAME` fix — **none of those five files needed any changes.**
- **What was actually changed**: `PRODUCT_BUNDLE_IDENTIFIER` (3 occurrences in `project.pbxproj`,
  plus the matching line in `AppInfo.xcconfig`) → `com.rustdesk.DirectIPRemoteSupport`; `PRODUCT_NAME`
  in `AppInfo.xcconfig` → `RustDesk-DirectIP-RemoteSupport` (which required updating every
  hardcoded `.../Release/RustDesk.app` path across `build.py`, `.github/workflows/flutter-build.yml`,
  `.github/workflows/playground.yml`, and `res/osx-dist.sh` to match the renamed bundle filename);
  and `Info.plist`'s `CFBundleURLSchemes`/`CFBundleURLName`, which were stale against the
  already-shipped `get_uri_prefix()` value (same bug class as the Linux `.desktop` `MimeType` fix in
  Phase 3).
- **Confirmed not a concern**: the entitlements/keychain-access-group risk flagged below doesn't
  apply — neither `.entitlements` file has any bundle-id-scoped entries (app sandbox is disabled).
- **Not implemented**: the Local/Remote install-time split mentioned below — same reasoning as the
  Linux decision in Phase 3 (single package, no build-time split), not revisited here since macOS
  packaging wasn't otherwise touched.

<details>
<summary>Original audit and proposed changes (2026-09-11, superseded above)</summary>

From the audit:
- **Bundle identifier**: `flutter/macos/Runner.xcodeproj/project.pbxproj` hardcodes
  `PRODUCT_BUNDLE_IDENTIFIER = com.carriez.rustdesk;` in three build configurations (Debug/
  Release/Profile); `Info.plist` references the same value for the `rustdesk://` URL scheme.
  Colliding with a real RustDesk.app means shared Launch Services registration, shared
  `~/Library/Containers/com.carriez.rustdesk`, shared preferences file.
- **launchd daemon/agent**: `src/platform/privileges_scripts/daemon.plist`
  (`Label: com.carriez.RustDesk_service`) and `agent.plist`
  (`Label: com.carriez.RustDesk_server`) both hardcode `AssociatedBundleIdentifiers` to
  `com.carriez.rustdesk` **and** hardcode the absolute path `/Applications/RustDesk.app/Contents/
  MacOS/...` — meaning even after a bundle-ID rename, these two files need their own explicit path
  fix, not just an identifier substitution. *(Superseded — see above: this turned out to already be
  handled generically by `correct_app_name()`.)*

**Files touched (estimate)**: `flutter/macos/Runner.xcodeproj/project.pbxproj`, `Info.plist`,
`src/platform/privileges_scripts/daemon.plist`, `src/platform/privileges_scripts/agent.plist`.

**Risk**: moderate-to-high for the bundle-ID change specifically (signing/entitlement side effects
noted above), no macOS build/signing environment available here at all — this phase can only be
verified by CI build success plus manual testing on a real Mac.

</details>

---

### Phase 5 — Android: same separation

**Status: IMPLEMENTED 2026-09-29** — as low-risk as originally estimated; see "Android App Identity"
in `docs/UPSTREAM_UPGRADE_GUIDE.md` for the full detail, summarized here:

- **Changed**: `flutter/android/app/build.gradle`'s `applicationId` →
  `com.rustdesk.directipremotesupport`. Nothing else — Android's `applicationId`/code-namespace
  split means no Kotlin source, `AndroidManifest.xml` `package=` attribute, or JNI symbol renaming
  was needed.
- **Confirmed out of scope, not applicable**: no `google-services.json`/Firebase configuration
  exists under `flutter/android/` at all — the dependency the original audit worried about doesn't
  exist for this fork.
- **Deliberately not changed**: the `rustdesk://` deep-link intent-filter scheme in
  `AndroidManifest.xml`. Android/iOS never run `core_main()` (excluded by its own `#[cfg(...)]`), so
  `APP_NAME`/`get_uri_prefix()` are never customized there in the first place — the app still
  genuinely expects literal `rustdesk://`, so changing the manifest's registered scheme would have
  been a regression, not a fix.
- **iOS: IMPLEMENTED 2026-09-29 as a follow-on** (never one of this plan's original five phases —
  see "iOS App Identity" in `docs/UPSTREAM_UPGRADE_GUIDE.md`). Changed `PRODUCT_BUNDLE_IDENTIFIER`
  (`com.carriez.flutterHbb` → `com.rustdesk.DirectIPRemoteSupport`) and the app display name, after
  confirming (not assuming) that the `aps-environment` entitlement and `GoogleService-Info.plist`
  (Firebase) it carries are both fully dead — no Firebase dependency is active in `pubspec.yaml`, no
  Firebase initialization call exists anywhere, and no push-notification handling code exists.
  Deleted the now-doubly-stale, already-orphaned `GoogleService-Info.plist` rather than trying to
  keep a dead file's bundle-id reference in sync. Left `CFBundleURLSchemes` unchanged for the same
  reason as Android (`core_main()` excludes iOS too).

<details>
<summary>Original audit and proposed changes (2026-09-11, superseded above)</summary>

From the audit: `flutter/android/app/build.gradle`'s `applicationId "com.carriez.flutter_hbb"` was
never changed from the upstream default, and per public information this likely matches the real
RustDesk Android app's own package id — a latent collision never previously flagged because this
investigation started from the Windows incident.

**Files touched (estimate)**: `flutter/android/app/build.gradle`, possibly
`flutter/android/app/google-services.json` and Android manifest intent-filter entries.

**Risk**: low for the `applicationId` line itself, but the Firebase/deep-link dependency is an
external-service concern this plan can't fully resolve from the repo alone — needs a decision on
whether Firebase/push notifications are even in use for this fork before proceeding. *(Resolved
above: no Firebase config exists, and the deep-link scheme is deliberately left unchanged.)*

</details>

---

## Documentation obligation (every phase)

This repo already maintains `docs/UPSTREAM_UPGRADE_GUIDE.md` as the authoritative record of every
fork-vs-upstream touch point, specifically so a future `git merge`/rebase onto a newer upstream
RustDesk release doesn't silently drop or corrupt fork behavior. Every phase in this plan modifies
either upstream-owned files directly (WiX templates, `.desktop`/`.service` files, `build.py`,
spec files, the Xcode project, launchd plists, `build.gradle`) or adds a new Rust-side dependency
on upstream internals (`start_server`, `ui_interface::OPTIONS`, `is_outgoing_only()`) — exactly the
kind of change that guide exists to track. **No phase is done until it has a corresponding entry
in `docs/UPSTREAM_UPGRADE_GUIDE.md`**, following the existing "Critical Hook Points" format
(what to verify, exact file/line references, what a future upstream change could silently break).

Concretely, each phase's entry should cover:

- **Phase 1 (Windows MSI)**: a new "App Identity (MSI)" hook point — record the `--app-name`/
  `--conn-type` build parameters as the new source of truth, note that `preprocess.py` edits
  `Package/Includes.wxi`/`RustDesk.wxs` in place (the git-checkout-between-passes requirement), and
  flag that a future upstream change to `res/msi/preprocess.py`'s argument names/defaults, or to
  the `$(var.Product)` usage inside the WiX templates, would need this hook re-verified.
- **Phase 2 (Windows local-mode IPC skip)**: extend the existing "Role Enforcement" hook point
  (already in the guide) to also cover: `start_server()`'s call site in `core_main.rs`, the
  `ui_interface::OPTIONS`/`ipc::connect` android/ios-style direct-`Config`-access branches added
  for `role=local`, and a note that a future upstream change to `ui_interface.rs`'s `OPTIONS`
  caching or `ipc::set_options`'s local-write fallback ([ipc.rs:1767](../src/ipc.rs:1767), already
  relied upon by this phase) should be re-checked for continued correctness.
- **Phases 3-5 (Linux/macOS/Android)**: one hook-point entry per platform, listing the exact files
  renamed/edited and their new identity value, so a future upstream upgrade that touches
  `res/rustdesk.desktop`, `res/rustdesk.service`, `build.py`, the `.pbxproj`, the launchd plists,
  or `build.gradle` prompts a re-application of the rename rather than a silent revert back to
  upstream's "RustDesk"/`com.carriez.*` identity.

This documentation update should land in the **same commit/PR** as each phase's implementation,
not deferred — matching how prior fork work in this repo (Direct-IP Enforcement, Minimal UI, etc.)
already added its own dated sections to this same guide as it was built.

## What does NOT change

- `res/msi/CustomActions/CustomActions.cpp` — untouched. (This was the highest-risk item
  identified in an earlier, broader "decouple exe name from product name" alternative to Phase 1;
  it is **not needed**, since Phase 1 renames the exe for the MSI build instead of trying to keep
  it as `rustdesk.exe` while the product identity differs.)

## Phase 2b — Fork Peer Marker (protocol-level, added to this plan 2026-09-11)

**Status: BLOCKED 2026-09-11, not implemented — reverted after discovering a structural blocker.**
Was going to be bundled with Phase 2 for the same testing round per explicit request, rather than
a separate later cycle; that's now moot since it isn't implemented at all.

A different concern from everything else in this plan: OS-level process/installer identity
(pipes, install paths, service names) prevents confusion *before* any connection exists. This
phase was meant to address the connection itself — once a `role=local` instance dials an IP:port,
how does it know the thing that answered is actually this fork's `role=remote`, not a real
RustDesk instance happening to listen there?

**The blocker**: `LoginRequest` (and every other protocol message) is defined in
`libs/hbb_common/protos/message.proto`, which lives inside `libs/hbb_common` — a **git submodule
pointing at the official upstream `rustdesk/hbb_common` repository**, not something this fork owns
or has push access to. Adding a `fork_marker` field there means patching a separate, upstream-owned
repo, which this project has consistently avoided elsewhere (see `docs/DECISIONS.md` "Fork Peer
Marker" for the full writeup, including why no existing field in `LoginRequest`/`OptionMessage`
can safely carry this without a schema change or repurposing a real, in-use field).

Draft implementation (added, then fully reverted, so `git diff`/`git log` won't show a trace of
it): a new `fork_marker` field on `LoginRequest`, set by `src/client.rs` to a constant in
`src/fork_config.rs`, checked as the first thing done with an incoming `LoginRequest` in
`src/server/connection.rs::on_message()`, hard rejection on mismatch. Kept here as a ready-to-apply
design in case a future decision (see `docs/DECISIONS.md`) unblocks it — e.g. forking
`libs/hbb_common` too.

**Not part of this plan's "done" state until unblocked.** See `docs/DECISIONS.md` for the options
under consideration (fork `hbb_common` too / drop this specific safeguard / some other approach).

## Risks / what can't be verified locally, summarized across phases

No WiX toolset, MSBuild, Xcode, Android SDK, or any install environment is available in this
sandbox for any platform. Every change here can be read and reasoned about, and CI build success
confirms packaging works, but **actual install/upgrade/uninstall/service-start behavior on a real
machine must be tested by hand** for each platform before relying on this. Phase 2 (Rust-only) is
the one phase where I can verify logic correctness with real confidence; Phases 1, 3, 4, 5 are all
build-scripting/packaging-manifest/native-project-file changes I cannot execute or observe running.

Testing checklist (Windows, from original draft — equivalent checks needed per-platform once each
phase is implemented):
- Local MSI installs, no service appears in `services.msc`, no tray icon auto-launches after
  install, `config.toml` present with `role = "local"`, app opens straight to the connect panel
  (no first-run dialog).
- Remote MSI installs, service appears and is running, `config.toml` present with
  `role = "remote"`, app opens straight to the waiting-for-connections panel.
- Both installers, on a machine with a real RustDesk MSI already installed: confirm install
  succeeds into a separate folder, real RustDesk is untouched (still present, still working), no
  shared service name collision in `services.msc`.
- Uninstalling either of our installers doesn't affect a real RustDesk install and vice versa.
- Upgrading (installing a newer build of the same variant over an older one) still works correctly
  (service stopped/recreated cleanly, no orphaned process) — the `TryStopDeleteService` C++ path is
  unmodified, so it should behave exactly as it does for any other WiX-packaged RustDesk-family
  build today.
- Phase 2: settings persist across restart in Local mode with no server thread running, outgoing
  connect still works, tray icon behaves, About tab fingerprint field acceptably blank.

## Sequencing suggestion

Given the very different risk/verifiability profile per phase, suggest tackling and landing them
independently rather than as one large change:

1. Review and adjust this plan (this step).
2. **Phase 2 first** (Windows, Rust-only, no `hbb_common`, verifiable logic, lowest risk of the
   group) — also the phase that most directly prevents recurrence of the original incident.
3. **Phase 1** (Windows installer split) — already scoped in full detail against the real
   `res/msi/` source.
4. **Phase 3** (Linux) — next most-tractable, plain text/script changes.
5. **Phase 4** (macOS) — flagged as more invasive (signing/entitlements); revisit scope once
   Phases 1-3 are done and reviewed.
6. **Phase 5** (Android) — smallest change, but blocked on a decision about the Firebase/push
   dependency first.

Each numbered phase above includes, as part of "done": its `docs/UPSTREAM_UPGRADE_GUIDE.md` entry
(see "Documentation obligation" above), landed in the same commit — not a separate follow-up step.

Each phase should be implemented, pushed for CI build verification, and then manually tested per
the checklist above before being considered done — not batched together into one untested change.

Each phase should be implemented, pushed for CI build verification, and then manually tested per
the checklist above before being considered done — not batched together into one untested change.
