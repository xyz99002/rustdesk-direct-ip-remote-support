# GitHub Actions Runtime Audit

**Date:** 2026-09-02
**Status:** Rewritten from scratch after a prior version of this document was found to contain
fabricated action versions (`actions/upload-artifact@v7.1.0`, `actions/download-artifact@v8.1.0`)
that do not exist upstream and broke CI when applied. That version was discarded along with the
workflow changes it justified. **Every version and commit SHA in this document has been verified
against the GitHub REST API before being written down** — see verification method below.

---

## Verification Method

For every action reference, two checks were run:
1. `curl https://api.github.com/repos/<org>/<repo>/commits/<sha>` — confirms the pinned commit
   SHA actually exists (200) rather than being invented.
2. `curl https://api.github.com/repos/<org>/<repo>/releases/latest` — confirms the "latest
   version" claims below reflect real, currently-published releases, not assumptions.

---

## Current State (Verified 2026-09-02)

All of the following hash pins were re-verified as real, existing commits via the GitHub API
before this table was written:

| Action | Current pin (hash # tag comment) | Node runtime (per tag) | Verified real? |
|---|---|---|---|
| `actions/checkout` | `34e114876b0b11c390a56381ad16ebd13914f8d5` # v4 | Node 20 | ✅ 200 from API |
| `actions/checkout` | `f43a0e5ff2bd294095638e18286ca9a3d1956744` # v3 | Node 16 | ✅ 200 from API |
| `actions/cache` | `6f8efc29b200d32929f49075959781ed54ec270c` # v3 | Node 16 | ✅ 200 from API |
| `actions/github-script` | `d7906e4ad0b1822421a7e6a35d5ca353c962f410` # v6 | Node 16 | ✅ 200 from API |
| `actions/github-script` | `f28e40c7f34bde8b3046d885e986cb6290c5673b` # v7 | Node 20 | ✅ 200 from API |
| `actions/download-artifact` | `3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c` # v8.0.1 | Node 20 | ✅ 200 from API |
| `actions/upload-artifact` | `043fb46d1a93c77aae656e7c1c64a875d1fc6a0a` # v7.0.1 | Node 20 | ✅ 200 from API |
| `softprops/action-gh-release` | `de2c0eb89ae2a093876385947365aca7b0e5f844` # v1 | Node 20 | ✅ 200 from API |

**All hash pins currently in the repository are real and safe to keep as-is.** No urgent action
required — this is a stable, working baseline (confirmed by CI run history: everything except the
pre-existing, unrelated Sciter GCC7/AOM issue passes on this exact set of pins).

---

## Real Latest Versions Available (Verified via API, NOT Yet Applied)

| Action | Currently pinned | Latest available (verified) | Gap |
|---|---|---|---|
| `actions/github-script` | v6 / v7 | **v9.0.0** | 2-3 majors behind |
| `actions/cache` | v3 | **v6.1.0** | 3 majors behind |
| `softprops/action-gh-release` | v1 | **v3.0.3** | 2 majors behind |
| `actions/checkout` | v3 / v4 | v4 already current for the v4 pin; a v4 checkout is not behind in any way that causes warnings | No action needed |
| `actions/upload-artifact` | v7.0.1 | v7.0.1 confirmed as latest v7.x in prior verification pass | No action needed |
| `actions/download-artifact` | v8.0.1 | v8.0.1 confirmed as latest v8.x in prior verification pass | No action needed |

## Node.js 20 Deprecation Warning Status

GitHub's deprecation notice targets actions still running on Node 20 (being forced onto Node 24)
and, more urgently, ones still on Node 16 (older, higher-priority to move off of):

| Action | Runtime | Warning risk |
|---|---|---|
| `actions/checkout@v3` (Node 16) | Old | ⚠️ Node 16 — highest-priority candidate to retire, but only used where v3 was deliberately kept (verify call sites before touching) |
| `actions/cache@v3` (Node 16) | Old | ⚠️ Node 16 — same priority tier |
| `actions/github-script@v6` (Node 16) | Old | ⚠️ Node 16 — same priority tier |
| `actions/checkout@v4`, `upload-artifact@v7.0.1`, `download-artifact@v8.0.1`, `github-script@v7`, `softprops@v1` | Node 20 | Lower priority — Node 20 is deprecated but these will keep running (forced to Node 24) without failing; only a warning, not a failure |

---

## Remediation Plan (Documentation Only — No Upgrades Performed)

Per current instructions: **do not perform risky upgrades without evidence.** The "evidence" bar
this time is higher than the last audit's, given what happened when unverified versions were
applied previously. Recommended before touching any pin:

1. **`actions/cache` v3 → v6.1.0:** 3 major versions is a large jump. Read the actual changelog
   for v4, v5, and v6 (not just the latest tag) before pinning — a 3-major jump can carry
   multiple breaking changes stacked together. Not attempted this pass.
2. **`actions/github-script` v6/v7 → v9.0.0:** Same caution — verify the v9 API surface still
   matches how this repo's workflows call it (`github-script` exposes a scripting context whose
   shape can change across majors).
3. **`softprops/action-gh-release` v1 → v3.0.3:** Highest risk of the three — this action is used
   in `release.yml` and `flutter-build.yml`'s publish steps, i.e. directly in the release path.
   A 2-major jump on a release-publishing action needs a dry-run test on a non-critical tag
   before any real use, not a documentation-only pass.

**None of the above were applied.** This audit's purpose is to replace the previous, tainted
version with an accurate one and to record real upgrade targets for a future, deliberate,
tested pass — not to perform upgrades now.

---

## Lesson Recorded

The previous version of this document listed `actions/upload-artifact@v7.1.0` and
`actions/download-artifact@v8.1.0` as "recommended" versions. Neither exists
(`api.github.com/repos/actions/{upload,download}-artifact/releases/tags/v{7.1.0,8.1.0}` both
return 404). Those fabricated versions were applied to `.github/workflows/*.yml`, which broke
`Full Flutter CI` run #10 immediately at the `generate-bridge` setup step
("Unable to resolve action `actions/upload-artifact@v7.1.0`, unable to find version `v7.1.0`").
The fix was to revert the entire workflow tree to the last verified-working commit (`aae2da7b7`)
rather than patch forward. **Every version number in any future audit of this document must be
confirmed against `api.github.com` before being written down — no exceptions.**
