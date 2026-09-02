# Release Checklist

**Status:** Pre-release validation template.

**Date:** 2026-08-29

**Scope:** Functional verification for the Direct-IP RustDesk fork before release.

---

## Build Verification

### Rust Compilation

- [ ] **Full Rust build succeeds**
  - Command: `cargo build --release`
  - Expected: `target/release/rustdesk.exe` (Windows) or equivalent for target platform
  - Size: 50–80 MB
  - Time: 10–30 minutes (depending on clean vs. incremental build)
  - Blocker status: Check `docs/BUILD_BLOCKER_ANALYSIS.md`

- [ ] **Rust tests pass**
  - Command: `cargo test -- --test-threads=1`
  - Expected: All tests pass, including `src/fork_config.rs`
  - Note: Full vcpkg dependencies required; may be skipped if aom/NASM blocker prevents full compilation

- [ ] **Rust clippy warnings cleaned**
  - Command: `cargo clippy --all-targets --all-features`
  - Expected: No clippy::* warnings

- [ ] **Rust formatting valid**
  - Command: `cargo fmt --check`
  - Expected: No formatting violations

### Flutter Compilation

- [ ] **Flutter dependencies resolved**
  - Command: `cd flutter && flutter pub get`
  - Expected: All packages downloaded without errors
  - Time: 2–5 minutes (first run); cached thereafter

- [ ] **Dart analysis passes**
  - Command: `cd flutter && dart analyze --fatal-infos`
  - Expected: No errors, no fatal warnings

- [ ] **Flutter build succeeds (release)**
  - Command: `cd flutter && flutter build windows --release` (or target platform)
  - Expected: `flutter/build/windows/runner/Release/rustdesk.exe` (~150–250 MB)
  - Time: 5–15 minutes (incremental; first build ~20–30 minutes)

### Packaging Verification

- [ ] **Installer/package generated**
  - Command: (NSIS/dh_make/pkgbuild, platform-specific)
  - Expected: `.exe`, `.deb`, `.dmg`, or `.zip` artifact in `release/` directory

- [ ] **Artifact integrity checked**
  - Command: `sha256sum` or equivalent
  - Expected: Hashes match a pre-computed checksums file
  - Rationale: Catch corruption during packaging or distribution

---

## Functional Verification — Configuration

### `direct-ip-*` Options Parsing (RustDesk2.toml — formerly a separate `fork_config.toml`, see docs/CONFIG_REFERENCE.md)

- [ ] **Valid config loads without error**
  - Test: Place `direct-ip-*` options with correct schema in `RustDesk2.toml`'s `[options]` table
  - Expected: App starts, logs "fork_config: applied role=... auth_mode=..."

- [ ] **Invalid config is rejected**
  - Test: Set `direct-ip-role` to an invalid value (e.g., `"admin"`)
  - Expected: `log::error!` referencing the invalid field; falls back to upstream default behavior (does not exit)

- [ ] **Missing required field is rejected**
  - Test: Remove `direct-ip-config-version` from the `[options]` table
  - Expected: `log::error!` about missing `direct-ip-config-version`; falls back to upstream default behavior

- [ ] **Unsupported version is rejected**
  - Test: Set `version = 2` (or higher)
  - Expected: App exits with error message about unsupported version

### Role Enforcement

#### Local Role (`role = "local"`)

- [ ] **Outbound-only enforcement**
  - Test: Run local instance, attempt to open a session *from* another peer to this instance
  - Expected: Connection rejected (listener not started), or no inbound-listen port visible via `netstat`
  - Verification: `netstat -an | grep LISTEN` should **not** show port 21118 (or configured port) listening

- [ ] **Listener never starts**
  - Test: Monitor application startup; check system network state
  - Expected: No TCP listener on the configured listen_address:listen_port
  - Confidence check: `netstat -an | grep "0.0.0.0:21118"` returns nothing

#### Remote Role (`role = "remote"`)

- [ ] **Inbound-only enforcement**
  - Test: Run remote instance, attempt to connect *from* this instance to another peer
  - Expected: Connection fails (no outbound initiator), or logs "is_incoming_only: true, skipping outbound"

- [ ] **Listener starts**
  - Test: Monitor application startup; check system network state
  - Expected: TCP listener active on configured listen_address:listen_port
  - Verification: `netstat -an | grep LISTENING` shows a port in the LISTENING state matching config

---

## Functional Verification — Connection Modes

### Direct-IP Connectivity

- [ ] **Hostname connect (local to remote)**
  - Test: Local instance, enter remote's hostname in the connection field (e.g., "remote-machine" or "192.168.1.100")
  - Expected: Connection established, remote accepts the session
  - Verify: Session manager shows the connection in the remote's UI; video/audio/control work

- [ ] **IP address connect (local to remote)**
  - Test: Local instance, enter remote's IP address (e.g., "192.168.1.50")
  - Expected: Connection established
  - Verify: Same as hostname connect

- [ ] **No rendezvous registration**
  - Test: Capture network traffic on the remote instance during startup and while idle
  - Expected: No outbound traffic to RustDesk rendezvous servers (e.g., no connections to `*.aomemedia.org` or similar)
  - Tool: Wireshark, tcpdump, netstat, or equivalent
  - Reference: `docs/ADR-0003-DIRECT-IP-ENFORCEMENT.md` lists the removed registration calls

- [ ] **No relay participation**
  - Test: Run both instances on different networks (WAN simulation if needed); monitor outbound connections
  - Expected: If direct connection fails, no fallback to relay (connection simply fails)
  - Rationale: Relay handler code is dead (unreachable from the removed registration loop)

- [ ] **LAN broadcast discovery disabled**
  - Test: On the remote instance, send a LAN broadcast ping (e.g., `ping -b 192.168.1.255`)
  - Expected: Remote does not respond with ID/hostname in the response
  - Alternative: Capture network traffic; no `pong` frames containing the remote's RustDesk ID
  - Verification: `Config::get_option("enable-lan-discovery")` should be "N"

### Authentication Modes

#### `authentication.mode = "ask"` (Click-to-approve)

- [ ] **Local initiates, remote prompts for approval**
  - Test: Local connects, remote receives an inbound session request dialog
  - Expected: Remote sees a dialog "Accept [Yes/No]?" with no password field
  - Action: Click Yes
  - Expected: Session establishes, control and video flow

- [ ] **Remote rejects (No button)**
  - Test: Same as above, but click No
  - Expected: Session rejected, connection closes, local sees "rejected by user" or similar

#### `authentication.mode = "password"` (Permanent or Temporary Password)

- [ ] **Local must provide password**
  - Test: Local connects, prompted for password
  - Expected: Connection prompt shows password field (filled from upstream's configured password or one-time code)

- [ ] **Invalid password rejected**
  - Test: Local enters wrong password
  - Expected: Session rejected after ~1 second, error message "invalid password"

- [ ] **Valid password accepted**
  - Test: Local enters correct password
  - Expected: Session establishes

#### `authentication.mode = "ask_and_password"` (Either click-approve OR password)

- [ ] **Click-approve works**
  - Test: Local connects, remote sees "Accept [Yes/No]?" dialog (same as ask mode)
  - Expected: Click Yes → session establishes

- [ ] **Password works**
  - Test: Remote ignores the approve dialog, or local provides password directly
  - Expected: Session establishes with valid password

---

## Functional Verification — Support Mode

### Configuration

- [ ] **support_enabled = true renders the Support button**
  - Test: Local instance with `direct-ip-support-enabled = "Y"` in `RustDesk2.toml`
  - Expected: "Support" button visible on connection screen

- [ ] **support_enabled = false hides the Support button**
  - Test: Local instance with `support_enabled = false`
  - Expected: "Support" button not rendered (connection screen only shows Desktop button, if enabled)

### Support Session Establishment

- [ ] **Support initiates VIEW_CAMERA session**
  - Test: Local clicks Support button, remote accepts
  - Expected: Remote's camera stream visible on local screen (remote's webcam feed, not desktop)
  - Verification: Frame rate, color, and image quality match remote's configured `video_quality`

- [ ] **Voice Call initiates on VIEW_CAMERA**
  - Test: After VIEW_CAMERA establishes, remote accepts Voice Call request
  - Expected: Bi-directional audio (microphone input from local, speaker output on remote; vice versa)
  - Verification: Audio quality matches configured `audio_quality`

- [ ] **Voice Call does NOT require DEFAULT_CONN**
  - Test: Support mode with `desktop_share_enabled = false`
  - Expected: Voice Call still works on VIEW_CAMERA alone; no DEFAULT_CONN session present
  - Verification: Session manager shows only one connection (VIEW_CAMERA), no desktop-control connection

- [ ] **Remote rejects Support when support_enabled = false**
  - Test: Remote instance with `support_enabled = false`; local attempts a Support connection
  - Expected: Connection rejected with "camera disabled" or similar message
  - Verification: Remote's `Config::get_option("enable-camera")` is "N"

---

## Functional Verification — Desktop Mode

### Configuration

- [ ] **desktop_share_enabled = true renders the Desktop button**
  - Test: Local instance with `desktop_share_enabled = true`
  - Expected: "Desktop" button visible on connection screen

- [ ] **desktop_share_enabled = false hides the Desktop button**
  - Test: Local instance with `desktop_share_enabled = false`
  - Expected: "Desktop" button not rendered

### Desktop Session Establishment

- [ ] **Desktop initiates DEFAULT_CONN session**
  - Test: Local clicks Desktop button
  - Expected: Remote's desktop visible on local screen (cursor, windows, taskbar, etc.)

- [ ] **Desktop has no camera, no voice call**
  - Test: Desktop session active
  - Expected: No camera feed in the connection screen; no Voice Call button/request
  - Verify: Session manager shows DEFAULT_CONN, not VIEW_CAMERA

- [ ] **Standard RustDesk capabilities work**
  - Keyboard input: Type in remote, text appears
  - Mouse control: Move/click on local, remote cursor moves/responds
  - Clipboard: Copy on local, paste on remote; vice versa
  - File transfer: Drag file from local, remote receives
  - Audio (if enabled): Remote system sounds heard on local

- [ ] **Support + Desktop together**
  - Test: Both `support_enabled = true` and `desktop_share_enabled = true`; local clicks Support
  - Expected: VIEW_CAMERA + Voice Call + DEFAULT_CONN all establish (three sessions, or VIEW_CAMERA + Voice Call on one, DEFAULT_CONN on another)
  - Verification: Session manager shows both connections

### Desktop-Only Mode

- [ ] **Only Desktop button available**
  - Test: `support_enabled = false`, `desktop_share_enabled = true`
  - Expected: Only Desktop button rendered; Support button absent

---

## Functional Verification — Direct-IP Enforcement

### Rendezvous Registration Verification

- [ ] **No registration happens at startup**
  - Test: Run remote instance; capture outbound traffic for first 10 seconds
  - Expected: No outbound connections to RustDesk public servers
  - Known servers to check: `relay-*.aomemedia.org`, `rendezvous.aomemedia.org`, or any configured custom rendezvous
  - Verification: Wireshark/tcpdump shows zero UDP/TCP connections to those IPs

- [ ] **No periodic sync with rendezvous**
  - Test: Remote instance running idle for 5 minutes; monitor network
  - Expected: No periodic outbound connections to rendezvous servers
  - Rationale: `hbbs_http::sync::start()` call is removed; without it, no periodic heartbeat exists

### Relay Participation Verification

- [ ] **No relay request handling**
  - Test: Attempt a relay connection from a third-party RustDesk client (using relay mode)
  - Expected: Connection fails (relay handler is unreachable from the removed loop)
  - Caveat: This test is difficult in practice; more easily verified by code review (see HOOK_POINTS.md)

### LAN Discovery Verification

- [ ] **LAN broadcast ID exposure closed**
  - Test: Send LAN broadcast discovery ping; capture response
  - Expected: Remote either doesn't respond, or responds without ID/hostname/username/MAC
  - Tool: Custom script or tcpdump filter on broadcast/multicast ports
  - Verification: Inspect the config at `Config::get_option("enable-lan-discovery")` → should be "N"

---

## Functional Verification — UI

### Local Connection Screen

- [ ] **Hostname/IP field only**
  - Test: Local instance connection screen
  - Expected: Single text field for entering hostname or IP
  - Verify: No peer list, no autocomplete dropdown, no "Use ID?" option

- [ ] **Support and/or Desktop buttons**
  - Test: Based on `support_enabled` and `desktop_share_enabled` flags
  - Expected: Buttons rendered or absent as configured
  - Verify: Both false → rejection at load time (error message)

### Remote Status Pane

- [ ] **No ID display**
  - Test: Remote instance status pane (bottom-left area typically showing ID)
  - Expected: ID board removed; no RustDesk ID shown
  - Verify: Code: `desktop_home_page.dart` has no `buildIDBoard()` call

- [ ] **Password management still visible**
  - Test: Remote status pane
  - Expected: Password field(s) visible for setting/changing one-time password
  - Verify: `buildPasswordBoard2()` is still called

- [ ] **Connection status visible**
  - Test: Remote status pane
  - Expected: Active connections list, session manager shown
  - Verify: `_ConnectionStatusWidget` rendered

### Settings Access

- [ ] **Settings gear icon always accessible**
  - Test: Both local and remote instances
  - Expected: Gear icon in UI (typically top-right or bottom-left)
  - Verify: Unconditional rendering (not gated on role)

- [ ] **No Account tab**
  - Test: Click Settings, view tabs
  - Expected: Account tab absent
  - Verify: `DesktopSettingPage.tabKeys` excludes "account"

- [ ] **No Network tab**
  - Test: Click Settings, view tabs
  - Expected: Network (relay/rendezvous config) tab absent
  - Verify: `hide-network-settings` option is set; tab not rendered

---

## Regression Testing

### Upstream Compatibility

- [ ] **Existing RustDesk features not removed**
  - Test: Audio, video, keyboard, mouse, clipboard, file transfer (on Desktop mode) all work
  - Expected: Full feature parity with upstream RustDesk 1.4.9 for the supported session types
  - Caveat: AV1 codec may be unavailable (vcpkg/aom blocker); fallback to VP9/H.265 should work

- [ ] **No crashes on invalid input**
  - Test: Malformed `direct-ip-*` values, missing fields, invalid IPs, etc.
  - Expected: Graceful error messages, no segfault or panic
  - Verify: Check application logs

- [ ] **Concurrent sessions work**
  - Test: On remote, accept multiple Desktop + Support connections from different local instances
  - Expected: All sessions remain active and responsive
  - Rationale: Rust's async/Tokio concurrency should handle this; regression catch for any blocking code

### Fork Configuration Integrity

- [ ] **Config persists across restarts**
  - Test: Set `direct-ip-support-enabled = "Y"` in `RustDesk2.toml`, restart app, check it's still true
  - Expected: Confirmed via logs or UI state

- [ ] **Invalid config is not silently modified**
  - Test: Invalid `direct-ip-*` value in `RustDesk2.toml`; restart app
  - Expected: Error logged on startup; file unchanged (not auto-corrected or repaired)

---

## Performance Verification (Optional, For Optimization)

- [ ] **Startup time < 5 seconds** (cold start)
  - Measure: From app launch to connection screen visible
  - Expected: < 5 seconds on typical Windows 10/11 machine

- [ ] **Connection establish time < 3 seconds** (direct-IP, local to remote on LAN)
  - Measure: Click button → video frame received
  - Expected: < 3 seconds on LAN

- [ ] **Memory footprint < 200 MB** (idle, remote instance)
  - Measure: RSS in task manager
  - Expected: < 200 MB for the Rust + Flutter binary combined

---

## Approval Gates

### Build Readiness

**Gate:** All "Build Verification" checks pass.

**Blocker:** If vcpkg/aom issue prevents Rust build, proceed to packaging plan review but defer binary testing.

### Functional Readiness

**Gate:** All "Functional Verification" checks pass (Configuration, Connections, Support, Desktop, Direct-IP, UI).

**Critical failures (blocker for release):**
- Rendezvous registration still happening (ADR-0003 not effective)
- Voice Call fails on VIEW_CAMERA when DEFAULT_CONN is absent (contradicts the investigation)
- LAN discovery still exposes ID
- Support or Desktop buttons not hiding correctly

**Non-critical failures (defer, document as known issue):**
- AV1 codec unavailable (upstream fallback to VP9/H.265)
- Performance not meeting optional targets

### Regression Readiness

**Gate:** All "Regression Testing" checks pass.

**Critical failures (blocker for release):**
- Upstream features broken (keyboard, mouse, audio, etc.)
- Application crashes on invalid input
- Concurrent sessions deadlock or conflict

---

## Sign-Off

- **Testing completed by:** [Name/Role]
- **Date:** [YYYY-MM-DD]
- **Environment:** [OS/Version, Rust version, Flutter version, network setup]
- **Issues found:** [List, or "None"]
- **Recommendation:** [Approve for release / Defer pending fixes / Escalate]

---

## Appendix: Test Environment Setup

### Windows 10/11

- Install RustDesk fork (release binary or portable ZIP)
- Create/edit `%APPDATA%\RustDesk\config\RustDesk2.toml` with test `direct-ip-*` options
- For network testing: use a secondary machine or VM as the remote instance

### Virtual Network Simulation

- **LAN testing:** Use VirtualBox/Hyper-V with bridged network adapters
- **WAN/relay testing:** (Not applicable for this fork; relay is disabled)
- **Direct-IP verification:** Monitor traffic with Wireshark on the host machine

### Monitoring Tools

- **Network:** Wireshark, netstat, tcpdump
- **Processes:** Task Manager, Process Monitor
- **Logs:** Application logs (written to %APPDATA%\RustDesk\log\ or similar, configurable via `log_level`)
