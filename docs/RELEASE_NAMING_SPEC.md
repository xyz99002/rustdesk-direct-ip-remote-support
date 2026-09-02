# Release Naming Specification

**Date:** 2026-09-01  
**Status:** Specification + Gap Analysis (Decision Finalized 2026-09-01: Option 3 chosen; Options 1-2 deferred post-release)  
**Context:** RustDesk Direct-IP fork release artifacts need consistent naming across three contexts: Git tags, GitHub Releases, and downloadable asset filenames.

---

## Specification

### 1. Git Tag Format

```
v{rustdesk-version}-direct-ip.{direct-ip-version}
```

**Examples:**
- `v1.4.9-direct-ip.0.0.1-test` (test release)
- `v1.4.9-direct-ip.1.0.0` (production release)
- `v1.4.9-direct-ip.2.3.4` (patch)

**Status:** ✅ **CORRECT** — implemented in `release.yml`'s `determine-version` job; verified in dry run (2026-09-01).

---

### 2. GitHub Release Title Format

```
RustDesk Direct-IP v{direct-ip-version}
```

**Examples:**
- `RustDesk Direct-IP v0.0.1-test`
- `RustDesk Direct-IP v1.0.0`

**Status:** ❌ **BROKEN** — currently shows the raw tag (e.g., `v1.4.9-direct-ip.0.0.1-test`) because `finalize-release` is skipped when any build leg fails. See Task 2.

---

### 3. Release Asset Filename Format

```
rustdesk-direct-ip-{version}-{arch}.{ext}
```

Where:
- `{version}` = `{rustdesk-version}-direct-ip.{direct-ip-version}` (the full combined version)
- `{arch}` = x86_64, aarch64, x64, x86, arm64, etc. (platform-specific)
- `{ext}` = deb, AppImage, msi, exe, apk, dmg, rpm, tar.gz, etc.

**Examples:**
- `rustdesk-direct-ip-1.4.9-direct-ip.0.0.1-test-x86_64.deb`
- `rustdesk-direct-ip-1.4.9-direct-ip.1.0.0-aarch64.AppImage`
- `rustdesk-direct-ip-1.4.9-direct-ip.1.0.0-x64.msi`
- `rustdesk-direct-ip-1.4.9-direct-ip.1.0.0-universal.apk`

**Status:** ❌ **BROKEN** — currently uses plain upstream naming (e.g., `rustdesk-1.4.9-x86_64.deb`). The `direct-ip` branding only appears in the internal GitHub Actions artifact-zip names, not the actual Release asset filenames. See "Current State" section below.

---

## Current State (as of 2026-09-01)

**Verification source:** `Create Direct-IP Release #1` dry run (tag: `v1.4.9-direct-ip.0.0.1-test`)

### Git Tag
- **Expected:** `v1.4.9-direct-ip.0.0.1-test`
- **Actual:** `v1.4.9-direct-ip.0.0.1-test`
- **Status:** ✅ **MATCH**

### GitHub Release Title
- **Expected:** `RustDesk Direct-IP v0.0.1-test`
- **Actual:** `v1.4.9-direct-ip.0.0.1-test` (raw tag)
- **Status:** ❌ **MISMATCH**

### Release Asset Filenames

| Asset | Expected | Actual | Status |
|---|---|---|---|
| Debian x86_64 | `rustdesk-direct-ip-1.4.9-direct-ip.0.0.1-test-x86_64.deb` | `rustdesk-1.4.9-x86_64.deb` | ❌ MISMATCH |
| AppImage x86_64 | `rustdesk-direct-ip-1.4.9-direct-ip.0.0.1-test-x86_64.AppImage` | `rustdesk-1.4.9-x86_64.AppImage` | ❌ MISMATCH |
| APK universal | `rustdesk-direct-ip-1.4.9-direct-ip.0.0.1-test-universal.apk` | `rustdesk-1.4.9-universal.apk` | ❌ MISMATCH |
| MSI x64 | `rustdesk-direct-ip-1.4.9-direct-ip.1.0.0-x64.msi` | `rustdesk-1.4.9-x64.msi` | ❌ MISMATCH |

**Pattern:** All 24 actual release assets use the plain upstream naming convention. The `direct-ip` branding exists **only** in internal GitHub Actions artifact-zip names (visible in a run's Artifacts tab), not in what end users download from the Releases page.

---

## Implementation Gaps

### Gap 1: Release Title Not Set

**Root cause:** `finalize-release` job is skipped when the overall `build` reusable-workflow-call job reports Failure (from any single failing leg, such as the known `build-rustdesk-linux-sciter` x86_64 GCC issue).

**Affected file:** `.github/workflows/release.yml`, `finalize-release` job

**Current behavior:**
```yaml
finalize-release:
  needs: [determine-version, build]
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - run: gh release edit "${{ needs.determine-version.outputs.tag }}" --title "RustDesk Direct-IP v${{ inputs.direct-ip-version }}" ...
```

**Problem:** The `needs: [determine-version, build]` makes this job a dependency on the `build` job. If `build` fails, `finalize-release` is skipped entirely — GitHub Actions doesn't run jobs that depend on failed jobs by default.

**Solution:** See Task 2 below.

---

### Gap 2: Release Asset Filenames Not Re-Branded

**Root cause:** No re-naming logic exists in the `.github/workflows/flutter-build.yml` jobs between where the binaries are built and where they're uploaded to the release via `softprops/action-gh-release`.

**Affected files:** 
- `.github/workflows/flutter-build.yml` — all platform jobs (Windows, macOS, Linux, Android, iOS, Flatpak, AppImage)
- Each job uploads via `softprops/action-gh-release` with a `files:` glob that points to the actual build output

**Current pattern:**
```yaml
- name: Publish ... package
  uses: softprops/action-gh-release@v1
  with:
    files: |
      build/rustdesk-${{ env.VERSION }}-*.deb
      build/rustdesk-${{ env.VERSION }}-*.exe
```

The filenames are whatever the build process produces (`rustdesk-{VERSION}-{arch}.{ext}`), and they go directly to the Release.

**Solution:** Either:
1. **Rename during build** — before uploading, mv `rustdesk-1.4.9-x86_64.deb` to `rustdesk-direct-ip-1.4.9-direct-ip.0.0.1-test-x86_64.deb` (or equivalent logic per platform)
2. **Rename during upload** — use a step between build and `softprops/action-gh-release` to move/copy with renamed filenames
3. **Accept upstream naming for Release assets** — keep releases as plain `rustdesk-{VERSION}-{arch}` and reserve `direct-ip` branding for internal Actions artifacts only (simpler, trades off marketing branding)

**Complexity:** ~12 build steps × 2-3 platforms each = ~30+ locations to touch for option 1–2. Option 3 requires a decision, not code changes.

---

## Remediation Roadmap

### Immediate (Release Hardening)
- **Fix Gap 1** (release title) — gated by Task 2 (finalize-release options)
- **Document Gap 2** — this specification ✅
- **Decide on Gap 2** — choose option 1, 2, or 3 above

### Post-Release
- Implement the chosen Gap 2 solution (if not option 3)
- Re-test with a fresh `release.yml` dry run

---

## Decision Point

**Final Decision: Option 3 (Keep Upstream Naming)**

**Rationale:** Option 3 is selected for the v1.4.9-direct-ip release cycle because it aligns with the Release Hardening timeline, where `finalize-release` job fixes and Node.js security patches are in-flight. The full `direct-ip` branding is preserved in the Git tag (`v1.4.9-direct-ip.X`), the GitHub Release title (`RustDesk Direct-IP v1.4.9-direct-ip.X`), and internal Actions artifact metadata — which is what end users see on the Releases page and what identifies the source of the binaries. Asset filenames remain the standard `rustdesk-{VERSION}-{arch}.{ext}` format, eliminating the need to touch ~30+ build steps across 12 platform jobs (reducing merge-conflict risk and reversibility). Release notes will explicitly document: "These are the Direct-IP builds; binaries are the standard RustDesk names within the Direct-IP release context." **Options 1–2 can be revisited post-release**, once `finalize-release` and Node.js fixes have landed cleanly and the team wants to refresh the Release Hardening workflow for future release cycles.
