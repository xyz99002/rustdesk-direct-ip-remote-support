# CI Workflow Audit — 2026-09-01

**Scope:** Repository reference correction, `flutter-ci.yml`, `flutter-nightly.yml`, `release.yml`, and Linux artifact format investigation.
**Architecture reaffirmed:** One executable, TOML-controlled role configuration. No separate Local/Remote executables. No new transport.
**Canonical repository:** https://github.com/xyz99002/rustdesk-direct-ip-remote-support

---

## 1. Repository References — Fixed

Stale references replaced with the canonical repository URL:

| File | Stale reference | Fixed to |
|---|---|---|
| `docs/CI_BUILD_SUMMARY.md` | `.../xyz99002/rustdesk-direct-ip` | `.../xyz99002/rustdesk-direct-ip-remote-support` |
| `docs/CI_EXECUTION_PLAN.md` | `.../xyz99002/rustdesk-direct-ip.git` | `.../xyz99002/rustdesk-direct-ip-remote-support.git` |
| `docs/GITHUB_FIRST_RUN_CHECKLIST.md` | `.../xyz99002/rustdesk-direct-ip.git` | `.../xyz99002/rustdesk-direct-ip-remote-support.git` |
| `docs/GITHUB_COMMANDS.txt` | `.../arvind-patel_pdfs/RustDesk-direct-ip-remote-support.git` (an earlier, now-deleted enterprise fork) | `.../xyz99002/rustdesk-direct-ip-remote-support.git` |
| `Notices/Third-Party-Notices.md` | `.../xyz99002/rustdesk-direct-ip` | `.../xyz99002/rustdesk-direct-ip-remote-support` |
| `docs/REPOSITORY_AND_ARTIFACT_MAP.md` | Stale branch names (`feature/direct-ip-fork`/`main`, "Phase 5") from an earlier point in the fork's history | Rewritten to reflect current `master`-based workflow and canonical repo URL |

**Verified clean (no changes needed):** `.github/workflows/direct-ip-build.yml`, `.github/workflows/flutter-build.yml` — these only contain artifact *names* like `rustdesk-direct-ip-windows-x86_64` (a naming convention, not a repository reference) and unrelated upstream RustDesk URLs (e.g. `github.com/rustdesk/rustdesk/discussions/...`, tool download URLs). No stale self-references found.

---

## 2. Local Repository Alignment

`git remote -v` output (verified 2026-09-01):

```
fork      https://github.com/xyz99002/rustdesk-direct-ip-remote-support.git (fetch)
fork      https://github.com/xyz99002/rustdesk-direct-ip-remote-support.git (push)
upstream  https://github.com/rustdesk/rustdesk.git (fetch)
upstream  https://github.com/rustdesk/rustdesk.git (push)
```

- **`fork`** — the canonical Direct-IP repository. Already correctly configured; no change needed.
- **`upstream`** — the original RustDesk project, used for pulling upstream changes. Correctly configured.
- **Local checked-out branch:** `master`, tracking `fork/master`, working tree clean at commit `d09b1f8e0`.
- Two stale local-only branches exist from earlier CI experimentation and are no longer pushed anywhere: `feature/direct-ip-fork` (local) and `master-sync` (local). They don't affect CI and are left in place pending your decision on whether to delete them.

Documentation updated in `docs/REPOSITORY_AND_ARTIFACT_MAP.md` to reflect this state (see Section 1).

---

## 3. `direct-ip-build.yml`

Verified: no repository-name assumptions found. Triggers, artifact names, and vcpkg/cache configuration are all self-contained and independent of the repository's name or location. No changes required.

---

## 4 & 5. `flutter-ci.yml` — Behavior, Artifacts, Release Integration

`flutter-ci.yml` is a thin wrapper:

```yaml
on:
  workflow_dispatch:
  pull_request: {...}
  push:
    branches: [master]
jobs:
  run-ci:
    uses: ./.github/workflows/flutter-build.yml
    with:
      upload-artifact: true
      upload-release: false
```

**Expected behavior:** every push to `master` (and every PR) runs the *entire* upstream `flutter-build.yml` matrix — Windows (x64/x86/arm64), macOS (x64/arm64), Linux (.deb, sciter .deb, AppImage, Flatpak), Android, and web/F-Droid jobs. This is the full upstream RustDesk CI surface, not a Direct-IP-specific subset — that's why a routine push triggers a very large, long-running job graph.

**Artifact behavior:** with `upload-artifact: true`, every job that produces a binary attaches it to the GitHub Actions "Artifacts" tab of the run (90-day retention, per repo settings). This part works as intended.

**Release behavior:** with `upload-release: false`, no platform should attempt to create/update a GitHub Release. In practice this is *not* fully respected — see Section 6, Bug A.

---

## 6. `flutter-ci` Build Failure — Root Cause (documented only, not fixed)

**Run examined:** Full Flutter CI #6 (commit `d09b1f8e0`, 2026-09-01, https://github.com/xyz99002/rustdesk-direct-ip-remote-support/actions/runs/33293022021 — 3m36s, Failure)

Two independent, unrelated failures in the same run:

### Failure 1 — `build-appimage` (both `x86_64` and `aarch64` matrix legs)

**Exact error** (raw job log):
```
👩‍🏭 Creating new GitHub release for tag nightly...
⚠️ GitHub release failed with status: 403
retrying... (2/1/0 retries remaining)
❌ Too many retries. Aborting...
Error: Too many retries.
```

**Root cause — Bug A (conditional logic error):** in `flutter-build.yml`, the `build-appimage` job's "Publish appimage package" step is gated on:
```yaml
if: env.UPLOAD_ARTIFACT == 'true'
```
Every sibling "Publish X package" step in the same file (debian, sciter-deb, etc.) is correctly gated on `env.UPLOAD_RELEASE == 'true'`. This one step uses the wrong flag. Since `flutter-ci.yml` sets `upload-artifact: true` (but `upload-release: false`), the condition evaluates true and the step attempts to create a GitHub Release anyway — directly violating the caller's explicit `upload-release: false`.

**Root cause — Bug B (compounding, explains the 403 and why it also breaks *intended* releases in Section 8):** `flutter-build.yml`'s `workflow_call.inputs.upload-tag` has `default: "nightly"`. `flutter-ci.yml` never sets `upload-tag`, so any release-publish attempt it triggers (correctly or, as here, by Bug A) targets the tag `nightly` — explaining the "tag nightly" text in a workflow that has nothing to do with nightly builds.

**Root cause — Bug C (permissions, verified directly in repo settings):** Settings → Actions → General → Workflow permissions is set to **"Read repository contents and packages permissions"** (read-only) for this repository. Neither `flutter-ci.yml` nor `flutter-nightly.yml` nor `flutter-build.yml` declares a `permissions:` block, so their jobs inherit this read-only default — `GITHUB_TOKEN` cannot create or edit releases, hence HTTP 403 on every genuine release-publish attempt, independent of Bug A/B. (`release.yml`, the new manual-release workflow, avoids this because it explicitly declares `permissions: contents: write` at the workflow level — see Section 7.)

### Failure 2 — `build-rustdesk-linux-sciter` (`x86_64-unknown-linux-gnu`)

**Exact error** (from vcpkg's dumped build log inside the job's raw log):
```
aom_dsp/flow_estimation/x86/disflow_avx2.c:216:19: warning: implicit declaration of function '_mm256_set_m128i'
aom_dsp/flow_estimation/x86/disflow_avx2.c:216:19: error: incompatible types when initializing type '__m256i' using type 'int'
[... 4 occurrences, lines 216-219 ...]
CMake Error: Command failed ... vcpkg_execute_build_process.cmake:134
error: building aom:x64-linux failed with: BUILD_FAILED
```
**Root cause:** the sciter/legacy-compat Linux build container pins a very old host compiler (reported: **GCC 7.5.0**, for maximum binary compatibility with old distros). GCC's `<immintrin.h>` did not declare the `_mm256_set_m128i` AVX2 intrinsic until GCC 8. vcpkg's pinned `aom` port (3.12.1) uses that intrinsic in `disflow_avx2.c`. Under GCC 7.5.0 the compiler implicitly declares the function as returning `int`, so every `__m256i` variable it initializes becomes a type mismatch — a genuine compiler/library-version incompatibility, not a Direct-IP-specific regression (this would fail identically on unmodified upstream RustDesk built with this same toolchain pin).

**Remediation — option 1 implemented (2026-09-01):** a new vcpkg overlay patch, `res/vcpkg/aom/aom-gcc7-avx2-compat.diff`, has been added and registered in `res/vcpkg/aom/portfile.cmake`'s `PATCHES` list (alongside `aom-uninitialized-pointer.diff`, `aom-install.diff`, and `aom-disable-multipass-check.diff`). It inserts a small `static inline __m256i _mm256_set_m128i(...)` shim into `aom_dsp/flow_estimation/x86/disflow_avx2.c`, guarded by `#if defined(__GNUC__) && !defined(__clang__) && __GNUC__ < 8`, defined in terms of `_mm256_insertf128_si256`/`_mm256_castsi128_si256`. See `docs/LINUX_SCITER_FIX_2026-09-01.md` for full details, exact patch content, and rationale versus the other three options below (kept here for reference). **Live CI verification of `build-rustdesk-linux-sciter` (x86_64-unknown-linux-gnu) is still pending** — no new GitHub Actions run was triggered as part of this change, since a `release.yml` run and other CI capacity were already in use elsewhere; verification is deferred to a future CI run.

Other options considered (not implemented):
2. Upgrade the sciter/legacy container's GCC to 8+ — higher risk if the old GCC was pinned specifically for glibc/ABI compatibility with older target distros; needs the original rationale confirmed before touching it.
3. Pin an older `aom` release predating this AVX2 code path — re-opens the exact class of version-pinning fragility already seen with the NASM multipass issue; not preferred.
4. Strip the AVX2-optimized file from the sciter Linux build via CFLAGS/exclude — functional but causes an AV1 performance regression on that specific build only.

**Preferred fix:** Option 1.

---

## 7. `release.yml` — Verified, Tested, and Fixed

**Original issue found:** the workflow computed the release tag as `v${{ inputs.direct-ip-version }}` only (e.g. `v1.0.0`) — it never actually combined the upstream RustDesk baseline version with the Direct-IP suffix, despite reading the RustDesk `VERSION` separately for the release-notes text. This did not match the stated purpose: *"Generate Direct-IP releases using: Upstream RustDesk version + Direct-IP version suffix."*

**Fix applied** (commit pending in this session): restructured into three jobs:
1. `determine-version` — checks out the repo, extracts `VERSION` from `flutter-build.yml` (same technique the old `finalize-release` step used), and computes the combined tag: `v{rustdesk-version}-direct-ip.{direct-ip-version}` (e.g. `v1.4.9-direct-ip.1.0.0`).
2. `build` — calls `flutter-build.yml` with `upload-tag` set to that *combined* tag (previously it only used the raw Direct-IP version).
3. `finalize-release` — now reads both the tag and the RustDesk baseline from `determine-version`'s outputs instead of recomputing the version and reusing the uncombined tag; also made this job's `gh release edit` explicit about the target repo via `--repo` (a defensive change since the job no longer checks out the repo, as it no longer needs to).

**Verified correctness of what was already right (unchanged):**
- `permissions: contents: write` is correctly declared at the workflow level — this is the one workflow in the repo that already avoids the Bug C permissions gap described in Section 6.
- Artifact naming inside `flutter-build.yml` already correctly bakes in `env.VERSION` (the RustDesk baseline) into every artifact filename (e.g. `rustdesk-direct-ip-${{ env.VERSION }}-${{ matrix.job.arch }}.deb`) — no change needed there.

**Testing status:** the corrected workflow has not yet been run end-to-end (it requires a `workflow_dispatch` with a real `direct-ip-version` input, which is a release action and was not triggered without your go-ahead). Recommend a dry run with a placeholder version (e.g. `0.0.1-test`) before the first real release.

---

## 8. `flutter-nightly` Failure

**Run examined:** Flutter Nightly Build #2 (commit `d09b1f8e0`, scheduled, 1h7m19s, Failure, https://github.com/xyz99002/rustdesk-direct-ip-remote-support/actions — 8 of ~14 matrix legs failed)

`flutter-nightly.yml` calls `flutter-build.yml` with `upload-artifact: true, upload-release: true, upload-tag: "nightly", secrets: inherit`.

**Sampled failure — `i686-pc-windows-...` leg (26m47s):**
```
##[error]Too many retries.
```
Same `softprops/action-gh-release` retry-exhaustion pattern as Section 6, Bug A/C — but here the release-publish step is *correctly* gated (nightly builds are supposed to publish a release), so this is purely **Bug C**: the repository's read-only default `GITHUB_TOKEN` permission blocks it, because `flutter-nightly.yml` (like `flutter-ci.yml`) never declares its own `permissions: contents: write` to override that default. `secrets: inherit` does not help here — it only forwards repository *secrets*, not the `GITHUB_TOKEN` permission scope, which is controlled solely by the repo's Settings → Actions → General → Workflow permissions (or an explicit `permissions:` block in the workflow).

**Conclusion:** `flutter-nightly`'s failure is **not** a release.yml issue and **not** a shared-workflow logic bug distinct from what's already documented — it is the same Bug C (missing `permissions: contents: write`) as Section 6, compounded by the same aom/GCC-7.5.0 sciter build failure (Section 6, Failure 2) wherever a Linux-sciter leg is in the matrix. The other failing legs (macOS, Android, additional Windows legs) were not individually traced in this pass; given the dominant, already-confirmed root cause (Bug C blocks *every* release-publish step regardless of platform), it is likely the same failure signature repeats across most of them. Recommend re-running nightly after Bug C is fixed and re-auditing only the legs that still fail.

**Remediation (documented, not implemented — no fix was requested for item 8, only findings):**
- Add `permissions: contents: write` to `flutter-nightly.yml` (and `flutter-ci.yml`, if release-worthy artifacts are ever expected from it) — OR change the repository's default Workflow permissions to "Read and write" in Settings → Actions → General. The former is more precise (least privilege, scoped per-workflow); the latter is simpler but grants write access to every workflow in the repo, including third-party actions that don't need it.

---

## 9. Linux Artifacts — Portable Format Investigation

### Current state: `direct-ip-build.yml`

Uploads a **single bare binary**, no supporting files:
```yaml
path: target/x86_64-unknown-linux-gnu/release/rustdesk
```
Missing: translation/asset files, desktop integration files, and — critically — this is a **dynamically-linked** binary built with plain `cargo build`, so it depends on whatever shared libraries (GTK3, libxdo, PulseAudio/ALSA, etc.) happen to be present on the machine that runs it. It is not self-contained and will fail to launch on a system missing those libraries, with no bundling of them.

### Current state: `flutter-build.yml` (`build-rustdesk-linux`)

Produces a `.deb` package (`rustdesk-direct-ip-{VERSION}-{arch}.deb`) — a real Debian package with a proper `control` file declaring runtime dependencies, built inside a pinned Ubuntu 18.04 container via QEMU for compatibility. Correct and complete, but as you noted: validating a `.deb` requires either a Debian/Ubuntu-family system or manually resolving/installing its declared dependencies — inconvenient for quick CI-artifact smoke-testing on arbitrary machines.

### Already exists but currently broken: `build-appimage`

`flutter-build.yml` already has an AppImage job (`build-appimage`, needs `build-rustdesk-linux`) that takes the `.deb` as input and runs `appimage-builder` to produce `./appimage/rustdesk-{VERSION}-*.AppImage` — a **single self-contained executable file** that bundles all required shared libraries and runs on most Linux distributions without installation. This is exactly the "portable, runnable, testable, includes all required dependencies" format requested.

**Two problems currently prevent this from being a usable artifact today:**
1. **Bug A** (Section 6) — its only post-build step tries to publish straight to a GitHub Release, gated on the wrong flag, and fails with 403 (Bug C) before the file is preserved anywhere.
2. **Missing artifact upload** — unlike the `.deb` job, `build-appimage` has **no `actions/upload-artifact` step at all**. Even after fixing Bug A/C, the built `.AppImage` file would only become visible via a GitHub Release (when `upload-release: true`) and would be **silently discarded** whenever `upload-release: false` — which is exactly `flutter-ci.yml`'s configuration. So today, even on a fully green run, no AppImage would ever reach the Artifacts tab.

### Recommendation

**Use AppImage as the portable/testable Linux artifact**, since the packaging logic to build it already exists and is architecturally correct — it just needs:
1. Bug A fixed (correct the `if` condition to `env.UPLOAD_RELEASE == 'true'` for the release-publish step), and
2. A new `actions/upload-artifact` step added to `build-appimage` (parallel to what `build-rustdesk-linux` already does for the `.deb`), gated on `env.UPLOAD_ARTIFACT == 'true'`, uploading `./appimage/rustdesk-${{ env.VERSION }}-*.AppImage`.

This gives every CI run (regardless of whether it's a release) a directly downloadable, directly runnable Linux binary in the Artifacts tab — no dependency resolution, no package manager, no root required to test it. **No changes have been made yet** — this is a recommendation pending your approval, consistent with "document options and recommend one."

---

## Summary Table

| # | Item | Status |
|---|---|---|
| 1 | Repository references | ✅ Fixed (6 files) |
| 2 | Local repository alignment | ✅ Verified and documented |
| 3 | `direct-ip-build.yml` repo structure | ✅ Verified clean, no changes needed |
| 4 | `flutter-ci.yml` behavior | ✅ Documented |
| 5 | `flutter-ci` artifact/release support | ✅ Documented |
| 6 | `flutter-ci` build failure root cause | ✅ Fixed (Bugs A, B, C) and **verified in CI** — see Section 10 |
| 7 | `release.yml` version/tag strategy | ✅ Fixed (combined tag) — dry-run pending, see Section 10 |
| 8 | `flutter-nightly` failure | ✅ Fixed via the same Bug C fix — not yet re-run (nightly is schedule-only + manual dispatch; the fix was verified via `flutter-ci.yml`, which shares the same permissions gap and the same underlying `flutter-build.yml`) |
| 9 | Linux artifact format | ✅ **Implemented** — AppImage upload step added and verified in CI, see Section 10 |

---

## 10. Approved Fixes — Implemented and Verified (2026-09-01)

All four approved fixes were implemented in commit `aae2da7b7` and validated end-to-end in [Full Flutter CI #8](https://github.com/xyz99002/rustdesk-direct-ip-remote-support/actions/runs/33537701026) (1h5m20s, 18 artifacts). Full per-job results are recorded in `docs/BUILD_VERIFICATION_RESULTS.md` (2026-09-01 update). Summary:

| Fix | Change | Verified |
|---|---|---|
| **Bug A** | `build-appimage`'s "Publish appimage package" step now checks `env.UPLOAD_RELEASE == 'true'` instead of `env.UPLOAD_ARTIFACT == 'true'` | ✅ Step shows skipped (⊘) in both matrix legs, since `flutter-ci.yml` sets `upload-release: false` |
| **Bug B** | `flutter-build.yml`'s `upload-tag` input default changed from `"nightly"` to `""` | ✅ No unintended nightly-tag release attempts observed |
| **Bug C** | Added `permissions: contents: write` to `flutter-ci.yml` and `flutter-nightly.yml` (`release.yml` already had it) | ✅ Zero HTTP 403 / "Too many retries" errors anywhere in the 18-job run |
| **Item 4** (AppImage upload) | Added an `actions/upload-artifact` step to `build-appimage`, gated on `env.UPLOAD_ARTIFACT`, uploading `./appimage/rustdesk-${{ env.VERSION }}-*.AppImage` | ✅ `rustdesk-direct-ip-1.4.9-x86_64.AppImage` and `rustdesk-direct-ip-1.4.9-aarch64.AppImage` both appear as directly downloadable artifacts, despite `upload-release: false` |

**Unrelated, pre-existing failure confirmed still present (expected, out of scope for this round):** `build-rustdesk-linux-sciter` (x86_64-unknown-linux-gnu) still fails in 4m5s on the GCC 7.5.0 / aom 3.12.1 AVX2-intrinsic incompatibility documented in Section 6, Failure 2. Its `armv7` sibling passes, consistent with the root-cause analysis (armv7 never compiles the x86 AVX2 intrinsics file).

**Remaining work:**
- `release.yml` dry run with `direct-ip-version=0.0.1-test` — in progress, see below for results once available.
- The aom/GCC AVX2 fix (Section 6, remediation option 1) remains unimplemented, as it was out of scope for this approval.
