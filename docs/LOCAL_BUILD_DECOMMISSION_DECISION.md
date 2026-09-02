# Local Build Decommission — Final Decision & Timeline

**Date:** 2026-09-01  
**Decision:** APPROVED — proceed with local build tool decommission per `LOCAL_BUILD_DECOMMISSION_PLAN.md`  
**Timeline:** Post-Release Hardening Phase 1

---

## Decision Summary

**GitHub Actions is the canonical build system.** Local build tools (vcpkg, Flutter, LLVM, NASM) are being decommissioned to streamline the developer machine, recover ~14 GB of disk, and eliminate the friction of maintaining two build paths.

**What developers keep:** Git, IDE, Claude Code, optionally Rust/Cargo.  
**What developers remove:** vcpkg (6.5 GB), Flutter SDK (0.9 GB), LLVM (2.8 GB), Windows SDK C++ workload (optional, risky).

**Impact:** 
- ✅ Cleaner developer machines
- ✅ Reduced onboarding friction for new developers
- ✅ Single, well-tested CI build path (no local-vs-CI divergence)
- ✅ ~14 GB disk recovery
- ⚠️ Requires discipline: all real builds go through CI, not local
- ⚠️ Emergency debugging is now Linux-sciter-specific (requires CI container or Docker emulation)

---

## Approved Classification & Removal Sequence

### Tier 1: Safe to Remove Now (Low Risk)

| Tool | Rationale | When | Disk Recovered |
|---|---|---|---|
| **vcpkg** checkout | GitHub Actions maintains its own per-workflow instance. NASM issue already resolved. Local copy only served investigation; CI is canonical. | Anytime after Release Hardening Phase 1 passes | 6.5 GB |
| **Flutter SDK** | `subosito/flutter-action` installs fresh pinned Flutter in every CI build. Local Flutter only useful if you want local `flutter analyze`/`flutter test` on Dart code — not part of the required dev workflow. | Anytime after Release Hardening Phase 1 passes | 0.9 GB |

**Uninstall command:** See `LOCAL_TOOL_REMOVAL_CHECKLIST.md` § Removable tier

---

### Tier 2: Optional (Your Choice)

| Tool | Keep-vs-Remove decision | Disk potential |
|---|---|---|
| **Rust/Cargo** | **KEEP (recommended).** Fast local `cargo check` / `cargo test --lib` on pure-Rust logic (fork_config.rs) catches mistakes in seconds vs. waiting on CI. Minimal footprint (4 GB), general-purpose dev tool. Cost of keeping > cost of removing. | Remove: 4.0 GB |
| **LLVM/Clang** | **REMOVE (recommended).** Local FFI bindgen was established as unreliable (environment-specific, CI already produces consistent output). Only value is in attempting hand-regen — a rare, low-value case. | 2.8 GB |
| **CMake** | **KEEP (low priority).** Minimal footprint, general-purpose dev tool. Low value in removing it specifically for this migration. | N/A |

---

### Tier 3: Do Not Touch (High Risk)

| Tool | Why not | Alternative |
|---|---|---|
| **Visual Studio Build Tools / C++ workload** | Shared system component used by other, unrelated work on this machine. Removing it risks breaking unrelated projects. Modest disk win relative to breakage risk. | Do not remove; treat as "belongs to the machine," not "belongs to RustDesk." |

---

## Timeline: Post-Release Hardening Phase 1

### **After Release Hardening Phase 1 lands (post-release, T+2 weeks)**

1. **Verify** GitHub Actions fully owns the build path:
   - `flutter-ci.yml` (routine push to master): all jobs passing
   - `release.yml` (test release with a test version): all jobs passing
   - No critical regressions from Node.js upgrades or finalize-release changes

2. **Remove Tier 1 tools** (vcpkg + Flutter):
   - Run commands from `LOCAL_TOOL_REMOVAL_CHECKLIST.md`
   - Verify no stale PATH entries or env vars remain
   - Document disk freed in a follow-up commit

3. **Optionally remove Tier 2 tools:**
   - **LLVM:** if you don't attempt local FFI bindgen work (recommended: yes, remove it)
   - **Rust:** if you prefer waiting on CI for all checks (recommended: no, keep it)

4. **Update docs** to reflect the new default:
   - `DEVELOPER_ONBOARDING.md`: emphasize "GitHub Actions is canonical"
   - `QUICK_START_FOR_NEW_DEVELOPER.md`: machine setup section lists minimal install
   - `BUILD_VERIFICATION_RESULTS.md`: mark local-build sections as historical
   - `LOCAL_BUILD_DECOMMISSION_PLAN.md`: close out as "executed" with date

5. **Celebrate:** ~14 GB freed, one build path to maintain instead of two.

---

## Constraints (Unchanged by This Decision)

**Do not:**
- Change the Direct-IP architecture ✅
- Change transport or authentication ✅
- Change Support/Desktop workflows ✅
- Change role configuration (TOML-based, one binary) ✅

**This decommission only affects:** Local developer machine setup, not product behavior or CI infrastructure.

---

## Rollback Plan

If a developer needs to restore local builds after decommissioning:

1. **vcpkg:** `git clone https://github.com/microsoft/vcpkg C:\Users\...\vcpkg && bootstrap-vcpkg.bat` (rebuilds from scratch)
2. **Flutter:** `git clone https://github.com/flutter/flutter.git -b stable C:\Users\...\flutter` (reinstalls)
3. **LLVM:** Download installer from https://github.com/llvm/llvm-project/releases

All tools are open-source and freely available; re-installation is cheap compared to the disk recovered.

---

## Sign-Off

**Decision Made:** 2026-09-01  
**Implementation Window:** Post-Release Hardening Phase 1  
**Scope:** Local developer machine configuration only  
**Risk Level:** LOW (reversible, no product changes)

**Approved by:** Release Hardening Phase 1 workstream  
**Related:** `LOCAL_BUILD_DECOMMISSION_PLAN.md`, `LOCAL_TOOL_REMOVAL_CHECKLIST.md`
