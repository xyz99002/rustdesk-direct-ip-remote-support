# GitHub Actions First Run Checklist

**Date:** 2026-08-29
**Repository:** https://github.com/xyz99002/rustdesk-direct-ip-remote-support.git
**Branch:** feature/direct-ip-fork
**Critical Question:** Does the FFI bindgen opaque struct error reproduce in GitHub Actions?

---

## Pre-Execution Setup

### ✅ Step 1: Verify Git Remotes

**Status:** ✅ CONFIGURED

```bash
git remote -v
# Should show:
# fork      https://github.com/xyz99002/rustdesk-direct-ip-remote-support.git (fetch)
# fork      https://github.com/xyz99002/rustdesk-direct-ip-remote-support.git (push)
# upstream  https://github.com/rustdesk/rustdesk.git (fetch)
# upstream  https://github.com/rustdesk/rustdesk.git (push)
```

**Verification:** Run the command above; both remotes must be present.

### ✅ Step 2: Verify Repository State

**Local working tree:**
```bash
git status
# Expected: "On branch feature/direct-ip-fork"
# Expected: "nothing to commit, working tree clean"
```

**Latest commit:**
```bash
git log -1 --oneline
# Expected: c5217fec8 Add comprehensive repository and artifact mapping documentation
```

### ✅ Step 3: Verify Workflow File

**File present:**
```bash
test -f .github/workflows/direct-ip-build.yml && echo "✅ Workflow file exists" || echo "❌ Workflow file missing"
```

**File syntax valid:**
- Check: `.github/workflows/direct-ip-build.yml` contains trigger configuration
- Triggers: `workflow_dispatch`, `push` to `feature/direct-ip-fork`, `pull_request`
- Jobs: `build-windows-direct-ip` and `build-linux-direct-ip`

---

## Repository Settings Verification

### GitHub Repository Configuration

**Location:** https://github.com/xyz99002/rustdesk-direct-ip-remote-support

**Required Settings:**

| Setting | Required Value | Where to Check |
|---------|---|---|
| Actions | Enabled | Settings → Actions → General → Allow all actions |
| Branch protection | None required | Settings → Branches (for initial run) |
| Secrets | None required | Settings → Secrets and variables → Actions |
| CODEOWNERS | Not required | Not needed for CI validation |

**Minimum Permissions:** 
- GitHub Actions default (contents: read, actions: read)
- No additional secrets or variables required

### Workflow Trigger Conditions

**Push Trigger:**
- Branch: `feature/direct-ip-fork`
- Triggered automatically on push
- Both jobs run in parallel (Windows + Linux)

**Pull Request Trigger:**
- Any PR triggers workflow
- Useful for review before merging to main

**Manual Dispatch:**
- GitHub UI: Actions tab → Direct-IP Build → Run workflow
- Useful for retry without pushing

---

## Expected Artifacts

### Windows Build Artifact

| Property | Value |
|----------|-------|
| **Artifact Name** | `rustdesk-direct-ip-windows-x86_64` |
| **Path** | `rustdesk/` |
| **Contents** | rustdesk.exe, data/, windows/ |
| **Size** | ~200-300 MB |
| **Success Indicator** | File named `rustdesk.exe` present |

### Linux Build Artifact

| Property | Value |
|----------|-------|
| **Artifact Name** | `rustdesk-direct-ip-linux-x86_64` |
| **Path** | `target/x86_64-unknown-linux-gnu/release/rustdesk` |
| **Contents** | Binary executable |
| **Size** | ~50-100 MB |
| **Success Indicator** | File named `rustdesk` present, executable |

### Artifact Retention

- **Duration:** 90 days (GitHub default)
- **Location:** Actions tab → Workflow run → Artifacts section
- **Download:** Click artifact name to download

---

## Expected Success Criteria

### Windows Build ✅

**Logs should show:**
```
-- All requested installations completed successfully
-- Installed 7 items
-- Finished 'release' mode [..]
-- Artifact rustdesk-direct-ip-windows-x86_64 has been successfully uploaded
```

**No errors for:**
- LLVM/Clang installation
- Flutter installation
- vcpkg setup
- vcpkg dependency install (NASM/aom should succeed with patch)
- Python build.py execution
- Artifact upload

### Linux Build ✅

**Logs should show:**
```
-- All requested installations completed successfully
-- Installed X items
-- Finished release [..]
-- test result: ok
-- Artifact rustdesk-direct-ip-linux-x86_64 has been successfully uploaded
```

**Critical: No bindgen errors**
```
❌ Should NOT see:
error[E0609]: no field 'rc_max_quantizer' on type 'aom_codec_enc_cfg'
error[E0560]: struct 'aom_codec_enc_cfg' has no field named 'g_threads'
```

### Overall Success ✅

- [x] Both jobs pass (green checkmarks in Actions)
- [x] Both artifacts uploaded successfully
- [x] No bindgen opaque struct errors in Linux cargo build
- [x] Test suite passes (`test result: ok`)

**Interpretation:** Bindgen issue is local/environmental; GitHub Actions is canonical build path.

---

## Expected Failure Criteria

### Scenario 1: NASM Multipass Error ❌

**Error message:**
```
CMake Error at build/cmake/aom_optimization.cmake:219
  Unsupported nasm: multipass optimization not supported
```

**Meaning:** NASM patch did not apply or multipass check bypass failed

**Action:** 
1. Verify `res/vcpkg/aom/aom-disable-multipass-check.diff` exists
2. Verify `res/vcpkg/aom/portfile.cmake` line 27 references patch
3. Check if patch syntax is compatible with vcpkg in CI

### Scenario 2: Bindgen Opaque Struct Error ❌

**Error messages (Linux cargo build):**
```
error[E0609]: no field 'rc_max_quantizer' on type 'aom_codec_enc_cfg'
  |
  | use of undeclared field 'aom_codec_enc_cfg::rc_max_quantizer'

error[E0560]: struct 'aom_codec_enc_cfg' has no field named 'g_threads'
```

**Meaning:** Bindgen generated opaque structs (missing field definitions)

**Critical Action:**
1. Capture exact error output
2. Compare to local bindgen errors (are they identical?)
3. Extract generated `aom_ffi.rs` from CI logs
4. Compare CI-generated vs local-generated `aom_ffi.rs`
5. Update `docs/FFI_BINDGEN_ANALYSIS.md` with findings
6. **DO NOT implement workarounds** until root cause is understood

### Scenario 3: Test Failure ❌

**Error message:**
```
test result: FAILED
failures:
    [test name]
```

**Meaning:** Test suite found a code issue

**Action:**
1. Identify failing test
2. Capture full test output
3. Check if test is fork-specific or upstream issue

### Scenario 4: Dependencies Not Found ❌

**Error message:**
```
CMake Error: Could not find [library]
```

**Meaning:** vcpkg install failed to build dependencies

**Action:**
1. Check vcpkg output for build errors
2. Verify `res/vcpkg/aom/` and other custom ports exist
3. Check vcpkg commit ID matches (`VCPKG_COMMIT_ID: 120deac306...`)

---

## Exact Git Commands for First Run

### Command 1: Verify Remotes

```bash
git remote -v
```

**Expected output:**
```
fork      https://github.com/xyz99002/rustdesk-direct-ip-remote-support.git (fetch)
fork      https://github.com/xyz99002/rustdesk-direct-ip-remote-support.git (push)
upstream  https://github.com/rustdesk/rustdesk.git (fetch)
upstream  https://github.com/rustdesk/rustdesk.git (push)
```

### Command 2: Check Branch Status

```bash
git status
```

**Expected output:**
```
On branch feature/direct-ip-fork
nothing to commit, working tree clean
```

### Command 3: Push Feature Branch (Triggers CI)

```bash
git push fork feature/direct-ip-fork
```

**Expected output:**
```
Enumerating objects: 47, done.
Counting objects: 100% (47/47), done.
Delta compression using up to 12 threads.
Compressing objects: 100% (22/22), done.
Writing objects: 100% (25/25), X.XX MiB | X.XX MiB/s, done.
Total 47 (delta 25), reused 0 (delta 0), writing 25.XX MiB
...
remote: GitHub has received your request to run this workflow
 * [new branch]      feature/direct-ip-fork -> feature/direct-ip-fork
```

### Command 4: Verify Workflow Trigger

Open GitHub Actions to confirm workflow started:
```
https://github.com/xyz99002/rustdesk-direct-ip-remote-support/actions
```

**Expected:** "Direct-IP Build" workflow run appears within 30 seconds.

---

## Execution Timeline

| Step | Duration | Status |
|------|----------|--------|
| Push branch | < 1 min | ⏳ Manual |
| Workflow trigger | 30 sec | ⏳ Automatic |
| Windows build | 5-10 min | ⏳ Automatic |
| Linux build | 5-10 min | ⏳ Automatic (parallel) |
| Artifact upload | 1-2 min | ⏳ Automatic |
| **Total** | **10-20 min** | ⏳ **Automatic** |

**First run (cold cache):** 15-25 minutes
**Subsequent runs:** 10-20 minutes (with binary cache)

---

## Critical Decision: Bindgen Opaque Struct

### The Question
> **Does the FFI bindgen opaque struct error reproduce in GitHub Actions CI?**

### Possible Outcomes

**Outcome A: CI Succeeds (No Bindgen Errors)**
- ✅ Both jobs pass
- ✅ Both artifacts created
- ✅ cargo build completes without error[E0609] or error[E0560]
- ✅ tests pass

**Result:** Bindgen issue is local/environmental
- Document as such in `docs/FFI_BINDGEN_ANALYSIS.md`
- Declare GitHub Actions as canonical build path
- Proceed to Phase 6: Packaging automation

---

**Outcome B: CI Fails (Bindgen Errors Reproduced)**
- ❌ Linux cargo build fails
- ❌ error[E0609] or error[E0560] in logs
- ❌ cargo test never runs

**Result:** Bindgen issue is CI-reproducible
- Extract exact error from CI logs
- Compare to local errors (same or different?)
- Update `docs/FFI_BINDGEN_ANALYSIS.md` with CI findings
- Present root cause analysis BEFORE implementing workarounds
- Do not proceed to packaging until resolved

---

**Outcome C: CI Fails (Different Error)**
- ❌ NASM multipass error, or
- ❌ vcpkg install error, or
- ❌ unrelated test failure

**Result:** Investigate specific failure
- Capture error logs
- Compare to expected failure scenarios above
- Update documentation
- Retry or implement targeted fix

---

## Post-Execution Report

Once workflow completes, document findings:

```markdown
# CI Execution Results: [DATE] [TIME]

## Windows Build
- Status: [✅ PASSED / ❌ FAILED]
- Duration: [X minutes Y seconds]
- Key findings: [list any notable messages or warnings]

## Linux Build
- Status: [✅ PASSED / ❌ FAILED]
- Duration: [X minutes Y seconds]
- Bindgen errors: [✅ None / ❌ Reproduced / ⚠️ Different errors]
- Key findings: [list any notable messages or warnings]

## Summary
- **Critical Question Answer:** [Bindgen error reproduced in CI? YES/NO]
- **Next Phase:** [Phase 6 Packaging | Root cause investigation | Other]
- **Blocking Issues:** [list or None]
- **Documents Updated:** [list or None]
```

---

## Workflow Documentation Cross-Reference

| Document | Purpose | Read Before |
|----------|---------|-------------|
| [CI_EXECUTION_PLAN.md](CI_EXECUTION_PLAN.md) | Full execution strategy | First run |
| [CI_FIRST_EXECUTION.md](CI_FIRST_EXECUTION.md) | Step-by-step walkthrough | First run |
| [CI_WORKFLOW_VERIFICATION.md](CI_WORKFLOW_VERIFICATION.md) | Detailed workflow validation | Verifying setup |
| [GITHUB_CI_STRATEGY.md](GITHUB_CI_STRATEGY.md) | Strategic context | Understanding why |
| [REPOSITORY_AND_ARTIFACT_MAP.md](REPOSITORY_AND_ARTIFACT_MAP.md) | Full traceability | Long-term reference |
| [FFI_BINDGEN_ANALYSIS.md](FFI_BINDGEN_ANALYSIS.md) | Bindgen investigation | When errors occur |

---

## No Workarounds Until CI Results

**DO NOT implement:**
- ❌ Additional bindgen workarounds
- ❌ Additional NASM workarounds
- ❌ Additional AOM patches
- ❌ AV1 feature changes/gating
- ❌ Environment-specific hacks

**Reason:** These would mask whether the issue is local/environmental or genuinely CI-reproducible.

**Decision point:** After CI results are known, implement targeted fixes based on actual findings.

---

## Ready for GitHub Actions Validation

✅ Remotes configured
✅ Workflow file verified
✅ Artifact paths correct
✅ Cache configured
✅ Documentation complete

**Next action:** `git push fork feature/direct-ip-fork`

**Then:** Monitor workflow at https://github.com/xyz99002/rustdesk-direct-ip-remote-support/actions
