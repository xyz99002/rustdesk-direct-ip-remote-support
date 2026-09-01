# Developer Workflow — RustDesk Direct-IP

**For:** New and existing developers  
**Goal:** Get from zero to merged PR, using GitHub Actions as the canonical build system  
**Time:** ~10 minutes to read, ~5 minutes to set up machine

---

## Machine Setup (One-Time)

### Required

```bash
# Clone the repo
git clone https://github.com/xyz99002/rustdesk-direct-ip-remote-support.git
cd rustdesk-direct-ip-remote-support

# Add upstream remote (for fetching RustDesk baseline changes)
git remote add upstream https://github.com/rustdesk/rustdesk.git

# Verify remotes
git remote -v
# Should show:
#   fork       https://github.com/xyz99002/rustdesk-direct-ip-remote-support.git (fetch/push)
#   upstream   https://github.com/rustdesk/rustdesk.git (fetch/push)
```

**Install:**
- **Git** (for version control)
- **An IDE or editor** (VS Code, JetBrains, vim, etc.)
- **Claude Code** (optional, but recommended for this workflow)

That's it. No local build tools required.

### Optional (Recommended)

```bash
# Install Rust toolchain for fast local checks
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup toolchain install stable-x86_64-pc-windows-msvc  # (Windows)
# or: rustup toolchain install stable  # (Linux/macOS)

# Verify
cargo --version
```

**Why:** `cargo check` on pure-Rust files (`src/fork_config.rs`) catches mistakes in seconds before pushing to CI. Optional but time-saving.

---

## The Workflow

### 1. Make a Change

Edit code or docs locally in your IDE.

```bash
# Example: fix a bug in fork_config.rs
$EDITOR src/fork_config.rs
```

### 2. Commit Locally

```bash
git add src/fork_config.rs
git commit -m "Fix: TOML parsing bug in role detection logic"
```

### 3. (Optional) Run Local Checks

If you have Rust installed, catch mistakes in 10 seconds:

```bash
cargo check                    # Type-check only
cargo test --lib              # Run Rust unit tests
```

**No local check needed?** Skip straight to step 4. GitHub Actions will catch any issues.

### 4. Push to Your Fork

```bash
git push fork <your-branch-name>
```

### 5. GitHub Actions Builds Automatically

**What happens next:**
- Push to `fork/master` → triggers `flutter-ci.yml` (full build matrix)
- Pull request opened → triggers `flutter-ci.yml` on that PR

**Watch the build at:** https://github.com/xyz99002/rustdesk-direct-ip-remote-support/actions

**Statuses:**
- ✅ All green → your change is good to merge
- ❌ Red job → CI caught a failure (see CI_TROUBLESHOOTING.md)

### 6. Iterate if Needed

If CI fails:
1. Read the error message in the failing job's logs
2. Fix the issue locally
3. Commit and push again
4. GitHub Actions re-runs automatically

---

## The Build Matrix (What CI Does)

When you push to `master` or open a PR, GitHub Actions runs:

| Platform | Architectures | What it does |
|---|---|---|
| **Windows** | x64, x86, arm64 | Builds, packages unsigned MSI installer |
| **macOS** | x64, arm64 | Builds, creates DMG |
| **Linux (generic)** | x86_64, aarch64, armv7 | Builds deb packages |
| **Linux (sciter legacy)** | x86_64, armv7 | Builds sciter-specific deb packages |
| **Linux (AppImage)** | x86_64, aarch64 | Builds self-contained AppImage executable |
| **Linux (Flatpak)** | x86_64 | Builds Flatpak bundle |
| **Android** | universal (arm64/x86) | Builds unsigned APK |

**Artifacts tab:** Every build job uploads binaries to GitHub Actions artifacts (90-day retention) for download and testing.

---

## When Something Fails

See **CI_TROUBLESHOOTING.md** for how to debug CI failures.

**TL;DR:** Click the red job, scroll to the failing step, read the error message, fix it locally, push again.

---

## Cutting a Release

Releases are manual, one-time events, not part of the routine workflow:

```bash
# Go to GitHub Actions → release.yml
# Click "Run workflow" button
# Enter the Direct-IP version (e.g., "1.0.0")
# GitHub Actions:
#   1. Computes the combined tag: v{rustdesk-version}-direct-ip.{your-version}
#   2. Builds everything (same build matrix as above)
#   3. Publishes a GitHub Release with all artifacts

# View the release at: https://github.com/xyz99002/rustdesk-direct-ip-remote-support/releases
```

More detail: see **RELEASE_WORKFLOW_AUDIT_2026-09-01.md**

---

## Key Constraints

**Do not change:**
- Architecture (one executable, TOML-configured roles)
- Transport or authentication
- Support/Desktop workflows
- Direct-IP functionality

Focus on:
- Release quality
- CI quality
- Artifact quality
- Developer experience

---

## Quick Reference

| Task | Command |
|---|---|
| Clone the repo | `git clone https://github.com/xyz99002/rustdesk-direct-ip-remote-support.git` |
| Check out a branch | `git checkout -b feature/my-feature` |
| Commit locally | `git add <files> && git commit -m "message"` |
| Local type-check (Rust only) | `cargo check` |
| Local unit tests (Rust only) | `cargo test --lib` |
| Push to fork | `git push fork <branch>` |
| Watch CI | https://github.com/xyz99002/rustdesk-direct-ip-remote-support/actions |
| View a failing job | Click the red job in the Actions UI; scroll to the error |
| Trigger a release | https://github.com/xyz99002/rustdesk-direct-ip-remote-support/actions/workflows/release.yml |

---

## References

- **Quick setup:** QUICK_START_FOR_NEW_DEVELOPER.md (1-minute version of this)
- **Full onboarding:** DEVELOPER_ONBOARDING.md (architecture, CI structure, repo layout)
- **CI troubleshooting:** CI_TROUBLESHOOTING.md (when builds fail)
- **Release process:** RELEASE_WORKFLOW_AUDIT_2026-09-01.md (detailed release steps)
- **Local tool decommission:** LOCAL_BUILD_DECOMMISSION_DECISION.md (why we don't need local builds)
