# Packaging Plan

**Status:** In planning — awaiting build readiness. **Historical note (2026-09-02): this plan's
two-executable premise (`rustdesk-local`/`rustdesk-remote`) was never the architecture actually
built — the real fork ships ONE executable with role selected by config (see
`docs/ADR-0003-DIRECT-IP-ENFORCEMENT.md` and `docs/architecture.md`). Additionally, every
`fork_config.toml` reference below is now stale: that file was retired and its contents merged
into upstream's own `RustDesk2.toml` (`[options]` table, `direct-ip-*` keys) — see
`docs/CONFIG_REFERENCE.md`. This document is kept for its packaging-tooling ideas (installers,
per-role pre-seeded config) but should not be followed literally for either the executable split
or the config filename.**

**Date:** 2026-08-29

**Scope:** Local Client and Remote Client executables, Flutter UI integration, packaging for Windows/macOS/Linux distribution.

---

## Overview

The Direct-IP RustDesk fork produces two independent executables and their associated Flutter UIs:

1. **Local Client (rustdesk-local)** — outbound-only, direct-IP initiator.
2. **Remote Client (rustdesk-remote)** — inbound-only, direct-IP listener.

Each executable is built from the same Rust source tree (`src/`) with role-specific configuration wired at startup via `fork_config.toml`. The Flutter UI is shared/conditional on role and feature flags (same binary, different initial config).

---

## Local Client Packaging

### Build Artifact

**Windows:**
- **Rust binary:** `target/release/rustdesk.exe` (compiled with `role = local` in fork_config.toml)
- **Size estimate:** ~50–80 MB (depends on optimization level, final link flags)
- **Dependencies:** MSVC runtime (vcredist, if not already installed), Direct-IP listener thread (Tokio async, built-in), no external services

**macOS:**
- **Rust binary:** `target/release/rustdesk` (architecture: x86_64 or ARM64, depending on build machine)
- **Size estimate:** ~60–90 MB
- **Dependencies:** None beyond system libraries

**Linux (glibc):**
- **Rust binary:** `target/release/rustdesk`
- **Size estimate:** ~60–90 MB
- **Dependencies:** glibc 2.31+ (Ubuntu 20.04+, Debian 11+, etc.)

**Linux (musl):**
- **Optional:** Static binary (musl) for maximum portability; larger size (~120 MB+).

### Flutter UI

**All platforms:**
- **Flutter binary:** `flutter/build/windows/runner/Release/rustdesk.exe` (Windows), `flutter/build/macos/Release/rustdesk.app/Contents/MacOS/rustdesk` (macOS), etc.
- **Size estimate:** ~150–250 MB per platform (Flutter+Dart AOT+assets)
- **Integration:** Calls Rust backend via FFI (`src/flutter_ffi.rs`)

### Launcher/Package

**Windows MSI/NSIS installer:**
- Includes rustdesk.exe (Rust binary), Flutter executable, asset files, shortcuts
- Installer size: ~300–500 MB (compressed)
- Post-install size: ~600–800 MB (unpacked)

**macOS .dmg or .app bundle:**
- Includes the .app directory (self-contained on macOS)
- Installer size: ~400–600 MB

**Linux .deb/.rpm:**
- Rust binary in `/usr/bin/rustdesk`, Flutter assets in `/usr/share/rustdesk/`, config in `/etc/rustdesk/`
- Package size: ~400–600 MB (including all architectures)

**Portable ZIP (all platforms):**
- Rust binary + Flutter executable + fork_config.toml (template) in a single ZIP
- No installer, run directly
- Size: ~350–500 MB (compressed)

---

## Remote Client Packaging

**Identical to Local Client** from a packaging perspective:
- Same Rust binary (role determined by fork_config.toml at startup)
- Same Flutter UI (same conditional logic based on role)
- Different pre-configured fork_config.toml (role = remote vs. role = local)

### Deployment Model

**Option A: Single Binary, Multiple Configurations**
- Ship a single `rustdesk.exe` (or equivalent) with multiple pre-configured `fork_config.toml` files.
- Installation scripts/docs explain how to set `role = local` or `role = remote` in the config file.
- **Advantage:** Minimal duplication, simpler release process.
- **Disadvantage:** Requires user to edit config or use a separate installer script for each role.

**Option B: Separate Executables (Named)**
- Build and package `rustdesk-local.exe` and `rustdesk-remote.exe` (same binary, different config bundled).
- Each comes with pre-set fork_config.toml.
- **Advantage:** Clear role separation, no user configuration needed.
- **Disadvantage:** Doubles binary artifact size and installer complexity.

**Recommendation:** **Option A for initial release** (simpler, matches upstream RustDesk's single-binary model with config variation). Revisit for Option B if user confusion arises.

---

## Windows Packaging Path (Detailed)

### Prerequisites

- Rust toolchain: `rustc 1.98+`, `cargo 1.98+`
- CMake 4.4+, NASM 3.02+ (or via vcpkg)
- vcpkg: `C:\Users\[user]\vcpkg` or similar
- Flutter SDK: stable channel, `fluter/bin/flutter` in PATH
- Visual Studio 2022 (MSVC compiler for C++ dependencies)
- NSIS or MSI build tools (for installer generation)

### Build Steps

1. **Resolve vcpkg dependencies:**
   ```bash
   cd C:\Work\RustDesk
   vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static libjpeg-turbo:x64-windows-static
   ```

2. **Build Rust binary (release):**
   ```bash
   cargo build --release
   # Output: target/release/rustdesk.exe (~50–80 MB)
   ```

3. **Build Flutter binary (release):**
   ```bash
   cd flutter
   flutter pub get
   flutter build windows --release
   # Output: build/windows/runner/Release/rustdesk.exe (Flutter app)
   ```

4. **Package installer:**
   - Combine Rust binary, Flutter executable, assets, and fork_config.toml into an installer (NSIS or MSI).
   - Or create a ZIP with the same files.

### Output Artifacts

- **rustdesk-local-[version]-x64.exe** (installer or portable ZIP)
- **rustdesk-remote-[version]-x64.exe** (same binary, different fork_config.toml included)
- **rustdesk-[version]-x64.zip** (portable, no installer)

**Total output directory:** `release/windows/`

---

## Flutter Packaging Path (Detailed)

### Flutter-Specific Considerations

1. **Asset bundling:** `flutter/assets/` are embedded in the Flutter binary; no separate asset download needed.
2. **Localization:** Flutter handles i18n via `flutter/lib/l10n/` (ARB files); compiled into the binary.
3. **Theming:** Light/dark mode CSS is in the Dart code; no separate theme files.

### Build Variants

**Debug:**
```bash
flutter build [windows|macos|linux] --debug
# Faster builds, larger binaries, unoptimized (for development)
```

**Release:**
```bash
flutter build [windows|macos|linux] --release
# Slower builds, optimized for distribution, production-ready
```

**Profile (optional):**
```bash
flutter build [windows|macos|linux] --profile
# Optimized like release, but with profiling enabled (rarely used for end-user releases)
```

### Expected File Structure (after `flutter build`)

**Windows:**
```
flutter/build/windows/runner/Release/
├── rustdesk.exe                    (Flutter app entry point)
├── flutter_windows.dll              (Flutter engine)
├── rustdesk_plugin.dll              (Rust FFI plugin)
├── assets/                          (bundled assets)
└── ... (MSVC runtime, etc.)
```

**macOS:**
```
flutter/build/macos/Release/
└── rustdesk.app/
    ├── Contents/
    │   ├── MacOS/rustdesk          (Flutter app entry point)
    │   ├── Frameworks/              (Flutter engine, plugins)
    │   ├── Resources/               (assets, localization)
    │   └── Info.plist               (app metadata)
```

**Linux:**
```
flutter/build/linux/release/bundle/
├── rustdesk                         (Flutter app entry point)
├── lib/                             (shared libraries)
└── data/                            (assets, localization)
```

---

## Required Runtime Files

### Rust Binary Dependencies

- **Windows:** MSVC runtime (vcredist_x64.exe, typically pre-installed on modern Windows)
- **macOS:** None (system libraries are sufficient)
- **Linux:** glibc 2.31+, standard system libraries (libdl, libpthread, etc. — all on standard distros)

### Fork Configuration File

**fork_config.toml** (required, example):
```toml
version = 1
role = "local"

support_enabled = true
desktop_share_enabled = true

listen_address = "0.0.0.0"
listen_port = 21118

video_quality = "medium"
audio_quality = "medium"

log_level = "info"

[authentication]
mode = "ask"
```

**Location:** 
- **Windows:** `%APPDATA%\RustDesk\fork_config.toml` or bundled in the installer as default
- **macOS:** `~/.rustdesk/fork_config.toml` or bundled in the .app
- **Linux:** `~/.config/rustdesk/fork_config.toml` or `/etc/rustdesk/fork_config.toml` (system-wide)

### Voice Call Audio Files (Optional)

- Ring/notification sounds (if custom audio is desired; upstream uses system defaults)
- Location: `flutter/assets/audio/` (bundled in Flutter binary)

---

## Expected Output Directories

### Build Outputs

**Rust:** `target/release/` contains the compiled binary and intermediate artifacts (~5 GB total for the debug/build directories; release binary ~50–80 MB).

**Flutter:** `flutter/build/[windows|macos|linux]/` contains the platform-specific Flutter binary and all dependencies (~500 MB–1 GB per platform during build; packaged ~150–250 MB per platform).

### Packaging Outputs

**Recommended structure:**
```
release/
├── windows/
│   ├── rustdesk-local-1.0.0-x64.exe        (NSIS or MSI installer)
│   ├── rustdesk-remote-1.0.0-x64.exe       (same, pre-configured for role=remote)
│   └── fork_config.toml (sample)
├── macos/
│   ├── rustdesk-local-1.0.0.dmg
│   ├── rustdesk-remote-1.0.0.dmg
│   └── fork_config.toml (sample)
├── linux/
│   ├── rustdesk-local_1.0.0_amd64.deb
│   ├── rustdesk-remote_1.0.0_amd64.deb
│   └── fork_config.toml (sample)
└── checksums.txt                            (SHA256 hashes of all artifacts)
```

### Size Estimates (Final Artifacts)

| Artifact | Size (compressed) | Size (unpacked) |
|---|---|---|
| Windows MSI installer | 350–500 MB | 600–800 MB |
| macOS .dmg | 400–600 MB | 1–1.5 GB |
| Linux .deb | 400–600 MB | 600–900 MB |
| Portable ZIP (any platform) | 350–500 MB | 600–800 MB |

---

## Post-Installation Configuration

### User Setup

**Windows:**
1. Run installer (or extract ZIP).
2. Launch rustdesk.exe.
3. On first run, fork_config.toml is read from the default location (or a template is provided).
4. If fork_config.toml is missing, the app fails with an error pointing the user to the sample file.

**macOS/Linux:**
1. Extract .dmg/.deb or run installer.
2. Create/edit ~/.rustdesk/fork_config.toml (or /etc/rustdesk/ for system-wide).
3. Launch the app.

### Configuration Validation

- `src/fork_config.rs::load_and_apply()` validates fork_config.toml at startup.
- If validation fails, the app logs the error and exits (no silent fallback).
- User must fix the config and restart.

---

## Testing the Package (Pre-Release)

1. **Extract/Install:** Verify the installer unpacks all files without corruption.
2. **Fork config loading:** Verify fork_config.toml is read and parsed correctly.
3. **Role enforcement:** Verify local role cannot accept inbound, remote role cannot initiate outbound.
4. **UI initialization:** Verify the Flutter UI launches and displays the connection screen.
5. **Configuration visibility:** Verify support_enabled and desktop_share_enabled flags correctly show/hide buttons.
6. **Direct-IP listener:** Verify listening on the configured address:port (use `netstat` or similar).
7. **No rendezvous:** Verify no outbound connections to RustDesk's default rendezvous servers (monitor network traffic).

---

## Release Checklist Integration

Refer to `docs/RELEASE_CHECKLIST.md` for functional verification of:
- Support mode (camera + voice call)
- Desktop mode (standard upstream capabilities)
- Authentication modes (ask, password, ask_and_password)
- Direct-IP enforcement (no relay, no rendezvous, no LAN discovery exposure)

---

## Known Limitations

1. **Build blocker:** Full package generation is blocked until the vcpkg/aom/NASM blocker is resolved (see `docs/BUILD_BLOCKER_ANALYSIS.md`).
2. **No auto-update mechanism:** Upstream RustDesk's auto-updater is not included in this fork (intentionally minimal).
3. **Single configuration per role:** Each package (local or remote) comes with one pre-set fork_config.toml; custom configuration requires manual file editing.
4. **No installers for non-Windows yet:** macOS (.dmg) and Linux (.deb/.rpm) packaging scripts are not yet written; manual packaging is required for those platforms.

---

## Next Steps

1. Resolve the build blocker (aom/NASM issue).
2. Verify `cargo build --release` produces a working binary.
3. Verify `flutter build [platform] --release` produces a working Flutter binary.
4. Create platform-specific packaging scripts (NSIS for Windows, pkgbuild for macOS, dh_make for Linux).
5. Generate the first release artifacts and validate against the Release Checklist.
