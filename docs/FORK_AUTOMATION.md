# FORK_AUTOMATION.md

# Goal
Create a repeatable process that transforms an upstream RustDesk release into the Direct-IP fork with minimal manual effort.

## Upgrade-verification artifact: `docs/FEATURE_ENFORCEMENT_MATRIX.md`
Before signing off on an upgrade, re-check every "Yes" cell in that matrix against the cited source in the new upstream version — it records, per feature, whether enforcement is UI-only, config-only, remote/protocol-level, or reuses an unmodified upstream mechanism. It's also where the `support_enabled` vs. `desktop_share_enabled` asymmetry (one is remotely enforced, one is UI-only) is the single source of truth — don't restate that asymmetry elsewhere without updating the matrix first.

## Strategy
Treat the fork as a configuration and UI transformation layer instead of a transport fork.

## Automation Pipeline
1. Fetch upstream RustDesk.
2. Verify version and commit.
3. Build baseline.
4. Apply fork profile.
5. Run validation suite.
6. Produce local-client and remote-client artifacts.

## Fork Profile Concepts
Configuration file format: TOML (confirmed 2026-08-28; reuses the `toml`/`confy` crates already in the dependency graph via `hbb_common` — no new dependency). Any YAML-fenced example elsewhere in the doc set is illustrative only, not the actual format.

Configuration should define (revised 2026-08-28, second revision — `desktop_share_enabled` added; Desktop is no longer unconditional):
- role
- authentication mode
- support_enabled (gates Support button visibility locally; also reused as the remote-side `enable-camera` permission to reject VIEW_CAMERA/Voice Call)
- desktop_share_enabled (gates Desktop button visibility locally; no remote-side enforcement exists — documented gap)
- validation: at least one of support_enabled/desktop_share_enabled must be true

## Stable Integration Points
### Role Control
Use upstream:
- `hbb_common::config::is_incoming_only()` / `is_outgoing_only()` — `libs/hbb_common/src/config.rs:2774-2790`, both read `HARD_SETTINGS.get("conn-type")` (`"incoming"` / `"outgoing"`).
- `HARD_SETTINGS` — `pub static ref HARD_SETTINGS: RwLock<HashMap<String, String>>` at `libs/hbb_common/src/config.rs:82`. Directly writable from fork code (`HARD_SETTINGS.write().unwrap().insert("conn-type".into(), ...)`) — no upstream API call needed, just populate the map before it's read.
- Enforcement call sites already wired against this map (confirmed present, not something the fork adds):
  - `src/client.rs:255-257` — outbound connect bails if `is_incoming_only()`.
  - `src/rendezvous_mediator.rs:118-122` — inbound listener/rendezvous registration skipped if `is_outgoing_only()`.
  - `src/core_main.rs:645` — additional core-level branch on `is_outgoing_only()`.
  - `src/flutter_ffi.rs:2467-2472` — `is_incoming_only()`/`is_outgoing_only()` exposed to the Flutter UI layer (so a future minimal-UI phase can hide controls, not just block the action).
  - `src/platform/windows.rs:1680,3687,3704` — tray/window behavior branches on `is_outgoing_only()`.
  - `src/ui.rs:291-296,729-730` and `src/ui/index.tis:9-10` — legacy Sciter UI exposure (dead path once building with `feature = "flutter"`, but present).
- **Fork startup hook:** `crate::fork_config::load_and_apply()`, called from `src/core_main.rs:35` (immediately after the existing `crate::load_custom_client();`, inside `pub fn core_main()`, `src/core_main.rs:31-40`). This is the single earliest point in the shared entry path — runs before argument parsing, before the inbound-listener decision, before any outbound-connect capability exists — for every process invocation of the binary, including the self-spawned `--server` child. Mirrors the existing `load_custom_client()` pattern (`src/common.rs:2083-2103,2181-2252`), which populates the same `HARD_SETTINGS` map via a different (signed) input.
- **Scope note:** `core_main()` (`src/core_main.rs:30`) is `#[cfg(not(any(target_os = "android", target_os = "ios")))]` — mobile has a separate entry path this hook does not cover. Not a gap for the current Windows/Linux/macOS desktop scope, but relevant if mobile is ever added.

### Authentication
Use upstream:
- `approve-mode` config option, read by `hbb_common::password_security::approve_mode()` — `libs/hbb_common/src/password_security.rs:77-86`. Exact string values: `"click"` → `ApproveMode::Click`, `"password"` → `ApproveMode::Password`, anything else (including unset/empty) → `ApproveMode::Both`.
- Set via the existing public API `hbb_common::config::Config::set_option(k: String, v: String)` — `libs/hbb_common/src/config.rs:1259-1274`. Setting an empty string removes the key (falls through to the `Both` default) rather than storing an empty value — confirmed at `config.rs:1268-1271`.
- **Note:** `set_option` writes through to persistent storage (`CONFIG2` → `config2.toml`), not just an in-memory override — the fork's authentication-mode mapping persists across restarts the same way a manual Settings-UI change would.
- `verification-method` (temporary vs. permanent password) exists as a separate, untouched option (`password_security.rs:42-50`) — not part of the Phase 3 `authentication.mode` mapping; left at whatever upstream default/administrator setting applies.

### Configuration infrastructure reused (not reinvented)
- `hbb_common::config::Config::path<P>(p: P) -> PathBuf` — `libs/hbb_common/src/config.rs:783-804` — OS-appropriate config directory resolution, reusable for a fork-owned config file without colliding with `config2.toml` (which is keyed off `APP_NAME`).
- `hbb_common::config::load_path<T>`/`store_path<T>` — `libs/hbb_common/src/config.rs:558-591` — generic TOML (via `confy`) load/store for any serde struct. Not used as-is for the fork's own loader (it silently falls back to `T::default()` on any error, missing-file or malformed-file alike, which isn't precise enough for real validation) — but confirms `toml`/`confy` are already resolved in the workspace via `hbb_common`, so adding `toml = "0.7"` directly to the root crate's `Cargo.toml` introduces no new external dependency.

### Connection Workflow (revised 2026-08-28, second revision — formerly "Session Orchestration")
Two independently-flagged buttons, each composed of existing upstream session/message mechanisms only:
- **Desktop** (shown when `desktop_share_enabled`) → one `DEFAULT_CONN` session, only. Standard upstream behavior, unmodified.
- **Support** (shown when `support_enabled`) → always `VIEW_CAMERA` + a Voice Call on it (`session_request_voice_call()`); additionally `DEFAULT_CONN` when `desktop_share_enabled` is also true.

`ConnType::VIEW_CAMERA` and `ConnType::DEFAULT_CONN` are separate upstream connection types (`src/client.rs:2745-2751`) that already support running concurrently to the same peer (distinct `SessionID`s, confirmed in `docs/session-orchestration-analysis.md` §4). Voice Call is a message-level feature layered on an existing session (`VoiceCallRequest`/`VoiceCallResponse`, `session_request_voice_call()`), confirmed in §9-10 to work completely with only a `VIEW_CAMERA` session present — no `DEFAULT_CONN`, no server-side audio-service change, needed at all. Two earlier rounds of planned server-side media changes (giving `VIEW_CAMERA` its own audio subscription) were both investigated and withdrawn as unnecessary.

### Recreating Support mode on a future upstream release
Still UI-layer wiring only — no server/media code to reapply:
1. Confirm `ConnType::VIEW_CAMERA`/`ConnType::DEFAULT_CONN` still run concurrently to the same peer, and that `VoiceCallRequest`/`VoiceCallResponse`/`AudioFrame`/`AudioFormat` remain in `is_view_camera_scoped_message`/`_misc`'s whitelist (`src/server/connection.rs:5508-5546`) — this is the single fact the whole Support design depends on.
2. Confirm `flutter/lib/common.dart`'s `connect()` still accepts `isViewCamera`, and `session_request_voice_call`/`sessionRequestVoiceCall` still exists and still takes only a `session_id`.
3. Re-wire the connection screen's Support button (`support_enabled`) to `connect(..., isViewCamera: true)` + a `sessionRequestVoiceCall` call once that session's `initState()` fires, and conditionally to plain `connect()` when `desktop_share_enabled`. Re-wire the Desktop button (`desktop_share_enabled`) to plain `connect()` only.
4. Confirm the remote side still enforces `enable-camera` at `src/server/connection.rs:2544-2551` — this is what `support_enabled`'s remote-side rejection depends on.
5. Re-verify `docs/FEATURE_ENFORCEMENT_MATRIX.md` — in particular that `support_enabled`'s remote-side enforcement (`enable-camera`) still holds, and that `desktop_share_enabled`'s known UI-only gap hasn't silently become worse (or better — if a future upstream release adds a `DEFAULT_CONN`-rejection permission, that's worth revisiting).
6. Run the regression checklist in `docs/UPSTREAM_UPGRADE_GUIDE.md`.

**Maintenance risk:** low, by design — this workflow deliberately avoids touching server-side media/audio code, so the maintenance burden is limited to the Dart connection-screen widget and the existing FFI call sites it drives, all already stable, documented integration points. The one open item (no existing permission to reject `DEFAULT_CONN` for `desktop_share_enabled=false`) is a documented gap, not a maintenance risk from upstream changes — it simply doesn't exist yet and would need explicit authorization to add.

### Recreating Minimal UI on a future upstream release
Implemented 2026-08-29. Two independent parts, both UI-layer:
1. **Local connect screen** (`flutter/lib/desktop/pages/connection_page.dart`): confirm the file was fully replaced, not merged with upstream's peer-list version, when importing a new upstream release — a naive merge would resurrect `PeerTabPage`/autocomplete/`OnlineStatusWidget`. Re-apply this fork's version (hostname/IP field + Support/Desktop buttons) on top of whatever the new upstream file looks like, re-checking that `connect()`'s signature (`isViewCamera`, etc.) hasn't changed.
2. **Account/Network settings hiding**: confirm `DesktopSettingPage.tabKeys` (`flutter/lib/desktop/pages/desktop_setting_page.dart`) still has its `account`/`network` tab conditionals reading `is_disable_account()`/`kOptionHideNetworkSetting`, and that `HARD_SETTINGS`/`BUILTIN_SETTINGS` are still plain, directly-writable maps. If upstream renames or restructures these, `src/fork_config.rs::apply()`'s unconditional writes need updating to match.
3. **Remote status pane** (`flutter/lib/desktop/pages/desktop_home_page.dart`): confirm `buildLeftPane`'s structure (ID board removed, password board kept, gear icon now unconditional) survived the merge, and that `_ConnectionStatusWidget` (the minimal replacement for upstream's `OnlineStatusWidget`) still compiles against whatever `stateGlobal.svcStatus`/`bind.mainGetConnectStatus()` look like in the new version.
4. Re-verify `docs/FEATURE_ENFORCEMENT_MATRIX.md`'s Account/Network rows and the "Direct-IP-only outbound"/"no relay/rendezvous surfaced" rows.
5. Run the regression checklist in `docs/UPSTREAM_UPGRADE_GUIDE.md`.

**Maintenance risk:** moderate for the connect-screen rewrite specifically (a full-file replacement is more upgrade-fragile than a small patch — upstream restructuring that page requires re-doing the rewrite, not just re-applying a diff), low for the Account/Network hiding (two `HashMap` writes, unlikely to break silently).

### Recreating Direct-IP Enforcement on a future upstream release (ADR-0003, 2026-08-29)
This is the one part of the fork that touches genuine transport-adjacent code, not just configuration/UI reuse — see `docs/ADR-0003-DIRECT-IP-ENFORCEMENT.md` for the full rationale.
1. In `src/rendezvous_mediator.rs::start_all()`, search for `DIRECT-IP FORK` — both edits are bounded by `--- BEGIN/END DIRECT-IP FORK ---` comments. Re-locate the `is_outgoing_only()` guard at the top of the function (the anchor point/pattern both edits mirror) and re-apply: (a) removing the `crate::hbbs_http::sync::start()` call, (b) replacing the per-server registration loop with `loop { sleep(1.).await; }`.
2. Confirm `direct_server(...)` and LAN listening are still spawned as independent tasks *before* the registration loop in the new upstream version — this fix depends on that structural separation continuing to hold.
3. Confirm `Config::get_rendezvous_servers()` (`libs/hbb_common/src/config.rs`) still has the same hardcoded-fallback behavior this ADR was written against (it doesn't have to — the fix works regardless — but if it's changed, note it in the ADR's "Upgrade considerations").
4. Re-apply `Config::set_option("enable-lan-discovery", "N")` in `src/fork_config.rs::apply()` if that function needed to be substantially reworked; confirm `src/lan.rs`'s `enable-lan-discovery` check still gates the ID-bearing response specifically.
5. Re-verify `docs/FEATURE_ENFORCEMENT_MATRIX.md`'s "No relay/rendezvous surfaced"/"LAN-discovery ID exposure" rows and the ADR-0003 regression checklist items in `docs/UPSTREAM_UPGRADE_GUIDE.md`.

**Maintenance risk:** moderate-to-high — this is the fork's only real code deletion in upstream logic (as opposed to configuration/UI reuse), so it's the most likely place a naive merge silently resurrects removed behavior (rendezvous registration coming back). The inline `DIRECT-IP FORK` markers exist specifically to make this discoverable in a diff/merge tool.

## Files Expected To Change Across Upgrades
- UI entry screens
- startup wiring
- config integration points
- `src/rendezvous_mediator.rs`'s rendezvous-registration section (revised 2026-08-29 — previously listed as stable; ADR-0003 made this fork-owned)

## Files Expected To Remain Stable
- transport security
- direct-IP implementation (`direct_server`, `src/client.rs`'s dial path — unaffected by ADR-0003)
- authentication internals

## Build Prerequisites (2026-08-29)

**Environment:**
- Rust: `rustc 1.98+`, `cargo 1.98+` (via `rustup stable`)
- C++ toolchain: MSVC (Windows), Clang (macOS), GCC (Linux)
- CMake: `4.4+`
- vcpkg: manifest-mode or classic-mode
- NASM: `3.02+` (or via vcpkg tools; see `docs/BUILD_BLOCKER_ANALYSIS.md` for NASM compatibility issue)
- Flutter SDK: stable channel

**Known blocker (2026-08-29):**
- aom 3.12.1 requires NASM with multipass optimization support; vcpkg's bundled NASM 3.01 lacks this. **Workaround:** downgrade aom to 3.9.1 (see `docs/BUILD_BLOCKER_ANALYSIS.md`, Strategy 1).
- Manifest-mode vcpkg installs to `<repo>/vcpkg_installed/` but build scripts look for `$VCPKG_ROOT/installed/`. **Workaround:** directory junction linking the two paths (in use; a real fix would set `VCPKG_ROOT` or use `VCPKGRS_TRIPLET`).

**Build commands (after blocker is resolved):**
```bash
# Resolve vcpkg dependencies
vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static libjpeg-turbo:x64-windows-static

# Build Rust binary (release)
cargo build --release

# Build Flutter binary (release, platform-specific)
cd flutter
flutter pub get
flutter build windows --release  # or macos/linux
```

**Expected output:**
- Rust binary: `target/release/rustdesk.exe` (Windows) or equivalent (~50–80 MB)
- Flutter binary: `flutter/build/[windows|macos|linux]/...` (~150–250 MB per platform)
- Build time: 30–60 minutes (cold start); 5–15 minutes (incremental)

## Packaging Prerequisites (2026-08-29)

**Tools:**
- NSIS or MSI builder (Windows .exe installer generation)
- pkgbuild (macOS .dmg generation)
- dh_make or fpm (Linux .deb/.rpm generation)
- Code signing tools (if signing is desired)

**Template files:**
- `RustDesk2.toml` pre-seeded with `direct-ip-*` options (sample/default config for each role —
  see `configs/local.toml`/`configs/remote.toml`)
- Installer scripts/specifications (NSIS script, pkgbuild plist, etc.)
- License/README files

**Packaging flow:**
1. Verify Rust binary (`target/release/rustdesk.exe`)
2. Verify Flutter binary (`flutter/build/.../runner`)
3. Collect runtime files (MSVC redist for Windows, system libs for Linux, etc.)
4. Generate installer/package for each platform
5. Sign and verify checksums

**Expected output:**
- Windows: `rustdesk-local-[version]-x64.exe` (installer or portable ZIP)
- macOS: `rustdesk-local-[version].dmg`
- Linux: `rustdesk-local_[version]_amd64.deb` (Debian/Ubuntu)
- All: checksums.txt (SHA256 hashes)

**See:** `docs/PACKAGING_PLAN.md` for detailed platform-specific instructions.

## Release Prerequisites and Validation (2026-08-29)

**Before shipping:**
1. Complete build readiness verification (full `cargo build`, `cargo test`, `flutter build` succeed).
2. Complete functional verification (all items in `docs/RELEASE_CHECKLIST.md` pass).
3. Verify Direct-IP enforcement (no rendezvous registration, no relay, no LAN discovery ID exposure).
4. Verify support_enabled/desktop_share_enabled controls work (buttons show/hide, remote enforces via enable-camera).
5. Sign packages (optional, recommended for Windows/macOS).
6. Publish release notes (link to `docs/DECISIONS.md`, `docs/architecture.md`, `docs/ADR-0003-DIRECT-IP-ENFORCEMENT.md` for technical context).

**See:** `docs/RELEASE_CHECKLIST.md` for complete functional and regression testing requirements.

## Tracking Build Blockers and Remediation (2026-08-29)

**Current blocker (2026-08-29):**
- aom 3.12.1 + NASM 3.01 multipass optimization compatibility
- **Status:** Documented; Strategy 1 (downgrade to aom 3.9.1) is recommended in `docs/BUILD_BLOCKER_ANALYSIS.md`
- **Next action:** Apply remediation (change `res/vcpkg/aom/portfile.cmake` to default to 3.9.1), re-verify `vcpkg install`, then `cargo build`.

**Future blockers:**
- Monitor upstream RustDesk for new vcpkg dependencies or build-system changes after upgrades
- Re-run `cargo build --release` and `flutter build [platform] --release` as part of upgrade verification
- Update `docs/BUILD_BLOCKER_ANALYSIS.md` with any new issues found

**See:** `docs/BUILD_BLOCKER_ANALYSIS.md` for full root-cause analysis and remediation strategies.

## Automation Deliverables
Generate:
- architecture report (docs/architecture.md)
- compatibility report (docs/FEATURE_ENFORCEMENT_MATRIX.md)
- upgrade report (docs/UPSTREAM_UPGRADE_GUIDE.md)
- build readiness report (docs/BUILD_BLOCKER_ANALYSIS.md)
- packaging plan (docs/PACKAGING_PLAN.md)
- release checklist (docs/RELEASE_CHECKLIST.md)
- test report (CHANGELOG_IMPLEMENTATION.md)

## Future Enhancement
Create a scripted upgrade tool that:
- imports upstream
- checks hook points (via `docs/HOOK_POINTS.md` and source inspection)
- applies fork profile (config mapping, UI conditionals)
- resolves build blockers (vcpkg/aom, or report blocker to user)
- runs verification tests (build, functional, Direct-IP enforcement)
- generates release artifacts (platform-specific binaries/installers)
- publishes release notes (referencing frozen docs, ADRs, changesets)
