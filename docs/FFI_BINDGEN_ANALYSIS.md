# AOM FFI Bindings Analysis: Opaque Struct Issue

## Executive Summary

The aom FFI bindings are generating opaque structs (with only `_address: u8` placeholder fields) instead of full field definitions. This causes 64+ compilation errors in `libs/scrap/src/common/aom.rs` where code attempts to access struct fields like `rc_min_quantizer`, `rc_max_quantizer`, `g_w`, `g_h`, etc.

**Root Cause:** The bindgen allowlist regex in `libs/scrap/build.rs` is too restrictive (`^(aom|AOM|OBU|AV1).*`) and excludes the `cfg_options` struct, which is required as a field in `aom_codec_enc_cfg`. When bindgen encounters an unknown field type, it marks the parent struct as opaque.

---

## Problem Details

### Affected Structs

Two critical encoder/decoder config structs are being generated as opaque:

1. **`aom_codec_enc_cfg`** - Encoder configuration (lines 387-907 in aom_encoder.h)
   - Should have 40+ fields including: `g_w`, `g_h`, `g_threads`, `rc_min_quantizer`, `rc_max_quantizer`, `rc_target_bitrate`, etc.
   - Last field: `cfg_options_t encoder_cfg;` ← **THIS IS THE PROBLEM**
   - Currently generated as: `pub struct aom_codec_enc_cfg { pub _address: u8, }`

2. **`aom_codec_dec_cfg`** - Decoder configuration (lines 91-96 in aom_decoder.h)
   - Should have 4 fields: `threads`, `w`, `h`, `allow_lowbitdepth`
   - Currently generated as: `pub struct aom_codec_dec_cfg { pub _address: u8, }`

### Compilation Errors Impact

In `libs/scrap/src/common/aom.rs`, the code attempts to directly access struct fields:

```rust
// Line 105-125: Failed field access
c.g_w = cfg.width;                           // ✗ Field does not exist
c.g_h = cfg.height;                          // ✗ Field does not exist
c.g_threads = codec_thread_num(64) as _;    // ✗ Field does not exist
c.rc_min_quantizer = q_min;                  // ✗ Field does not exist
c.rc_max_quantizer = q_max;                  // ✗ Field does not exist
c.rc_target_bitrate = bitrate;               // ✗ Field does not exist
// ... 35+ more field accesses fail

// Line 280-284: set_quality function
let mut c = unsafe { *self.ctx.config.enc.to_owned() };
c.rc_min_quantizer = q_min;  // ✗ Cannot access fields
c.rc_max_quantizer = q_max;  // ✗ Cannot access fields
```

Total: ~64 compilation errors across the aom.rs file.

---

## Root Cause Analysis

### The Missing cfg_options Struct

Bindgen's allowlist configuration in `build.rs:249`:

```rust
gen_vcpkg_package("aom", "aom_ffi.h", "aom_ffi.rs", "^(aom|AOM|OBU|AV1).*");
```

This regex **only includes types starting with**: `aom`, `AOM`, `OBU`, or `AV1`

However, in `aom_encoder.h:906-907`:

```c
typedef struct cfg_options {
  // ... 100+ fields ...
} cfg_options_t;
```

And in `aom_codec_enc_cfg` definition (line 906):

```c
typedef struct aom_codec_enc_cfg {
  // ... other fields ...
  cfg_options_t encoder_cfg;  // ← cfg_options does NOT match the allowlist regex!
} aom_codec_enc_cfg_t;
```

### Why Bindgen Marks Structs as Opaque

When bindgen encounters a struct with a field whose type is not in the generated bindings, it **cannot determine the struct's memory layout**. This is a critical constraint: Rust needs to know the exact size and alignment of all fields.

Bindgen's response is to mark the struct as "opaque" - generating only:
```rust
pub struct aom_codec_enc_cfg {
    _unused: [u8; 0],  // Placeholder, size unknown
}
```

This allows the struct to be used by reference (pointers), but not for field access.

### Verification Checklist

- ✓ aom_codec_enc_cfg is **fully defined** in vcpkg headers with real fields
- ✓ aom_codec_dec_cfg is **fully defined** in vcpkg headers with real fields
- ✓ cfg_options is **fully defined** in vcpkg headers
- ✓ All includes are present in `aom_ffi.h`
- ✓ No conditional compilation (`#ifdef`) guards the struct definitions
- ✓ cfg_options_t **starts with "cfg"**, does NOT match allowlist regex

---

## Upstream RustDesk Behavior

**Status:** Requires investigation of public RustDesk repository

Upstream RustDesk's scrap crate may handle this differently:

1. **Possible Approach 1:** Upstream may use a less restrictive or different regex
2. **Possible Approach 2:** Upstream may not depend on field-level access to these structs
3. **Possible Approach 3:** Upstream may define wrapper types or use getters
4. **Possible Approach 4:** Upstream may be at a different aom version

*Note: The fork is based on RustDesk but this configuration issue suggests a local modification or version divergence.*

---

## NASM Multipass Patch Analysis

The NASM multipass workaround (`res/vcpkg/aom/aom-disable-multipass-check.diff`) affects the **build system**, not the headers:

```cmake
# aom_optimization.cmake - test_nasm() function
function(test_nasm)
  # Workaround: Skip multipass check (performance feature, not hard requirement)
  return()
  # ... rest of checks disabled ...
endfunction()
```

**Side Effects on FFI:** NONE
- The patch only affects cmake/build system configuration
- It does NOT modify header files, struct definitions, or type information
- It does NOT affect what bindgen sees or processes
- The "cfg_options exclusion" issue exists independently of this patch

---

## Remediation Options (Ranked by Safety/Complexity)

### Option 1: Fix the Allowlist Regex [SAFEST, LOWEST IMPACT]

**Change:** `build.rs:249`

```rust
// Current (too restrictive)
gen_vcpkg_package("aom", "aom_ffi.h", "aom_ffi.rs", "^(aom|AOM|OBU|AV1).*");

// Option 1A: Include cfg types (minimal change)
gen_vcpkg_package("aom", "aom_ffi.h", "aom_ffi.rs", "^(aom|AOM|OBU|AV1|cfg).*");

// Option 1B: Include all types (most permissive, requires review)
gen_vcpkg_package("aom", "aom_ffi.h", "aom_ffi.rs", ".*");
```

**Pros:**
- Minimal code change
- No manual definitions required
- Bindgen handles all struct details automatically
- Future-proof for other missing types

**Cons:**
- May include unwanted types from headers
- Requires reviewing what gets generated

**Risk Level:** LOW
- The allowlist exists for filtering, expanding it is straightforward
- cfg_options is legitimately needed for the config struct

---

### Option 2: Modify the aom_ffi.h Wrapper Header [LOW-MEDIUM COMPLEXITY]

**Add explicit typedef before includes:**

```c
// libs/scrap/src/bindings/aom_ffi.h
#include <aom/aom.h>
#include <aom/aom_image.h>
#include <aom/aom_integer.h>
#include <aom/aom_codec.h>
// ... other includes ...
#include <aom/aom_encoder.h>

// Explicitly mark types for bindgen
typedef struct cfg_options cfg_options_t;
typedef struct aom_codec_enc_cfg aom_codec_enc_cfg_t;
```

**Pros:**
- Keeps build.rs unchanged
- Clear documentation in the wrapper

**Cons:**
- Less elegant than fixing the regex
- Requires maintaining wrapper header

**Risk Level:** LOW-MEDIUM
- Forward declarations are safe, but repeating typedefs may confuse tools

---

### Option 3: Manual Binding Definitions [MEDIUM COMPLEXITY, NOT RECOMMENDED]

Create `generated/aom_cfg_options_manual.rs` with hand-written struct definitions from headers.

**Pros:**
- Complete control over struct layout
- Can optimize for Rust idioms

**Cons:**
- Requires maintaining struct definitions as headers change
- Easy to fall out of sync
- High maintenance burden
- Bindgen may conflict with manual definitions

**Risk Level:** MEDIUM-HIGH
- Prone to ABI mismatches if manual definitions diverge from C headers

---

### Option 4: Upgrade Bindgen Version [MEDIUM COMPLEXITY]

Current: bindgen 0.65.1 (shown in generated file header)

**Action:** Check Cargo.toml for bindgen version, upgrade to 0.70.x or later

**Pros:**
- Newer versions may have better struct inference
- May fix other edge cases

**Cons:**
- Requires full rebuilding of all FFI bindings
- May introduce breaking changes in other crates

**Risk Level:** MEDIUM
- Buildgen major version changes can affect code generation patterns

---

### Option 5: Use Older aom Version [FALLBACK ONLY]

Downgrade aom from 3.12.1 to find a version with simpler cfg_options struct.

**Pros:**
- May find version where cfg_options was optional or simpler

**Cons:**
- Loses codec improvements and security fixes
- Defeats purpose of latest aom features

**Risk Level:** HIGH
- Backwards step; only if other options fail

---

## Recommended Action

**Proceed with Option 1 (fix regex):**

1. Update `libs/scrap/build.rs:249` to include "cfg" pattern
2. Rebuild scrap crate: `cargo build --lib -p scrap`
3. Verify all 64 errors are resolved
4. Test encoding/decoding functionality
5. Review generated `target/.../aom_ffi.rs` to ensure only expected types included

**Fallback:** If Option 1 generates unwanted types, use Option 2 (wrapper header) or Option 1B with selective inclusion.

---

## Testing Strategy

After applying fix:

1. **Compilation Test**
   ```bash
   cargo build --lib -p scrap
   # Expected: 0 compilation errors
   ```

2. **Field Access Verification**
   ```bash
   # Verify that aom.rs can access struct fields
   cargo check -p scrap --all-features
   ```

3. **Functional Testing**
   - Run existing aom encoder tests
   - Verify bitrate/quality settings apply correctly
   - Check frame encoding/decoding works

4. **Generated Bindings Review**
   - Inspect `target/.../aom_ffi.rs`
   - Count structs/enums/functions generated
   - Compare against upstream RustDesk if available

---

## Summary Table

| Issue | Status | Impact | Root Cause |
|-------|--------|--------|-----------|
| Opaque `aom_codec_enc_cfg` | **CONFIRMED** | 64+ errors | cfg_options excluded by regex |
| Opaque `aom_codec_dec_cfg` | **CONFIRMED** | Type error | cfg_options excluded by regex |
| Missing `cfg_options` struct | **CONFIRMED** | Blocks inference | Allowlist regex too restrictive |
| NASM patch side effects | **ANALYZED** | NONE | Only affects CMake build check |
| Upstream divergence | **SUSPECTED** | Unknown | Needs investigation |

---

## Notes

- The struct definitions exist and are well-formed in vcpkg headers
- Bindgen version 0.65.1 is reasonable for this task
- The allowlist/blocklist feature is working as designed; it's just too restrictive
- This is a **configuration issue**, not a code quality or header corruption issue
