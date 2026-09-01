# Release Asset Inventory and Naming Strategy

**Date:** 2026-09-01  
**Status:** Direct-IP Release v1.0.0 (baseline RustDesk 1.4.9)

---

## Current Artifact Naming (flutter-build.yml)

### Windows
- **x64:** `rustdesk-direct-ip-1.4.9-x86_64.exe` (unsigned portable)
- **x86:** `rustdesk-direct-ip-1.4.9-x86.exe` (unsigned portable)
- **arm64:** `rustdesk-direct-ip-1.4.9-arm64.exe` (unsigned portable)

### macOS
- **x64:** `rustdesk-direct-ip-1.4.9-x86_64.dmg` (unsigned)
- **arm64:** `rustdesk-direct-ip-1.4.9-arm64.dmg` (unsigned)

### Linux (deb packages)
- **Generic x86_64:** `rustdesk-direct-ip-1.4.9-x86_64.deb`
- **Generic aarch64:** `rustdesk-direct-ip-1.4.9-aarch64.deb`
- **Generic armv7:** `rustdesk-direct-ip-1.4.9-armv7.deb`
- **Sciter x86_64:** `rustdesk-direct-ip-1.4.9-x86_64-sciter.deb`
- **Sciter armv7:** `rustdesk-direct-ip-1.4.9-armv7-sciter.deb`

### Linux (AppImage)
- **x86_64:** `rustdesk-direct-ip-1.4.9-x86_64.AppImage`
- **aarch64:** `rustdesk-direct-ip-1.4.9-aarch64.AppImage`

### Linux (Flatpak)
- **x86_64:** `rustdesk-direct-ip-1.4.9-x86_64.flatpak`

### Android
- **arm64-v8a:** `rustdesk-direct-ip-1.4.9-arm64-v8a.apk` (unsigned)
- **armeabi-v7a:** `rustdesk-direct-ip-1.4.9-armeabi-v7a.apk` (unsigned)
- **x86_64:** `rustdesk-direct-ip-1.4.9-x86_64.apk` (unsigned)
- **x86:** `rustdesk-direct-ip-1.4.9-x86.apk` (unsigned)

---

## Naming Analysis

### Current Pattern
**`rustdesk-direct-ip-{version}-{platform}-{architecture}.{ext}`**

Where:
- `rustdesk-direct-ip` = Direct-IP brand prefix (consistent across ALL artifacts)
- `{version}` = RustDesk baseline version (1.4.9)
- `{platform}` = Optional: `-sciter` for legacy Linux builds
- `{architecture}` = x86_64, x86, arm64, aarch64, armv7, etc.
- `{ext}` = Platform-specific extension (.exe, .dmg, .deb, .apk, .AppImage, .flatpak)

### Assessment

| Aspect | Status | Finding |
|--------|--------|---------|
| **Consistency** | ✅ All have `rustdesk-direct-ip` prefix | Direct-IP branding is uniform |
| **User-visible** | ✅ Yes, in download filenames | Users understand this is Direct-IP |
| **Release-visible** | ✅ Yes, GitHub Release lists all | Release name includes `direct-ip` in tag |
| **Clarity** | ✅ Good | Platform and architecture clear from filename |
| **Ambiguity** | ⚠️ Minor | Sciter suffix only on deb, not deb alternatives |

---

## Recommendation: Adopt Current Strategy (Option A)

**Keep:** `rustdesk-direct-ip-{version}-{platform}-{architecture}.{ext}`

### Rationale

1. **Already Implemented:** All artifacts use this pattern except one historical edge case (multipass patch format issue, now fixed).
2. **User Clarity:** The `direct-ip` prefix is immediately recognizable; users know what they're downloading.
3. **Release Branding:** Aligns with release tag `v1.4.9-direct-ip.1.0.0` and release title "RustDesk Direct-IP v1.0.0."
4. **Consistency:** Every platform follows the same rule; no special cases.
5. **No Ambiguity:** Unlike "Option B" (plain `rustdesk-...` in a Direct-IP release), this makes Direct-IP explicit in every filename.

### Implementation Status

✅ **Windows, macOS, Android, deb:** Already implemented in flutter-build.yml  
✅ **AppImage:** Fixed in commit 44037eaf6 (add rename step)  
✅ **Flatpak:** Already implemented  

---

## Release Guidance

### For Release Notes

**Direct-IP v1.0.0 based on RustDesk 1.4.9**

All downloadable artifacts use the naming pattern:

```
rustdesk-direct-ip-1.4.9-{platform}-{arch}.{ext}
```

Examples:
- Windows: `rustdesk-direct-ip-1.4.9-x86_64.exe`
- macOS: `rustdesk-direct-ip-1.4.9-arm64.dmg`
- Linux: `rustdesk-direct-ip-1.4.9-x86_64.AppImage` (recommended)
- Linux (alt): `rustdesk-direct-ip-1.4.9-x86_64.deb`
- Android: `rustdesk-direct-ip-1.4.9-arm64-v8a.apk`

The `direct-ip` prefix indicates this is the Direct-IP fork; all support/authentication is Direct-IP-only.

### For GitHub Release Assets

Attach all artifacts with their full names. No renaming on publish.

---

## Edge Cases Resolved

| Issue | Resolution | Status |
|-------|-----------|--------|
| AppImage filename mismatch (appimage-builder generated `rustdesk-1.4.9-*.AppImage`, uploaded as `rustdesk-direct-ip-...`) | Added rename step in flutter-build.yml build-appimage job | ✅ Implemented in 44037eaf6 |
| Multipass patch format error preventing build | Fixed patch format; both aom patches now properly formatted | ✅ Fixed in c8edcaa29 |

---

## No Further Changes Required

All naming is consistent, implemented, and validated.  
Release artifact strategy is finalized.  

Next: Proceed to Workstream 3 (Release Validation).
