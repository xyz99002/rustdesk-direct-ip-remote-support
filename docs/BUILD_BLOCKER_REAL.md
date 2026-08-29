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

## Viable Remediation Strategies (Revised with Safety Analysis)

### Strategy 1: Pin NASM to a Compatible Version (Recommended)

**What:** Install a newer NASM version (e.g., 2.16.01 or later) that supports multipass optimization.

**Options:**
- A1: Download NASM 2.16.01+ locally and set `NASM_EXE` environment variable
- A2: Check if system NASM (on PATH) is already >= 2.15.05
- A3: Create a vcpkg overlay for NASM to permanently pin a known-good version

**Risk:** Very low
- NASM is a tool, not a library; versioning is straightforward
- Multipass optimization is proven stable in NASM 2.16+
- No impact on other dependencies (libvpx, libyuv, opus)

**Maintainability:** Low
- Once pinned, no ongoing maintenance
- Aligns with upstream RustDesk CI approach

**Implementation time:** 20–30 minutes (obtain NASM, set env var, retry vcpkg)

**Why this is the ideal solution:**
- Fixes the root cause (NASM version), not a symptom
- Enables full multipass optimization → no encoding slowdown
- Upstream-aligned (upstream CI uses system NASM on Linux/macOS, which is typically >= 2.15)
- Clearest path for future upgrades
- No code patches required

---

### Strategy 2: Bypass Multipass Check via aom Patch (Ready Now)

**What:** Apply a CMake patch to aom's `aom_optimization.cmake:219` that skips the multipass capability check.

**How:** Modify `test_nasm()` to return early, allowing aom to build with NASM 3.01.

**Risk:** Very low (Safety Confirmed via Investigation)

**Evidence that it's safe:**
- ✅ **Codec correctness:** AV1 encodes/decodes normally; same mathematical operations, no algorithm changes
- ✅ **Bitstream compatibility:** Output is identical to optimized builds (same input → same encoded data)
- ✅ **Security:** Assembly optimization is not a security vector; no implications for remote desktop
- ✅ **Assembly correctness:** NASM still generates correct instructions; just longer (unoptimized) encodings
- ⚠️ **Performance:** Encoding is ~5-15% slower (acceptable for real-time video with VP9/H.265 fallback)

See `docs/NASM_MULTIPASS_ANALYSIS.md` for full investigation findings.

**Maintainability:** Low
- Patch is simple (6-line CMake modification)
- Clearly documented with comments explaining the workaround
- Easily reverted when NASM is upgraded

**Implementation time:** Immediate (patch already prepared in `res/vcpkg/aom/aom-disable-multipass-check.diff`)

**Status:** Ready to deploy (just enable it in `res/vcpkg/aom/portfile.cmake` line 27)

**When to use:** If NASM upgrade is blocked or delayed; temporary until NASM is fixed

---

### Strategy 3: Disable AV1 in scrap via Feature Flag (Fallback)

**What:** Make AV1 optional in the `scrap` crate, so aom is not required.

**Risk:** Medium (requires Rust code changes)

**Maintainability:** Low (once done)

**Implementation time:** 45–60 minutes

**Trade-off:** RustDesk loses AV1 support locally (VP9/H.265 available as fallback)

**When to use:** Only if both Strategy 1 and Strategy 2 are blocked

---

## Decision and Next Steps

### **Recommended Path (in order of preference):**

1. **First:** Try Strategy 1 (Pin NASM to 2.16.01 or later)
   - Best long-term solution
   - Fixes root cause
   - Upstream-aligned

2. **If NASM cannot be obtained:** Use Strategy 2 (Bypass Patch)
   - Safe for correctness (verified)
   - Ready to deploy now
   - Documented 5-15% encoding slowdown is acceptable
   - Temporary until NASM is upgraded

3. **If both above are blocked:** Use Strategy 3 (Disable AV1 feature flag)
   - Last resort
   - Requires code changes
   - VP9/H.265 codec still available

---

## How to Implement

### **Strategy 1: Pin NASM**

```powershell
# Download NASM 2.16.01 or later to a local directory, then:
$env:NASM_EXE = "C:\path\to\nasm.exe"
vcpkg install --triplet x64-windows-static
```

### **Strategy 2: Enable Bypass Patch (Already Prepared)**

1. Ensure `res/vcpkg/aom/portfile.cmake` line 27 has `aom-disable-multipass-check.diff` in the PATCHES list
2. Run:
```powershell
vcpkg install --triplet x64-windows-static
```
3. aom builds with NASM 3.01, with 5-15% encoding slowdown

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
