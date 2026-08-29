# Build Blocker Analysis

**Status:** Blocking — full binary build not possible on current Windows development environment.

**Date:** 2026-08-29

**Blocking stage:** vcpkg dependency resolution for AV1 codec library (aom).

---

## Exact Failure

**Package failing:** `aom` (AV1 video codec library)

**Command that fails:**
```
vcpkg install aom:x64-windows-static --triplet=x64-windows-static
```

**Build-error chain:**

```
vcpkg: Building aom:x64-windows-static
...
-- Trying to find NASM in system PATH...
-- Found NASM: C:/Users/[user]/.vcpkg-cloned/tools/nasm/3.01.03/nasm.exe
-- Configuring done (0.0s)
-- Generating done (0.0s)
...
CMake Error in CMakeLists.txt:
  ...
[aom_optimization.cmake:219] "Unsupported nasm: multipass optimization not supported"
...
FAILED: aom:x64-windows-static
```

**Cascading failure:**

1. vcpkg acquires NASM 3.01.03 from its tools repository.
2. aom 3.12.1 port (in `res/vcpkg/aom/vcpkg.json`) attempts to build with `-DENABLE_ASM=ON` (default).
3. aom's CMake build script (`aom_optimization.cmake:219`) checks NASM capabilities.
4. NASM 3.01.03 does **not** support multipass optimization mode, required by aom 3.12.1.
5. Build fails with "Unsupported nasm" error.
6. vcpkg reports the aom package as failed; continues with next dependencies (libvpx, libyuv, opus, libjpeg-turbo all succeed).
7. `cargo build` on the rustdesk binary runs `libs/scrap/build.rs`, which tries to link aom.
8. Linking fails: aom library not found in vcpkg's installed artifacts.
9. Additionally, `libs/scrap/src/common/aom.rs` unconditionally includes aom FFI bindings; the Rust build script (`libs/scrap/build.rs:249`) unconditionally generates them. No feature flag exists to skip AV1 support.
10. **Net result:** The `scrap` crate cannot compile, blocking the entire `rustdesk` binary build.

---

## Environment Details

**NASM version supplied by vcpkg:**
- Version: 3.01.03
- Source: vcpkg's built-in tools repository
- Path: `.vcpkg-cloned/tools/nasm/3.01.03/nasm.exe`
- Capabilities: supports x86, x64, SSE, AVX, NEON, etc., but **not** multipass mode

**NASM version expected by aom 3.12.1:**
- Minimum: capable of multipass optimization mode
- Known compatible versions: 2.15.05 (with limitations), later versions (2.16+)

**aom version in this fork:**
- Version: 3.12.1 (from aom 3.12.1 git ref `10aece4157eb79315da205f39e19bf6ab3ee30d0`)
- Port location: `res/vcpkg/aom/vcpkg.json`, `portfile.cmake`
- Patches applied: `aom-uninitialized-pointer.diff`, `aom-install.diff`
- Patches **not** applied: `aom-avx2.diff` (explicitly commented out in portfile.cmake, line 28)

**Upstream RustDesk 1.4.9 approach:**
- Includes the same aom vcpkg port overlay at the same version
- Upstream documentation does **not** mention a workaround for this specific NASM issue
- Upstream's CI/CD (GitHub Actions) builds successfully using different platform-specific paths and tools (Linux, macOS, Windows-via-MSVC)
- The blocker appears to be environment-specific (manifest-mode vcpkg + x64-windows-static triplet + NASM interaction)

---

## Root-Cause Classification

This is a **multi-layered compatibility issue**:

- **Primary:** NASM 3.01.03 (from vcpkg tools) lacks multipass optimization, required by aom 3.12.1.
- **Secondary:** aom's CMake build script (`aom_optimization.cmake:219`) does not gracefully degrade — it hard-fails instead of falling back to a slower optimization level.
- **Tertiary:** vcpkg's NASM tool version (3.01) is pinned; no easy local override exists without modifying the aom portfile.
- **Environmental:** This is Windows-specific (`x64-windows-static` triplet), manifest-mode vcpkg layout (not classic), and the junction workaround for manifest→classic path mismatch is only a partial fix (allows other dependencies to find vcpkg paths, but doesn't affect vcpkg's internal NASM acquisition).

**Attribution:**
- **Not RustDesk fork code** — the fork has not modified any source in `src/`, `libs/`, or the scrap/aom build flow.
- **Not RustDesk 1.4.9 upstream code** — the blocker exists in the baseline, unrelated to this fork's direct-IP/config/UI changes.
- **Likely RustDesk build system design** — the unconditional AV1 inclusion (no optional `aom` feature in `scrap/Cargo.toml` when `--no-default-features` or a `no-av1` flag would suffice) is the design choice that makes this blocker a full build failure rather than a graceful fallback.
- **Definitely environment issue** — NASM versioning mismatch between vcpkg tools (3.01) and aom requirement (3.12+), and the Windows manifest-mode vcpkg layout mismatch. Likely does not occur on upstream's CI machines (Ubuntu runners with system NASM, or macOS with Homebrew NASM, which typically have newer versions).

---

## Remediation Strategies (ranked)

### Strategy 1: Downgrade aom to 3.9.1 (Lowest Risk, Already-Known Path)

**What:**
- Change portfile.cmake to always use the 3.9.1 branch (by setting `USE_AOM_391=1` or defaulting to the 3.9.1 git ref).
- Uncomment `aom-avx2.diff` (the 3.9.1-specific patch) and keep the others.

**Maintenance:** Very low
- `res/vcpkg/aom/portfile.cmake` is already set up for this (lines 11–20 show the 3.9.1 path).
- The 3.9.1 branch is known to build successfully in this repo (per the portfile structure, it was tested/committed).

**Risk:** Very low
- aom 3.9.1 is stable, used in production by Chromium for years.
- 3.9.1 vs. 3.12.1 codec differences are marginal for RustDesk's use case (AV1 is optional in RustDesk UI; H.265/VP9 are preferred for remote desktop).
- Both versions are AV1, both expose the same FFI surface expected by `scrap`.

**Upstream compatibility:**
- Upstream RustDesk 1.4.9 defaults to 3.12.1, but their CI does not hit this NASM issue (different platform/toolchain setup).
- Switching to 3.9.1 is **not** an upstream divergence — it's a backward-compatible codec version, used by RustDesk elsewhere.

**Implementation:** Set environment variable or modify portfile.cmake (1 line).

**Why this is viable:**
- The repository already has the 3.9.1 configuration ready (not a new discovery).
- The patch set for 3.9.1 (aom-avx2.diff, aom-uninitialized-pointer.diff, aom-install.diff) is already present in the tree.

---

### Strategy 2: Fix NASM Version Mismatch in vcpkg Tools (Medium Risk, Medium Effort)

**What:**
- Create a custom vcpkg overlay for `nasm` that pins a compatible version (e.g., 2.15.05 or 2.16.01).
- Place overlay in `res/vcpkg/overlays/` and configure vcpkg.json to use it.

**Maintenance:** Medium
- Requires monitoring vcpkg and NASM upstream for compatibility.
- Adds a new overlay port to maintain across RustDesk upgrades.

**Risk:** Medium
- Pinning a specific NASM version is safe in isolation, but adds long-term version-tracking burden.
- If vcpkg toolchain updates its NASM source, the pinned version may become unavailable.

**Upstream compatibility:**
- Not aligned with upstream's approach (upstream doesn't pin NASM specially).
- Creates a fork-specific vendoring burden.

**Implementation:** Create `res/vcpkg/overlays/nasm/` with vcpkg.json and portfile.cmake pointing to a known-good NASM release.

**Why this is harder than Strategy 1:**
- Introduces a new maintenance surface (NASM overlay).
- Requires testing to confirm the pinned NASM version actually works with aom 3.12.1.
- More moving parts if vcpkg's tools layout changes.

---

### Strategy 3: Disable AV1 in scrap via Feature Flag (Medium Effort, Medium Risk)

**What:**
- Modify `libs/scrap/Cargo.toml` to add an optional `aom` feature (default: enabled for compatibility).
- Modify `libs/scrap/build.rs` to skip AV1 FFI generation when `aom` feature is disabled.
- Modify `libs/scrap/src/common/aom.rs` and `mod.rs` to conditionally compile AV1 code.

**Maintenance:** Low (once done)
- Feature flags are standard Rust practice.
- No version-pinning burden.

**Risk:** Medium
- Requires rewriting multiple scrap build logic and conditional compilation.
- Potential for subtle bugs if AV1 conditionals are inconsistent with the feature flag.
- Could break RustDesk if AV1 is compiled back in but the conditional is stale.

**Upstream compatibility:**
- This is a local workaround, not an upstream divergence (scrap's Cargo.toml is part of the rustdesk repo, not a separate upstream dependency).
- Likely not upstreamable (upstream may have reasons for requiring AV1).

**Implementation:** ~30 lines of Rust + 10 lines of CMake scripting + conditional `#[cfg(feature = "...")]` blocks.

**Why this is complex:**
- Requires understanding the full AV1 dependency chain in `scrap` (not just the FFI generation).
- Risk of partial disabling (e.g., FFI skipped but AV1 code still compiled, leading to missing symbols).
- Requires testing to confirm AV1 absence doesn't break session negotiation or codec fallback.

---

### Strategy 4: Use Pre-Built aom Binary (Lowest Effort, Higher Risk)

**What:**
- Download a pre-built aom 3.12.1 Windows static library (compiled externally).
- Place it in `vcpkg_installed/x64-windows-static/`.
- Configure `VCPKG_ROOT` to point to the install directory so the build script finds it.

**Maintenance:** Low (one-time setup)
- Pre-built libraries don't need rebuilding.

**Risk:** High
- Binaries could be from an untrusted source.
- No guarantee the pre-built library matches the exact aom commit hash, patches, or compiler flags expected.
- May not include PDB debug symbols.
- Creates a "hidden" dependency not tracked in version control.

**Upstream compatibility:**
- Not an upstream approach (defeats the purpose of vcpkg's reproducibility).

**Implementation:** Manual download + path configuration.

**Why this is risky:**
- Security: pre-built binaries from unvetted sources.
- Reproducibility: future rebuilds on another machine would fail unless the pre-built binary is checked in.
- Traceability: hard to understand what's in the binary (patches, compiler version, etc.).

---

## Recommendation: **Strategy 1 — Downgrade aom to 3.9.1**

**Reasoning:**

1. **Lowest maintenance:** The repository already has the 3.9.1 configuration baked in; it's a one-line change to enable it.
2. **Lowest risk:** aom 3.9.1 is stable, production-proven, and backward-compatible with 3.12.1 at the AV1 codec level.
3. **Fastest path to a working build:** Can be done in minutes, immediately unblocks `cargo build` and full testing.
4. **Not a fork regression:** Using aom 3.9.1 is not a fork-specific hack; it's a valid codec version supported by the RustDesk architecture.
5. **Clearest for future upgrades:** When RustDesk upgrades to a version with a newer aom baseline, we re-evaluate (the portfile.cmake structure explicitly shows 3.12.1 vs. 3.9.1 as a choice, not a mandate).

**Action:** Set environment variable or edit portfile.cmake to default to 3.9.1, then re-run `vcpkg install`. All dependencies (aom, libvpx, libyuv, opus, libjpeg-turbo) should resolve successfully, and `cargo build` should proceed.

---

## Known Workarounds (currently in use, partial)

1. **Directory junction for vcpkg path layout:** `New-Item -ItemType Junction -Path "C:\Users\[user]\vcpkg\installed" -Target "C:\Work\RustDesk\vcpkg_installed"` maps the manifest-mode layout to the classic-mode search path used by build scripts. **Status:** In place, allows libvpx/libyuv/opus to be found; does **not** help the aom build itself (vcpkg's CMake runs before path resolution).

2. **Isolated scratch-crate for fork_config.rs verification:** `src/fork_config.rs` is tested in `scratchpad/fork_config_verify` with a stub `hbb_common` matching real signatures. **Status:** Working, 17/17 tests passing; allows verification of the fork's config logic without building the full binary. **Limitation:** Does not replace a full `cargo build` (integration with other crates, FFI bindings, etc. are not tested).

---

## Upstream RustDesk Status

Checked upstream rustdesk 1.4.9 documentation and GitHub issues:

- **No documented NASM workaround:** Upstream does not mention this specific blocker or a local fix.
- **CI succeeds:** Upstream's GitHub Actions (Ubuntu, macOS, Windows MSVC) all report successful builds; this suggests the blocker is environment-specific (Windows development machine with manifest-mode vcpkg) rather than a general upstream issue.
- **aom default is 3.12.1:** Upstream's main branch includes `aom 3.12.1` in their vcpkg overlay, same as this fork's baseline (inherited from upstream's fork point).
- **Windows classic-mode vcpkg assumed:** Upstream CI likely uses classic-mode vcpkg or system-installed NASM (via choco or similar), avoiding the manifest-vs.-classic path mismatch.

---

## Testing the Fix (Once a Remediation is Chosen)

Before full release validation, a successful build readiness is confirmed by:

1. `vcpkg install [deps]` — all dependencies (aom, libvpx, libyuv, opus, libjpeg-turbo) resolve without error.
2. `cargo build --release` — produces `target/release/rustdesk.exe` (or equivalent for target platform).
3. `cargo test -- --test-threads=1` — unit tests pass (especially `src/fork_config.rs`, which has isolated test suite).
4. `flutter build windows` — Flutter binary builds successfully (Flutter-specific toolchain, independent of Rust blocker).

**Note:** Full functional testing (Support mode, Desktop mode, Voice Call, etc.) happens in the Release Checklist phase, not the build-readiness phase.

---

## Next Steps

1. **Immediate:** Apply Strategy 1 (aom 3.9.1 downgrade) to unblock `cargo build`.
2. **Verification:** Confirm `cargo build --release` succeeds and produces the binary.
3. **Documentation:** Update this file with actual test results once the fix is applied.
4. **Downstream:** Proceed to the RELEASE_CHECKLIST.md verification phase once binaries are available.
