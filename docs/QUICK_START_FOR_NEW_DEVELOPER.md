# Quick Start for New Developers

**TL;DR version of developer setup.** For full depth, see `docs/DEVELOPER_ONBOARDING.md`; for the daily workflow, see `docs/DEVELOPER_WORKFLOW.md`.

---

## Machine Setup (5 minutes)

## What you need on your machine

- **Git** — to clone/push/pull.
- **An IDE or editor.**
- **Claude Code** (if you're using it for this workflow).
- *(Optional)* Rust/Cargo, for fast `cargo check`/unit tests on pure-Rust logic like `src/fork_config.rs`. Not required.

You do **not** need Flutter, vcpkg, NASM, LLVM, or Visual Studio Build Tools installed locally. GitHub Actions builds, tests, packages, and releases everything. See `docs/LOCAL_BUILD_DECOMMISSION_PLAN.md` for the full reasoning and `docs/LOCAL_TOOL_REMOVAL_CHECKLIST.md` if you're cleaning up a machine that has them installed from before this shift.

## Get the repo

```bash
git clone https://github.com/xyz99002/rustdesk-direct-ip-remote-support.git
cd rustdesk-direct-ip-remote-support
git remote add upstream https://github.com/rustdesk/rustdesk.git
```

Branch: `master`. That's what CI watches and what `release.yml` builds from.

## What this fork is, in one paragraph

One executable, TOML-controlled role configuration (see `configs/local.toml`/`configs/remote.toml` and `docs/FORK_PROFILE_SPEC.md`). Config lives in `RustDesk2.toml` (`[options]` table, `direct-ip-*` keys) — no separate `fork_config.toml` file since 2026-09-02. No separate Local/Remote executables, no new transport, no auth/voice-call changes. Full rationale in `docs/ADR-0003-DIRECT-IP-ENFORCEMENT.md`.

## Making a change

1. Edit code/docs locally.
2. Commit, push to `fork`.
3. Push to `master` triggers `flutter-ci.yml` automatically (the full build/test matrix). Watch it at:
   `https://github.com/xyz99002/rustdesk-direct-ip-remote-support/actions`
4. If you only touched a pure-Rust file with no FFI dependency, `cargo check`/`cargo test --lib` locally first if you have Rust installed — it's faster than waiting on CI for a typo.

## Cutting a release

Actions tab → `Create Direct-IP Release` (`release.yml`) → Run workflow → enter a version like `1.0.0`. It computes a tag combining the RustDesk baseline and your version (e.g. `v1.4.9-direct-ip.1.0.0`), builds everything, and publishes a GitHub Release. Detail: `docs/RELEASE_WORKFLOW_AUDIT_2026-09-01.md`.

## When something fails in CI

Start here: `docs/CI_WORKFLOW_AUDIT_2026-09-01.md` and `docs/BUILD_VERIFICATION_RESULTS.md` — both are living records of what's known-broken and why. As of this writing there's one known failure (`build-rustdesk-linux-sciter`, x86_64 leg, a GCC/aom compiler-version issue unrelated to this fork's own code — see `docs/LINUX_SCITER_FIX_2026-09-01.md`).

## Full depth

`docs/DEVELOPER_ONBOARDING.md` covers all of the above with more detail, plus a full table of every CI workflow file, what triggers it, and where its output goes.
