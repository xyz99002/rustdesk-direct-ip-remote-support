# Phase 1 — Repository Plan

## Current state

`C:\Work\RustDesk` is a plain folder, not a git repository, containing only the spec/template files listed in `docs/phase1-environment-discovery.md`. No RustDesk source is present.

## Proposed plan (to be executed as Phase 2, pending confirmation — see risk assessment)

1. `git init` this directory as the working repository.
2. Add a read-only reference remote: `git remote add upstream https://github.com/rustdesk/rustdesk.git`.
3. Fetch tags and select the newest stable release tag (exact tag confirmed during Phase 2, not `master`) as a reproducible, auditable baseline — rather than a moving branch.
4. Create `main`, checked out at that tag, as the untouched upstream baseline — preserves upstream history, authorship, and the AGPL-3.0 `LICENSE` file exactly as published.
5. Create a working branch, `feature/direct-ip-fork`, off `main`. All subsequent phases (4–11) commit here, never on `main`.
6. Leave the existing planning artifacts (`CLAUDE_MASTER_PROMPT.md`, `PROMPTS/`, `CHANGELOG_TEMPLATE.md`, `REQUIREMENTS_TRACEABILITY_TEMPLATE.md`, `docs/`) in place at the repo root alongside the upstream tree; no filename collisions were found against RustDesk's known layout (`src/`, `flutter/`, `libs/`, `Cargo.toml`, `.github/`, etc.).
7. Instantiate `CHANGELOG_IMPLEMENTATION.md` from `CHANGELOG_TEMPLATE.md` (done this session — see repo root) and `REQUIREMENTS_TRACEABILITY.md` from its template once concrete requirements/tests exist to map (Phase 3 onward).
8. Adopt upstream's `.gitignore` as a base, appended with any local build-artifact paths (`target/`, `build/`, `.dart_tool/`, vcpkg install trees) needed for this environment.

## Licensing

RustDesk is licensed AGPL-3.0. The plan preserves `LICENSE` and all copyright headers unmodified. Because AGPL-3.0's network-use clause is directly relevant to a remote-access tool, `docs/licensing.md` (Phase 11 — Packaging) will document the fork's obligations explicitly rather than leaving it implicit.

## Why fork-in-place rather than submodule/subtree

RustDesk's own build tooling (Cargo workspace, Flutter project, CI scripts) expects to run from the repository root. A submodule or subtree would require path indirection that upstream's own scripts don't expect, adding risk for no benefit here. A direct, tag-pinned clone with our docs alongside it is simpler and matches how RustDesk itself is typically forked by third parties.

No source files have been created, modified, or deleted to implement this plan yet — it is a plan awaiting confirmation before the `git init`/clone (an admin-adjacent, disk- and network-consuming action) is executed.
