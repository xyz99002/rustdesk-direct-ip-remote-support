# CI Troubleshooting Guide

**For:** When GitHub Actions builds fail  
**Goal:** Quickly identify and fix the issue  
**Canonical reference:** CI_WORKFLOW_AUDIT_2026-09-01.md (what's known-broken)

---

## The Golden Path: A Failing Job

When GitHub Actions shows a red ❌:

1. **Click the red job name** in the Actions tab
2. **Scroll down** to find the failing step (marked with red ❌)
3. **Read the error message** at the top of that step's output
4. **Match the error to a category below**
5. **Fix it locally, commit, push**
6. **GitHub Actions re-runs automatically**

---

## Common Failures & Fixes

### **Build Failure (Windows/macOS/Android)**

**Error pattern:** `error: could not compile ...`, `linking with` error, `undefined reference`

**Diagnosis:**
- Syntax error in Rust code
- Missing import
- Type mismatch
- C/FFI linking issue

**Fix:**
1. Read the exact error message (line number + what's wrong)
2. Fix the issue in `src/...` locally
3. Run `cargo check` locally (if you have Rust) to verify
4. Commit and push

**Example:**
```
error[E0433]: cannot find function `parse_role_config` in scope
  --> src/fork_config.rs:42:18
```
→ The function doesn't exist or isn't imported. Check the function name and imports.

---

### **Sciter Linux Build Failure (x86_64 or armv7)**

**Error pattern:** `GCC 7.5.0`, `aom`, `_mm256_set_m128i`, `incompatible types`

**Current status:** ✅ **FIXED** (as of 2026-09-01)

A GCC 7.5.0 / aom 3.12.1 incompatibility was patched. If you still see this error:
- It means the patch didn't apply correctly (unlikely, but report it)
- Or there's a different GCC/aom issue

**Reference:** LINUX_SCITER_FIX_2026-09-01.md

**Action:** File an issue with the exact error message.

---

### **Node.js 20 Deprecation Warning**

**Error pattern:** `Node.js 20 is deprecated. The following actions target Node.js 20...`

**Current status:** ✅ **FIXED** (Phase 1 upgrades, 2026-09-01)

All major actions upgraded to latest patches. If you still see this:
- It's likely an old artifact cache; rebuild should clear it
- Or a new action was added without version pinning

**Fix:** Update the action to the latest patch version in `.github/workflows/...yml`

**Reference:** GITHUB_ACTIONS_RUNTIME_AUDIT.md

---

### **Flutter Build Failure**

**Error pattern:** `flutter`, `pub get`, `dart compile`

**Diagnosis:**
- Missing or corrupted Flutter cache in CI
- Dart code syntax error
- Package version conflict

**Fix:**
1. Check the exact error message
2. If it's a Dart/Flutter code error, fix locally and push
3. If it's a cache issue, the next CI run should clear it (CI installs fresh Flutter per-build)

**Reference:** flutter-build.yml (the build recipe)

---

### **Permissions Error (403)**

**Error pattern:** `Error: Too many retries`, `403 Forbidden`, `GITHUB_TOKEN`

**Current status:** ✅ **FIXED** (Bug C fix, permissions blocks added to workflows)

Workflows now have `permissions: contents: write` to allow release publication.

If you still see this:
- A new workflow is missing the permissions block
- Or the repo's default workflow permissions were changed back to read-only

**Fix:** Add to the workflow file:
```yaml
permissions:
  contents: write
```

**Reference:** CI_WORKFLOW_AUDIT_2026-09-01.md § Section 6 (Bug C)

---

### **Android Build Failure**

**Error pattern:** `gradle`, `NDK`, `clang`, `linker`

**Diagnosis:**
- NDK version mismatch
- Gradle cache corruption
- Rust toolchain issue (aarch64-linux-android target missing)

**Fix:**
1. Read the error to understand what failed
2. If it's a Rust target issue: `rustup target add aarch64-linux-android` (locally)
3. Commit and push; CI will use its pinned toolchains

**Reference:** flutter-build.yml (Android build recipe)

---

### **Artifact Upload/Download Failure**

**Error pattern:** `actions/upload-artifact`, `actions/download-artifact`, `404 Not Found`

**Diagnosis:**
- Artifact path doesn't exist
- Artifact name typo in download step
- Artifact not produced by an earlier job (did that job fail?)

**Fix:**
1. Check the step that uploads/downloads the artifact
2. Verify the path or artifact name is correct
3. Verify the job that creates the artifact actually ran (check earlier in the logs)

**Reference:** flutter-build.yml (artifact steps in each job)

---

## Advanced: Reading CI Logs

**The structure of a failing job:**

```
=== Set up job ===
[setup steps...]

=== Checkout source code ===
[checkout output]

=== [Your Step Here] ===
##[error] Error message
[detailed output of what failed]

=== [Next Step] ===
[skipped because previous step failed]

=== Complete job ===
[cleanup]
```

**Key:** The first `##[error]` line tells you which step failed. Scroll up a bit to read the full error context.

---

## If You're Stuck

**When in doubt, ask yourself:**

1. **Does the code compile locally?**
   - If no: fix the code locally
   - If yes (or you didn't test): check if CI is testing a different code path

2. **Is this a known issue?**
   - Check CI_WORKFLOW_AUDIT_2026-09-01.md § known issues
   - Check recent commits to see if someone just fixed it

3. **Is this a platform-specific issue?**
   - Does the same code fail on Windows but not Linux? (architecture/linker differences)
   - Does it fail on one matrix job but not others? (version/environment difference)

4. **Is this a dependency issue?**
   - Did you update Cargo.toml or pubspec.yaml? (dependency version conflict)
   - Did GitHub Actions update a tool? (CI environment change)

**If still stuck:** File an issue with:
- The failing job name
- The exact error message
- Which commit/branch triggered it
- Whether it's reproducible locally

---

## The Big Picture

**GitHub Actions workflow timeline:**

```
You push code
  ↓
flutter-ci.yml (or release.yml) triggered
  ↓
determine-version job runs (computes tag, version)
  ↓
Build matrix runs in parallel:
  - build-rustdesk-windows-*
  - build-rustdesk-macOS-*
  - build-rustdesk-linux-*
  - build-rustdesk-linux-sciter-*
  - build-rustdesk-appimage-*
  - build-rustdesk-flatpak-*
  - build-rustdesk-android-*
  ↓
If all pass: finalize-release job (only in release.yml)
  ↓
GitHub Release published with all artifacts
```

**If ONE matrix job fails:** The entire build shows red, but other jobs keep running. No earlier job was skipped. The failed job and all downstream jobs (finalize-release) are marked ❌.

---

## Reporting a Bug

When filing an issue:

1. **Link the CI run:** https://github.com/xyz99002/rustdesk-direct-ip-remote-support/actions (find the run in the list)
2. **Quote the error message:** Copy the first `##[error]` line and surrounding context
3. **Provide the command:** What did you do to trigger the failure? (e.g., "pushed to master", "opened PR")
4. **Provide context:** Is this new or have you seen it before?

---

## References

- **CI structure:** CI_WORKFLOW_AUDIT_2026-09-01.md
- **Build verification:** BUILD_VERIFICATION_RESULTS.md
- **Known issues:** CI_WORKFLOW_AUDIT_2026-09-01.md § Section 10 (known failures & fixes)
- **Workflow definitions:** .github/workflows/*.yml
