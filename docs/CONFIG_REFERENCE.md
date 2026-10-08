# Configuration Reference — Direct-IP Fork

**Source-verified.** Every entry below is traced to an exact file and line in this repository.
No option is documented unless it exists in code.

**2026-09-02: consolidated.** This fork previously used a separate `fork_config.toml` file
alongside upstream's own `RustDesk2.toml`. That second file has been retired — every value below
now lives inside `RustDesk2.toml`'s own `[options]` table, under a set of `direct-ip-*`-prefixed
keys, read via the exact same `Config::get_option()`/`Config::set_option()` mechanism every other
RustDesk option already uses. See `docs/UPSTREAM_CONFIG_REFERENCE.md` for the full ~130-key
upstream catalog this now sits alongside, in the same file.

---

## 1. Config File Identity

**File:** `RustDesk2.toml` — upstream RustDesk's own config file (`hbb_common::config::Config2`).
There is no fork-specific config file anymore.

**Location** (OS-standard per-user config directory, unchanged by this fork):
- Windows: `%APPDATA%\RustDesk\config\RustDesk2.toml`
- Linux: `~/.config/RustDesk/RustDesk2.toml`
- macOS: `~/Library/Application Support/RustDesk/RustDesk2.toml`

## 2. Read Path & Load Order

Resolved by `read_raw_from_config2()` ([src/fork_config.rs](../src/fork_config.rs)): every
`direct-ip-*` key is looked up individually via `Config::get_option(key)`, which internally
applies upstream's own resolution order — `OVERWRITE_SETTINGS` (admin-pushed, highest priority)
→ `Config2.options` (this file) → `DEFAULT_SETTINGS` (compiled-in fallback) → empty string if
nowhere found. An empty-string result is treated as "key absent."

`Config2` itself is a `lazy_static`, loaded automatically on first access — no explicit
initialization ordering is needed. `load_and_apply()` is invoked from `src/core_main.rs`,
immediately after `crate::load_custom_client()` — before the inbound listener or any
outbound-connect capability is reachable, same as before the consolidation.

## 3. Fallback Behavior

| Condition | Behavior |
|---|---|
| `direct-ip-role` key absent (sentinel for "not configured at all") | `log::warn!`, then **pure upstream default behavior**: no role restriction, upstream's own default authentication mode. Not treated as an error. |
| Keys present but fail validation (bad enum value, bad IP, etc.) | `log::error!` with the specific `ConfigError`, falls back to upstream default behavior. |
| Keys present and valid | `apply()` is called, see Section 4. |

There is **no partial application** — either every required key validates and every mapping in
`apply()` runs, or none of it does.

---

## 4. Complete Option Enumeration

Every key below is read individually via `Config::get_option()`; none of them exist as struct
fields on `Config2` itself — they're just entries in its generic `options: HashMap<String, String>`
table, same as every upstream option. All are **required** except `direct-ip-show-setup-ui` — a
missing required key is `ConfigError::MissingField`.

### 4.1 `direct-ip-config-version`

| | |
|---|---|
| **Type** | string, parsed as `u32` |
| **Required** | Yes |
| **Valid values** | `"1"` (only) — `SUPPORTED_CONFIG_VERSION` |
| **Source** | [src/fork_config.rs](../src/fork_config.rs) `keys::CONFIG_VERSION`, checked in `validate()` |
| **Applied to** | Nothing — gates whether the rest is parsed at all |
| **Status** | ✅ **Working** |

### 4.2 `direct-ip-role`

| | |
|---|---|
| **Type** | string enum |
| **Required** | Yes |
| **Valid values** | `"local"`, `"remote"` |
| **Source** | [src/fork_config.rs](../src/fork_config.rs) `parse_role()` / `apply()` |
| **Applied to** | `HARD_SETTINGS["conn-type"]` = `"outgoing"` (local) / `"incoming"` (remote) — **in-memory only, never persisted back to `RustDesk2.toml`** |
| **Consumed by** | `hbb_common::config::is_incoming_only()` / `is_outgoing_only()` — gate `src/client.rs` outbound connects and `src/rendezvous_mediator.rs` inbound listener |
| **Status** | ✅ **Working** — unit-tested |

### 4.3 `direct-ip-auth-mode`

| | |
|---|---|
| **Type** | string enum |
| **Required** | Yes |
| **Valid values** | `"ask"`, `"password"`, `"ask_and_password"` |
| **Source** | [src/fork_config.rs](../src/fork_config.rs) `parse_auth_mode()` / `apply()` |
| **Applied to** | `Config::set_option("approve-mode", ...)` → `"click"` / `"password"` / `""` |
| **Consumed by** | `password_security::approve_mode()` in `libs/hbb_common/src/password_security.rs:77-86` |
| **Status** | ✅ **Working** — unit-tested |
| **Precedence note** | This **overwrites** `approve-mode` in the same `[options]` table on every startup. If you hand-edit `approve-mode` directly, it will not survive a restart while `direct-ip-auth-mode` is also present and valid. |

### 4.4 `direct-ip-support-enabled`

| | |
|---|---|
| **Type** | boolean (`"Y"`/`"N"`) |
| **Required** | Yes |
| **Source** | [src/fork_config.rs](../src/fork_config.rs) `bool_from_yn()` / `apply()` |
| **Applied to** | `Config::set_option("enable-camera", "Y"/"N")` |
| **Consumed by** | Local UI: `connection_page.dart:110`. Remote enforcement: `src/server/connection.rs:2544-2551` rejects `VIEW_CAMERA` (and Voice Call) when off. |
| **Status** | ✅ **Working**, both locally and remotely |
| **Precedence note** | Overwrites `enable-camera` every startup, same caveat as 4.3. |

### 4.5 `direct-ip-desktop-share-enabled`

| | |
|---|---|
| **Type** | boolean (`"Y"`/`"N"`) |
| **Required** | Yes |
| **Source** | [src/fork_config.rs](../src/fork_config.rs) `apply()` |
| **Applied to** | `Config::set_option("desktop-share-enabled", "Y"/"N")` — **fork-specific key, no upstream meaning** |
| **Consumed by** | Local UI only: `connection_page.dart:114` |
| **Status** | ⚠️ **Partially working** — local UI gating works; **no remote-side enforcement exists**. See `docs/CONFIG_FEATURE_VALIDATION.md` Section 2. |

### 4.6 Combined constraint: at least one of 4.4/4.5 must be `"Y"`

`ConfigError::NoConnectionModeEnabled` if both are `"N"` — rejected outright.

### 4.7 `direct-ip-show-setup-ui` — NEW (2026-09-02)

| | |
|---|---|
| **Type** | boolean (`"Y"`/`"N"`) |
| **Required** | **No — the one deliberate exception.** Defaults to `true` (shown) if absent. |
| **Source** | [src/fork_config.rs](../src/fork_config.rs) `apply()`, `validate()`'s `raw.show_setup_ui.unwrap_or(true)` |
| **Applied to** | `Config::set_option("show-setup-ui", "Y"/"N")` — fork-specific key |
| **Consumed by** | `DesktopSettingPage.switch2page()` ([desktop_setting_page.dart](../flutter/lib/desktop/pages/desktop_setting_page.dart)) — the single chokepoint both gear-icon entry points in `desktop_home_page.dart` call. `!mainGetBoolOptionSync("show-setup-ui")` short-circuits before the Settings page ever opens. |
| **Status** | ✅ **Working** (implemented 2026-09-02, per `docs/GUI_CONFIGURATION_CONTROL.md`) — desktop only; mobile has a separate settings entry point not yet gated (flagged as a gap in the original design doc) |

### 4.13 `voice-call-enabled` — NEW (2026-10-08)

| | |
|---|---|
| **Type** | boolean (`"Y"`/`"N"`) |
| **Required** | No. Defaults to `true` (enabled) if absent, so existing `config.toml` files keep working. |
| **Source** | [src/fork_config.rs](../src/fork_config.rs) `apply()`, `validate()`'s `raw.voice_call_enabled.unwrap_or(true)` |
| **Applied to** | `Config::set_option("voice-call-enabled", "Y"/"N")` — fork-specific key, no upstream meaning |
| **Consumed by** | Remote role: `src/server/connection.rs`'s `VoiceCallRequest` handler rejects the request (`VoiceCallResponse(accepted=false)` + a `MessageBox` with msgtype `voice-call-disabled`, shown by the caller as a non-fatal notice) before the connection manager is prompted. Local role: `isVoiceCallEnabledLocally()` ([common.dart](../flutter/lib/common.dart)) hides the "Voice call" entry in the desktop toolbar (`remote_toolbar.dart`) and the mobile chat menus, and skips the Support button's automatic call (`desktop/pages/view_camera_page.dart`). |
| **Not the same as** | upstream's `enable-audio` permission, which only gates the remote → local audio stream; with it off a call still connects and the remote still hears the caller. |
| **Status** | ✅ **Working** (2026-10-08). Fully enforced remotely; the local-side gating is a courtesy. |

### 4.8–4.12 `direct-ip-listen-address`, `direct-ip-listen-port`, `direct-ip-video-quality`, `direct-ip-audio-quality`, `direct-ip-log-level`

All required, all parsed and validated exactly as before the consolidation.
`direct-ip-listen-address`/`-video-quality`/`-audio-quality`/`-log-level` remain
**❌ Not wired to any behavior** — `#[allow(dead_code)]` on the corresponding `ForkConfig` fields.

**`direct-ip-listen-port` is now wired (2026-09-30)** — found and fixed via a real
machine-to-machine connection test that failed silently: `apply()` forces this value into
upstream's own `direct-access-port` option every startup, and unconditionally forces
`direct-server = "Y"` alongside it (see section 5) — together these are the *only* thing that
makes `rendezvous_mediator.rs::direct_server()` actually bind a listening socket, since this fork
has no rendezvous/relay accept path at all (`docs/ADR-0003-DIRECT-IP-ENFORCEMENT.md`). Before this
fix, `configs/remote.toml` shipped `direct-server = "N"`, so a "remote"-role instance never
accepted any inbound connection — this is also why the sample configs no longer list
`direct-server`/`direct-access-port` directly; they're fully overridden.
See [src/fork_config.rs](../src/fork_config.rs) for validation rules (IP format, nonzero port,
enum values).

---

## 5. Unconditional Settings (Not Config-Driven)

Applied inside `apply()` **every time** any `direct-ip-*` configuration validates successfully,
regardless of field values:

| Setting | Value | Consumed by | Status |
|---|---|---|---|
| `HARD_SETTINGS["disable-account"]` | `"Y"` | Hides Account tab | ✅ Working |
| `BUILTIN_SETTINGS["hide-network-settings"]` | `"Y"` | Hides Network tab | ✅ Working |
| `Config::set_option("enable-lan-discovery", "N")` | `"N"` | Suppresses LAN-broadcast discovery reply | ✅ Working |
| `OVERWRITE_SETTINGS["direct-server"]` | `"Y"` | Forces "Enable direct IP access" on (the only accept path this fork has); also makes the Settings UI toggle read-only via `is_option_fixed()` | ✅ Working (2026-09-30) |
| `OVERWRITE_SETTINGS["direct-access-port"]` | from `direct-ip-listen-port` | The actual port `direct_server()` listens on | ✅ Working (2026-09-30) |

If no `direct-ip-*` configuration is present/valid, none of these apply either — pure
upstream behavior.

---

## 6. Status Summary Table

| Key | Status |
|---|---|
| `direct-ip-config-version` | ✅ Working |
| `direct-ip-role` | ✅ Working |
| `direct-ip-auth-mode` | ✅ Working |
| `direct-ip-support-enabled` | ✅ Working |
| `direct-ip-desktop-share-enabled` | ⚠️ Partially working (local-only, no remote enforcement) |
| `direct-ip-show-setup-ui` | ✅ Working (new, optional, defaults to shown) |
| `voice-call-enabled` | ✅ Working (2026-10-08, optional, defaults to enabled; enforced remotely) |
| `direct-ip-listen-port` | ✅ Working (2026-09-30 — forced into `direct-access-port`) |
| `direct-ip-listen-address` / `-video-quality` / `-audio-quality` / `-log-level` | ❌ Not wired |
| *(unconditional)* disable-account / hide-network-settings / enable-lan-discovery / direct-server / direct-access-port | ✅ Working |

---

## 7. Related Files

- `configs/local.toml`, `configs/remote.toml` — ready-to-use `RustDesk2.toml`
  samples
- `configs/all-options-reference.toml` — every `direct-ip-*` key plus a curated upstream subset,
  fully commented
- `docs/UPSTREAM_CONFIG_REFERENCE.md` — the ~130-key upstream catalog this now shares a file with
- `docs/CONFIG_FEATURE_VALIDATION.md` — behavioral validation of `support_enabled`/`desktop_share_enabled`
- `docs/GUI_CONFIGURATION_CONTROL.md` — `show_setup_ui` design (now implemented)
- `docs/FORK_PROFILE_SPEC.md` — product-level behavior spec
- `docs/ADR-0003-DIRECT-IP-ENFORCEMENT.md` — why LAN discovery/rendezvous removal is unconditional
