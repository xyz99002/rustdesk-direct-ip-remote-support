# FORK_AUTOMATION.md

# Goal
Create a repeatable process that transforms an upstream RustDesk release into the Direct-IP fork with minimal manual effort.

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

Configuration should define (revised 2026-08-28 — `support_enabled` replaces `camera enablement`/`audio enablement`/`desktop enablement`, which have no referent under the Support/Desktop button model):
- role
- authentication mode
- support_enabled (gates Support button visibility)

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

### Connection Workflow (revised 2026-08-28 — formerly "Session Orchestration")
Two buttons, each a single existing upstream session mechanism — not a combined action:
- **Desktop** → one `DEFAULT_CONN` session. Standard upstream behavior, unmodified.
- **Support** → one `DEFAULT_CONN` + one `VIEW_CAMERA` session, opened together, gated on `support_enabled`.

`ConnType::VIEW_CAMERA` and `ConnType::DEFAULT_CONN` are separate upstream connection types (`src/client.rs:2745-2751`) that already support running concurrently to the same peer (distinct `SessionID`s, confirmed in `docs/session-orchestration-analysis.md` §4) — so Support is simply "open both," not a new combined `ConnType`. **No server-side media/audio change is needed**: an earlier design (single "Start Session" launching camera+audio together) would have required a small `src/server.rs`/`src/server/connection.rs` change to give `VIEW_CAMERA` sessions audio (investigated in `docs/session-orchestration-analysis.md` §6-7 — the capability already exists upstream but is never triggered by the current client); the Support/Desktop model avoids needing that entirely because `DEFAULT_CONN` already carries audio.

### Recreating Support mode on a future upstream release
This is UI-layer wiring only — no server/media code to reapply:
1. Confirm `ConnType::VIEW_CAMERA` and `ConnType::DEFAULT_CONN` still exist and can still run as concurrent, independent sessions to the same peer (`docs/HOOK_POINTS.md` "Connection Workflow" rows).
2. Confirm `flutter/lib/common.dart`'s `connect()` (or its renamed/moved equivalent) still accepts an `isViewCamera` flag and still dispatches to the camera vs. desktop window/page paths independently.
3. Re-wire the connection screen's Desktop button to the plain `connect()` call, and the Support button (rendered only when `support_enabled = true`) to both the plain and `isViewCamera: true` calls, for the same target host.
4. Confirm audio still flows automatically for `DEFAULT_CONN` (i.e. `try_sub_monitor_services`/`audio_enabled()` in `src/server/connection.rs` still exists with the same gating) — if upstream changes this, Support's audio depends on it exactly as much as a stock desktop connection does, no more.
5. Run the regression checklist in `docs/UPSTREAM_UPGRADE_GUIDE.md`.

**Maintenance risk:** low, by design — this workflow deliberately avoids touching server-side media/audio code, so the maintenance burden is limited to the Dart connection-screen widget and the two existing FFI call sites it drives, both already stable, documented integration points.

## Files Expected To Change Across Upgrades
- UI entry screens
- startup wiring
- config integration points

## Files Expected To Remain Stable
- transport security
- direct-IP implementation
- authentication internals

## Automation Deliverables
Generate:
- architecture report
- compatibility report
- upgrade report
- test report
- packaging report

## Future Enhancement
Create a scripted upgrade tool that:
- imports upstream
- checks hook points
- applies fork profile
- runs verification tests
- generates release artifacts
