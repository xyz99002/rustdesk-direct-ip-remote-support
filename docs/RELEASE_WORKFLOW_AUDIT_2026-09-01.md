# Release Workflow Deeper Audit — 2026-09-01

**Scope:** `.github/workflows/release.yml` and `.github/workflows/flutter-build.yml`, focused on tagging correctness (beyond the already-fixed/verified tag formula), artifact-naming consistency, and release-notes correctness. Builds on `docs/CI_WORKFLOW_AUDIT_2026-09-01.md` (Bugs A/B/C, already fixed) — this document only covers new findings from a second pass.

---

## 1. Race condition risk — `softprops/action-gh-release` called in parallel with the same tag

**Pattern:** every platform job in `flutter-build.yml` (Windows flutter x64/arm64, Windows sciter x86, macOS x64/arm64, Android per-arch + universal, Linux deb/rpm/pacman, Linux sciter deb, AppImage x86_64/aarch64, Flatpak x3) independently calls `softprops/action-gh-release@de2c0eb89ae2a093876385947365aca7b0e5f844` with the same `tag_name: ${{ env.TAG_NAME }}`, and these jobs run concurrently (no `needs:` serializing them against each other, aside from a few narrow producer/consumer edges like `build-appimage` needing `build-rustdesk-linux`).

**Research:** `softprops/action-gh-release`'s own documentation only states that if a release for the tag already exists, "the existing release will be updated with the release assets" — implying a create-if-missing / upsert-if-present flow. It does **not** document any locking, concurrency guard, or explicit guidance for many parallel jobs racing to create a release for the same brand-new tag. In practice the action does: GET release by tag → if 404, POST to create → then upload assets to that release id. When N jobs finish training their builds around the same time and the release does not yet exist, more than one can observe the 404 and attempt to create it; GitHub's release-create API returns a 422 `already_exists` error for the loser(s) of that race. Community reports of exactly this failure mode exist for this action (search "already_exists" + this action in its issue tracker) — it is a known, if infrequent, risk rather than a purely theoretical one.

**Assessment for this repo:** the risk window is real but narrow — it only bites during the few seconds after the very first artifact-producing job finishes and before the release object exists; every job that loses that race gets a hard failure on its "Publish ... package" step (its assets would not be attached), not a silent corruption. Given the current matrix (roughly a dozen jobs, several with long/staggered build times — Windows ARM, macOS, Android, Linux-under-QEMU all finish at different times), the actual chance of two "Publish" steps landing within the same few-hundred-ms window is low but non-zero, and would worsen if more platforms are added or build times become more uniform (e.g. from caching).

**Verdict: needs monitoring.** Not fixed in this pass — the fix (e.g. adding a `needs:`-based "create-release-first" job that only creates an empty draft release for the tag before any platform job runs, or serializing the first upload) is a structural change to the job graph, which is larger than the "small, safe, one-line" bar for this audit. Recommend watching for `422`/`already_exists` errors in future release runs (including the in-progress `0.0.1-test` run) and revisiting if one is observed.

---

## 2. Artifact naming consistency — Actions-artifact name vs. actual Release asset filename

For every job that both uploads a GitHub Actions artifact and publishes to the Release, this table compares the Actions-artifact `name:` against the real downloadable filename(s) from the release `files:` glob (which resolve to files on disk with their on-disk names, not the artifact name):

| Job | Actions artifact `name:` (file:line) | Release asset `files:` glob (file:line) | Release asset filename carries "direct-ip"? |
|---|---|---|---|
| `build-for-windows-flutter` | `rustdesk-direct-ip-windows-${{ matrix.job.arch }}` (flutter-build.yml:326) | `./SignOutput/rustdesk-*.msi`, `./SignOutput/rustdesk-*.exe` (flutter-build.yml:378-380) | **No** — on-disk files are named `rustdesk-${VERSION}-${arch}.msi`/`.exe` (flutter-build.yml:346,363) |
| `build-for-windows-sciter` | `rustdesk-direct-ip-windows-${{ matrix.job.arch }}` (flutter-build.yml:498) | `./SignOutput/rustdesk-*.exe` (flutter-build.yml:531-532) | **No** — on-disk file is `rustdesk-${VERSION}-${arch}-sciter.exe` (flutter-build.yml:517) |
| `build-for-macOS` | `rustdesk-direct-ip-macos-${{ matrix.job.arch }}` (flutter-build.yml:835) | `rustdesk*-${{ matrix.job.arch }}.dmg` (flutter-build.yml:869-870) | **No** — on-disk file is `rustdesk-${VERSION}-${arch}.dmg` (flutter-build.yml:829/851-860) |
| `build-rustdesk-android` | `rustdesk-direct-ip-${{ env.VERSION }}-${{ matrix.job.arch }}.apk` (flutter-build.yml:1186) | signed: `${{steps.sign-rustdesk.outputs.signedReleaseFile}}` (flutter-build.yml:1195-1196); unsigned: `signed-apk/rustdesk-${{ env.VERSION }}-${{ matrix.job.arch }}.apk` (flutter-build.yml:1204-1205) | **No** — the signed-apk output path and the unsigned path are both the plain `rustdesk-${VERSION}-${arch}.apk` produced at flutter-build.yml:1126/1135/1144/1153/1158 |
| `build-rustdesk-android-universal` | `rustdesk-direct-ip-${{ env.VERSION }}-${{ matrix.job.arch }}.apk` (flutter-build.yml:1370) — note: `matrix.job.arch` does not exist in this job's (non-matrixed) context, so this artifact name literally renders with an empty arch segment | signed: same signedReleaseFile mechanism (flutter-build.yml:1379-1380); unsigned: `signed-apk/rustdesk-${{ env.VERSION }}-universal${{ env.suffix }}.apk` (flutter-build.yml:1388-1389) | **No** for the release asset. The artifact-name bug (empty `matrix.job.arch`) is a separate, pre-existing cosmetic issue in the Actions-artifact name only — it does not affect the release asset filename, so it is flagged here but not fixed (out of the "release asset naming" scope and not release-breaking) |
| `build-rustdesk-linux` | `rustdesk-direct-ip-${{ env.VERSION }}-${{ matrix.job.arch }}.deb` (flutter-build.yml:1697) | `rustdesk-*.deb`, `rustdesk-*.rpm` (flutter-build.yml:1689-1691) | **No** — on-disk files are `rustdesk-${VERSION}-${arch}.deb` / `.rpm` / `-suse.rpm` |
| `build-rustdesk-linux-sciter` | `rustdesk-direct-ip-${{ env.VERSION }}-${{ matrix.job.arch }}-sciter.deb` (flutter-build.yml:1952) | `rustdesk-${{ env.VERSION }}-${{ matrix.job.arch }}-sciter.deb` (flutter-build.yml:1945-1946) | **No** |
| `build-rustdesk-linux` (pacman) | *(no artifact upload for this file)* | `res/rustdesk-${{ env.VERSION }}*.zst` (flutter-build.yml:1725-1726) | **No** (no artifact counterpart to compare against) |
| `build-appimage` | `rustdesk-direct-ip-${{ env.VERSION }}-${{ matrix.job.arch }}.AppImage` (flutter-build.yml:2001) | `./appimage/rustdesk-${{ env.VERSION }}-*.AppImage` (flutter-build.yml:2010-2011) | **No** |
| `build-flatpak` | *(no artifact upload for this file)* | `flatpak/rustdesk-${{ env.VERSION }}-${{ matrix.job.arch }}${{ matrix.job.suffix }}.flatpak` (flutter-build.yml:2095-2096) | **No** (no artifact counterpart) |

**Finding:** the pattern is 100% consistent and one-directional — **every** Actions-artifact name in this file was already renamed to include `direct-ip` (as `CI_WORKFLOW_AUDIT_2026-09-01.md` §1 noted), but **not a single actual GitHub Release download filename anywhere in the matrix carries "direct-ip"**. A user downloading from the Releases page for a Direct-IP release sees plain upstream-looking filenames (`rustdesk-1.4.9-x86_64.deb`, `rustdesk-1.4.9-x86_64.msi`, etc.) with no visual indication these are Direct-IP builds and not stock RustDesk. This is cosmetic (does not break functionality) but is a real, previously-undocumented branding/consistency gap between the two naming schemes used side-by-side in the same file.

**Verdict: needs fix, but not applied in this pass.** Renaming every release asset's on-disk filename to carry `direct-ip` touches roughly a dozen `mv`/build steps across every platform (Windows, macOS, Android, Linux, AppImage, Flatpak) plus their corresponding `files:` globs — this is a broad, multi-file, mechanically repetitive change with real risk of missing a spot or breaking a glob, so it exceeds the "small, safe, obviously correct one-line fix" bar set for this audit. Recommend doing it as its own dedicated, reviewed change (one commit touching every `mv ... rustdesk-${VERSION}-...` line and matching `files:` glob) rather than folding it into this pass.

### Bug found and fixed while building this table: Android release-publish gated on the wrong flag

While tracing the Android rows above, both Android jobs turned out to have the **same class of bug as the already-documented Bug A** (wrong `if` flag on a release-publish step), previously undocumented:

- `build-rustdesk-android`, step "Publish signed apk package" — flutter-build.yml:1189-1190 was `if: env.ANDROID_SIGNING_KEY != null && env.UPLOAD_ARTIFACT == 'true'`
- `build-rustdesk-android-universal`, step "Publish signed apk package" — flutter-build.yml:1373-1374, same pattern

Both used `UPLOAD_ARTIFACT` where every sibling "Publish ..." step in the file (and the "Publish unsigned apk package" step right next to each of these, at flutter-build.yml:1199 and 1383) correctly uses `UPLOAD_RELEASE`. Practical impact: whenever a caller sets `upload-artifact: true, upload-release: false` (this is exactly `flutter-ci.yml`'s configuration) **and** `ANDROID_SIGNING_KEY` is configured as a repo secret, this step would attempt to create/update a GitHub Release on every CI/PR run — hitting the same 403 (Bug C class) or, once Bug C is fixed, actually publishing unwanted releases from routine CI runs. It reproduces the exact "Too many retries" / 403 failure mode documented in `CI_WORKFLOW_AUDIT_2026-09-01.md` §6 Bug A, just for Android instead of AppImage.

**Fix applied** (small, one-line-per-site, mirrors the already-approved Bug A fix): changed `env.UPLOAD_ARTIFACT` to `env.UPLOAD_RELEASE` in both conditions (flutter-build.yml:1190, 1374). This has not yet been run in CI — flag for verification on the next `flutter-ci` or nightly run.

### Second bug found and fixed: `build-flatpak` downloads an artifact name that never existed

`build-flatpak`'s "Download Binary" step (flutter-build.yml:2053-2057, before fix) requested:
```
name: rustdesk-${{ env.VERSION }}-${{ matrix.job.arch }}${{ matrix.job.suffix }}.deb
```
but the two jobs it depends on (`build-rustdesk-linux`, `build-rustdesk-linux-sciter`) actually upload their `.deb` artifacts as `rustdesk-direct-ip-${{ env.VERSION }}-...` (flutter-build.yml:1697, 1952) — i.e. with the `direct-ip-` prefix the Actions-artifact renaming added. The `build-flatpak` job's download step was never updated to match, so **every flatpak leg would fail outright** with an artifact-not-found error whenever it actually runs (`build-flatpak` is gated on `inputs.upload-artifact`, so this fires on every `flutter-ci`, nightly, and release run). This is a functional break, not merely cosmetic.

**Fix applied:** flutter-build.yml:2056 changed to `name: rustdesk-direct-ip-${{ env.VERSION }}-${{ matrix.job.arch }}${{ matrix.job.suffix }}.deb`, matching the actual upload names for both the plain and `-sciter` suffix cases. Not yet verified in CI — flag for verification on the next full matrix run.

---

## 3. Release notes correctness — `finalize-release` ordering and `--prerelease=false`

**Question 1 — does `gh release edit --prerelease=false` at the end correctly finalize a release every one of whose assets was uploaded with `prerelease: true`?** Yes. `prerelease` is a mutable field on the release object itself, not a per-asset attribute — every `softprops/action-gh-release` call in the matrix re-asserts `prerelease: true` while assets are still being attached (this is intentional/harmless idempotent metadata-setting, not a race on the flag itself), and `finalize-release`'s final `gh release edit ... --prerelease=false` simply flips that one field after all uploads are done. There is nothing that could re-flip it back to `true` afterward, since no job runs after `finalize-release`.

**Question 2 — does `finalize-release` (needs: `[determine-version, build]`) truly run only after every internal job of the reusable `build` call has finished?** Yes, and this is a hard guarantee, not a timing coincidence. Per GitHub Actions' documented reusable-workflow semantics, a caller job with `uses: ./path/to/workflow.yml` (`build:` here, release.yml:44-51) is not marked complete until **every job inside the called reusable workflow reaches a terminal state** (success, failure, cancelled, or skipped) — the calling job's own status is derived as an aggregate of the called workflow's job graph. `finalize-release: needs: [determine-version, build]` therefore cannot start until literally every job in `flutter-build.yml` (all Windows/macOS/Android/Linux/AppImage/Flatpak legs) has finished, regardless of how long the slowest leg takes. This holds even if some inner jobs fail — GitHub still considers the caller job "complete" (as failure) once all inner jobs are terminal, and `finalize-release`'s `needs: build` will consequently either wait for that outcome or (per default `needs` semantics) not run at all if `build` reports overall failure. That second half is worth calling out explicitly:

**New, previously-undocumented finding:** `finalize-release` has no `if: success() || always()` override, so by default `needs: [determine-version, build]` requires `build` to have **succeeded** for `finalize-release` to run at all. Since `build` aggregates the entire flutter-build.yml matrix, if even one leg genuinely fails (not skipped — an actual failure, e.g. the already-known sciter/GCC-7.5.0 AVX2 build failure documented in `CI_WORKFLOW_AUDIT_2026-09-01.md` §6 Failure 2, which is expected to still be present), then the whole `build` reusable-workflow-call job is reported as failed, and `finalize-release` is skipped entirely — leaving the release stuck as a draft/prerelease titled with the workflow-default name and without the intended title/notes rewrite, even though most of the other platform assets uploaded successfully. This is consistent with, and explains in advance, a risk for the currently in-progress `0.0.1-test` dry run: if the known sciter build failure occurs in that run (likely, since it hasn't been fixed), `finalize-release` may not run and the release will be left un-finalized (still `prerelease: true`, default title) even though it will otherwise contain most of the built assets.

**Verdict: safe / correct as designed, with one gap worth monitoring.** The core ordering guarantee (finalize-release truly last) is solid and requires no fix. The undocumented risk is that a partial-failure release run (one bad leg) silently skips finalization rather than still tidying up the release title/notes/prerelease flag for the assets that did upload. Not fixed in this pass (adding `if: ${{ !cancelled() }}` or similar to `finalize-release` is a legitimate one-line candidate fix, but changing failure-handling semantics for a release workflow is a judgment call about intended behavior, not an obviously-safe mechanical fix, so it is flagged here for a decision rather than applied).

---

## Summary

| # | Item | Verdict | Action taken |
|---|---|---|---|
| 1 | Parallel `action-gh-release` calls racing to create the release for a new tag | Needs monitoring | Documented only; no fix applied (structural change) |
| 2a | Release asset filenames never carry "direct-ip" branding (Actions-artifact names do) | Needs fix | Documented with full file:line table; fix deferred (broad, multi-file change) |
| 2b | Android `Publish signed apk package` gated on `UPLOAD_ARTIFACT` instead of `UPLOAD_RELEASE` (2 sites) | Fixed | flutter-build.yml:1190, 1374 |
| 2c | `build-flatpak` downloads a `.deb` artifact name that doesn't match what's actually uploaded (missing `direct-ip-` prefix) | Fixed | flutter-build.yml:2056 |
| 3 | `finalize-release` ordering vs. reusable-workflow completion semantics | Safe (by GH Actions design) | No fix; noted gap: a real leg failure skips finalize-release entirely (no `if: always()`/`!cancelled()` override) — flagged for a decision, not applied |

**Files touched in this pass:** `.github/workflows/flutter-build.yml` (3 one-line fixes: lines 1190, 1374, 2056). `.github/workflows/release.yml` was read but not modified — its tag computation, permissions, and finalize logic were all verified correct as-is. No changes made outside `.github/workflows/*.yml` and this doc.

**Not yet verified in CI:** none of the three fixes in this pass have been exercised by a workflow run yet (the in-progress `0.0.1-test` release run was already dispatched before these fixes landed). Recommend a follow-up `flutter-ci` or release dry-run to confirm the Android publish-gating fix and the flatpak artifact-name fix both behave as expected.
