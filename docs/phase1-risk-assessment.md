# Phase 1 — Risk Assessment

| # | Risk | Impact | Mitigation |
|---|---|---|---|
| 1 | Rust/Flutter/CMake/vcpkg/NASM/clang/Python toolchain entirely missing | High — blocks any build/test from Phase 4 onward | Install before starting Phase 4. Installing dev toolchains is a system-modifying action, so per the master spec ("ask before admin actions") this requires explicit user go-ahead rather than being installed silently. Candidate commands (winget-based, no elevation expected for per-user scope) are ready once approved. |
| 2 | Non-elevated shell | Medium | Prefer per-user installs (`rustup`, Flutter as a zip/git checkout, portable CMake) over machine-wide winget installs that may prompt UAC. |
| 3 | Architecture proposal is based on general knowledge, not verified source | Medium — could misdirect later phases if wrong | Phase 2 upstream analysis explicitly re-verified every assumption in the original proposal (see `docs/upstream-analysis.md`) before any config/auth/transport code was written; superseded by the final `docs/architecture.md`. |
| 4 | AGPL-3.0 network-use obligations | Medium (legal/compliance, not technical) | Preserve `LICENSE` and headers unmodified; document obligations explicitly in Phase 11 packaging docs; never relicense. |
| 5 | ~~Removing rendezvous also removes whatever abuse-prevention it provided~~ **Superseded 2026-08-28** | — | Requirements were frozen to preserve upstream authentication/transport unchanged (see `docs/architecture.md`) — rendezvous/relay are not being removed, and no brute-force hardening is in scope. This row is kept for history only. |
| 6 | Target platform(s) unstated in the master spec | Low/Medium (scope risk) | Open question to user below — affects which toolchains/vcpkg triplets to install and which of RustDesk's platform-specific code paths are in scope. |
| 7 | Large, long build (RustDesk + vcpkg deps historically 30–60+ min clean build on Windows) | Low | Hardware is well above requirements (16 logical CPUs, ~63 GB RAM, 1.5 TB free disk) — not expected to be a real bottleneck, just a time expectation to set. |
| 8 | No test infrastructure exists yet, so "stop on failed tests" can't be honored literally in Phase 1/2 | Low | Expected — this requirement naturally activates once tests exist (Phase 4 onward); not a current blocker, just noted so it isn't mistaken for non-compliance later. |

## Open questions — resolved 2026-08-28

1. **Target platform(s):** Windows + Linux + macOS (full desktop cross-platform, matching upstream RustDesk's usual scope). Note: this dev machine is Windows-only, so Linux/macOS can be kept buildable/portable in source but cannot be built or tested locally in this environment — flagged as a standing limitation, not a blocker for Windows-side work.
2. **Toolchain installation:** approved — install Rust/Flutter/CMake/vcpkg/NASM/clang/Python now, per-user scope where possible, via winget/rustup/official installers.
3. **Repository setup:** approved — proceed with `git init` + clone of upstream RustDesk per `docs/repository-plan.md`.
4. **Baseline version:** use the newest stable upstream release tag; Claude Code determines and reports it during Phase 2.

No source files were created, modified, or deleted during Phase 1. This document, along with the environment discovery, architecture proposal, and repository plan, is the complete gate required before any source-affecting action begins. Phase 1 is now closed; Phase 2 (repository setup + upstream analysis) begins next.

**2026-08-28 update:** requirements were subsequently frozen, reversing the authentication/transport direction this risk assessment was written against. See `docs/architecture.md` (final) and `docs/DECISIONS.md` for the current authoritative decisions; this document is preserved as a historical record of the Phase 1 gate.
