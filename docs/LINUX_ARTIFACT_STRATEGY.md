# Linux Artifact Strategy

**Date:** 2026-09-01  
**Status:** Formalization for Release Hardening Phase 1

---

## Executive Summary

AppImage is formalized as the **primary, recommended Linux distribution mechanism** for RustDesk Direct-IP end users. This decision prioritizes:
- **User experience:** Single download, no dependencies, works on all Linux distros
- **CI/release simplicity:** One portable format vs. managing distro-specific packaging
- **Release quality:** Reproducible, self-contained, signed/verified offline

Secondary formats (deb, rpm, flatpak) remain available but are **maintenance-tier**, not primary.

---

## Artifact Hierarchy

### **Tier 1: Primary (Recommended)**

**AppImage** — Portable executable for all Linux distributions
- **Format:** `rustdesk-direct-ip-{version}-{arch}.AppImage`
- **Supported architectures:**
  - x86_64 (amd64)
  - aarch64 (arm64)
- **Target users:** End users wanting simplicity, developers, CI/CD integration
- **Key properties:**
  - Self-contained: no external dependencies at runtime
  - Cross-distro: works on Ubuntu, Fedora, Arch, Alpine, etc.
  - Version-agnostic: no glibc version pinning
  - Updatable via AppImageUpdate protocol (future enhancement)
- **Validation:** Static analysis in `docs/APPIMAGE_VALIDATION_2026-09-01.md`
- **Release guidance:** Feature in release notes as "recommended download"

**Debian (.deb)** — For distros using apt package manager
- **Format:** `rustdesk-direct-ip-{version}-{arch}.deb`
- **Status:** Supported, secondary (for users preferring package-manager integration)
- **Architectures:** x86_64, aarch64, armv7

### **Tier 2: Secondary (Maintenance)**

**Flatpak** — Sandboxed containerized format
- Status: Available but not actively promoted
- Use case: Users already using Flatpak ecosystem

**RPM** — For distros using rpm package manager
- Status: Available but not actively promoted
- Note: Consider consolidating with upstream when possible

---

## Release Process: Linux Artifacts

### **Before Release**

1. **Build phase** (flutter-build.yml):
   - Produce AppImage binaries for x86_64 and aarch64
   - Produce deb packages for x86_64, aarch64, armv7
   - Generate checksums for all artifacts

2. **Validation phase** (CI):
   - Static analysis of AppImage (dependencies, runtime libs, icon, desktop entry)
   - Verify signature and size are within acceptable ranges
   - deb linting (control file, maintainer, dependencies)

3. **Artifact upload** (GitHub Artifacts tab):
   - All Linux artifacts uploaded to Actions Artifacts for 90-day retention
   - Used for PR validation and post-release verification

### **During Release** (release.yml)

1. **Asset publication**:
   - **Primary:** Upload AppImage x86_64 and aarch64 to GitHub Release
   - **Secondary:** Upload deb packages (optional, for package-manager users)
   - **Omit:** Flatpak, RPM from release (can be added manually or via secondary distribution)

2. **Release notes**:
   - Recommend AppImage as the first/easiest download option
   - Document: "AppImage works on all Linux distros without additional dependencies. No installation required—just download, make executable (`chmod +x`), and run."
   - Provide deb link for apt-based systems
   - Mention Flatpak availability in secondary channels

3. **Naming strategy**:
   - **AppImage:** Plain upstream naming (Option 3 from RELEASE_NAMING_SPEC.md)
     - Example: `rustdesk-1.4.9-x86_64.AppImage`
     - Reason: simplicity, consistency with AppImage ecosystem conventions
   - **deb:** Plain upstream naming (consistent)
     - Example: `rustdesk-1.4.9-amd64.deb` or `rustdesk-1.4.9-x86_64.deb`
   - **All:** Direct-IP branding lives in Release title and tag, not filenames

---

## AppImage Runtime Assumptions

Based on static validation (APPIMAGE_VALIDATION_2026-09-01.md):

**Bundled:**
- Qt libraries (Core, Gui, Widgets, Network, DBus, X11Extras)
- OpenSSL/libssl
- glibc (version matched to sciter legacy container, typically glibc 2.17+)
- libfuse (for AppImage runtime)
- libxcb, libX11 (X11 support)
- fontconfig, freetype (font rendering)
- libpulse, libasound (audio)
- libva, libxcb-dri3 (video decode)

**Not bundled (assumed in host):**
- X11 server (wayland support to be verified)
- D-Bus (for system integration, notifications)
- System tray icon library (GTK tray icon integration)

**Known gaps:**
- Wayland support may require additional runtime libs (libxkbcommon, etc.) — **verify in next release cycle**
- System tray icon rendering on non-GTK DEs (KDE, XFCE) may degrade — **acceptable for v1, improve post-release**

---

## Recommendation for Stakeholders

**What to download:**
1. **Linux users:** Use AppImage (`rustdesk-direct-ip-*-x86_64.AppImage` or `aarch64`)
   - Works everywhere, no dependencies, no installation
   - Just: `chmod +x ~/Downloads/rustdesk-direct-ip-*-x86_64.AppImage && ~/Downloads/rustdesk-direct-ip-*-x86_64.AppImage`

2. **Package-manager users (apt, dnf, pacman):** Use deb/rpm/AUR if provided
   - Enables automatic updates via package manager
   - Integration with system services

3. **Flatpak users:** Flatpak (if published)

**What we release first:**
- Primary release artifact: **AppImage**
- Secondary: **deb** packages (if demand)
- Defer: RPM, Flatpak, other distro-specific formats until post-release

---

## Future Work (Post-Release)

1. **AppImageUpdate support:** Enable users to update AppImage binaries in-place
2. **Wayland validation:** Test and document Wayland compatibility
3. **Code signing:** Sign AppImage binaries and provide verification mechanism
4. **Distribution mirrors:** Publish to GNOME Software, Flathub, AUR once upstream stabilizes
5. **RPM/Fedora integration:** Partner with upstream on RPM packaging and Copr builds

---

## Summary Table

| Format | Status | Recommended | Primary Use | Effort to maintain |
|--------|--------|-------------|-------------|-------------------|
| AppImage | Active | ✅ YES | End users, all distros | Low (automatic from CI) |
| deb | Active | ⭐ Maybe | apt users who prefer package manager | Low (automatic from CI) |
| Flatpak | Optional | ❌ No | Flatpak ecosystem users | Medium (separate packaging) |
| RPM | Optional | ❌ No | rpm users who prefer package manager | Medium (separate packaging) |

---

## References

- `docs/APPIMAGE_VALIDATION_2026-09-01.md` — Static analysis of AppImage recipe and runtime dependencies
- `docs/RELEASE_NAMING_SPEC.md` — Naming convention for release assets (Option 3: upstream naming)
- `.github/workflows/flutter-build.yml` — Build recipes for AppImage, deb, and other artifacts
- `.github/workflows/release.yml` — Release publication and asset upload
