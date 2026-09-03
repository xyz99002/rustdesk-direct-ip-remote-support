# Developer Onboarding — RustDesk Direct-IP Fork

Welcome. This document is the map: a single entry point that orients a new
developer and points to the detailed docs that already exist in this
repository. It intentionally does not duplicate their content — read it,
then follow the links for depth.

**Start here based on your immediate goal:**
- **First 5 minutes?** → `QUICK_START_FOR_NEW_DEVELOPER.md`
- **Daily workflow (edit → push → CI)?** → `DEVELOPER_WORKFLOW.md`
- **CI build failed?** → `CI_TROUBLESHOOTING.md`
- **Full architectural understanding?** → This document (DEVELOPER_ONBOARDING.md)

---

## 1. What this fork is

This is a Direct-IP-only fork of upstream RustDesk. The architecture is
**one executable, controlled by TOML role configuration** — there are no
separate "Local" and "Remote" executables, and no new transport,
authentication, or voice-call code was written to achieve this.
(2026-09-02: role configuration lives in upstream's own `RustDesk2.toml`
under `direct-ip-*` keys — see Section 2 — not a separate `fork_config.toml`
file as in earlier versions of this fork.) A single binary reads this
config at startup and reconfigures itself (via upstream's own existing settings
mechanisms) into either an outbound-only ("local") or inbound-only
("remote") role. As of `docs/ADR-0003-DIRECT-IP-ENFORCEMENT.md`, this is
enforced not just in the UI but at the protocol level: rendezvous-server
registration and relay participation are removed from
`src/rendezvous_mediator.rs::start_all()`, and LAN-discovery's ID-exposing
reply is disabled via the existing `enable-lan-discovery` option.

**Explicitly NOT done, by design** (see ADR-0003 "What was explicitly NOT
touched" and `docs/FORK_PROFILE_SPEC.md`):
- No separate Local/Remote executables or build targets.
- No new transport, encryption, or framing — direct-IP dial/listen paths
  are upstream's own `src/client.rs`/direct-IP listener code.
- No changes to authentication (`src/server/connection.rs` login/approval,
  `approve-mode` mapping) — the fork only maps its own `authentication.mode`
  TOML key onto upstream's existing `approve-mode` option.
- No changes to Voice Call or `VIEW_CAMERA` session establishment — Support
  mode reuses these upstream mechanisms unmodified.
- No new authentication path added to reject `DEFAULT_CONN` remotely
  (documented as a known gap in `FORK_PROFILE_SPEC.md`, not silently
  worked around).

Start with `docs/ADR-0003-DIRECT-IP-ENFORCEMENT.md` for the full reasoning,
and `docs/FORK_PROFILE_SPEC.md` for the product-level behavioral spec.

---

## 2. Repository layout

**Remotes** (see `docs/REPOSITORY_AND_ARTIFACT_MAP.md` for the full,
up-to-date picture):
- `fork` → `https://github.com/xyz99002/rustdesk-direct-ip-remote-support.git`
  (canonical repo for this fork; fetch and push)
- `upstream` → `https://github.com/rustdesk/rustdesk.git` (for pulling
  upstream RustDesk changes)

**Branch:** local development happens on `master`, tracking `fork/master`.

**Fork-specific code** (the surface area a new contributor should actually
touch):
- `src/fork_config.rs` — reads `direct-ip-*` options from `RustDesk2.toml`
  (via `Config::get_option`, same mechanism as every other upstream option)
  and applies them onto upstream's existing config/settings mechanisms
  (role, auth mode, Support/Desktop button gating, Settings UI visibility,
  minimal-UI settings, `enable-lan-discovery=N`). Read its module doc
  comment for the mapping table from option key to upstream mechanism.
- `configs/local.toml`, `configs/remote.toml`,
  `configs/all-options-reference.toml` — sample/reference `RustDesk2.toml`
  files (repo root has no config file of its own since 2026-09-02).
- `src/rendezvous_mediator.rs` — contains the `--- BEGIN/END DIRECT-IP
  FORK ---` marked blocks that remove rendezvous registration and relay
  participation (ADR-0003). Treat this file as upgrade-sensitive.
- `res/vcpkg/aom/portfile.cmake` and
  `res/vcpkg/aom/aom-disable-multipass-check.diff` — an aom vcpkg overlay
  patch bypassing an NASM multipass capability check (see
  `docs/NASM_MULTIPASS_ANALYSIS.md`; confirmed safe, only affects encoding
  performance by 5–15%, not codec correctness).
- `appimage/` — AppImage packaging recipe used by `flutter-build.yml`'s
  `build-appimage` job.
- `.github/workflows/*.yml` — see Section 4.

**Everything else** (`src/` broadly, `flutter/`, `libs/`, etc.) is
untouched upstream RustDesk code, except for the narrow, explicitly-marked
hooks above plus the Minimal UI Dart changes referenced in
`FORK_PROFILE_SPEC.md` (`connection_page.dart`, `desktop_home_page.dart`,
`desktop_setting_page.dart`).

---

## 3. Configuration

The fork is driven by upstream's own `RustDesk2.toml` (the `[options]`
table), under a set of `direct-ip-*`-prefixed keys — not a separate config
file (as of 2026-09-02; see `docs/CONFIG_REFERENCE.md` for why the earlier
two-file design was consolidated). If those keys are absent, the app runs
as plain upstream RustDesk; if present but invalid, the same fallback
applies with the error logged.

Keys: `direct-ip-config-version`, `direct-ip-role` (`"local"`/`"remote"`),
`direct-ip-support-enabled`, `direct-ip-desktop-share-enabled`,
`direct-ip-show-setup-ui` (optional, defaults to shown),
`direct-ip-listen-address`, `direct-ip-listen-port`,
`direct-ip-video-quality`, `direct-ip-audio-quality`,
`direct-ip-log-level`, `direct-ip-auth-mode`
(`"ask"` / `"password"` / `"ask_and_password"`). At least one of
`direct-ip-support-enabled`/`direct-ip-desktop-share-enabled` must be `"Y"`.

This is a summary only — for the authoritative schema and behavioral
details see `docs/CONFIG_REFERENCE.md`, `docs/FORK_PROFILE_SPEC.md`, and
the fully-commented `configs/all-options-reference.toml`. For how each key
is applied onto upstream mechanisms, skim `src/fork_config.rs`'s module doc
comment.

---

## 4. CI/CD workflows

All triggers, jobs, and artifacts below were verified directly against the
workflow YAML files in `.github/workflows/`.

| Workflow file | Trigger | What it builds | Where artifacts land | Known issues |
|---|---|---|---|---|
| `direct-ip-build.yml` ("Direct-IP Build") | `workflow_dispatch`; PRs; push to `feature/direct-ip-fork` (paths-ignore `docs/**`, `README.md`) | Generates the Flutter/Rust bridge glue, then builds Windows x64 (Flutter, portable) and Linux x86_64 (`cargo build --release` + `cargo test` under `xvfb-run`) | `rustdesk-direct-ip-windows-x86_64` and `rustdesk-direct-ip-linux-x86_64` GitHub Actions artifacts (bare binary for Linux, not packaged) | None found in the audit — self-contained, no repo-name assumptions |
| `flutter-ci.yml` ("Full Flutter CI") | `workflow_dispatch`; PRs; push to `master` | Calls the full upstream `flutter-build.yml` matrix (Windows x64/x86/arm64, macOS x64/arm64, Linux `.deb`/sciter `.deb`/AppImage/Flatpak, Android, web/F-Droid) with `upload-artifact: true`, `upload-release: false` | GitHub Actions "Artifacts" tab, 90-day retention, for every job that produces a binary | `build-rustdesk-linux-sciter` (x86_64 leg) fails — see Section 7 |
| `flutter-nightly.yml` ("Flutter Nightly Build") | Scheduled `0 0 * * *` (midnight UTC); `workflow_dispatch` | Same `flutter-build.yml` matrix, with `upload-release: true`, `upload-tag: "nightly"` | A GitHub Release tagged `nightly`, plus Actions artifacts | Same sciter x86_64 failure as above; historically also hit the same permissions gap flutter-ci.yml had (now fixed — both workflows declare `permissions: contents: write`) |
| `release.yml` ("Create Direct-IP Release") | `workflow_dispatch` only, with required input `direct-ip-version` | `determine-version` job computes a combined tag from the RustDesk baseline (read from `flutter-build.yml`'s `env.VERSION`) + the input; `build` job invokes `flutter-build.yml` with that tag; `finalize-release` sets the GitHub Release title/notes and marks it non-prerelease | Same per-platform artifacts as `flutter-build.yml`, attached to a real GitHub Release named `v{rustdesk-version}-direct-ip.{direct-ip-version}` | Corrected tag-computation fix has not yet been exercised end-to-end (dry run pending) |
| `vcpkg-cache-warmer.yml` | Scheduled `0 2 * * *` (2 AM UTC); `workflow_dispatch` | Pre-installs vcpkg dependencies for both Linux (`x64-linux`) and Windows (`x64-windows-static`) triplets to populate the GitHub Actions binary cache | No build artifact — just a warmed cache used by subsequent builds | None found |

For full root-cause detail on the known issues (including the now-fixed
`flutter-ci` release-publish bugs), see
`docs/CI_WORKFLOW_AUDIT_2026-09-01.md`. For a narrative summary and typical
build-time expectations, see `docs/CI_BUILD_SUMMARY.md`. For copy-paste-ready
commands to verify remotes and trigger CI via `git push`, see
`docs/GITHUB_COMMANDS.txt`.

---

## 5. How to build/test

**Be honest about this up front: local Windows builds (vcpkg/NASM/FFI
bindgen) have historically been blocked on this project** — see
`docs/NASM_MULTIPASS_ANALYSIS.md` and `docs/BUILD_VERIFICATION_RESULTS.md`
for the history. GitHub Actions is the canonical, recommended build path
for this fork, not a local `cargo build`/`flutter build`.

To trigger a build:
1. Go to the repository's GitHub **Actions** tab
   (`https://github.com/xyz99002/rustdesk-direct-ip-remote-support/actions`).
2. Pick the workflow you want (`Direct-IP Build`, `Full Flutter CI`,
   `Flutter Nightly Build`, or `vcpkg Cache Warmer`) and click **Run
   workflow** (`workflow_dispatch`) — most of these need no inputs.
3. Alternatively, pushing to `master` triggers `flutter-ci.yml`
   automatically, and pushing to `feature/direct-ip-fork` triggers
   `direct-ip-build.yml`.
4. Watch the run under the Actions tab; once it completes, download
   artifacts from the run's **Artifacts** section (bottom of the run page).

If you do need to attempt a local build, read
`docs/NASM_MULTIPASS_ANALYSIS.md` and `docs/BUILD_VERIFICATION_RESULTS.md`
first to understand what's already known to fail and why.

---

## 6. How to cut a release

Releases are produced by `release.yml` ("Create Direct-IP Release"),
which only runs via manual `workflow_dispatch`:

1. Go to Actions → **Create Direct-IP Release** → **Run workflow**.
2. Supply the `direct-ip-version` input (e.g. `1.0.0`).
3. The `determine-version` job reads the RustDesk baseline version out of
   `flutter-build.yml`'s `env.VERSION` and computes the combined tag
   `v{rustdesk-version}-direct-ip.{direct-ip-version}` (e.g.
   `v1.4.9-direct-ip.1.0.0`).
4. The `build` job runs the full `flutter-build.yml` matrix with
   `upload-release: true` against that tag.
5. `finalize-release` sets the GitHub Release title/notes (noting the
   RustDesk baseline and that Windows/Android builds are unsigned) and
   marks the release non-prerelease.

All platform artifacts attach directly to that GitHub Release — there is
no separate `releases/` directory in this repository. For the detailed
history of what was found and fixed in this workflow (the tag used to be
computed incorrectly), see `docs/CI_WORKFLOW_AUDIT_2026-09-01.md` Section 7,
and `docs/REPOSITORY_AND_ARTIFACT_MAP.md` for the full artifact-naming
convention.

---

## 7. Where to look when CI fails

Start with these documents rather than re-diagnosing from scratch:
- `CI_TROUBLESHOOTING.md` — **First stop**: golden path to fix common failures (click the red job, read the error, fix locally, push again)
- `CI_WORKFLOW_AUDIT_2026-09-01.md` — Detailed audit of every workflow, with root causes and fixes for issues found
- `BUILD_VERIFICATION_RESULTS.md` — Build execution results log and historical context

One currently-known issue worth flagging without going deep here: the
`build-rustdesk-linux-sciter` job's `x86_64` leg fails on a GCC 7.5.0 /
aom 3.12.1 AVX2-intrinsic incompatibility (`_mm256_set_m128i` not declared
before GCC 8) inside that job's pinned legacy-compatibility container. The
`armv7` leg of the same job is unaffected. See
`docs/CI_WORKFLOW_AUDIT_2026-09-01.md` Section 6, Failure 2 for the full
root cause and remediation options (a small compiler-compat shim via a new
vcpkg overlay patch is the recommended, not-yet-implemented fix).

---

## 8. Explicit non-goals

The following are considered stable and **out of scope for casual
contributions** to this fork. See `docs/ADR-0003-DIRECT-IP-ENFORCEMENT.md`
for the reasoning behind each:

- **Transport** — direct-IP dial/listen paths, encryption, and framing are
  upstream RustDesk's own mechanisms, unmodified.
- **Authentication** — `ask` / `password` / `ask_and_password` modes map
  directly onto upstream's existing `approve-mode`; no new auth logic.
- **Voice Call** — the existing `VoiceCallRequest`/`VoiceCallResponse`
  message-level mechanism on `VIEW_CAMERA` sessions is reused unmodified.
- **The Support/Desktop role workflow** — Support always opens
  `VIEW_CAMERA` + Voice Call (optionally plus `DEFAULT_CONN`), Desktop
  always opens a plain `DEFAULT_CONN` session; this session model is
  considered settled (see `docs/FORK_PROFILE_SPEC.md` "Session Profile").
- **The configuration schema shape** — the `direct-ip-*` key set and
  meaning (Section 3 above) is the stable contract; changing it requires a
  version bump and migration guidance per `FORK_PROFILE_SPEC.md`'s
  "Upgrade Rules."

If a change seems to require touching any of the above, treat it as a
design decision, not a routine PR — read ADR-0003 first and raise it for
discussion.
