# Configuration Reference — Direct-IP Fork

**Source-verified.** Every entry below is traced to an exact file and line in this repository.
No option is documented unless it exists in code.

---

## 1. Config File Identity

**Filename:** `fork_config.toml` (constant `CONFIG_FILE_NAME`, [src/fork_config.rs:46](../src/fork_config.rs))

## 2. Search Path & Load Order

Resolved by `config_path()` ([src/fork_config.rs:317-332](../src/fork_config.rs)):

| Build type | Path checked | Order |
|---|---|---|
| Debug (`#[cfg(debug_assertions)]`) | `./fork_config.toml` (current working directory) | Checked **first** |
| Debug, if not found above | `<exe_dir>/fork_config.toml` | Checked second |
| Release | `<exe_dir>/fork_config.toml` only | Only path checked |

`<exe_dir>` = the directory containing the running executable, via `std::env::current_exe()`.

Called once, early in startup: `load_and_apply()` is invoked from `src/core_main.rs`, immediately
after `crate::load_custom_client()` — before the inbound listener or any outbound-connect
capability is reachable.

## 3. Fallback Behavior

| Condition | Behavior |
|---|---|
| File not found | `log::warn!`, then **pure upstream default behavior**: no role restriction, upstream's own default authentication mode. Not treated as an error. |
| File found but unreadable (I/O error) | `log::error!`, falls back to upstream default behavior (same as "not found"). |
| File found but invalid TOML / fails validation | `log::error!` with the specific `ConfigError`, falls back to upstream default behavior. |
| File found and valid | `apply()` is called, see Section 4. |

There is **no partial application** — either the whole file validates and every mapping in
`apply()` runs, or none of it does.

---

## 4. Complete Option Enumeration

Schema is flat except `[authentication]`. Every field in `RawForkConfig`
([src/fork_config.rs:118-130](../src/fork_config.rs)) is **required** — there are no implicit
defaults; a missing field is a validation error (`ConfigError::MissingField`).

### 4.1 `version`

| | |
|---|---|
| **Type** | integer (`u32`) |
| **Required** | Yes |
| **Valid values** | `1` (only) — `SUPPORTED_CONFIG_VERSION` |
| **Source** | [src/fork_config.rs:44,238-241](../src/fork_config.rs) |
| **Applied to** | Nothing — gates whether the rest of the file is parsed at all |
| **Status** | ✅ **Working** |

### 4.2 `role`

| | |
|---|---|
| **Type** | string enum |
| **Required** | Yes |
| **Valid values** | `"local"`, `"remote"` |
| **Source** | [src/fork_config.rs:174-183](../src/fork_config.rs) (parse), [:339-346](../src/fork_config.rs) (apply) |
| **Applied to** | `HARD_SETTINGS["conn-type"]` = `"outgoing"` (local) / `"incoming"` (remote) |
| **Consumed by** | `hbb_common::config::is_incoming_only()` / `is_outgoing_only()` — gate `src/client.rs` outbound connects and `src/rendezvous_mediator.rs` inbound listener |
| **Status** | ✅ **Working** — unit-tested (`apply_sets_outgoing_only_for_local_role`, `apply_sets_incoming_only_for_remote_role`) |

### 4.3 `authentication.mode`

| | |
|---|---|
| **Type** | string enum, in `[authentication]` table |
| **Required** | Yes |
| **Valid values** | `"ask"`, `"password"`, `"ask_and_password"` |
| **Source** | [src/fork_config.rs:185-195](../src/fork_config.rs) (parse), [:348-355](../src/fork_config.rs) (apply) |
| **Applied to** | `Config::set_option("approve-mode", ...)` → `"click"` / `"password"` / `""` (empty clears the option, upstream then falls to its own `ApproveMode::Both` default) |
| **Consumed by** | `password_security::approve_mode()` in `libs/hbb_common/src/password_security.rs:77-86` |
| **Status** | ✅ **Working** — unit-tested (`apply_maps_authentication_modes_to_approve_mode_option`) |
| **Ordering note** | Must be the **last** section in the TOML file — any scalar key placed after `[authentication]` is silently nested under it instead of the top level (TOML semantics, not a fork bug) |

### 4.4 `support_enabled`

| | |
|---|---|
| **Type** | boolean |
| **Required** | Yes |
| **Source** | [src/fork_config.rs:254-256](../src/fork_config.rs) (parse), [:361-364](../src/fork_config.rs) (apply) |
| **Applied to** | `Config::set_option("enable-camera", "Y"/"N")` |
| **Consumed by** | (1) Local UI: `connection_page.dart:110` `mainGetBoolOptionSync("enable-camera")` gates the Support button. (2) Remote enforcement: `src/server/connection.rs:2544-2551` login handler rejects `VIEW_CAMERA` (and therefore Voice Call, which rides on it) when this permission is off. |
| **Status** | ✅ **Working**, both locally and remotely — unit-tested (`apply_maps_support_enabled_to_enable_camera_permission`) |

### 4.5 `desktop_share_enabled`

| | |
|---|---|
| **Type** | boolean |
| **Required** | Yes |
| **Source** | [src/fork_config.rs:257-259](../src/fork_config.rs) (parse), [:370-378](../src/fork_config.rs) (apply) |
| **Applied to** | `Config::set_option("desktop-share-enabled", "Y"/"N")` — **not** an upstream option; this key exists only for this fork |
| **Consumed by** | Local UI only: `connection_page.dart:114` `mainGetBoolOptionSync("desktop-share-enabled")` gates the Desktop button and whether `onSupport()` also opens a `DEFAULT_CONN` session ([connection_page.dart:120-125](../flutter/lib/desktop/pages/connection_page.dart)) |
| **Status** | ⚠️ **Partially working** — local UI gating works; **no remote-side enforcement exists**. No upstream permission rejects a plain `DEFAULT_CONN` login outright, so a remote host with `desktop_share_enabled = false` cannot actually refuse a Desktop-mode connection at the protocol level if a client attempts one directly (bypassing this fork's UI). Documented as a known gap in `docs/FORK_PROFILE_SPEC.md`. See Workstream 2 doc (`CONFIG_FEATURE_VALIDATION.md`) for detail. |

### 4.6 Combined constraint: at least one of 4.4/4.5 must be true

[src/fork_config.rs:260-262](../src/fork_config.rs): if both `support_enabled` and
`desktop_share_enabled` are `false`, validation fails with `ConfigError::NoConnectionModeEnabled`
— rejected outright rather than producing a connection screen with no buttons.

### 4.7 `listen_address`

| | |
|---|---|
| **Type** | string (IP address) |
| **Required** | Yes |
| **Valid values** | Any value parseable by `std::net::IpAddr` (`validate_listen_address`, [src/fork_config.rs:223-232](../src/fork_config.rs)) |
| **Source** | [src/fork_config.rs:104,264-267](../src/fork_config.rs) |
| **Applied to** | **Nothing.** Field is `#[allow(dead_code)]` |
| **Status** | ❌ **Not wired** — parsed and validated (so the file format is stable across future phases) but no caller reads it yet |

### 4.8 `listen_port`

| | |
|---|---|
| **Type** | integer (`u16`) |
| **Required** | Yes |
| **Valid values** | 1–65535 (0 explicitly rejected, [src/fork_config.rs:272-277](../src/fork_config.rs)) |
| **Source** | [src/fork_config.rs:106,269-277](../src/fork_config.rs) |
| **Applied to** | **Nothing.** Field is `#[allow(dead_code)]` |
| **Status** | ❌ **Not wired** |

### 4.9 `video_quality`

| | |
|---|---|
| **Type** | string enum |
| **Required** | Yes |
| **Valid values** | `"low"`, `"medium"`, `"high"` |
| **Source** | [src/fork_config.rs:108,197-207,279-282](../src/fork_config.rs) |
| **Applied to** | **Nothing.** Field is `#[allow(dead_code)]` |
| **Status** | ❌ **Not wired** |

### 4.10 `audio_quality`

| | |
|---|---|
| **Type** | string enum |
| **Required** | Yes |
| **Valid values** | `"low"`, `"medium"`, `"high"` |
| **Source** | [src/fork_config.rs:110,284-287](../src/fork_config.rs) |
| **Applied to** | **Nothing.** Field is `#[allow(dead_code)]` |
| **Status** | ❌ **Not wired** |

### 4.11 `log_level`

| | |
|---|---|
| **Type** | string enum |
| **Required** | Yes |
| **Valid values** | `"error"`, `"warn"`, `"info"`, `"debug"`, `"trace"` |
| **Source** | [src/fork_config.rs:112,209-221,289-292](../src/fork_config.rs) |
| **Applied to** | **Nothing.** Field is `#[allow(dead_code)]` |
| **Status** | ❌ **Not wired** |

---

## 5. Unconditional Settings (Not Config-Driven)

These are applied inside `apply()` **every time** a valid config file loads, regardless of any
field's value. They do not correspond to a TOML key and cannot be turned off via
`fork_config.toml`:

| Setting | Value | Source | Consumed by | Status |
|---|---|---|---|---|
| `HARD_SETTINGS["disable-account"]` | `"Y"` | [src/fork_config.rs:390-393](../src/fork_config.rs) | `hbb_common::config::is_disable_account()` → hides Account tab in `desktop_setting_page.dart` tabKeys | ✅ Working |
| `BUILTIN_SETTINGS["hide-network-settings"]` | `"Y"` | [src/fork_config.rs:394-397](../src/fork_config.rs) | `common::get_builtin_option()` → hides Network tab in `desktop_setting_page.dart` tabKeys | ✅ Working |
| `Config::set_option("enable-lan-discovery", "N")` | `"N"` | [src/fork_config.rs:406](../src/fork_config.rs) | `src/lan.rs` — LAN-broadcast discovery reply is suppressed | ✅ Working |

**Important:** if `fork_config.toml` is absent or invalid, none of these three apply either — the
app falls back to *pure* upstream behavior (Account/Network tabs visible, LAN discovery active).
See `docs/ADR-0003-DIRECT-IP-ENFORCEMENT.md` for why these three are permanent product decisions,
not runtime toggles.

---

## 6. Status Summary Table

| Key | Status |
|---|---|
| `version` | ✅ Working |
| `role` | ✅ Working |
| `authentication.mode` | ✅ Working |
| `support_enabled` | ✅ Working |
| `desktop_share_enabled` | ⚠️ Partially working (local-only, no remote enforcement) |
| `listen_address` | ❌ Not wired |
| `listen_port` | ❌ Not wired |
| `video_quality` | ❌ Not wired |
| `audio_quality` | ❌ Not wired |
| `log_level` | ❌ Not wired |
| *(unconditional)* disable-account / hide-network-settings / enable-lan-discovery | ✅ Working |

No deprecated options exist yet — schema version 1 is the only version ever shipped.

---

## 7. Related Documents

- `docs/FORK_PROFILE_SPEC.md` — product-level behavior spec
- `docs/ADR-0003-DIRECT-IP-ENFORCEMENT.md` — why LAN discovery / rendezvous removal is unconditional
- `docs/CONFIG_FEATURE_VALIDATION.md` — behavioral validation of `support_enabled`/`desktop_share_enabled` and related upstream options (Workstream 2)
- `configs/example-local.toml`, `configs/example-remote.toml`, `configs/all-options-reference.toml` — generated example files (source-verified, no invented options)
