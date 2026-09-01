# Release Finalization Behavior: Options Analysis

**Date:** 2026-09-01  
**Issue:** `finalize-release` job is skipped when the `build` reusable-workflow-call job reports Failure, leaving the GitHub Release un-finalized (with raw tag as title, marked Pre-release, and no release notes).

**Context:** `release.yml` has three sequential jobs:
1. `determine-version` — computes the release tag
2. `build` — calls `flutter-build.yml` (which contains ~12 internal jobs across all platforms)
3. `finalize-release` — sets the Release title, notes, and marks non-prerelease

**Problem:** If ANY single internal job in `build` fails (e.g., the known `build-rustdesk-linux-sciter` x86_64 GCC issue), the entire `build` job reports Failure. GitHub Actions then skips `finalize-release` (because it `needs: [determine-version, build]` and the `build` job failed).

**Result:** The Release exists with assets and a tag, but no title/notes and still marked as a prerelease — it's stranded in a half-finalized state.

---

## Current Implementation

```yaml
finalize-release:
  needs: [determine-version, build]
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - name: Set release title and notes
      run: gh release edit "${{ needs.determine-version.outputs.tag }}" \
             --title "RustDesk Direct-IP v${{ inputs.direct-ip-version }}" \
             --notes "..." \
             --prerelease=false
```

The problem: if `build` fails, this entire job is skipped by GitHub Actions' dependency resolution.

---

## Option A: Allow Finalization on Partial Success

**Change:** Remove the hard dependency on `build`; instead, conditionally run `finalize-release` regardless of build success, and document which legs failed (if any) in the release notes.

**Implementation:**

```yaml
finalize-release:
  needs: [determine-version, build]
  if: always()  # Run even if build fails
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - name: Set release title and notes
      run: |
        NOTES="Released artifacts built from commit ${{ needs.determine-version.outputs.commit }}."
        if [ "${{ needs.build.result }}" == "failure" ]; then
          NOTES="${NOTES}\n\n**⚠️ Build Partial Failure**\nSome platforms may not have artifacts. See the Actions run for details."
        fi
        gh release edit "${{ needs.determine-version.outputs.tag }}" \
          --title "RustDesk Direct-IP v${{ inputs.direct-ip-version }}" \
          --notes "${NOTES}" \
          --prerelease=false
```

**Pros:**
- Release is always finalized, users see a proper title/notes even on partial failure
- No race conditions or timing concerns
- Transparent about which legs failed (in notes)

**Cons:**
- A prerelease with missing assets can confuse users ("I can see the release but some binaries are missing")
- May hide build issues if they're not obvious in the notes
- Requires the `build` job to output which legs failed (extra complexity)

**Verdict:** ✅ **Reasonable** — especially with clear notes about partial failure. This is the most user-visible solution.

---

## Option B: Split Release Finalization from Build Success

**Change:** Remove the `needs: [build]` dependency. Instead, have `finalize-release` depend ONLY on `determine-version`, and let the `build` job run independently (not as a dependency of anything).

**Implementation:**

```yaml
finalize-release:
  needs: [determine-version]  # Only depends on tag computation, not build success
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - name: Wait for build artifacts (best effort)
      # Optional: add a polling loop to wait for assets to appear, with a timeout
      run: |
        echo "Finalizing release with tag ${{ needs.determine-version.outputs.tag }}"
        echo "Note: This runs independently of build job status."
    - name: Set release title and notes
      run: gh release edit "${{ needs.determine-version.outputs.tag }}" \
             --title "RustDesk Direct-IP v${{ inputs.direct-ip-version }}" \
             --notes "..." \
             --prerelease=false
```

**Pros:**
- Release is **always** finalized, regardless of build outcome
- Decouples release finalization from platform build success
- Clear separation of concerns: tag computation, platform builds, and release finalization are independent

**Cons:**
- No guaranteed ordering: `finalize-release` could run BEFORE `build` has uploaded any assets (race condition)
- Requires extra logic to wait for assets or accept missing-asset releases
- More complex error handling (what if the release tag doesn't exist yet?)

**Verdict:** ⚠️ **Risky** — without careful synchronization, finalization could happen before build assets exist. This would require a polling/wait mechanism, adding complexity.

---

## Option C: Conditional Finalization (Best of Both)

**Change:** `finalize-release` depends on `build` and runs only if `build` succeeds (current), BUT also add a separate fallback job that runs if `build` fails to at least set the title/notes with a "partial failure" note.

**Implementation:**

```yaml
finalize-release:
  needs: [determine-version, build]
  if: success()  # Only run if build succeeds
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - name: Set release title and notes (full)
      run: gh release edit "${{ needs.determine-version.outputs.tag }}" \
             --title "RustDesk Direct-IP v${{ inputs.direct-ip-version }}" \
             --notes "Full release with all platforms." \
             --prerelease=false

finalize-release-on-failure:
  needs: [determine-version, build]
  if: failure()  # Only run if build fails
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - name: Set release title and notes (partial)
      run: gh release edit "${{ needs.determine-version.outputs.tag }}" \
             --title "RustDesk Direct-IP v${{ inputs.direct-ip-version }} [Partial]" \
             --notes "Build failed on some platforms. See Actions run ${{ github.run_id }} for details." \
             --prerelease=true
```

**Pros:**
- Release is **always** finalized with appropriate title/notes
- Clear visual distinction in the title if the build had issues ([Partial])
- Two separate job paths make the logic obvious
- Pre-release status reflects the build outcome (marked as pre-release on partial failure)

**Cons:**
- Two jobs doing similar work (duplication)
- "Partial" release might still confuse end users
- More YAML code to maintain

**Verdict:** ✅ **Good** — explicit, maintainable, and always produces a finalized release. Slight duplication is acceptable for clarity.

---

## Option D: Explicit Pipeline Stages (Strictest)

**Change:** Make the release finalization explicitly a separate, final stage that runs ONLY after all builds are guaranteed complete, using a deterministic completion check (not GitHub's implicit job dependency skipping).

**Implementation:**

```yaml
build-windows:
  # ... runs independently
  
build-linux:
  # ... runs independently

# ... etc for all platforms

await-all-builds:
  needs: [build-windows, build-linux, build-appimage, ...]
  runs-on: ubuntu-latest
  steps:
    - run: echo "All builds complete (or failed). Proceeding to finalization."

finalize-release:
  needs: [determine-version, await-all-builds]
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - name: Finalize release
      run: gh release edit "${{ needs.determine-version.outputs.tag }}" ...
```

**Pros:**
- Explicit, deterministic flow
- No ambiguity about when finalization happens
- Can report on which specific jobs failed, if needed

**Cons:**
- Requires splitting internal `build` jobs out of the reusable workflow (major refactor)
- Adds overhead (extra job that does nothing but wait)
- More complex to understand at first glance

**Verdict:** ❌ **Not practical** — requires restructuring the workflow architecture, which is out of scope for this release cycle.

---

## Recommendation for Release Hardening

**Choose: Option C (Conditional Finalization with Fallback)**

Rationale:
- **Simplest fix with maximum clarity**: Two explicit jobs, one for success and one for failure
- **Always produces a finalized release**: No stranded, incomplete releases
- **User-facing transparency**: Title includes [Partial] badge for partial failures; pre-release status reflects build outcome
- **No race conditions**: Both jobs depend on the completed `build` job
- **Maintainable**: Clear separation of concern, even if a bit of duplication

**Implementation effort:** ~20–30 lines of YAML

**Timeline:** Can implement and test in next CI run (together with sciter fix verification, Task 3)

---

## Decision Point: IMPLEMENTED

**Decision: Option C (Conditional Finalization with Fallback) — IMPLEMENTED 2026-09-01**

**Implementation in `.github/workflows/release.yml` (commit 46b7ff2f5):**

✅ **`finalize-release` job (success path):**
   - Condition: `if: success()`
   - Runs only if the `build` job succeeds
   - Sets full release title: `RustDesk Direct-IP v${{ inputs.direct-ip-version }}`
   - Release notes: standard full release message
   - Prerelease flag: `false` (full release)

✅ **`finalize-release-on-failure` job (failure path):**
   - Condition: `if: failure()`
   - Runs only if the `build` job fails
   - Sets partial release title: `RustDesk Direct-IP v${{ inputs.direct-ip-version }} [Partial]`
   - Release notes: includes warning, explanation, and link to Actions run
   - Prerelease flag: `true` (partial release)

**Result:** Release is always finalized with appropriate title/notes even on partial build failure.
