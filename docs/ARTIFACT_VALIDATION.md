# Artifact Validation

**Scope note:** per current project priorities, this is artifact-correctness validation only —
no production release is being prepared, announced, or discussed here.

**Method:** static inspection of `.github/workflows/flutter-build.yml` build/package/upload steps
(source-level trace of what each job actually produces and names). No binaries were downloaded
or executed in this pass — CI run #13 was still in progress (Sciter matrix re-run) at the time of
writing; execution-based validation (launch test, dependency-bundling confirmation) should be a
follow-up once that run completes and artifacts are downloadable.

---

## Windows Artifact

**Build step:** `flutter-build.yml:345-346` (Flutter/non-sciter) and `:516-517` (sciter fallback)
```
mv ./target/release/rustdesk-portable-packer.exe ./SignOutput/rustdesk-${{ env.VERSION }}-${{ matrix.job.arch }}.exe
```

| Check | Result |
|---|---|
| Executable naming | `rustdesk-{version}-{arch}.exe` — matches upstream convention, **no `direct-ip` branding in the filename itself** |
| GitHub Actions artifact label | `rustdesk-direct-ip-windows-${{ matrix.job.arch }}` (`:326`) — **label differs from the actual `.exe` filename inside it** (see Finding 1) |
| Portable packer | `rustdesk-portable-packer.exe` is a self-extracting portable build (upstream's own packer, unmodified) — bundling model is upstream's existing mechanism, not fork-specific |
| Signing | `res/job.py sign_files` invoked (`:370`) — requires `SIGN_BASE_URL`/`SIGN_SECRET_KEY` secrets; **unsigned if secrets absent**, consistent with prior documentation that Windows builds are unsigned in this fork's releases |
| Launch test | **Not performed this pass** — requires downloading and running the artifact, deferred to follow-up |

---

## AppImage Artifact

**Build step:** `flutter-build.yml:1982-1995` (`appimage-builder` invocation, no rename step at
current baseline)

| Check | Result |
|---|---|
| Generated filename | `rustdesk-{version}-{arch}.AppImage` (appimage-builder's own default naming from `AppImageBuilder-*.yml`'s `app_info.name: rustdesk` + `app_info.version: 1.4.9`) |
| GitHub Actions artifact label | `rustdesk-direct-ip-${{ env.VERSION }}-${{ matrix.job.arch }}.AppImage` (`:2001`) |
| Upload path glob | `./appimage/rustdesk-${{ env.VERSION }}-*.AppImage` (`:2002`) — **matches the actual generated filename correctly** |
| **Finding 1: Label/filename mismatch** | The GitHub Actions artifact *label* (what a user sees in the Artifacts tab, and what would appear as the download name on a GitHub Release) says `rustdesk-direct-ip-...`, but the *actual file inside* is named plain `rustdesk-{version}-{arch}.AppImage` with no `direct-ip` marker. A user who downloads and later renames/moves the file would lose the only place the Direct-IP branding appeared. This was previously addressed with an explicit rename step in a prior session iteration, but was reverted while resolving an unrelated CI regression (see `BUILD_VERIFICATION_RESULTS.md`) and has not been re-applied, since release-asset naming is out of scope for the current artifact-correctness-only priority. **Documented, not fixed, per current scope.** |
| AppImage recipe validity | `appimage/AppImageBuilder-x86_64.yml` and `-aarch64.yml`: valid YAML structure, `exec: usr/share/rustdesk/rustdesk`, version pinned `1.4.9` (hardcoded — does not read `env.VERSION` dynamically; would need a bump if the baseline RustDesk version changes) |
| Dependency bundling | Recipe pulls `libc6`, `libgtk-3-0`, `libxcb-*`, `libxdo3`, `libasound2`, etc. via `apt` into the AppDir (`AppImageBuilder-x86_64.yml:39-50`) — standard AppImage self-containment approach, consistent with upstream project's own recipe (not fork-modified) |
| Launch test | **Not performed this pass** — deferred |

---

## Release Asset Naming — Cross-Reference

Per `flutter-build.yml` at current baseline (all workflow files reset to commit `aae2da7b7`):

| Platform | Uploaded artifact label | Actual filename | Consistent? |
|---|---|---|---|
| Windows | `rustdesk-direct-ip-windows-{arch}` | `rustdesk-{version}-{arch}.exe` | ❌ Label/filename mismatch (same pattern as AppImage) |
| macOS | `rustdesk-direct-ip-macos-{arch}` | `rustdesk-{version}.dmg` | ❌ Same mismatch pattern |
| Android (apk) | `rustdesk-direct-ip-{version}-{arch}.apk` | Same filename pattern used in `mv` step (`:1158`) | ✅ Consistent |
| Linux deb (generic) | `rustdesk-direct-ip-{version}-{arch}.deb` | `rustdesk-{version}-{arch}.deb` (`:1980` shows the plain name used internally for the appimage/flatpak build inputs) | ❌ Label says direct-ip, uploaded `path:` still references the plain name |
| Linux deb (sciter) | `rustdesk-direct-ip-{version}-{arch}-sciter.deb` | Not independently re-verified this pass | Likely same pattern |
| AppImage | `rustdesk-direct-ip-{version}-{arch}.AppImage` | `rustdesk-{version}-{arch}.AppImage` | ❌ See Finding 1 |
| Flatpak | `rustdesk-{version}-{arch}{suffix}.deb` — **note: this label itself has no `direct-ip` prefix at all** (`:2056`) | N/A | ❌ Inconsistent with every other platform's label convention |

**Overall finding:** the `direct-ip` branding is applied inconsistently — present in most upload
*labels* (which only the uploader/CI operator sees, not an end user downloading a Release asset)
but largely **absent from the actual filenames** end users would see and download. This is a
correctness finding under the current "artifact correctness" priority, independent of any release
timing decision.

---

## Version Correctness

`env.VERSION` is read once from `flutter-build.yml`'s own `env:` block and threaded through every
job. Cross-checked: the AppImage recipe's `app_info.version: 1.4.9` is a **separately
hardcoded** value inside `AppImageBuilder-x86_64.yml`/`-aarch64.yml`, not derived from
`env.VERSION`. If the workflow's version is ever bumped without also updating these two YAML
files, the AppImage's internal metadata version would silently drift out of sync with the actual
binary version. **Flagged as a correctness risk, not fixed this pass** (fixing it is a one-line
edit per file but touches release-adjacent tooling, deferred per current priority ordering).

---

## Executable Launch Verification

Not performed. Requires either:
1. Downloading a completed artifact from a finished CI run (run #13 was still in progress at
   time of writing), or
2. A local build — explicitly out of scope per this project's GitHub-Actions-canonical build
   policy (`docs/DEVELOPER_WORKFLOW.md`).

**Recommended follow-up:** once CI run #13 completes, download the Windows `.exe` and Linux
`.AppImage` artifacts and confirm: (a) the executable launches without missing-DLL/missing-`.so`
errors, (b) the Direct-IP connection screen (Support/Desktop buttons, no peer list, no
Account/Network tabs) renders as expected per `FORK_PROFILE_SPEC.md`, (c) a pre-seeded
`RustDesk2.toml` with `direct-ip-*` options is actually picked up (per `docs/CONFIG_REFERENCE.md`
Section 2).

---

## Summary

| Artifact | Naming correct? | Dependencies verified? | Launches? |
|---|---|---|---|
| Windows exe | ❌ Label/filename mismatch | Not verified (static-only) | Not tested |
| AppImage | ❌ Label/filename mismatch (Finding 1) | Recipe structurally sound | Not tested |
| macOS dmg | ❌ Label/filename mismatch | N/A (signing-dependent) | Not tested |
| Android apk | ✅ Consistent | Not verified | Not tested |
| Linux deb | ❌ Label/filename mismatch | Not verified | Not tested |
| Flatpak | ❌ Label missing `direct-ip` entirely | Not verified | Not tested |

**No production release readiness claim is made here** — this table reflects artifact-generation
correctness only, per current project priority.
