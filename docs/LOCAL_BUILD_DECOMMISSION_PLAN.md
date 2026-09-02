# Local Build Decommission Plan

**Date:** 2026-09-01
**Status:** Proposal only — **nothing has been uninstalled**. This is the inventory + classification pass requested before any removal action.
**Goal:** move from local-build-centric development to GitHub-Actions-centric development, per the architecture already established in `CI_BUILD_SUMMARY.md` and `CI_WORKFLOW_AUDIT_2026-09-01.md` ("GitHub Actions is the canonical build path").

---

## 1. Inventory of Local Build Tools (this machine, verified 2026-09-01)

| Tool | Version | Location | On PATH? | Disk footprint |
|---|---|---|---|---|
| Git | 2.55.0 | system | ✅ | (shared/small) |
| Rust (rustup + cargo) | 1.98.0, toolchain `stable-x86_64-pc-windows-msvc` | `C:\Users\arvindkumarp\.rustup` + `.cargo` | ✅ | 1.3 GB (rustup) + 2.7 GB (cargo, mostly registry cache) = **4.0 GB** |
| Flutter SDK | (not queried — `flutter` binary not on PATH this session) | `C:\Users\arvindkumarp\flutter` | ❌ (installed but not on PATH in this shell) | 0.9 GB |
| Visual Studio Build Tools | VS Professional 2026 | `C:\Program Files\Microsoft Visual Studio\18\Professional` | ❌ (`cl.exe` needs Developer Command Prompt init) | not measured — full VS install, several GB, shared with any other VS-based work on this machine |
| CMake | 4.4.3 | system (git-bash toolchain) | ✅ | small |
| NASM | 3.01 | **only** at `C:\Users\arvindkumarp\vcpkg\downloads\tools\nasm\nasm-3.01\nasm.exe` | ❌ | included in vcpkg's footprint below — **not a separate top-level install** |
| vcpkg | (bootstrapped, `vcpkg.exe` present) | `C:\Users\arvindkumarp\vcpkg` | ❌ | **6.5 GB** (buildtrees + downloads + installed + packages) |
| Windows SDK | 10.0.26100.0 | `C:\Program Files (x86)\Windows Kits\10` | n/a (used via VS) | bundled with VS |
| LLVM/Clang | present (`clang.exe` found) | `C:\Program Files\LLVM` | ❌ | 2.8 GB |
| `VCPKG_ROOT` env var | not set (User or Machine scope) | — | — | — |
| `ANDROID_HOME` env var | not set | — | — | — |
| `JAVA_HOME` env var | not set | — | — | — |

**Total measured disk footprint of the removable-candidate tools: ~14.2 GB** (vcpkg 6.5 + Rust 4.0 + LLVM 2.8 + Flutter 0.9), before counting Visual Studio/Windows SDK.

**Notable finding:** `VCPKG_ROOT`/`ANDROID_HOME`/`JAVA_HOME` were never set as persistent environment variables on this machine — earlier session work that used them did so as ephemeral per-terminal variables (e.g. inside the vcpkg bootstrap flow), not permanent machine state. There is nothing to unset there.

---

## 2. Classification

### Required — needed even after migration

| Tool | Why |
|---|---|
| **Git** | Pushing to `fork`, pulling from `upstream`, all local repo work. CI migration doesn't touch source control. |
| **A code editor/IDE** | Editing code and docs locally regardless of where builds happen. |
| **Claude Code** | The tooling this development process itself runs on. |

### Optional — useful for debugging only

| Tool | Why keep it (optionally) |
|---|---|
| **Rust/Cargo (rustup)** | `cargo check` / `cargo test --lib` against pure-Rust logic (e.g. `src/fork_config.rs`'s TOML parsing and role-mapping unit tests) runs in seconds locally and doesn't need vcpkg/Flutter at all — catches simple mistakes before spending CI minutes. Keep if you want that fast feedback loop; safe to remove if you're fully comfortable waiting for CI on every change. |
| **CMake** | Tiny footprint, general-purpose dev tool other unrelated projects on this machine may expect. Low value in removing it specifically for this migration. |
| **LLVM/Clang** | Only useful locally for attempting FFI bindgen regeneration by hand — and recall from `FFI_BINDGEN_ANALYSIS.md`/this session's earlier work that local bindgen output was already established as inconsistent with CI's (the original "opaque struct" issue was confirmed environment-specific). Its main historical local use case is already known to be unreliable. |

### Removable — no longer needed now that GitHub Actions is canonical

| Tool | Why removable | Disk recovered |
|---|---|---|
| **vcpkg** (full checkout: buildtrees/downloads/installed/packages) | GitHub Actions installs and caches its own vcpkg per-workflow (`vcpkg-cache-warmer.yml` + `lukka/run-vcpkg` in every build workflow); a local vcpkg checkout only mattered for the local NASM/aom investigation, which is resolved (see `NASM_MULTIPASS_ANALYSIS.md`) and superseded by CI. **Largest single win.** | 6.5 GB |
| NASM 3.01 | Not a separate install — lives inside vcpkg's `downloads/` cache above; removed automatically if vcpkg is removed. | (included above) |
| **Flutter SDK** | All Flutter/Dart builds happen in CI (`flutter-build.yml` installs a fresh, pinned Flutter 3.24.5 every run via `subosito/flutter-action`). Local Flutter is only useful for `flutter analyze`/`flutter test` on Dart code — if that's not part of your workflow, remove it. | 0.9 GB |

### Removable with caution — flagged, not simply recommended

| Tool | Why caution |
|---|---|
| **Visual Studio Build Tools / Windows SDK** | These are large, shared system components. Removing "just the C++ workload" via the VS Installer is possible without uninstalling VS itself, but VS may be used on this machine for other, unrelated work. Do not blanket-uninstall Visual Studio; only consider removing the "Desktop development with C++" workload if you've confirmed nothing else on this machine needs it. Given the modest disk win relative to the risk of breaking an unrelated VS-based workflow, this is the **lowest-priority, highest-risk** item — treat it as optional rather than default-removable. |

---

## 3. What's Still Needed — CI Maintenance, Release Troubleshooting, Emergency Debugging

This is the direct answer to "identify which tools are still needed" before any removal:

| Use case | What's actually needed | What's NOT needed |
|---|---|---|
| **CI workflow maintenance** (editing `.github/workflows/*.yml`) | Git, a text editor — YAML is just text. No local build tooling required to edit or reason about workflow files. | vcpkg, Flutter, LLVM, NASM |
| **Release troubleshooting** (diagnosing a `release.yml` run) | Git, a browser (GitHub Actions UI/raw logs), optionally the `gh` CLI (not currently installed — worth adding, it's small and useful for `gh run view`/`gh release` inspection without leaving the terminal) | vcpkg, Flutter, LLVM, NASM |
| **Emergency local debugging** | **Rust/Cargo only**, for `cargo check`/isolated unit tests on pure-Rust logic (`fork_config.rs`) that doesn't touch FFI/vcpkg-linked crates. This is genuinely useful and cheap to keep. | Flutter (Dart-side issues can't be meaningfully debugged without also standing up the full toolchain anyway — not worth keeping just for this); vcpkg/NASM (see below) |

**Important, concrete point on vcpkg/NASM/LLVM's actual debugging value:** the one build failure currently tracked in CI (`build-rustdesk-linux-sciter`, the GCC 7.5.0 / aom AVX2 intrinsic issue — see `CI_WORKFLOW_AUDIT_2026-09-01.md` §6 and `LINUX_SCITER_FIX_2026-09-01.md`) is **Linux/GCC-specific**. This is a Windows machine. The local vcpkg/NASM/LLVM stack here was never able to reproduce that failure and can't validate its fix either — it can only reproduce *Windows-side* vcpkg issues (like the earlier NASM-multipass problem, which is already resolved). That significantly limits how much local-debugging value the Windows vcpkg stack actually has going forward, reinforcing that it's a good removal candidate rather than an "optional keep."

**Recommended minimal keep-set:** Git + IDE + Claude Code (Required) + Rust/Cargo (Optional, cheap, genuinely useful) = everything else is a reasonable candidate for removal.

---

## 4. Documentation Review

The following docs describe or assume a local-build-centric workflow and need to be reframed around the model below. This plan does not yet rewrite them in full — see the companion checklist and follow-up commits for execution.

| Doc | Current framing | Needed change |
|---|---|---|
| `docs/REPOSITORY_AND_ARTIFACT_MAP.md` | Already updated earlier this session to reference GitHub Actions as the artifact source of truth; packaging section already reflects CI-produced deb/AppImage/etc. | Minor: add a pointer to this decommission plan near the top. |
| `docs/BUILD_VERIFICATION_RESULTS.md` | Historical record of the local NASM/vcpkg blocker (2026-08-29) plus the 2026-09-01 CI verification update. | Add a short note at the top clarifying the local-build section is historical/superseded, pointing to this plan and to CI as canonical — do not delete the history. |
| `docs/FULL_BUILD_VERIFICATION.md` | A step-by-step local build execution guide (vcpkg install → cargo build → cargo test → flutter build → quality checks). | Add a prominent banner: this procedure is superseded by CI for regular development; keep the doc for the "emergency local debugging" and "CI maintainer reproducing an issue" cases, explicitly reframed as such rather than the default path. |
| `docs/GITHUB_CI_STRATEGY.md` | Strategic rationale for adopting GitHub Actions (written before it was fully working). | Update status: the strategy is now implemented and verified (link `CI_WORKFLOW_AUDIT_2026-09-01.md`), not just proposed. |
| `docs/QUICK_START_FOR_NEW_DEVELOPER.md` | Did not exist. | **Created** (see below) — a short, action-first entry point distinct from the fuller `DEVELOPER_ONBOARDING.md`. |

Developer guidance going forward is reframed as:

- **Developer machine:** Git, an IDE, Claude Code. (Optionally: Rust/Cargo for fast local checks.)
- **GitHub Actions:** Build, Test, Package, Release — everything else.

---

## 5. Next Steps (not yet executed)

1. Review this plan and the companion `docs/LOCAL_TOOL_REMOVAL_CHECKLIST.md`.
2. Decide on the Visual Studio Build Tools question explicitly (it's the one "removable with caution" item — everything else in the Removable tier is a comparatively low-risk call).
3. When ready, work through the checklist's uninstall commands one tool at a time, verifying CI still passes and no other local workflow breaks, before moving to the next tool.
4. Only after that: apply the full documentation rewrites listed in Section 4 above.

**No uninstall actions have been taken as part of producing this plan.**
