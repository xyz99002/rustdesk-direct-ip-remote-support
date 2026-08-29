# First CI Execution: Step-by-Step Guide

**Date:** 2026-08-29
**Status:** Ready for execution
**Branch:** feature/direct-ip-fork (all changes committed locally)

---

## Prerequisites Checklist

- [x] Local repository clean (working tree)
- [x] All commits created (c5217fec8 is the latest)
- [x] Workflow file created (.github/workflows/direct-ip-build.yml)
- [x] CI strategy documented (docs/GITHUB_CI_STRATEGY.md)
- [ ] GitHub fork repository created (user action required)
- [ ] Git remote configured (user action required)

---

## Step 1: Create Direct-IP Fork on GitHub (If Not Exists)

### GitHub Web UI Method

1. Go to: https://github.com/[YOUR_ORG_OR_USERNAME]
2. Click: "+" menu → "New repository"
3. Fill in:
   - **Repository name:** `rustdesk-direct-ip`
   - **Description:** "Direct-IP enforcement fork of RustDesk 1.4.9"
   - **Visibility:** Public (or Private if preferred)
   - **Initialize:** No README, no .gitignore, no license (fork will have these)
4. Click: "Create repository"

### Result
- **New repository URL:** `https://github.com/[YOUR_ORG_OR_USERNAME]/rustdesk-direct-ip.git`
- **Settings → Actions:** Enabled (default)
- **Ready to receive pushes**

---

## Step 2: Add Fork as Git Remote

**Currently configured remote:**
```bash
git remote -v
# upstream    https://github.com/rustdesk/rustdesk.git
```

**Add fork remote:**
```bash
git remote add fork https://github.com/[YOUR_ORG_OR_USERNAME]/rustdesk-direct-ip.git
```

**Verify:**
```bash
git remote -v
# Should show:
# fork      https://github.com/[YOUR_ORG_OR_USERNAME]/rustdesk-direct-ip.git
# upstream  https://github.com/rustdesk/rustdesk.git
```

---

## Step 3: Push feature/direct-ip-fork Branch

**Current state:**
```bash
git status
# On branch feature/direct-ip-fork
# nothing to commit, working tree clean
```

**Latest commit:**
```bash
git log -1 --oneline
# c5217fec8 Add comprehensive repository and artifact mapping documentation
```

**Push to fork:**
```bash
git push fork feature/direct-ip-fork
```

**Expected output:**
```
Enumerating objects: 47, done.
Counting objects: 100% (47/47), done.
Delta compression using up to X threads.
Compressing objects: 100% (22/22), done.
Writing objects: 100% (25/25), X.XX MiB | X.XX MiB/s, done.
Total 47 (delta 25), reused 0 (delta 0), writing 25.XX MiB
...
remote: GitHub has received your request to run this workflow
 * [new branch]      feature/direct-ip-fork -> feature/direct-ip-fork
```

---

## Step 4: Monitor Workflow Execution

### Navigate to Actions

1. Go to: `https://github.com/[YOUR_ORG_OR_USERNAME]/rustdesk-direct-ip/actions`
2. Click: "Direct-IP Build" workflow run (should appear immediately after push)
3. Observe:
   - **build-windows-direct-ip** job (runs on windows-2022)
   - **build-linux-direct-ip** job (runs on ubuntu-24.04)

### Real-Time Monitoring

**Windows job steps to watch:**
1. `Export GitHub Actions cache environment variables`
2. `Checkout source code`
3. `Install LLVM and Clang`
4. `Install flutter`
5. `Install Rust toolchain`
6. `Setup vcpkg with Github Actions binary cache`
7. `Install vcpkg dependencies` ← **CRITICAL: Watch for NASM/aom errors**
8. `Build rustdesk`
9. `Upload Windows Build Artifacts`

**Linux job steps to watch:**
1. `Free Disk Space (Ubuntu)`
2. `Checkout source code`
3. `Install prerequisites` (includes NASM)
4. `Setup vcpkg with Github Actions binary cache`
5. `Install vcpkg dependencies`
6. `Install Rust toolchain`
7. `Build` ← **CRITICAL: Watch for bindgen opaque struct errors**
8. `Run tests`
9. `Upload Linux Build Artifacts`

---

## Step 5: Capture Execution Results

### If Jobs Complete Successfully ✅

**Expected in workflow logs:**

Windows:
```
-- Elapsed time to handle rustdesk build (system-specific)
-- All requested installations completed successfully
-- Finished 'release' mode [..]
-- Artifact rustdesk-direct-ip-windows-x86_64 has been successfully uploaded
```

Linux:
```
-- All requested installations completed successfully
-- Finished release [..]
-- test result: ok
-- Artifact rustdesk-direct-ip-linux-x86_64 has been successfully uploaded
```

**Action:**
1. Document successful completion
2. Download artifacts (optional, for manual testing)
3. Proceed to Phase 6: Packaging automation

### If Jobs Fail ❌

**Capture exact error:**
1. Click failing job (Windows or Linux)
2. Scroll to failing step
3. Copy full error output
4. Look for:
   - NASM multipass error → `CMake Error at aom_optimization.cmake:219`
   - Bindgen error → `error[E0609]: no field` or `error[E0560]`
   - Other build error → note the exact message

**Action:**
1. Save error logs to local file
2. Update `docs/FFI_BINDGEN_ANALYSIS.md` with CI results
3. If bindgen error: compare to local error output
4. Report findings before implementing workarounds

---

## Exact Command Sequence

**For copy-paste execution:**

```bash
# Step 1: Add remote
git remote add fork https://github.com/[YOUR_ORG_OR_USERNAME]/rustdesk-direct-ip.git

# Step 2: Verify remote added
git remote -v

# Step 3: Push branch to trigger CI
git push fork feature/direct-ip-fork

# Step 4: Watch progress (opens in browser)
# Go to: https://github.com/[YOUR_ORG_OR_USERNAME]/rustdesk-direct-ip/actions
```

**That's it.** The workflow will run automatically on push.

---

## What NOT to Do

❌ Do **not** modify code before CI completes
- Want to keep local state stable for comparing CI vs local bindgen output

❌ Do **not** implement bindgen workarounds
- Wait for CI results to determine if issue is environmental

❌ Do **not** create a PR yet
- First CI run is validation; use push + Actions tab for now

❌ Do **not** delete the fork repository
- Artifacts stay available for 90 days

---

## After CI Execution

### Report Expected Format

Once workflow completes (10-20 minutes), create a summary:

```markdown
# CI Execution Results: [DATE] [TIME]

## Windows Build
- Status: [✅ PASSED / ❌ FAILED]
- Duration: [minutes:seconds]
- Key findings: [list observations]
- Artifacts: [captured artifact names and sizes if available]

## Linux Build
- Status: [✅ PASSED / ❌ FAILED]
- Duration: [minutes:seconds]
- Bindgen opaque struct errors: [✅ None / ❌ Reproduced / ⚠️ Different errors]
- Key findings: [list observations]
- Artifacts: [captured artifact names and sizes if available]

## Summary
- Bindgen issue: [Is local/environmental | Reproduced in CI | Platform-specific]
- Next phase: [Phase 6 Packaging | Root cause investigation | Other]
- Blocking issues: [list or None]
```

---

## Contact/Support

If issues arise during setup:

1. **Git push fails** → Check remote URL, verify network access
2. **Workflow doesn't trigger** → Check branch name matches exactly: `feature/direct-ip-fork`
3. **Workflow fails immediately** → Check `.github/workflows/direct-ip-build.yml` is in the fork
4. **Jobs run but fail** → Capture full logs; compare to `docs/CI_EXECUTION_PLAN.md` failure scenarios

---

## Timeline

**Estimated:**
- Setup (Steps 1-2): 5 minutes
- Push (Step 3): < 1 minute
- Workflow execution (Step 4): 10-20 minutes
- **Total: ~15-25 minutes for first run**

**Total time to answer: "Does the bindgen issue reproduce in CI?"**
- **25-30 minutes from now**
