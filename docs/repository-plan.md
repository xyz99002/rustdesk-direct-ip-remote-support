# Repository Plan (Final)

**Status:** Frozen 2026-08-28. This plan has already been executed (Phase 2); this document now records the final, confirmed repository structure rather than a pending proposal. It supersedes the earlier `docs/phase1-repository-plan.md` draft it replaces.

## Confirmed structure

- `C:\Work\RustDesk` is a git repository (`git init`, done).
- `upstream` remote: `https://github.com/rustdesk/rustdesk.git` (read-only reference).
- `main`: upstream RustDesk tag `1.4.9` (commit `6c578292e8ebbbec708b76986ba8c4bc7c509747`), merged with this project's planning docs via `git merge --allow-unrelated-histories` — an untouched baseline, never committed to directly.
- `feature/direct-ip-fork`: the working branch, created off `main`. All implementation phases commit here.
- Submodule `libs/hbb_common` (`https://github.com/rustdesk/hbb_common`, pinned `7e1c392c62d39c364127307cd408421dd5f8cfb0`) initialized and checked out.
- Planning artifacts (`CLAUDE_MASTER_PROMPT.md`, `docs/DECISIONS.md`, `PROMPTS/`, `CHANGELOG_TEMPLATE.md`, `REQUIREMENTS_TRACEABILITY_TEMPLATE.md`, `docs/`) sit at the repo root alongside the upstream tree — no filename collisions with RustDesk's own layout (`src/`, `flutter/`, `libs/`, `Cargo.toml`, `.github/`, etc.).
- `CHANGELOG_IMPLEMENTATION.md` instantiated from `CHANGELOG_TEMPLATE.md` and kept current every phase. `REQUIREMENTS_TRACEABILITY.md` still pending — to be instantiated once concrete requirement/test pairs exist to map (Configuration phase onward).
- `.gitignore`: upstream's base, plus `.claude/` for this session's local tooling directory.

## Scope impact of the frozen requirements (2026-08-28)

The authentication and connectivity decisions in `docs/architecture.md` and `docs/DECISIONS.md` are config/UI-surface changes, not engine changes. Consequences for repo structure:

- No new or excluded Cargo workspace members are needed for authentication or transport — there was never a separate `hbbs`/`hbbr` crate in this repo to remove (confirmed in `docs/upstream-analysis.md` §2), and no server-side auth code is being rewritten.
- No new build-time feature flags (e.g. a "direct-IP-only" compile-time mode) are planned — direct-IP-only is a product decision enforced by what the UI exposes, not by conditional compilation.
- The `feature/direct-ip-fork` branch name predates this scope narrowing but is kept as-is; it still accurately describes the product's direct-IP-only UI/config restriction, even though the underlying transport code is unmodified.

## Licensing

RustDesk is licensed AGPL-3.0. `LICENCE` and all copyright metadata (including the `Cargo.toml` `winres` `LegalCopyright` field) are preserved unmodified. Because AGPL-3.0's network-use clause is relevant to a remote-access tool, `docs/licensing.md` (Packaging phase) will document the fork's obligations explicitly rather than leaving it implicit.

## Why fork-in-place rather than submodule/subtree

RustDesk's own build tooling (Cargo workspace, Flutter project, CI scripts) expects to run from the repository root. A submodule or subtree would require path indirection that upstream's own scripts don't expect, adding risk for no benefit here. A direct, tag-pinned clone with our docs alongside it — which is what was done — matches how RustDesk itself is typically forked by third parties.

No RustDesk source files (as opposed to planning docs and git/branch structure) have been modified under this plan.
