# Local Tool Removal Checklist

**Status:** Reference only — **do not execute any command in this file yet**. Pairs with `docs/LOCAL_BUILD_DECOMMISSION_PLAN.md`; read that first for the rationale behind each classification.

Each entry: what it is, the actual uninstall command for this machine, risks, and how to roll back if removing it turns out to be a mistake.

---

## Removable tier

### 1. vcpkg checkout (includes vcpkg-managed NASM)

- **Location:** `C:\Users\arvindkumarp\vcpkg` (6.5 GB)
- **Uninstall command (PowerShell):**
  ```powershell
  Remove-Item -Recurse -Force "C:\Users\arvindkumarp\vcpkg"
  ```
- **Risks:**
  - Irreversible deletion of the directory — includes `buildtrees/`, `downloads/` (incl. the vendored NASM 3.01), `installed/`, `packages/`. No uninstaller; it's a plain checkout.
  - If any local script or IDE configuration still references `C:\Users\arvindkumarp\vcpkg` (e.g. a stale `VCPKG_ROOT` set in a specific terminal profile, or a CMake cache referencing it), those will break until repointed or removed.
  - Confirmed this session: no persistent `VCPKG_ROOT` env var exists at User or Machine scope, so this is unlikely to silently break anything outside an already-open terminal session.
- **Rollback:**
  ```powershell
  git clone https://github.com/microsoft/vcpkg C:\Users\arvindkumarp\vcpkg
  C:\Users\arvindkumarp\vcpkg\bootstrap-vcpkg.bat
  ```
  This restores a working vcpkg but does **not** restore any previously-built packages — those rebuild from scratch on first use (this is exactly what CI does every run already, so it's a known-working path).

### 2. Flutter SDK

- **Location:** `C:\Users\arvindkumarp\flutter` (0.9 GB)
- **Uninstall command (PowerShell):**
  ```powershell
  Remove-Item -Recurse -Force "C:\Users\arvindkumarp\flutter"
  ```
  Also check for and remove any PATH entry pointing at `C:\Users\arvindkumarp\flutter\bin` if one was ever added (none was found in the User PATH this session, but re-verify before removing: `[Environment]::GetEnvironmentVariable("Path","User") -split ';' | Select-String flutter`).
- **Risks:**
  - No uninstaller; plain directory deletion.
  - Any local `flutter analyze`/`flutter test`/`flutter pub get` workflow you rely on stops working immediately.
  - `flutter_rust_bridge_codegen` (used in this session's CI investigation work) also needs a Flutter install to run `flutter pub get` — but that only ever ran inside CI's `generate-bridge` job, never locally on this machine.
- **Rollback:**
  ```powershell
  git clone https://github.com/flutter/flutter.git -b stable C:\Users\arvindkumarp\flutter
  ```
  (Or match the pinned CI version — `FLUTTER_VERSION: "3.24.5"` in `flutter-build.yml` — by checking out that tag specifically if version parity with CI matters for local testing.)

---

## Removable-with-caution tier

### 3. Visual Studio "Desktop development with C++" workload (not all of VS)

- **Location:** VS Professional 2026 at `C:\Program Files\Microsoft Visual Studio\18\Professional`
- **Uninstall command:** do **not** delete the directory directly. Use the Visual Studio Installer to remove only the C++ workload, leaving VS itself and any other workloads intact:
  ```powershell
  & "C:\Program Files (x86)\Microsoft Visual Studio\Installer\setup.exe" modify --installPath "C:\Program Files\Microsoft Visual Studio\18\Professional" --remove Microsoft.VisualStudio.Workload.NativeDesktop --quiet
  ```
- **Risks:**
  - **High** relative to the other items on this list: if this VS install is used for anything else on this machine (other C++ projects, other language workloads that share components), removing the workload could affect unrelated work. This plan explicitly does not recommend doing this by default — see `LOCAL_BUILD_DECOMMISSION_PLAN.md` §2.
  - The Windows SDK bundled with this VS install may also be shared by other tools; removing the C++ workload does not necessarily remove the SDK, but verify before assuming full cleanup.
  - Removing this also removes `cl.exe`, breaking any local `cargo build` that targets `x86_64-pc-windows-msvc` for a crate needing a linker.
- **Rollback:** re-run the VS Installer and add the "Desktop development with C++" workload back:
  ```powershell
  & "C:\Program Files (x86)\Microsoft Visual Studio\Installer\setup.exe" modify --installPath "C:\Program Files\Microsoft Visual Studio\18\Professional" --add Microsoft.VisualStudio.Workload.NativeDesktop --quiet
  ```

---

## Optional tier — only remove if you're certain you won't want fast local checks

### 4. LLVM/Clang

- **Location:** `C:\Program Files\LLVM` (2.8 GB)
- **Uninstall command:** LLVM's Windows installer registers an uninstaller — use it rather than deleting the directory:
  ```powershell
  Get-Package -Name "LLVM*" | Uninstall-Package
  ```
  (If that doesn't find it, check Windows "Add or Remove Programs" for "LLVM" and uninstall from there — the installer is NSIS-based and leaves an `Uninstall.exe` in the install directory as a fallback: `C:\Program Files\LLVM\Uninstall.exe`.)
- **Risks:** breaks local FFI bindgen regeneration attempts entirely (already established as low-value locally — see main plan §3) and any other local tool on this machine that happens to depend on this specific LLVM install.
- **Rollback:** reinstall from https://github.com/llvm/llvm-project/releases (match `LLVM_VERSION: "15.0.6"` from `flutter-build.yml`/`direct-ip-build.yml` if version parity with CI matters).

### 5. Rust toolchain (rustup + cargo)

- **Location:** `C:\Users\arvindkumarp\.rustup` (1.3 GB) + `C:\Users\arvindkumarp\.cargo` (2.7 GB)
- **Uninstall command:**
  ```powershell
  rustup self uninstall
  ```
  (This is the official, safe removal path — it cleans up both directories and any PATH entries it added.)
- **Risks:** removes the "emergency local debugging" capability this plan's §3 specifically recommends keeping. **Recommendation: do not remove this one** unless you're fully committing to a CI-only workflow with no local Rust checks at all.
- **Rollback:** reinstall via https://rustup.rs, then `rustup toolchain install stable-x86_64-pc-windows-msvc` to match the currently-installed toolchain.

### 6. CMake

- **Location:** system-level (bundled with the git-bash toolchain in use)
- **Uninstall command:** not a standalone recommendation — see main plan §2 (low value, possible shared dependency, skip this one).
- **Risks/Rollback:** n/a — not recommended for removal.

---

## Do not remove (nothing to do)

- `VCPKG_ROOT`, `ANDROID_HOME`, `JAVA_HOME` — confirmed not set as persistent environment variables. Nothing to clean up.
- Git, IDE, Claude Code — Required tier, not candidates for removal at all.

---

## Suggested order of operations, when you do proceed

1. vcpkg checkout (biggest win, lowest risk, no PATH/env entanglement found)
2. Flutter SDK (similarly low risk)
3. LLVM/Clang — only if you're sure you won't want local bindgen attempts
4. Rust toolchain — only if you're fully committing to CI-only, no local Rust checks
5. Visual Studio C++ workload — last, and only after explicitly confirming nothing else on this machine needs it

Verify GitHub Actions CI still passes and nothing else on the machine broke after each step before moving to the next.
