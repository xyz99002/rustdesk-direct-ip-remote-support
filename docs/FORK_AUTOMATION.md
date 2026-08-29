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
Configuration should define:
- role
- authentication mode
- camera enablement
- audio enablement
- desktop enablement

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

### Session Orchestration
Single user action starts:
- camera
- audio
- optional desktop

Not yet implemented (Phase 3 covers configuration/role only) — `ConnType::VIEW_CAMERA` and `ConnType::DEFAULT_CONN` (desktop) are mutually exclusive upstream connection types (`src/client.rs:2745-2751`), so this will require opening two connections behind one UI action, not a new combined `ConnType`. See `docs/upstream-analysis.md` §4.

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
