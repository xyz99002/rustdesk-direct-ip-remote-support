# AppImage Build/Recipe Validation — 2026-09-01

Follow-up to the `build-appimage` artifact-upload fix (commit `aae2da7b7`), verified working in
[Full Flutter CI #8](https://github.com/xyz99002/rustdesk-direct-ip-remote-support/actions/runs/33537701026).
This pass validates the *content* of the AppImage recipe and the packaging job, since producing a
downloadable artifact does not by itself prove the AppImage runs correctly on a target Linux host.

## 1. Build job recap

`.github/workflows/flutter-build.yml`, job `build-appimage` (lines 1955–2012):

- Downloads the per-arch `.deb` artifact produced by `build-rustdesk-linux`.
- Renames it to `appimage/rustdesk.deb`.
- Installs `libarchive-tools`, `libfuse2`, then pip-installs
  `git+https://github.com/rustdesk-org/appimage-builder.git` (a RustDesk-org fork of
  [`AppImageCrafters/appimage-builder`](https://github.com/AppImage/appimage-builder)).
- Runs `sudo appimage-builder --skip-tests --recipe ./AppImageBuilder-${arch}.yml` from inside `appimage/`.
- Uploads `./appimage/rustdesk-${VERSION}-*.AppImage` via `actions/upload-artifact`.

`--skip-tests` means the recipe's own `test:` section (which runs the AppImage inside
fedora/debian/archlinux/centos/ubuntu-xenial containers via `appimagecrafters/tests-env`) is **not
executed in CI**. That test matrix exists in both recipe files but is dead weight today — nothing
currently exercises it, so a regression that only shows up on an older/newer base image would not
be caught by this pipeline.

## 2. Recipe analysis

Files: `appimage/AppImageBuilder-x86_64.yml`, `appimage/AppImageBuilder-aarch64.yml`. Both recipes
are otherwise identical except for `apt.arch`, sourceline (focal vs. ports.ubuntu.com for arm64),
the `AppDir/*-linux-gnu` library paths, and the top-level `AppImage.arch`.

### 2a. Dependency completeness vs. the `.deb`'s declared `Depends:`

The `.deb` control file is generated at runtime by `build.py:generate_control_file()` (line 292),
not checked into `debian/control`:

```
Depends: libgtk-3-0t64 | libgtk-3-0, libxcb-randr0, libxdo3 | libxdo4, libxfixes3, libxcb-shape0,
         libxcb-xfixes0, libasound2t64 | libasound2, libsystemd0, curl, libva2, libva-drm2,
         libva-x11-2, libgstreamer-plugins-base1.0-0, libpam0g, gstreamer1.0-pipewire
Recommends: libayatana-appindicator3-1
```

Comparing this against each recipe's `AppDir.apt.include` list (x86_64 lines 39–65, aarch64 lines
39–62):

| deb Depends/Recommends entry | in AppImage recipe? |
|---|---|
| libgtk-3-0t64 \| libgtk-3-0 | yes (`libgtk-3-0`) |
| libxcb-randr0 | yes |
| libxdo3 \| libxdo4 | yes (`libxdo3`) |
| libxfixes3 | yes |
| libxcb-shape0 | yes |
| libxcb-xfixes0 | yes |
| libasound2t64 \| libasound2 | yes (`libasound2`) |
| libsystemd0 | yes |
| curl | yes |
| libva2 / libva-drm2 / libva-x11-2 | yes |
| libgstreamer-plugins-base1.0-0 | yes |
| libpam0g | yes |
| gstreamer1.0-pipewire | yes |
| **libayatana-appindicator3-1** (Recommends only) | **no** — not in either recipe |

Finding: every hard `Depends:` is covered. The one gap is `libayatana-appindicator3-1`, which is
only a `Recommends:` on the `.deb` (used for the system-tray indicator icon on some desktop
environments). Its absence from the AppImage will not crash the app but may mean the tray icon
silently doesn't appear on distros where indicator support isn't already installed system-wide.
This is a minor, non-blocking gap — recommending inclusion, but not making the edit myself since it
changes runtime UI behavior rather than being a pure "obviously safe" addition.

The AppImage recipes also bundle several packages that go *beyond* the `.deb`'s Depends (`libdrm2`,
`libwayland-client0/cursor0/egl1`, `libpulse0`, `packagekit-gtk3-module`, `libcanberra-gtk3-module`),
which is expected — AppImage bundling errs on the side of including anything GTK/Wayland/audio
related that isn't guaranteed present on an arbitrary target host, since (unlike a `.deb` install)
there's no package manager to pull in transitive deps at install time. No obviously-missing runtime
lib was found relative to the apt-get install lists used elsewhere in `flutter-build.yml`
(`libasound2-dev`, `libgtk-3-dev`, `libva-dev`, `libxdo-dev`, etc. at lines 966–988, 1238–1260,
1529–1549, 1809–1831) — those are `-dev` build-time headers/static libs, not runtime shared libs,
and their runtime counterparts are all present in the recipe's `apt.include`.

### 2b. AppRun / desktop file / icon

- `script:` (lines 3–14) explicitly deletes the `.deb`'s shipped `usr/share/applications` directory
  before the `AppDir:` build step runs. The commented-out `sed` line (line 13) shows this used to be
  patched instead of deleted — the shipped desktop file pointed `Icon=` at an absolute
  `/usr/share/rustdesk/files/rustdesk.png` path that doesn't resolve relative to `$APPDIR`.
- `AppDir.app_info` (lines 17–23) supplies `id`, `name`, `icon: rustdesk`, `exec:
  usr/share/rustdesk/rustdesk`, `exec_args: $@`. appimage-builder auto-generates `AppRun` and a
  `.desktop` file from this section whenever they're not already present in `AppDir` — this is
  documented behavior of the tool (confirmed generally, though the recipe-reference page fetched
  during this review did not itself spell out the mechanism in prose). Given the script step removes
  the pre-existing desktop file, the recipe is relying on this auto-generation, which is consistent
  with the `icon: rustdesk` value matching the icon files copied into
  `usr/share/icons/hicolor/{32,64,128}x{32,64,128}/apps/rustdesk.png` and
  `.../scalable/apps/rustdesk.svg` by lines 10–11.
- No obvious bug here, but it is untested by `--skip-tests` — see §1. A silent failure mode to watch
  for: if a future appimage-builder version changes its auto-generation trigger condition (e.g.
  requiring an explicit `Icon=`/`Exec=` match), the desktop file could go missing entirely and the
  AppImage would still build (builder doesn't hard-fail on a missing desktop entry) but wouldn't
  register properly in a desktop environment's app menu. This wouldn't be caught by CI today.

### 2c. GLIBC / kernel version sensitivity

Both recipes pull packages from **Ubuntu 20.04 "focal"** repositories (`archive.ubuntu.com` for
x86_64, `ports.ubuntu.com` for aarch64) rather than the ubuntu-22.04 runner's own repos. This is a
deliberate and correct choice for AppImage-style bundling: since `libc6`/glibc is forward-compatible
but not backward-compatible, building/bundling against an *older* glibc (focal ships glibc 2.31,
released April 2020) maximizes the range of newer host systems the AppImage can run on. Running the
AppImage requires **glibc >= 2.31** on the host (satisfied by any mainstream distro from ~2020
onward: Ubuntu 20.04+, Debian 11+, Fedora 32+, etc.) — this is not out of the ordinary for AppImages
targeting "focal" as a baseline, and is far more conservative than building against Ubuntu 22.04's
glibc 2.35, which would raise the floor unnecessarily.

That said, the recipe comment at lines 41-42 (x86_64) flags a known, *unresolved* rough edge:

```yaml
# https://github.com/rustdesk/rustdesk/issues/9103
# Because of APPDIR_LIBRARY_PATH, this libc6 is not used, use LD_PRELOAD: ... may help,
# If you have time, please have a try.
```

I.e. `libc6:amd64` is listed in `apt.include` but `APPDIR_LIBRARY_PATH` (line 83/80) deliberately
puts the *host's* system lib dirs (`/lib64`, `/usr/lib/x86_64-linux-gnu`) ahead of the bundled
`$APPDIR` lib dirs — meaning the bundled focal glibc is present in the AppImage but not actually used
at runtime; the host's own glibc is used instead. This is a known upstream RustDesk issue, not
something introduced by this fork, and not something I've attempted to fix here (it's outside the
"trivial, obviously-safe" bar and touches library-loading behavior, which could easily break things
if changed blind). Practical implication: a host with **glibc older than what the host's own distro
line requires for GTK3/GStreamer/etc.** could still fail even though the bundled libc6 exists,
because that bundled libc6 is shadowed. This reinforces the value of the manual verification
checklist below — it can't be confirmed without actually running the binary on real hosts.

appimage-builder itself (upstream docs at
https://appimage-builder.readthedocs.io) does not document a fixed minimum glibc/kernel — it treats
that as a function of whichever `apt.sources` the recipe author points at, which is exactly what's
being controlled here via the focal sourcelines.

## 3. Runtime validation — what I *could not* do in this session

This is a Windows sandbox; there is no way to execute a Linux ELF/AppImage binary here, and
downloading the binary artifacts requires explicit user permission (not yet given, so not done). I
did, however, list the CI run's Artifacts panel in the browser to get metadata without downloading:

| Artifact | Size | SHA-256 |
|---|---|---|
| `rustdesk-direct-ip-1.4.9-x86_64.AppImage` | 80.4 MB | `a1c5401679fa231531ba8fc844fb403fcca76ed31ff3bbb09865dc280c49848b` |
| `rustdesk-direct-ip-1.4.9-aarch64.AppImage` | 77.7 MB | `4a58a7cf333a46d45c1f6ff1d1efd468cbd5dfb5fd7c12fa392f557304832ac8` |

Both are in the expected "tens of MB" range for a Flutter+Rust RustDesk build with bundled GTK/GStreamer
libraries (consistent with the sibling `.deb` artifacts at 20.5–22.1 MB plus the extra bundled shared
libraries an AppImage carries) — i.e. no sign of a truncated/empty/placeholder build from size alone.
I did not download the files themselves, so I could not inspect ELF headers, run `file`, compute
`ldd` against bundled libs, or execute `--version`. That verification is deferred to the checklist
below.

## 4. Manual verification checklist (for a human, or a future Linux CI job)

Run on a real Linux host (ideally more than one distro/glibc vintage) that has internet access to
download the artifacts, or in a Linux CI job with `runs-on: ubuntu-*`:

1. **Download & permissions**
   ```
   chmod +x rustdesk-direct-ip-1.4.9-x86_64.AppImage
   file rustdesk-direct-ip-1.4.9-x86_64.AppImage   # expect: ELF 64-bit LSB executable, x86-64 ... (appimage runtime)
   ```
2. **Size sanity** — file should be tens-of-MB (this run: 80.4 MB / 77.7 MB). A file only a few KB or
   MB in size indicates a broken/incomplete `appimage-builder` run (e.g. AppDir wasn't populated).
3. **Basic execution smoke test**
   ```
   ./rustdesk-direct-ip-1.4.9-x86_64.AppImage --version
   echo $?     # expect 0, and a version string containing 1.4.9
   ```
   If it exits non-zero or segfaults immediately, capture stderr — most likely cause would be a
   missing bundled `.so` (see step 5) or the `AppRun`/`app_info.exec` path being wrong.
4. **Headless GUI launch check** (since RustDesk is a GUI app, `--version` alone won't prove the GTK
   stack works):
   ```
   xvfb-run -a ./rustdesk-direct-ip-1.4.9-x86_64.AppImage
   ```
   then check the process doesn't immediately die (`ps`, exit code) and check
   `~/.config/rustdesk`/logs for GTK/GDK-backend errors.
5. **Dependency resolution check** — extract and inspect what the binary actually links against vs.
   what's bundled:
   ```
   ./rustdesk-direct-ip-1.4.9-x86_64.AppImage --appimage-extract
   ldd squashfs-root/usr/share/rustdesk/rustdesk | grep "not found"
   ```
   Expect zero "not found" lines. Pay special attention to `libxdo`, `libgtk-3`, `libasound`,
   `libva*`, `libwayland-*`, `libpulse` — these are the ones explicitly bundled per §2a.
6. **Cross-distro matrix** — run step 3–5 on at least: a focal-or-later Ubuntu, a recent Fedora, and
   Debian stable, to confirm the glibc-forward-compat assumption in §2c actually holds and that the
   `APPDIR_LIBRARY_PATH` host-shadowing behavior (RustDesk issue #9103) doesn't break on a host
   that's *older* than expected.
7. **Desktop integration check** — use an AppImage integration tool (e.g. `appimaged` or manually run
   `./AppImage --appimage-extract` and inspect `squashfs-root/rustdesk.desktop` /
   `squashfs-root/*.png`/`.svg`) to confirm the auto-generated desktop file and icon (§2b) actually
   exist and point at valid paths inside the AppImage.
8. **Tray icon regression check** (given the `libayatana-appindicator3-1` gap in §2a) — on a desktop
   environment that doesn't already have libayatana-appindicator installed system-wide, confirm
   whether RustDesk's tray icon appears or silently fails.

## 5. Summary

- Recipe dependency coverage is complete against the `.deb`'s hard `Depends:`; the only gap is the
  soft `Recommends: libayatana-appindicator3-1` (tray icon), not included in either AppImage recipe.
  Not fixed in this pass — flagged as a minor, non-blocking follow-up.
- AppRun/desktop/icon setup relies on appimage-builder's implicit auto-generation from `app_info`
  after the script step deletes the shipped desktop file; this is consistent with the icon files
  the script stages, but is unverified by CI (`--skip-tests` skips the recipe's own test matrix).
- The recipes' choice of Ubuntu 20.04 "focal" as the apt source is a sound, deliberate choice for
  glibc-forward-compatibility; there's a known, upstream, unresolved caveat (RustDesk issue #9103)
  where `APPDIR_LIBRARY_PATH` shadows the bundled `libc6` with the host's own, which is worth keeping
  in mind but out of scope to fix here.
- No file/recipe edits were made — no gap found was "trivial and obviously safe" enough to qualify
  under the task's edit constraint.
- Could not execute or extract the actual AppImage binaries in this Windows session; sizes/digests
  were read from the GitHub Actions Artifacts panel without downloading (80.4 MB / 77.7 MB — a sane
  range, not indicative of a truncated build). Real runtime validation requires the checklist in §4
  to be run on Linux, by a human or a follow-up Linux CI job.
