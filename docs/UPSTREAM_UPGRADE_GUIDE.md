# UPSTREAM_UPGRADE_GUIDE.md

# Purpose
This document describes how to upgrade the Direct-IP RustDesk fork to a newer upstream RustDesk release while preserving the fork behavior.

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

Mappings:
- ask -> click
- password -> password
- ask_and_password -> both/default

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

### Minimal UI (implemented 2026-08-29)
Verify:
- `flutter/lib/desktop/pages/connection_page.dart` still has no peer list, autocomplete, ID-lookup, or public-server messaging after merging a new upstream release — this file was fully rewritten, so a naive merge/patch is the most likely thing to silently resurrect removed UI.
- `DesktopSettingPage.tabKeys` (`flutter/lib/desktop/pages/desktop_setting_page.dart`) still conditionally excludes `account`/`network` based on `is_disable_account()`/`kOptionHideNetworkSetting`, and `HARD_SETTINGS`/`BUILTIN_SETTINGS` are still plain `pub static` maps `fork_config.rs::apply()` can write directly.
- `flutter/lib/desktop/pages/desktop_home_page.dart`'s remote status pane still has no ID board, still shows password management (`buildPasswordBoard2`) and connection status (`_ConnectionStatusWidget`), and the Settings gear icon is still shown for both roles (not just `isOutgoingOnly`).
- `server_page.dart`'s `ConnectionManager`/`_CmHeader`/`_PrivilegeBoard` (connection manager, Voice Call accept/reject) remain untouched — this phase deliberately did not modify them.

### Direct-IP Enforcement (implemented 2026-08-29, ADR-0003)
Verify:
- `src/rendezvous_mediator.rs::start_all()` still has both `--- BEGIN/END DIRECT-IP FORK ---` blocks: the `hbbs_http::sync::start()` call removed, and the registration loop replaced with `loop { sleep(1.).await; }`.
- No path outside this function calls `RendezvousMediator::start()`/`start_udp()`/`start_tcp()`/`register_pk()`/`register_peer()` directly (re-run `grep -rn "RendezvousMediator::start\(" src/` and confirm the only match is inside `start_all()` itself, now unreachable).
- `direct_server(...)` and LAN listening are still spawned as independent tasks *before* the removed loop, and both still start successfully for `role=remote`.
- `Config::set_option("enable-lan-discovery", "N")` is still present in `fork_config.rs::apply()`, and `src/lan.rs`'s ping-response handler still gates the ID-bearing `pong` on that exact option.
- A `role=remote` instance, monitored at the network level, sends **no** outbound UDP/TCP traffic to any rendezvous server address, and does not respond to a LAN-broadcast discovery ping with its ID.
- `RendezvousMediator::restart()`'s call sites (`flutter_ffi.rs`, `ipc.rs`, `ui_interface.rs`) still compile — the function itself is intentionally unmodified even though its effect is now inert.

## Newly Discovered Upgrade Risks (found during Phase 3 implementation)

- **Startup call-order dependency.** The fork's config loader hooks in at `src/core_main.rs:35`, immediately after the existing `crate::load_custom_client();` call inside `pub fn core_main()`, and relies on running before argument parsing and before the inbound-listener/outbound-connect decision. If a future upstream release reorders `core_main()` — e.g. moves argument parsing or server-spawn logic earlier — the fork's role/auth mapping could apply too late (after the listener already started, or after an outbound connect was already permitted). **Upgrade check:** confirm `load_custom_client()` (or its replacement) still runs before all branching in `core_main()`, and re-anchor the fork hook to the same relative position.
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
- Settings page has no Account or Network tab, on both local and remote builds.
- Connection-manager accept/reject dialogs (including Voice Call's) still appear and function normally.

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
   - Verify the `direct-ip-*` key set matches `src/fork_config.rs`'s `ForkConfig` struct.
   - Provide both local and remote examples (`configs/example-local.toml`/`example-remote.toml`).

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
