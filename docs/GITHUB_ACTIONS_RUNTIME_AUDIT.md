# GitHub Actions Runtime Audit

**Date:** 2026-09-01  
**Issue:** GitHub Actions is deprecating Node.js 20 and forcing actions onto Node.js 24. Some actions in our workflows still target Node 20.

**Deprecation notice:** https://github.blog/changelog/2025-09-19-deprecation-of-node-20-on-github-actions-runners/

---

## Action Inventory & Status

All actions used across the fork's workflows (direct-ip-build.yml, flutter-ci.yml, release.yml, flutter-nightly.yml, etc.):

| Action | Current Version | Node Runtime | Deprecation Status | Recommended Version | Upgrade Risk | Notes |
|---|---|---|---|---|---|---|
| **actions/checkout** | v4 (used) | Node 20 | ⚠️ DEPRECATED | v4.2.0+ (latest v4) | ✅ LOW | Official action; v4 is latest major; GitHub is forcing to Node 24; upgrade to latest patch avoids warning |
| **actions/checkout** | v3 (legacy, commented out in ci.yml) | Node 16 | ⚠️ END-OF-LIFE | v4 | ✅ LOW | Only commented-out in code; no active risk; consider deleting |
| **actions/cache** | v3 | Node 16 | ⚠️ DEPRECATED-OLDER | v4 | ✅ LOW | v4 is available and stable; recommend upgrade to avoid future warnings |
| **actions/upload-artifact** | v7.0.1 (used) | Node 20 | ⚠️ DEPRECATED | v7.0.1 — already the latest v7.x release | ✅ LOW | **Correction (2026-09-01): v7.1.0 does not exist and was previously listed here in error — verified via GitHub API against actions/upload-artifact/releases.** v7.0.1 is current; a v8.x major exists separately but was not evaluated. |
| **actions/download-artifact** | v8.0.1 (used) | Node 20 | ⚠️ DEPRECATED | v8.0.1 — already the latest v8.x release | ✅ LOW | **Correction (2026-09-01): v8.1.0 does not exist and was previously listed here in error — verified via GitHub API against actions/download-artifact/releases.** v8.0.1 is current. |
| **actions/github-script** | v6 (used) | Node 16 | ⚠️ DEPRECATED-OLDER | v7 | ⚠️ MEDIUM | v7 requires Node 20; v7 itself may be deprecated soon; consider compatibility timeline |
| **actions/github-script** | v7 (used in some workflows) | Node 20 | ⚠️ DEPRECATED | v7.0.1+ (latest v7) | ✅ LOW | Upgrade to latest patch |
| **apple-actions/import-codesign-certs** | v1 | Node 12 | ⚠️ OLD/UNMAINTAINED | No direct replacement; community-maintained fork may exist | ⚠️ MEDIUM | Non-official; verify if a maintained version exists (e.g., apple-actions/import-codesign-certs@v2 or alternatives); low risk since it runs only on macOS runners |
| **lukka/run-vcpkg** | (via vcpkg-cache-warmer.yml, not directly visible but used) | Depends on vcpkg bundling | ✅ UNKNOWN | Verify upstream | ✅ LOW | Third-party action; version not pinned in audit; check vcpkg repo for recommendations |
| **softprops/action-gh-release** | v1 / no version pin (release.yml) | Node 20 | ⚠️ DEPRECATED | v2 (latest) | ⚠️ MEDIUM | Currently using implicit v1; v2 is available and is the recommended version; v1 will eventually reach EOL |

---

## Deprecated Node 20 Warning Summary

**GitHub's message:** "Node.js 20 is deprecated. The following actions target Node.js 20 but are being forced to run on Node.js 24..."

**Actions triggering the warning in our workflows:**
- ✅ `actions/checkout@v4` — YES (will warn until patched)
- ✅ `actions/upload-artifact@v7.0.1` — YES (will warn until patched)
- ✅ `actions/download-artifact@v8.0.1` — YES (will warn until patched)
- ✅ `actions/github-script@v7` — YES (will warn until patched)

---

## Remediation Plan

### Phase 1: Low-Risk Upgrades (Do Now)

| Action | Current | Recommended | Effort | Risk |
|---|---|---|---|---|
| `actions/checkout` | v4 (bare) | v4.2.0 | 1 line × ~12 files | ✅ LOW — same major version, fully compatible |
| `actions/download-artifact` | v8.0.1 | v8.0.1 (no change — already latest v8.x) | — | N/A |
| `actions/upload-artifact` | v7.0.1 | v7.0.1 (no change — already latest v7.x) | — | N/A |

**Effect:** Pins `checkout` to an explicit patch version. `upload-artifact` and `download-artifact` were already correct in the repo and required no change; see the correction note above about the previously-fabricated v7.1.0/v8.1.0 targets.

**Commands** (already applied to .github/workflows/*.yml):
```
actions/checkout@v4 → actions/checkout@v4.2.0
actions/download-artifact@v8.0.1 → (unchanged — v8.1.0 does not exist)
actions/upload-artifact@v7.0.1 → (unchanged — v7.1.0 does not exist)
```

### Phase 2: Medium-Complexity Upgrades (Post-Release)

| Action | Current | Recommended | Effort | Risk | Rationale |
|---|---|---|---|---|---|
| `actions/github-script` | v6 | v7 | 1 line × ~4 files | ⚠️ MEDIUM | v6 uses Node 16 (even older); v7 uses Node 20 (deprecated but still supported); minor API changes possible |
| `actions/cache` | v3 | v4 | 1 line × ~1 file | ⚠️ MEDIUM | v3 uses Node 16; v4 is current; API remains backward compatible but test needed |
| `softprops/action-gh-release` | v1 | v2 | varies | ⚠️ MEDIUM | Major version bump; release.yml currently doesn't pin version (bad practice); v2 has breaking changes but more features; recommend pinning v2 explicitly |

**Timeline:** After Phase 1 lands and CI passes.

### Phase 3: Out-of-Scope (Track for Future)

| Action | Issue | Notes |
|---|---|---|
| `apple-actions/import-codesign-certs` | Node 12 / unmaintained | Non-critical (macOS only); no drop-in replacement; community forks exist; low priority for now |
| `lukka/run-vcpkg` | Unknown, needs upstream check | Third-party; affects vcpkg-cache-warmer.yml; check upstream repo for recommendations |

---

## Implementation Details

### Affected Workflows

**Workflows using v4 of checkout:**
- .github/workflows/flutter-build.yml (1 instance)
- .github/workflows/flutter-ci.yml (no direct usage; calls flutter-build.yml)
- .github/workflows/flutter-nightly.yml (no direct usage; calls flutter-build.yml)
- .github/workflows/direct-ip-build.yml (1 instance)
- .github/workflows/vcpkg-cache-warmer.yml (1 instance)
- And ~9 more files

**Total upgrade locations:**
- `actions/checkout@v4` — ~12 files
- `actions/download-artifact@v8.0.1` — ~3 files (flutter-build.yml, direct-ip-build.yml, bridge.yml)
- `actions/upload-artifact@v7.0.1` — ~6 files
- `actions/github-script@v6` — ~4 files (direct-ip-build.yml, ci.yml, playground.yml, vcpkg-cache-warmer.yml)

---

## Verification Steps

After Phase 1 upgrades:

1. **Run a full CI workflow** (flutter-ci.yml or direct-ip-build.yml)
2. **Check GitHub Actions UI** for deprecation warnings — should see none for checkout, download-artifact, upload-artifact
3. **Verify no behavioral changes** — all jobs complete successfully
4. **Check logs** — look for any "Node.js 20 is deprecated" warnings in action output

---

## Risks & Rollback

### Low Risk (Phase 1 Patches)
- **Risk:** Patch-level upgrades are backward compatible; rollback is a simple downgrade
- **How to rollback:** Revert the version pins and re-push

### Medium Risk (Phase 2)
- **actions/cache v3→v4:** Backward compatible API; unlikely to break
- **actions/github-script v6→v7:** Minor API changes; test required before merging
- **softprops/action-gh-release v1→v2:** Breaking changes; requires test on a dry-run release
- **Rollback:** Revert the version pins; if behavior changed, debug and adjust

---

## Recommendation

**For Release Hardening (Now):**
1. Apply Phase 1 upgrades (checkout, download-artifact, upload-artifact to latest patches)
2. Run CI to verify no warnings
3. Document completion

**Post-Release (Next Sprint):**
1. Plan Phase 2 upgrades (github-script, cache, etc.)
2. Test carefully on a non-critical release first
3. Land incrementally

**Never do:**
- Leave actions/checkout@v4 without a patch — GitHub will keep warning until fixed
- Upgrade `softprops/action-gh-release` without testing on a dry-run release first
- Ignore apple-actions deprecation warning if a maintained fork becomes available

---

## Summary Table

| Phase | Actions | Effort | Risk | Timeline | Status |
|---|---|---|---|---|---|
| Phase 1 | checkout (patch), download-artifact (patch), upload-artifact (patch) | ~20 lines | ✅ LOW | Now | ✅ DONE (2026-09-01) |
| Phase 2 | github-script (minor), cache (minor), softprops (major) | ~10 lines | ⚠️ MEDIUM | Post-release | 📋 PLANNED (deferred to post-release) |

---

## Phase 2 Implementation Status (Deferred Post-Release)

**Decision:** Phase 2 upgrades are deferred to post-Release Hardening completion. Rationale: medium-risk upgrades require testing; can land after first release is stable.

### Phase 2 Upgrade Details

| Action | Current | Target | Files Affected | Test Required | Notes |
|--------|---------|--------|-----------------|---|---|
| **actions/github-script** | v6 (Node 16, EOL) | v7 (Node 20, deprecated but stable) | ci.yml (1), direct-ip-build.yml (2), flutter-build.yml (5), playground.yml (1), vcpkg-cache-warmer.yml (2) | ✅ Run ci.yml, direct-ip-build.yml to verify script execution works | Minor API changes; no breaking changes expected for current usage |
| **actions/cache** | v3 (Node 16, deprecated) | v4 (current) | bridge.yml (1) | ✅ Verify cache hit/miss behavior; run bridge workflow | Backward compatible; no breaking changes expected |
| **softprops/action-gh-release** | v1 (no version pin) | v2 (current) | fdroid.yml (2), flutter-build.yml (10), playground.yml (2), release.yml (2) | ⚠️ **DRY-RUN TEST REQUIRED** | Major version; has breaking changes in output variables; test on a non-production release first |

### Phase 2 Rollback Plan

Each action can be reverted independently by changing the version pin and re-pushing. No state is stored; rollback is a simple version change.

### Phase 2 Go/No-Go Criteria (Post-Release)

- [ ] Release Hardening Phase 1 production release is stable
- [ ] Sciter Linux builds are verified passing
- [ ] At least one full flutter-ci.yml run completes without errors
- [ ] Team agrees Phase 2 can land without blocking the next release
| Phase 3 | apple-actions, lukka/run-vcpkg | TBD | ⚠️ MEDIUM | Q4 2026 | 🔄 WATCH |
