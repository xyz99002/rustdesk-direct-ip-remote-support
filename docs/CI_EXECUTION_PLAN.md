# CI Execution Plan: Direct-IP Fork GitHub Actions Validation

**Date:** 2026-08-29
**Objective:** Validate Direct-IP fork through GitHub Actions; determine if bindgen issue is CI-reproducible

---

## Repository Prerequisites

### Required GitHub Setup

1. **Direct-IP Fork Repository**
   - Create if not exists: `[org]/rustdesk-direct-ip` (or similar)
   - Must have GitHub Actions enabled (default)
   - Must allow write access to run workflows

2. **Git Remote Configuration**
   ```bash
   git remote add fork https://github.com/xyz99002/rustdesk-direct-ip-remote-support.git
   ```

3. **Branch Naming Convention**
   - Feature branch: `feature/direct-ip-fork` (already local)
   - Target for PRs: `main` (or `develop` if preferred)

### Workflow Permissions

**Required:** GitHub Actions default permissions
- `contents: read` (for checkout)
- `actions: read` (for cache)

**No secrets required** for this CI run (no signing, no deployment).

---

## Workflow Triggers

### What Triggers the CI

The workflow in `.github/workflows/direct-ip-build.yml` triggers on:

1. **Manual Dispatch**
   - GitHub UI: Actions tab → Direct-IP Build → Run workflow
   - Useful for testing without pushing

2. **Push to feature/direct-ip-fork**
   - Command: `git push fork feature/direct-ip-fork`
   - Automatically runs on remote push
   - Triggers both Windows and Linux jobs

3. **Pull Requests**
   - Any PR against any branch (not path-filtered)
   - Useful for review before merging to main

### Paths Ignored (No Trigger)
- `docs/**`
- `README.md`

---

## Expected Artifacts

### Windows x64 Build

**Artifact Name:** `rustdesk-direct-ip-windows-x86_64`

**Contents:**
```
rustdesk/
├── rustdesk.exe                 (main executable)
├── data/                        (Flutter assets)
└── windows/                     (Windows integration)
```

**Expected Size:** ~200-300 MB

**Availability:** Download from Actions tab after workflow completes

### Linux x86_64 Build

**Artifact Name:** `rustdesk-direct-ip-linux-x86_64`

**Contents:**
```
rustdesk                         (binary executable)
```

**Expected Size:** ~50-100 MB

**Availability:** Download from Actions tab after workflow completes

### Retention

- **Duration:** 90 days (GitHub default)
- **Manual deletion:** Available in Actions tab if needed

---

## Expected Logs

### Windows Build Logs

**Key sections to review:**

1. **vcpkg install** (Lines from "Install vcpkg dependencies")
   - Look for: `All requested installations completed successfully`
   - Or: `CMake Error at build/cmake/aom_optimization.cmake:219`

2. **Build output** (Lines from "Build rustdesk")
   - Look for: `Finished 'debug' mode [..]` or error messages

3. **Artifact upload** (Lines from "Upload Windows Build Artifacts")
   - Look for: `Artifact rustdesk-direct-ip-windows-x86_64 has been successfully uploaded`

### Linux Build Logs

**Key sections to review:**

1. **vcpkg install** (Lines from "Install vcpkg dependencies")
   - Look for: `All requested installations completed successfully`
   - Or: bindgen errors from cargo build

2. **Cargo build** (Lines from "Build")
   - Look for: `Finished release` or error messages
   - **CRITICAL:** If bindgen errors appear here, compare to local errors

3. **Cargo test** (Lines from "Run tests")
   - Look for: `test result: ok` or failures

4. **Artifact upload** (Lines from "Upload Linux Build Artifacts")
   - Look for: `Artifact rustdesk-direct-ip-linux-x86_64 has been successfully uploaded`

---

## Success Criteria

### Windows Build Success ✅
- vcpkg install completes without NASM multipass error
- Flutter build completes successfully
- Artifact `rustdesk-direct-ip-windows-x86_64` is created and uploaded
- Expected output: `Finished 'release' mode [..]`

### Linux Build Success ✅
- vcpkg install completes without errors
- `cargo build --locked --release` succeeds without bindgen errors
- `cargo test --locked --release` passes
- Artifact `rustdesk-direct-ip-linux-x86_64` is created and uploaded
- Expected output: `test result: ok`

### Overall Success Criteria ✅
- Both Windows and Linux jobs pass
- Both artifacts available for download
- No bindgen opaque struct errors in cargo logs
- **Interpretation:** Bindgen issue is local/environmental; GitHub Actions is canonical build path

---

## Failure Criteria

### Windows Build Failure ❌
- **Scenario 1:** `CMake Error at aom_optimization.cmake:219 (Unsupported nasm: multipass...)`
  - **Meaning:** NASM multipass patch did not work in CI
  - **Action:** Investigate vcpkg configuration in CI environment

- **Scenario 2:** `error[E0609]: no field 'rc_max_quantizer'...` (bindgen errors)
  - **Meaning:** Bindgen opaque struct issue reproduced in Windows CI
  - **Action:** Not expected (Windows uses Flutter/Python build, not direct cargo)

### Linux Build Failure ❌
- **Scenario 1:** `CMake Error at aom_optimization.cmake:219`
  - **Meaning:** NASM multipass patch failed in Linux CI
  - **Action:** Investigate vcpkg configuration differences

- **Scenario 2:** `error[E0609]: no field 'g_threads'...` (bindgen errors)
  - **Meaning:** Bindgen opaque struct issue reproduced in Linux CI
  - **Action:** CRITICAL — captures exact CI output for root cause analysis

- **Scenario 3:** Test failure
  - **Meaning:** Code issue, not build issue
  - **Action:** Investigate test failure logs

### Critical: Bindgen Opaque Struct in CI

If cargo build fails with bindgen errors in Linux CI:
1. **Capture exact error output** from workflow logs
2. **Compare to local errors:**
   - Are errors identical or different?
   - Same struct names but different field counts?
3. **Update docs/FFI_BINDGEN_ANALYSIS.md** with CI findings
4. **DO NOT implement workarounds** until root cause is understood

---

## Execution Steps

### Step 1: Configure Remote (One-Time)
```bash
git remote add fork https://github.com/xyz99002/rustdesk-direct-ip-remote-support.git
# Verify:
git remote -v
```

### Step 2: Push Feature Branch
```bash
git push fork feature/direct-ip-fork
# Expected output:
# Total X (delta Y), reused Z
# remote: GitHub has received your request to run this workflow
```

### Step 3: Monitor Workflow Execution
- Navigate to: `https://github.com/xyz99002/rustdesk-direct-ip-remote-support/actions`
- Click: "Direct-IP Build" workflow run
- Watch: Windows and Linux jobs execute in parallel

### Step 4: Assess Results
- **Both pass?** → Move to packaging phase
- **Windows passes, Linux fails?** → Investigate bindgen in CI
- **Both fail?** → Capture logs, compare to local environment

---

## Next Steps After CI Validation

### If CI Succeeds (Both Jobs Pass)
1. Document Windows CI artifacts location
2. Document Linux CI artifacts location
3. Declare GitHub Actions as canonical build system
4. Document local bindgen issue as environment-specific
5. Begin Phase 6: Packaging automation

### If CI Fails with Bindgen Errors
1. Extract exact error messages from Linux CI logs
2. Compare CI bindgen output to local bindgen output
3. Update `docs/FFI_BINDGEN_ANALYSIS.md` with new findings
4. Present root cause analysis before any workaround implementation
5. **Do not proceed to packaging** until root cause is resolved

### If CI Fails with NASM Errors
1. Investigate vcpkg NASM configuration in CI vs local
2. Verify `res/vcpkg/aom/aom-disable-multipass-check.diff` is being applied
3. Check if patch syntax is correct for CI environment
4. Update docs and retry CI

---

## Communication Plan

**After CI Execution:**

Report findings in this format:

```
CI Execution Report
==================

Timestamp: [when workflow completed]

Windows Build:
- Status: [✅ PASSED | ❌ FAILED]
- Duration: [minutes]
- Key artifacts: [list]

Linux Build:
- Status: [✅ PASSED | ❌ FAILED]
- Duration: [minutes]
- Key artifacts: [list]
- Bindgen status: [✅ No opaque errors | ❌ Opaque struct errors | N/A]

Overall:
- Next phase: [Phase 6 Packaging | Root cause investigation | Other]
- Blocking issues: [list or None]
- Documents to update: [list or None]
```

---

## Duration Estimate

- **vcpkg install** (both platforms): 2-5 minutes (with binary cache)
- **Windows Flutter build**: 5-10 minutes
- **Linux cargo build + test**: 5-10 minutes
- **Total workflow time**: 10-20 minutes

**Monitoring:** Check workflow tab periodically or enable notifications

---

## Key Decision Point

**The critical question CI will answer:**

> Does the FFI bindgen opaque struct error reproduce in GitHub Actions CI?

- **YES (both platforms)** → Environmental issue isolated; proceed with GitHub Actions as canonical path
- **YES (Linux only)** → Platform-specific issue; investigate Linux environment
- **NO (both pass)** → Bindgen issue is local machine-specific; GitHub Actions is canonical build path

**Do not implement additional bindgen workarounds until this question is answered.**
