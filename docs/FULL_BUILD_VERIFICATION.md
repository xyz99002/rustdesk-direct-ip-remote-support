# Full Build Verification

**Purpose:** End-to-end verification that the Direct-IP RustDesk fork builds successfully after the aom 3.9.1 downgrade remediation.

**Date:** 2026-08-29

**Status:** Awaiting execution — build blocker fix applied; verification checklist ready.

> **2026-09-01 note:** this local, step-by-step procedure is **superseded as the default path** for regular development — GitHub Actions now builds, tests, packages, and releases this fork on every push and on-demand (see `docs/CI_WORKFLOW_AUDIT_2026-09-01.md`, `docs/DEVELOPER_ONBOARDING.md`). Keep this document for two specific cases: a CI maintainer trying to reproduce a build failure by hand, or genuine emergency local debugging when GitHub Actions itself is unavailable. See `docs/LOCAL_BUILD_DECOMMISSION_PLAN.md` for which of the tools this procedure requires are worth keeping installed for those cases versus safe to remove.

---

## Environment Checklist

Before starting the build, verify the environment:

- [ ] **Rust toolchain installed and in PATH**
  - Command: `rustc --version`
  - Expected output: `rustc 1.98.0` or later
  - Command: `cargo --version`
  - Expected output: `cargo 1.98.0` or later

- [ ] **vcpkg installed and configured**
  - Command: `$env:VCPKG_ROOT` (PowerShell) or `echo $VCPKG_ROOT` (bash)
  - Expected: Path to vcpkg installation (e.g., `C:\Users\[user]\vcpkg`)
  - Verify git repo: `cd $env:VCPKG_ROOT && git status`

- [ ] **CMake installed and in PATH**
  - Command: `cmake --version`
  - Expected output: `cmake version 4.4+`

- [ ] **Flutter SDK installed and in PATH**
  - Command: `flutter --version`
  - Expected output: `Flutter X.X.X` with stable channel

- [ ] **Windows build tools (MSVC) available** (Windows only)
  - Command: `cl.exe /version` or check Visual Studio is installed
  - Expected: MSVC compiler version

---

## Step 1: vcpkg Dependency Resolution

**Objective:** Verify that aom 3.9.1 and all other dependencies build successfully.

**Command:**
```powershell
cd C:\Work\RustDesk
vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static libjpeg-turbo:x64-windows-static
```

**Expected output:**
```
...
Building aom:x64-windows-static
...
Building package aom[core]:x64-windows-static... 
...
aom:x64-windows-static: Elapsed time = X.XX s
aom:x64-windows-static: Installing
aom:x64-windows-static: Determining port version: OK
aom:x64-windows-static: Downloading aom - 3.9.1 from ...
...
(All packages succeed without error)
vcpkg install command exit code: 0
```

**Failure mode (previous blocker):**
```
CMake Error in CMakeLists.txt:
  ...
[aom_optimization.cmake:219] "Unsupported nasm: multipass optimization not supported"
```

**If this error reoccurs:**
1. Confirm `USE_AOM_312=1` is **not** set: `echo $env:USE_AOM_312` (should be empty)
2. Check portfile.cmake was modified correctly: `type res/vcpkg/aom/portfile.cmake | grep "3.9.1"`
3. Clear vcpkg cache and retry: `rm -r vcpkg_installed/ && vcpkg install ...`
4. If still fails, document the error and escalate to new blocker analysis

**Time estimate:** 20–30 minutes (cold rebuild of all deps including aom)

**Verification:** After success, confirm the aom installation:
```powershell
ls C:\Users\[user]\vcpkg\installed\x64-windows-static\lib\aom.lib
# Expected: file exists (~2-5 MB)

ls C:\Users\[user]\vcpkg\installed\x64-windows-static\include\aom\aom.h
# Expected: file exists
```

---

## Step 2: Rust Compilation

**Objective:** Build the complete Rust binary (rustdesk core).

**Command:**
```powershell
cd C:\Work\RustDesk
cargo build --release
```

**Expected output (first build):**
```
Compiling rustdesk v1.4.9 (...)
...
(many dependencies compile)
...
Finished `release` profile [optimized] target(s) in XXsXXs
```

**Expected output (incremental):**
```
Finished `release` profile [optimized] target(s) in XXsXXs
```
(if no source changes, build completes in <1 second)

**Potential failure modes:**

1. **aom linking failure:**
   ```
   error: linking with `cl.exe` failed: exit code: 1169
   ...
   aom.lib not found
   ```
   → Indicates vcpkg step failed; re-check Step 1

2. **Missing FFI bindings:**
   ```
   error[E0432]: unresolved import `aom_ffi`
   ```
   → Indicates `libs/scrap/build.rs` failed to generate AV1 bindings from aom headers
   → Run with verbose output: `RUST_LOG=debug cargo build --release` to see full build.rs output

3. **Rust version mismatch:**
   ```
   error: Edition 2021 is unstable
   ```
   → Update Rust: `rustup update stable`

**Time estimate:** 30–60 minutes (cold build); 5–15 minutes (incremental)

**Artifact location:**
```
target/release/rustdesk.exe
# Expected size: 50–80 MB
# Expected timestamp: just now (recent)
```

**Verification after success:**
```powershell
ls target/release/rustdesk.exe
file target/release/rustdesk.exe  # PowerShell: Get-Item target/release/rustdesk.exe
```

---

## Step 3: Rust Tests

**Objective:** Verify all Rust tests pass, especially fork-specific config tests.

**Command:**
```powershell
cargo test -- --test-threads=1
```

**Note:** `--test-threads=1` serializes tests to avoid race conditions in fork_config.rs tests (which mutate shared HARD_SETTINGS statics).

**Expected output:**
```
running XXX test
test fork_config::tests::apply_hides_account_network_and_lan_discovery_unconditionally ... ok
test fork_config::tests::apply_maps_authentication_modes_to_approve_mode_option ... ok
test fork_config::tests::apply_maps_support_enabled_to_enable_camera_permission ... ok
test fork_config::tests::apply_sets_incoming_only_for_remote_role ... ok
test fork_config::tests::apply_sets_outgoing_only_for_local_role ... ok
...
test result: ok. XXX passed; 0 failed
```

**Potential failure modes:**

1. **fork_config tests fail:**
   ```
   test result: FAILED. XXX passed; Y failed
   ```
   → Indicates a regression in configuration logic
   → Review the specific test failure message and diff recent changes to `src/fork_config.rs`

2. **Other tests fail:**
   ```
   error: linking failed for tests
   ```
   → Likely aom-related; re-check Step 2

**Time estimate:** 5–15 minutes (depends on number of tests)

**If tests pass:** Continue to Step 4
**If tests fail:** Stop; investigate before proceeding

---

## Step 4: Flutter Compilation

**Objective:** Build the Flutter UI binary (platform-specific).

**For Windows:**
```powershell
cd flutter
flutter pub get
dart analyze --fatal-infos
flutter build windows --release
```

**For macOS:**
```bash
cd flutter
flutter pub get
dart analyze --fatal-infos
flutter build macos --release
```

**For Linux:**
```bash
cd flutter
flutter pub get
dart analyze --fatal-infos
flutter build linux --release
```

**Expected output (`flutter pub get`):**
```
Running "flutter pub get" in flutter...
...
Resolving dependencies...
Got dependencies in XXsXXs.
```

**Expected output (`dart analyze`):**
```
Analyzing flutter...
No issues found!
```

**Expected output (`flutter build [platform]`):**
```
Running Gradle assemble for app...
...
Built [flutter/build/windows/runner/Release/rustdesk.exe] (Windows)
or
Built [flutter/build/macos/Release/rustdesk.app] (macOS)
or
Built [flutter/build/linux/release/bundle/] (Linux)
```

**Potential failure modes:**

1. **Dart analysis errors:**
   ```
   error: ...
   ```
   → Review the specific Dart syntax or import error; check for recent Flutter UI changes

2. **Flutter build fails:**
   ```
   Gradle build failed
   or
   ProcessException: Process exited abnormally
   ```
   → Run with verbose output: `flutter build [platform] --release -v`

3. **Missing dependencies:**
   ```
   error: unable to locate pubspec.yaml in parent directories
   ```
   → Ensure you're in the `flutter/` directory: `pwd` should show `.../RustDesk/flutter`

**Time estimate:** 20–40 minutes (cold build); 5–10 minutes (incremental)

**Artifact location:**
```powershell
# Windows
flutter/build/windows/runner/Release/rustdesk.exe  # Flutter app entry point

# macOS
flutter/build/macos/Release/rustdesk.app/Contents/MacOS/rustdesk

# Linux
flutter/build/linux/release/bundle/rustdesk
```

**Verification:**
```powershell
ls flutter/build/windows/runner/Release/rustdesk.exe
# Expected size: ~150–250 MB
```

---

## Step 5: Cargo fmt and Clippy Checks

**Objective:** Ensure code formatting is valid and there are no clippy warnings (best practices).

**Commands:**
```powershell
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

**Expected output:**
```
(No output, or "Checking" messages)
exit code: 0
```

**Potential failure modes:**

1. **Format violations:**
   ```
   error: code formatting does not match current rustfmt
   ```
   → Fix: `cargo fmt` (without `--check`) to auto-format

2. **Clippy warnings:**
   ```
   warning: ...
   error: aborting due to XX previous errors
   ```
   → Review warnings and fix code, or update clippy directives

**Time estimate:** 2–5 minutes

---

## Summary: Build Status

After completing all steps, fill in this summary:

### Rust Build
- [ ] vcpkg dependencies resolved (aom 3.9.1 + others)
- [ ] `cargo build --release` succeeded
  - **Artifact:** `target/release/rustdesk.exe`
  - **Size:** ___ MB
  - **Time:** ___ minutes
- [ ] `cargo test -- --test-threads=1` passed
  - **Tests passed:** ___
  - **Tests failed:** ___

### Flutter Build
- [ ] Flutter dependencies resolved (`flutter pub get`)
- [ ] Dart analysis passed (`dart analyze`)
- [ ] `flutter build [windows|macos|linux] --release` succeeded
  - **Artifact:** `flutter/build/[platform]/.../rustdesk.exe/app/bundle`
  - **Size:** ___ MB
  - **Time:** ___ minutes

### Code Quality
- [ ] `cargo fmt --check` passed
- [ ] `cargo clippy` passed (no warnings)

### New Blockers Encountered
- [ ] None
- [ ] Yes, list below:
  1. ___
  2. ___

---

## Next Steps

### If all steps succeeded:
1. ✅ Build readiness confirmed
2. Proceed to **PACKAGING_PLAN.md** for artifact packaging
3. Proceed to **RELEASE_CHECKLIST.md** for functional verification

### If any step failed:
1. ❌ Build readiness not confirmed
2. Document the failure:
   - **Step:** (which step)
   - **Error message:** (full error text)
   - **Environment:** (Rust version, cmake version, etc.)
   - **Attempted workarounds:** (if any)
3. Update **docs/BUILD_BLOCKER_ANALYSIS.md** with the new blocker (if applicable)
4. Create a new task or issue for investigation

---

## Appendix: Build Performance Baselines

**Typical times (for reference):**

| Step | Cold Build | Incremental | Notes |
|---|---|---|---|
| vcpkg install (all deps) | 30–60 min | 2–5 min | Depends on network; first aom 3.9.1 build is ~20 min |
| cargo build --release | 30–60 min | 1–5 sec | Incremental is nearly instant if no source changes |
| cargo test | 5–15 min | 2–10 min | Depends on number of tests; serial execution (-threads=1) is slower |
| flutter build [platform] | 20–40 min | 5–10 min | First build is slow; incremental is fast |
| **Total (cold)** | **1.5–2.5 hours** | — | On first run; subsequent builds much faster |

---

## Appendix: Troubleshooting

### Symptom: "vcpkg: command not found"
- **Cause:** vcpkg not in PATH
- **Fix:** Add `$VCPKG_ROOT/` to PATH, or use full path: `C:\Users\[user]\vcpkg\vcpkg.exe install ...`

### Symptom: "VCPKG_INSTALLED_ROOT not found"
- **Cause:** Build script couldn't locate vcpkg manifest directory
- **Fix:** Set environment variable: `$env:VCPKG_INSTALLED_ROOT = "C:\Work\RustDesk\vcpkg_installed"`

### Symptom: "cl.exe not found" (Windows)
- **Cause:** MSVC compiler not in PATH
- **Fix:** Run from Visual Studio Developer Command Prompt, or add MSVC bin to PATH

### Symptom: "flutter command not found"
- **Cause:** Flutter SDK not in PATH
- **Fix:** Add `C:\Users\[user]\flutter\bin\` to PATH

### Symptom: Dart/Flutter version mismatch
- **Cause:** Flutter SDK version incompatible with Dart version in pubspec
- **Fix:** Update Flutter: `flutter upgrade` or `flutter channel stable && flutter upgrade`

---

## Approval Gates

**Build Readiness Confirmed:**
- [ ] All 5 steps completed successfully
- [ ] No new blockers encountered
- [ ] Artifacts verified (sizes, timestamps, existence)

**Approval to proceed to Packaging/Functional Testing:**
- [ ] Build readiness confirmed
- [ ] No blocking issues (only optional improvements)

---

**Execution Notes:**

- Timestamp started: ___
- Timestamp completed: ___
- Total elapsed: ___
- Executor: ___
- Environment: [Windows 10/11, Rust version, vcpkg version, etc.]
