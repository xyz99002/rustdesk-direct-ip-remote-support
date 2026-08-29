# Build Blocker - Real Root Cause (Discovered 2026-08-29)

**Status:** Confirmed — The actual blocker is NASM version, not aom version.

**Discovery:** During execution of Step 1 (vcpkg dependency resolution), the aom 3.9.1 downgrade strategy FAILED with the same NASM multipass error, proving the previous assumption was incorrect.

---

## Evidence

**Test performed:**
- Portfile.cmake was correctly modified to default to aom 3.9.1 (ref `8ad484f8a18ed1853c094e7d3a4e023b2a92df28`)
- Ran: `vcpkg install --triplet x64-windows-static`
- vcpkg correctly downloaded aom 3.9.1 source (confirmed by the git ref in the output)
- CMake configure still failed at `aom_optimization.cmake:219`

**Error message (from aom 3.9.1 build):**
```
-- Found assembler: C:/Users/arvindkumarp/vcpkg/downloads/tools/nasm/nasm-3.01/nasm.exe
CMake Error at build/cmake/aom_optimization.cmake:219 (message):
  Unsupported nasm: multipass optimization not supported.
```

**Conclusion:**
- BOTH aom 3.9.1 AND aom 3.12.1 require NASM multipass optimization support
- The issue is NOT a version-specific aom problem
- The issue IS a fundamental incompatibility between NASM 3.01 and AOM (any recent version)

---

## Revised Root Cause

**The real blocker:** NASM 3.01 from vcpkg does not support multipass optimization mode.

**Why it matters:** aom's CMake build system requires multipass NASM capabilities, and hard-fails if unavailable. This is not a "nice to have" optimization; it's a required build feature.

**Why previous analysis was wrong:** The BUILD_BLOCKER_CONFIRMATION.md assumed aom 3.9.1 had lower optimization requirements than 3.12.1, based on it being an older release. This assumption was **incorrect** — both versions require the same NASM capability.

---

## Viable Remediation Strategies (Revised Ranking)

### Strategy 1: Pin NASM to a Compatible Version (Revised Priority: NOW #1)

**What:** Install a newer NASM version (e.g., 2.15.05, 2.16.01, or late 3.x) that supports multipass optimization.

**Options:**
- A1: Use system NASM (if available and recent enough)
- A2: Create a vcpkg overlay for NASM and pin to a known-good version
- A3: Set `NASM_EXE` environment variable to point to a pre-installed NASM

**Risk:** Very low (NASM is a tool, not a library; versioning is straightforward)

**Maintainability:** Low (once pinned, no ongoing maintenance)

**Implementation time:** 10–20 minutes

**Why this is now the correct choice:**
- The problem is fundamentally about NASM version, not aom version
- Pinning NASM fixes the root cause, not just a symptom
- Other tools (ffmpeg, libvpx) don't have multipass requirements, so NASM version won't break them
- Upstream RustDesk likely uses a newer NASM on their CI machines

**Command to verify:**
```
nasm --version
# Expected output should indicate support for multipass
# or the version should be > 3.01
```

---

### Strategy 2: Disable AV1 in scrap via Feature Flag (Revised Priority: #2)

**What:** Make AV1 optional in the `scrap` crate build system, so aom is not required.

**Risk:** Medium (requires Rust code changes, risk of partial disabling)

**Maintainability:** Low (once done)

**Implementation time:** 45–60 minutes

**Why this is a fallback:**
- Solves the blocker by eliminating the aom requirement entirely
- Clean Rust approach using feature flags (standard practice)
- No version-pinning burden
- Cleaner for future upgrades

**Trade-off:** RustDesk loses AV1 support locally (falls back to VP9/H.265)

---

### Strategy 3: Revert aom Downgrade + Strategy 1

**What:** Revert the portfile.cmake change back to aom 3.12.1 (since 3.9.1 doesn't help anyway), then fix NASM.

**Implementation:**
- Delete the 2026-08-29 modification from portfile.cmake (set `USE_AOM_312=1` or revert the conditional)
- Implement Strategy 1 (pin NASM)

**Rationale:** Simplest path — use aom 3.12.1 (what upstream uses) with a working NASM version.

---

## Immediate Action Required

**Recommendation: Implement Strategy 1 (Pin NASM)** combined with **reverting the failed aom downgrade**.

**Steps:**

1. **Revert portfile.cmake** back to defaulting to aom 3.12.1 (or delete the 2026-08-29 changes):
   ```cmake
   if(DEFINED ENV{USE_AOM_391})
       # 3.9.1 path
   else()
       # 3.12.1 path (default)
   endif()
   ```

2. **Determine available NASM version:**
   - Try to locate a system NASM installation that's newer than 3.01
   - Or download a newer NASM manually (e.g., from nasm.us)
   - Recommended: NASM 2.16.01 or later

3. **Set NASM_EXE environment variable** (temporary test):
   ```powershell
   $env:NASM_EXE = "C:\path\to\nasm.exe"
   vcpkg install --triplet x64-windows-static
   ```

4. **If that works:** Create a vcpkg overlay for NASM to make the fix permanent

5. **Re-run vcpkg install** and proceed with build verification

---

## Documentation Updates Needed

1. **BUILD_BLOCKER_ANALYSIS.md** — Mark Strategy 1 (aom 3.9.1 downgrade) as **FAILED** and replace with Strategy 1 (NASM pinning)
2. **BUILD_BLOCKER_CONFIRMATION.md** — Document the discovery that both aom 3.9.1 and 3.12.1 require multipass
3. **FULL_BUILD_VERIFICATION.md** — Add a check for NASM version before Step 1

---

## Next Steps (Upon Approval)

1. Locate/install a NASM version >= 2.15.05 or later 3.x
2. Set `NASM_EXE` environment variable
3. Retry: `vcpkg install --triplet x64-windows-static`
4. Confirm aom builds successfully
5. Continue with Steps 2–5 of FULL_BUILD_VERIFICATION.md (cargo, tests, flutter)

**Expected outcome:** aom builds without errors; full build chain proceeds.
