# GitHub Actions CI Strategy for Direct-IP RustDesk Fork

**2026-09-01 status update:** this strategy is no longer just proposed — it is implemented and verified. See `docs/CI_WORKFLOW_AUDIT_2026-09-01.md` for the working workflows (`direct-ip-build.yml`, `flutter-ci.yml`, `flutter-nightly.yml`, `release.yml`, `vcpkg-cache-warmer.yml`), their verified behavior, and known issues; see `docs/LOCAL_BUILD_DECOMMISSION_PLAN.md` for the corresponding move away from local build tooling now that this strategy is live.

## Overview

This document outlines the CI/CD strategy for the Direct-IP RustDesk fork, based on analysis of upstream workflows and the fork's specific build requirements.

## Analysis of Upstream Workflows

### flutter-build.yml (Windows/macOS/iOS/Android)
- **vcpkg Setup**: Uses `lukka/run-vcpkg` action with GitHub Actions binary cache
- **vcpkg Configuration**: Manifest mode with `vcpkg.json` defining dependencies
- **vcpkg Commit**: Pinned to `120deac3062162151622ca4860575a33844ba10b` (2025.08.27 baseline)
- **NASM Resolution**:
  - **Linux**: Installed via `sudo apt-get install -y nasm`
  - **macOS**: Explicitly downloaded (version 2.16.03) to ensure multipass support
  - **Windows**: Handled by vcpkg's `vcpkg_find_acquire_program(NASM)`
- **Triplets**: x64-windows-static, arm64-windows-static, etc.
- **Rust Versions**: Sciter 1.75, macOS 1.81, Flutter build varies by target

### ci.yml (Linux-only)
- **Simple approach**: Minimal dependencies
- **NASM**: Installed via `sudo apt-get install -y nasm`
- **vcpkg**: Manifest mode, same setup as flutter-build.yml
- **Rust**: Uses stable toolchain
- **Build**: Direct `cargo build` and `cargo test`

## NASM Multipass Issue Resolution

### Problem Summary
- aom 3.12.1 requires NASM multipass support (feature for multiple passes over source files)
- vcpkg's bundled NASM 3.01 lacks multipass support
- Upstream bypasses this with `VCPKG_OVERRIDE_DISABLE_NASM_MULTIPASS_CHECK=1` (patched into aom's portfile.cmake)

### Direct-IP Fork Status
- **aom-disable-multipass-check.diff** patch already in place at `res/vcpkg/aom/portfile.cmake`
- The patch configures aom to skip NASM multipass check
- This allows aom 3.12.1 to build with vcpkg's NASM 3.01 without issue
- No additional patches or workarounds needed in CI

### Why No NASM Multipass Patch Needed in CI
1. The fork already has the aom patch applied (lines 11-16 of portfile.cmake)
2. vcpkg's NASM acquisition will work on all platforms
3. System NASM (apt-installed) on Linux supports multipass but isn't strictly required since aom is patched
4. aom 3.12.1 will compile successfully with this patch regardless of NASM version

## vcpkg Configuration for Direct-IP Fork

### Approach: Manifest Mode (Used by Upstream)
The fork uses vcpkg manifest mode, configured in `vcpkg.json`:
- Dependencies declared in manifest (aom, ffmpeg, libvpx, libyuv, opus, libsodium, mfx-dispatch, etc.)
- Overlay ports at `res/vcpkg` (aom, ffmpeg, libvpx, libyuv, mfx-dispatch, opus custom patches)
- Baseline: vcpkg commit `120deac3062162151622ca4860575a33844ba10b`
- Overrides: ffnvcodec 12.1.14.0, amd-amf 1.4.35

### Binary Cache Strategy
- GitHub Actions binary cache enabled: `VCPKG_BINARY_SOURCES: "clear;x-gha,readwrite"`
- Significantly speeds up CI runs (avoiding from-source builds)
- Used by all upstream workflows

### Platform Triplets
- **Windows**: x64-windows-static, arm64-windows-static
- **Linux**: x64-linux, arm64-linux
- **macOS**: x64-osx, arm64-osx
- **Android**: arm64-android, etc.

## Platform Matrix for Direct-IP Fork

### Phase 1 Baseline (Minimum Viable CI)
Focus on eliminating developer machine complexity while supporting Direct-IP targets:

1. **Windows x64 + Flutter** (primary UI target for Direct-IP)
   - Runner: windows-2022
   - Rust: 1.75 (stable for this branch)
   - vcpkg triplet: x64-windows-static
   - Build: `python3 build.py --portable --flutter --hwcodec`
   - Artifacts: rustdesk.exe

2. **Linux x86_64 + Cargo** (headless / server operations)
   - Runner: ubuntu-24.04
   - Rust: stable
   - vcpkg triplet: x64-linux
   - Build: `cargo build --locked --release`
   - Test: `cargo test --locked`
   - Artifacts: rustdesk binary

### Future Expansion (Not in Baseline)
- Windows arm64 (if Direct-IP targets it)
- macOS x64/arm64 (if Direct-IP targets desktop on macOS)
- Linux arm64 (if Direct-IP targets IoT/edge)
- Android/iOS (likely deferred unless needed for mobile Direct-IP support)

## Artifact Strategy

### Direct-IP Windows Build
Collect:
- rustdesk.exe (main executable)
- Driver files (if applicable)
- Configuration templates

### Direct-IP Linux Build
Collect:
- rustdesk binary
- Build logs for troubleshooting

No MSI/portable packing in baseline (upstream Flutter-specific feature).

## Cleanup from Upstream Workflows

The direct-ip-build.yml will **omit**:
- Bridge generation (not needed for cargo build)
- Flutter-specific steps (not needed for Linux)
- MSI/DMG packaging (downstream concern)
- Android/iOS builds (future expansion)
- RustDeskTempTopMostWindow (Windows-specific, not in scope for Phase 1)
- Signing/notarization (can be added later)
- Portable packer (downstream concern)

## Upstream Pattern: GitHub Actions Cache

Both upstream workflows use the `lukka/run-vcpkg` action with:
```yaml
uses: lukka/run-vcpkg@b1a0dd252f06b9e25b3c022a9a03bd7a427fb6a2 # v11
with:
  vcpkgDirectory: /opt/artifacts/vcpkg  # Linux
  # OR
  vcpkgDirectory: C:\vcpkg              # Windows
  vcpkgGitCommitId: ${{ env.VCPKG_COMMIT_ID }}
  doNotCache: false
```

This is battle-tested in production. No changes recommended.

## Rust Toolchain Strategy

**For Direct-IP fork**:
- Use Rust 1.75 (same as upstream sciter version, tested with this codebase)
- Avoids ABI issues mentioned in upstream comments (1.78+ changes)
- Matches existing Cargo.toml: `rust-version = "1.75"`

## Summary: What Works Out-of-the-Box

1. **aom 3.12.1 in CI**: Already patched in fork, no multipass issue
2. **vcpkg manifest mode**: Proven in upstream, supports overlay ports
3. **NASM**: System apt-get on Linux, vcpkg-acquired on Windows, no action needed
4. **GitHub Actions cache**: Direct copy from upstream, saves build time
5. **Triplet configuration**: Upstream matrix provides reference implementations

## Next Steps for Implementation

1. Create `direct-ip-build.yml` with Windows x64 + Linux x86_64 jobs
2. Use upstream flutter-build.yml as baseline for Windows job (simplify as needed)
3. Use upstream ci.yml as baseline for Linux job
4. Test on actual runners to validate environment assumptions
5. Expand matrix post-Phase-1 (arm64 support, etc.)
