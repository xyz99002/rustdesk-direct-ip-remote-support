# Linux Sciter Build Fix — GCC 7 / aom AVX2 Compatibility (2026-09-01)

## Summary

`build-rustdesk-linux-sciter` has been failing in CI for several runs (Full Flutter CI #6 and #8; observed again during Create Direct-IP Release #1) on the `x86_64-unknown-linux-gnu` leg only. The `armv7` leg of the same job passes, because it never compiles the x86-specific AVX2 intrinsics file discussed below.

This document records the fix that was implemented: a new vcpkg overlay patch, `res/vcpkg/aom/aom-gcc7-avx2-compat.diff`, registered in `res/vcpkg/aom/portfile.cmake`.

Full root-cause diagnosis lives in `docs/CI_WORKFLOW_AUDIT_2026-09-01.md`, Section 6, "Failure 2". This doc summarizes that root cause, gives the exact patch content, explains why this fix was chosen over the three other options considered, and states what is still pending.

## Root cause (recap)

- The sciter Linux build container pins the host compiler to GCC 7.5.0, specifically for binary/glibc compatibility with older target distros.
- vcpkg's pinned `aom` port (version 3.12.1, commit `10aece4157eb79315da205f39e19bf6ab3ee30d0`, fetched from `https://aomedia.googlesource.com/aom`) builds `aom_dsp/flow_estimation/x86/disflow_avx2.c`, which uses the AVX2 intrinsic `_mm256_set_m128i` inside `compute_flow_vector()`.
- GCC's `<immintrin.h>` did not declare `_mm256_set_m128i` until GCC 8. Under GCC 7.5.0, the compiler implicitly declares the missing function as returning `int` (an old-style implicit-declaration fallback), so every `__m256i` variable initialized from its result becomes a type mismatch.
- This produces four "error: incompatible types when initializing type '__m256i' ... using type 'int'" errors, at the four call sites assigning to `px0`, `px1`, `px2`, `px3` in `compute_flow_vector()` (originally reported around lines 216-219 of the file, in the vertical-convolution loop that packs two rows at a time via `_mm256_set_m128i(rowN, rowN-1)`).

## Fix implemented

New file `res/vcpkg/aom/aom-gcc7-avx2-compat.diff`, a unified diff in the same style as the three pre-existing overlay patches in that directory (`aom-uninitialized-pointer.diff`, `aom-install.diff`, `aom-disable-multipass-check.diff`). It inserts a small compatibility shim into `aom_dsp/flow_estimation/x86/disflow_avx2.c`, immediately after the file's existing `#include` block (verified against the real upstream source at the pinned commit) and before the `DISFLOW_PATCH_SIZE` sanity check:

```c
// GCC < 8 compatibility shim.
// GCC's <immintrin.h> did not declare _mm256_set_m128i until GCC 8, so under
// the GCC 7.5.0 host compiler pinned for glibc/binary compatibility with
// older distros, the compiler implicitly declares this as returning int,
// causing "incompatible types when initializing type '__m256i' ... using
// type 'int'" errors at every call site in compute_flow_vector() below.
#if defined(__GNUC__) && !defined(__clang__) && __GNUC__ < 8
static inline __m256i _mm256_set_m128i(__m128i hi, __m128i lo) {
  return _mm256_insertf128_si256(_mm256_castsi128_si256(lo), hi, 1);
}
#endif
```

The patch was registered in `res/vcpkg/aom/portfile.cmake`'s `PATCHES` list for the `aom` port, appended after `aom-disable-multipass-check.diff`:

```
PATCHES
    aom-uninitialized-pointer.diff
    # aom-avx2.diff
    aom-install.diff
    aom-disable-multipass-check.diff
    aom-gcc7-avx2-compat.diff
```

The include-block context the patch anchors on (`#include "aom_dsp/x86/synonyms_avx2.h"` followed by `#include "config/aom_dsp_rtcd.h"`) was fetched and verified directly against the real upstream file at the pinned commit (`https://aomedia.googlesource.com/aom/+/10aece4157eb79315da205f39e19bf6ab3ee30d0/aom_dsp/flow_estimation/x86/disflow_avx2.c`), rather than reconstructed purely from the compiler error text, so the diff hunk should apply cleanly.

## Why this approach over the alternatives

Four options were documented in the original audit (`docs/CI_WORKFLOW_AUDIT_2026-09-01.md`, Section 6):

1. **Chosen — compatibility shim via vcpkg overlay patch.** Lowest risk: it only affects compilers that lack the intrinsic (`__GNUC__ < 8`), is a no-op on any compiler that already has `_mm256_set_m128i` (including the armv7 leg, which never compiles this file at all), and follows an established pattern already used in this exact port directory (`aom-disable-multipass-check.diff` is a prior instance of patching aom's build for toolchain-compatibility reasons). No behavioral or codec-correctness change — the shim is bit-identical in effect to the real intrinsic (`_mm256_insertf128_si256(_mm256_castsi128_si256(lo), hi, 1)` is precisely how the real AVX2 intrinsic is implemented on compilers that do have it).
2. **Rejected — upgrade GCC to 8+ in the sciter container.** Higher risk: the GCC 7.5.0 pin exists specifically for glibc/ABI compatibility with older target Linux distributions that the sciter build needs to support. Changing it could silently break runtime compatibility for end users on those older distros, and the original rationale for the pin was not being re-investigated as part of this narrowly-scoped fix.
3. **Rejected — pin an older `aom` release predating this AVX2 code path.** This would re-open the same class of version-pinning fragility already seen and worked around for the NASM multipass issue (see `aom-disable-multipass-check.diff` and its accompanying notes) — trading one hard-to-track pinned-version problem for another.
4. **Rejected — strip the AVX2-optimized file from the sciter Linux build.** Would work, but causes a real AV1 encode/decode performance regression specific to that one build leg, which the shim avoids entirely.

## Verification pending

No new GitHub Actions run was triggered as part of this change — CI capacity was already committed to an in-progress `release.yml` run and other work in this session, and this task was explicitly scoped to not trigger CI itself.

A future green run of `build-rustdesk-linux-sciter` (`x86_64-unknown-linux-gnu` leg) needs to show all of the following to confirm the fix actually works:

1. No compiler error at `aom_dsp/flow_estimation/x86/disflow_avx2.c` lines ~216-219 (the `_mm256_set_m128i` / `compute_flow_vector` type-mismatch errors described above must not reappear).
2. The `aom:x64-linux` vcpkg port build step completing successfully (previously the point of failure).
3. The "Build rustdesk sciter binary for x86_64" step succeeding, and doing so in a time comparable to the `armv7` leg (observed around 4-5 minutes for the failing run before it errored out; a successful run should complete in that same rough window, not open-endedly longer).

Until such a run is observed and confirmed green, this fix should be treated as implemented-but-unverified.
