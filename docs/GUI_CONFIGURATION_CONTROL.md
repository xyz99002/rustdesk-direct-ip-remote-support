# GUI Configuration Control — `show_setup_ui` Design

**Status: ✅ Implemented (2026-09-02).** Originally design-only per Workstream 4; implemented as
part of the `fork_config.toml` → `RustDesk2.toml` consolidation. Key name is
`direct-ip-show-setup-ui` (not `show_setup_ui` — see the naming-collision-avoidance rationale in
`docs/UPSTREAM_CONFIG_REFERENCE.md` Section 3), read via `src/fork_config.rs`, written to the
upstream-facing `show-setup-ui` option, and consumed by
`DesktopSettingPage.switch2page()` exactly as designed below (Section "Smallest Hook Point").
**Mobile is still not gated** — that gap, flagged in the original design, remains open.

---

## Goal

Add an option, `direct-ip-show-setup-ui = "Y"|"N"`, that lets a deployment hide the Settings
entry point entirely (for a locked-down "remote" host that should never be touched locally by
whoever is physically at that machine), while defaulting to shown (`"Y"`) so existing deployments
are unaffected.

---

## Smallest Hook Point

**Primary entry point:** `flutter/lib/desktop/pages/desktop_home_page.dart:278-296` — the gear
icon in the top toolbar:

```dart
InkWell(
  child: Obx(() => Icon(Icons.settings, ...)),
  onTap: () => {
    if (DesktopSettingPage.tabKeys.isNotEmpty)
      { DesktopSettingPage.switch2page(DesktopSettingPage.tabKeys[0]) }
  },
  onHover: (value) => _editHover.value = value,
),
```

**Secondary entry point:** same file, `desktop_home_page.dart:403-411` — a second gear-style
affordance that jumps directly to `SettingsTabKey.safety`.

Both entry points ultimately call `DesktopSettingPage.switch2page(...)`. **The single smallest
hook is inside `switch2page()` itself** (`desktop_setting_page.dart:92-112`):

```dart
static void switch2page(SettingsTabKey page) {
  try {
    int index = tabKeys.indexOf(page);
    if (index == -1) {
      return;
    }
    // ... opens the settings window/tab ...
```

Adding a single guard clause at the top of `switch2page()` — `if (!showSetupUiEnabled) return;`
— closes both entry points (and any future third one that might call `switch2page` directly)
with one change, rather than hunting down every `Icons.settings` tap handler individually.

**This is the recommended hook point:** one function, already the common chokepoint for every
"open settings" action in the desktop UI.

---

## Plumbing (Mirrors the Existing `desktop-share-enabled` Pattern)

1. **Schema:** add `show_setup_ui: Option<bool>` to `RawForkConfig`
   (`src/fork_config.rs:118-130`). Following the existing pattern, it would be **required** like
   every other field today — but see "Default Value Question" below for why this one may warrant
   an exception.

2. **Validation:** no parsing needed beyond the existing bool handling; no new `ConfigError`
   variant required.

3. **Apply:** in `apply()` (`src/fork_config.rs:338-416`), add:
   ```rust
   Config::set_option(
       "show-setup-ui".to_owned(),
       if config.show_setup_ui { "Y" } else { "N" }.to_owned(),
   );
   ```
   This is a **new fork-specific option key** (`show-setup-ui`), same pattern as
   `desktop-share-enabled` — not an upstream key, since no equivalent upstream concept exists.

4. **Dart side:** in `desktop_setting_page.dart`'s `switch2page()`:
   ```dart
   static void switch2page(SettingsTabKey page) {
     if (!mainGetBoolOptionSync("show-setup-ui")) return;
     // ... existing body unchanged ...
   }
   ```

---

## Default Value Question — RESOLVED: Option B Chosen

Every other `fork_config.toml` field today is **required** with no implicit default — a missing
field is a hard validation error. Two choices for `show_setup_ui`:

| Choice | Behavior when field is absent from an existing `fork_config.toml` | Consistency |
|---|---|---|
| **A. Required, like all other fields** | Every existing deployment's config file becomes invalid until updated — the whole file falls back to pure upstream behavior (per `load_and_apply()`'s existing fail-safe), which **also re-enables Account/Network tabs and LAN discovery** — a much bigger regression than just Settings visibility | Consistent with current schema philosophy, but has an outsized blast radius for a small new field |
| **B. `#[serde(default = "default_true")]`, defaulting to `true`** | Existing deployments keep working unchanged; only deployments that explicitly opt into `show_setup_ui = false` get the new behavior | Breaks the "no implicit defaults" rule that's been deliberate since `fork_config.rs`'s original design (module doc: "every field is optional at the parse layer so that a missing/invalid field is reported explicitly during validation") |

**Recommendation:** Option B, as a deliberate, documented exception — the existing "no defaults"
policy exists to catch *typos and incomplete configs for security-relevant fields*
(role, authentication mode, permissions). `show_setup_ui` is a UI-convenience field with a safe
default (`true` = current behavior, nothing hidden), which is exactly the kind of field a default
is appropriate for. This should be called out explicitly in `fork_config.example.toml`'s
comments and `docs/CONFIG_REFERENCE.md` if implemented, so the exception is documented, not
silent.

---

## Pages Affected

| Page/File | Impact |
|---|---|
| `desktop_setting_page.dart` | ✅ Done — `switch2page()` has the guard clause |
| `desktop_home_page.dart` | ✅ No change needed — both gear icons naturally become no-ops via `switch2page()` |
| Mobile settings (`flutter/lib/mobile/pages/settings_page.dart`) | ❌ **Still not covered.** Mobile has its own settings entry point, not routed through `DesktopSettingPage.switch2page()`. Remains an open gap. |
| `fork_config.rs` | ✅ Done — `direct-ip-show-setup-ui` key + `Config::set_option("show-setup-ui", ...)` in `apply()` |
| `configs/all-options-reference.toml` | ✅ Done — documented field |

---

## Impact on Upgrades

- **Existing deployments without the key:** No impact — default `true` (`"Y"`) preserves current
  behavior exactly.
- **Config version:** No breaking change to `SUPPORTED_CONFIG_VERSION` (still `1`).
- **No migration needed.**

---

## Out of Scope / Not Designed Here

- Mobile UI equivalent (flagged above, needs separate design).
- Any UI affordance to *change* `show_setup_ui` from within the app itself (this is explicitly a
  deployment-time, file-based control, consistent with how `role` and `authentication.mode` work
  today — not meant to be toggled by whoever is sitting at the machine).

**Implemented on desktop (2026-09-02). Mobile remains open** — a separate design/implementation
pass is needed for `flutter/lib/mobile/pages/settings_page.dart`'s own settings entry point.
