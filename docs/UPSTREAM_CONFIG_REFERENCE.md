# Upstream RustDesk Configuration Reference — `RustDesk2.toml`

**2026-09-02: this is now the ONLY config file.** The fork previously used a separate
`fork_config.toml` alongside this file; that has been retired (see `docs/CONFIG_REFERENCE.md`).
The fork's own settings now live in this same `[options]` table, under `direct-ip-*`-prefixed
keys, read via the exact same `Config::get_option()` mechanism documented below. This document
covers upstream RustDesk's own, much larger configuration system, which this fork does not
replace — `fork_config.rs::apply()` writes into this same system via `Config::set_option(...)`
calls (see Section 3).

**Source:** `libs/hbb_common/src/config.rs`. Not modified by this fork.

---

## 1. File Identity & Location

| Struct | Serializes to | Purpose |
|---|---|---|
| `Config` | `{APP_NAME}.toml` → **`RustDesk.toml`** | Identity: `id`, keypair, salt |
| `Config2` | `{APP_NAME}{"2"}.toml` → **`RustDesk2.toml`** | Everything else: rendezvous server, NAT type, SOCKS proxy, and — most relevantly — the **`options` table**, a flat `HashMap<String, String>` holding the majority of RustDesk's runtime-configurable behavior |
| `LocalConfig` | `RustDesk_local.toml` | Local-machine-only UI state (not synced/relevant to server behavior) |

Filename construction: `format!("{}{}", APP_NAME, suffix)` + `.toml` extension
(`libs/hbb_common/src/config.rs:742-745`; `APP_NAME` defaults to `"RustDesk"`,
line 72).

**Directory:** `directories_next::ProjectDirs::from("", org, APP_NAME).config_dir()`
(`config.rs:783-806`) — the OS-standard per-user config directory:
- Windows: `%APPDATA%\RustDesk\config\`
- Linux: `~/.config/RustDesk/`
- macOS: `~/Library/Application Support/RustDesk/` (or under an org-specific path if `ORG` is set)

This file is normally written by the app itself as the user changes settings in the UI — but it
*can* be pre-seeded (the app will happily read a hand-written file matching this schema on first
launch), which is exactly how this fork's `direct-ip-*` keys are meant to be supplied: see
`configs/local.toml`/`configs/remote.toml`.

---

## 2. `[options]` Table Structure

`Config2.options` (`config.rs:259-260`) is `HashMap<String, String>`, serialized by the `toml`
crate as a table:

```toml
[options]
enable-camera = "Y"
approve-mode = "click"
whitelist = "192.168.1.0/24,10.0.0.5"
```

**Resolution order when the app reads any option** (`Config::get_option`, `config.rs:1245-1253`):
`OVERWRITE_SETTINGS` (admin-pushed, highest priority) → `CONFIG2.options` (this TOML file) →
`DEFAULT_SETTINGS` (compiled-in fallback) → empty string if nowhere found.

**Value convention:** almost every `enable-*`, `allow-*`, `disable-*`, and `hide-*` prefixed key
is a boolean encoded as the string `"Y"` or `"N"` (checked via `option2bool()`), consistent with
how `fork_config.rs` itself already writes `"Y"`/`"N"` for `enable-camera` and
`desktop-share-enabled`. Keys without one of those prefixes hold free-form strings or numbers
(e.g. `whitelist` is comma-separated CIDR entries, `auto-disconnect-timeout` is a number of
seconds) — see the per-key notes in Section 4 where the name alone doesn't make the format obvious.

---

## 3. Overlap With This Fork

`fork_config.rs::apply()` sets exactly five of the keys below via `Config::set_option`:
`approve-mode`, `enable-camera`, `enable-lan-discovery`, and the fork-specific
`desktop-share-enabled` and `show-setup-ui` (neither is an upstream key — see
`docs/CONFIG_REFERENCE.md` Sections 4.5 and 4.7). Every other key in this reference is
**upstream behavior, untouched by this fork**, but still active and configurable via
`RustDesk2.toml` on any deployment.

**No key-name collision is possible** between the fork's own inputs and any upstream key: the
fork reads its inputs from `direct-ip-*`-prefixed keys (a namespace verified against every
`OPTION_*` constant in this file to guarantee no overlap) and writes its outputs to the *different*
keys named above. `direct-ip-auth-mode`, `direct-ip-support-enabled`, and
`direct-ip-desktop-share-enabled` are never the same string as `approve-mode`, `enable-camera`,
or `desktop-share-enabled` — so both the "source" and "destination" of each translation live in
this one file without ambiguity. The only thing to know: `approve-mode`, `enable-camera`, and
`enable-lan-discovery` are overwritten from their `direct-ip-*` counterparts every startup — a
hand-edited value for those three specific keys will not survive a restart while the
corresponding `direct-ip-*` key is also present and valid.

---

## 4. Full Option Key Reference (Source: `config.rs:2854-3054`)

Grouped by function. Every key name below is copied verbatim from a `pub const OPTION_*` string
literal in `libs/hbb_common/src/config.rs` — none invented.

### Session / Remote-Control Permissions
*(Y/N; read at login time by `src/server/connection.rs`, same mechanism `fork_config.rs` reuses
for `enable-camera`)*

| Key | Constant |
|---|---|
| `enable-keyboard` | `OPTION_ENABLE_KEYBOARD` |
| `enable-clipboard` | `OPTION_ENABLE_CLIPBOARD` |
| `enable-file-transfer` | `OPTION_ENABLE_FILE_TRANSFER` |
| `enable-camera` | `OPTION_ENABLE_CAMERA` *(already used by this fork)* |
| `enable-terminal` | `OPTION_ENABLE_TERMINAL` |
| `terminal-persistent` | `OPTION_TERMINAL_PERSISTENT` |
| `enable-audio` | `OPTION_ENABLE_AUDIO` |
| `enable-tunnel` | `OPTION_ENABLE_TUNNEL` |
| `enable-remote-restart` | `OPTION_ENABLE_REMOTE_RESTART` |
| `enable-record-session` | `OPTION_ENABLE_RECORD_SESSION` |
| `enable-block-input` | `OPTION_ENABLE_BLOCK_INPUT` |
| `enable-privacy-mode` | `OPTION_ENABLE_PRIVACY_MODE` |
| `enable-remote-printer` | `OPTION_ENABLE_REMOTE_PRINTER` |
| `enable-file-copy-paste` | `OPTION_ENABLE_FILE_COPY_PASTE` |
| `access-mode` | `OPTION_ACCESS_MODE` — not Y/N; values are `"full"` / `"view"` / custom (see `desktop_setting_page.dart`'s `_AccessMode` enum) |
| `allow-remote-config-modification` | `OPTION_ALLOW_REMOTE_CONFIG_MODIFICATION` |
| `allow-remote-cm-modification` | `OPTION_ALLOW_REMOTE_CM_MODIFICATION` |
| `permanent-password-set` (read-only marker, not user-set) | — |

### Security / Network

| Key | Constant | Notes |
|---|---|---|
| `approve-mode` | `OPTION_APPROVE_MODE` | `"click"` / `"password"` / `""` — **already used by this fork** |
| `verification-method` | `OPTION_VERIFICATION_METHOD` | One-time / permanent / both password mode |
| `temporary-password-length` | `OPTION_TEMPORARY_PASSWORD_LENGTH` | Numeric string |
| `whitelist` | `OPTION_WHITELIST` | Comma-separated CIDR list — see `docs/CONFIG_FEATURE_VALIDATION.md` Section 3 |
| `enable-lan-discovery` | `OPTION_ENABLE_LAN_DISCOVERY` | **Already used by this fork** (set unconditionally to `"N"`) |
| `direct-server` | `OPTION_DIRECT_SERVER` | Enables the direct-TCP listener (flagged as needing runtime verification in `docs/SETUP_UI_AUDIT.md`) |
| `direct-access-port` | `OPTION_DIRECT_ACCESS_PORT` | Numeric string |
| `custom-rendezvous-server` | `OPTION_CUSTOM_RENDEZVOUS_SERVER` | Hostname/IP |
| `relay-server` | `OPTION_RELAY_SERVER` | Hostname/IP |
| `api-server` | `OPTION_API_SERVER` | URL |
| `key` | `OPTION_KEY` | Public key for server verification |
| `ice-servers` | `OPTION_ICE_SERVERS` | JSON/string list |
| `allow-websocket` | `OPTION_ALLOW_WEBSOCKET` | |
| `disable-udp` | `OPTION_DISABLE_UDP` | |
| `enable-udp-punch` | `OPTION_ENABLE_UDP_PUNCH` | |
| `enable-ipv6-punch` | `OPTION_ENABLE_IPV6_PUNCH` | |
| `allow-insecure-tls-fallback` | `OPTION_ALLOW_INSECURE_TLS_FALLBACK` | |
| `allow-https-21114` | `OPTION_ALLOW_HTTPS_21114` | |
| `use-raw-tcp-for-api` | `OPTION_USE_RAW_TCP_FOR_API` | |
| `allow-hostname-as-id` | `OPTION_ALLOW_HOSTNAME_AS_ID` | |
| `allow-numeric-one-time-password` | `OPTION_ALLOW_NUMERNIC_ONE_TIME_PASSWORD` | |
| `allow-auto-disconnect` | `OPTION_ALLOW_AUTO_DISCONNECT` | |
| `auto-disconnect-timeout` | `OPTION_AUTO_DISCONNECT_TIMEOUT` | Seconds, numeric string |
| `allow-only-conn-window-open` | `OPTION_ALLOW_ONLY_CONN_WINDOW_OPEN` | |
| `allow-scope-violation-close` | `OPTION_ALLOW_SCOPE_VIOLATION_CLOSE` | |
| `allow-scope-violation-alarm` | `OPTION_ALLOW_SCOPE_VIOLATION_ALARM` | |
| `disable-change-permanent-password` | `OPTION_DISABLE_CHANGE_PERMANENT_PASSWORD` | |
| `disable-change-id` | `OPTION_DISABLE_CHANGE_ID` | |
| `disable-unlock-pin` | `OPTION_DISABLE_UNLOCK_PIN` | |
| `default-connect-password` | `OPTION_DEFAULT_CONNECT_PASSWORD` | |
| `allow-logon-screen-password` | `OPTION_ALLOW_LOGON_SCREEN_PASSWORD` | |
| `allow-deep-link-password` | `OPTION_ALLOW_DEEP_LINK_PASSWORD` | |
| `allow-deep-link-server-settings` | `OPTION_ALLOW_DEEP_LINK_SERVER_SETTINGS` | |
| `enable-trusted-devices` | `OPTION_ENABLE_TRUSTED_DEVICES` | |
| `register-device` | `OPTION_REGISTER_DEVICE` | |
| `proxy-url` / `proxy-username` / `proxy-password` | `OPTION_PROXY_*` | |

### Display / Media

| Key | Constant | Notes |
|---|---|---|
| `view_style` | `OPTION_VIEW_STYLE` | |
| `scroll_style` | `OPTION_SCROLL_STYLE` | |
| `edge-scroll-edge-thickness` | `OPTION_EDGE_SCROLL_EDGE_THICKNESS` | |
| `image_quality` | `OPTION_IMAGE_QUALITY` | |
| `custom_image_quality` | `OPTION_CUSTOM_IMAGE_QUALITY` | |
| `custom-fps` | `OPTION_CUSTOM_FPS` | |
| `codec-preference` | `OPTION_CODEC_PREFERENCE` | |
| `enable-hwcodec` | `OPTION_ENABLE_HWCODEC` | |
| `enable-abr` (adaptive bitrate) | `OPTION_ENABLE_ABR` | |
| `i444` | `OPTION_I444` | Chroma subsampling toggle |
| `av1-test` | `OPTION_AV1_TEST` | |
| `use-texture-render` | `OPTION_TEXTURE_RENDER` | |
| `allow-d3d-render` | `OPTION_ALLOW_D3D_RENDER` | Windows |
| `allow-always-software-render` | `OPTION_ALLOW_ALWAYS_SOFTWARE_RENDER` | |
| `enable-directx-capture` | `OPTION_ENABLE_DIRECTX_CAPTURE` | Windows |
| `enable-android-software-encoding-half-scale` | `OPTION_ENABLE_ANDROID_SOFTWARE_ENCODING_HALF_SCALE` | Android |
| `show_remote_cursor` | `OPTION_SHOW_REMOTE_CURSOR` | |
| `follow_remote_cursor` | `OPTION_FOLLOW_REMOTE_CURSOR` | |
| `follow_remote_window` | `OPTION_FOLLOW_REMOTE_WINDOW` | |
| `zoom-cursor` | `OPTION_ZOOM_CURSOR` | |
| `show_quality_monitor` | `OPTION_SHOW_QUALITY_MONITOR` | |
| `disable_audio` | `OPTION_DISABLE_AUDIO` | Note: distinct from `enable-audio` (permission) — this is a client-side playback toggle |
| `disable_clipboard` | `OPTION_DISABLE_CLIPBOARD` | |
| `reverse_mouse_wheel` | `OPTION_REVERSE_MOUSE_WHEEL` | |
| `swap-left-right-mouse` | `OPTION_SWAP_LEFT_RIGHT_MOUSE` | |
| `displays_as_individual_windows` | `OPTION_DISPLAYS_AS_INDIVIDUAL_WINDOWS` | |
| `use_all_my_displays_for_the_remote_session` | `OPTION_USE_ALL_MY_DISPLAYS_FOR_THE_REMOTE_SESSION` | |
| `trackpad-speed` | `OPTION_TRACKPAD_SPEED` | |
| `show-virtual-mouse` | `OPTION_SHOW_VIRTUAL_MOUSE` | |
| `show-virtual-joystick` | `OPTION_SHOW_VIRTUAL_JOYSTICK` | |
| `lock_after_session_end` | `OPTION_LOCK_AFTER_SESSION_END` | |
| `privacy_mode` | `OPTION_PRIVACY_MODE` | Client-requested privacy mode state |
| `touch-mode` | `OPTION_TOUCH_MODE` | |
| `allow-remove-wallpaper` | `OPTION_ALLOW_REMOVE_WALLPAPER` | |
| `allow-linux-headless` | `OPTION_ALLOW_LINUX_HEADLESS` | |
| `keep-screen-on` | `OPTION_KEEP_SCREEN_ON` | |
| `keep-awake-during-incoming-sessions` | `OPTION_KEEP_AWAKE_DURING_INCOMING_SESSIONS` | Visible in Settings → Safety → Security, see `docs/SETUP_UI_AUDIT.md` |
| `keep-awake-during-outgoing-sessions` | `OPTION_KEEP_AWAKE_DURING_OUTGOING_SESSIONS` | |
| `pre-elevate-service` | `OPTION_PRE_ELEVATE_SERVICE` | Windows |

### File Transfer / Recording

| Key | Constant | Notes |
|---|---|---|
| `file-transfer-max-files` | `OPTION_FILE_TRANSFER_MAX_FILES` | Documented inline in source (`config.rs:2956-2964`): positive int = limit, `0` = built-in default, unset/negative = no limit |
| `one-way-file-transfer` | `OPTION_ONE_WAY_FILE_TRANSFER` | |
| `allow-auto-record-incoming` | `OPTION_ALLOW_AUTO_RECORD_INCOMING` | |
| `allow-auto-record-outgoing` | `OPTION_ALLOW_AUTO_RECORD_OUTGOING` | |
| `video-save-directory` | `OPTION_VIDEO_SAVE_DIRECTORY` | Filesystem path |
| `one-way-clipboard-redirection` | `OPTION_ONE_WAY_CLIPBOARD_REDIRECTION` | |
| `sync-init-clipboard` | `OPTION_SYNC_INIT_CLIPBOARD` | |

### Printer

| Key | Constant |
|---|---|
| `printer-incomming-job-action` | `OPTION_PRINTER_INCOMING_JOB_ACTION` *(note: upstream typo "incomming" preserved verbatim from source)* |
| `allow-printer-auto-print` | `OPTION_PRINTER_ALLOW_AUTO_PRINT` |
| `printer-selected-name` | `OPTION_PRINTER_SELECTED_NAME` |

### UI Visibility / Branding (Custom-Client Mechanism — Same One This Fork Reuses)

*This is the exact mechanism `fork_config.rs` uses for `disable-account` and
`hide-network-settings` (see `docs/CONFIG_REFERENCE.md` Section 5) — these live in
`HARD_SETTINGS`/`BUILTIN_SETTINGS`, not `Config2.options`, but are listed here for completeness
since they're part of the same overall settings-visibility system:*

| Key | Constant |
|---|---|
| `hide-security-settings` | `OPTION_HIDE_SECURITY_SETTINGS` |
| `hide-network-settings` | `OPTION_HIDE_NETWORK_SETTINGS` *(already used by this fork)* |
| `hide-server-settings` | `OPTION_HIDE_SERVER_SETTINGS` |
| `hide-proxy-settings` | `OPTION_HIDE_PROXY_SETTINGS` |
| `hide-remote-printer-settings` | `OPTION_HIDE_REMOTE_PRINTER_SETTINGS` |
| `hide-websocket-settings` | `OPTION_HIDE_WEBSOCKET_SETTINGS` |
| `hide-stop-service` | `OPTION_HIDE_STOP_SERVICE` |
| `hide-username-on-card` | `OPTION_HIDE_USERNAME_ON_CARD` |
| `hide-help-cards` | `OPTION_HIDE_HELP_CARDS` |
| `hide-tray` | `OPTION_HIDE_TRAY` |
| `hide-powered-by-me` | `OPTION_HIDE_POWERED_BY_ME` |
| `disable-group-panel` | `OPTION_DISABLE_GROUP_PANEL` |
| `disable-discovery-panel` | `OPTION_DISABLE_DISCOVERY_PANEL` |
| `disable-floating-window` | `OPTION_DISABLE_FLOATING_WINDOW` |
| `allow-command-line-settings-when-settings-disabled` | `OPTION_ALLOW_COMMAND_LINE_SETTINGS_WHEN_SETTINGS_DISABLED` |
| `main-window-always-on-top` | `OPTION_MAIN_WINDOW_ALWAYS_ON_TOP` |

### Floating Window (Android/Mobile)

| Key | Constant |
|---|---|
| `floating-window-size` | `OPTION_FLOATING_WINDOW_SIZE` |
| `floating-window-untouchable` | `OPTION_FLOATING_WINDOW_UNTOUCHABLE` |
| `floating-window-transparency` | `OPTION_FLOATING_WINDOW_TRANSPARENCY` |
| `floating-window-svg` | `OPTION_FLOATING_WINDOW_SVG` |

### Address Book / Presets (Enterprise/Preset-Config Features)

| Key | Constant |
|---|---|
| `preset-address-book-name` / `-tag` / `-alias` / `-password` / `-note` | `OPTION_PRESET_ADDRESS_BOOK_*` |
| `preset-device-username` / `preset-device-name` / `preset-note` | `OPTION_PRESET_DEVICE_*` / `OPTION_PRESET_NOTE` |
| `preset-device-group-name` | `OPTION_PRESET_DEVICE_GROUP_NAME` |
| `preset-user-name` | `OPTION_PRESET_USERNAME` |
| `preset-strategy-name` | `OPTION_PRESET_STRATEGY_NAME` |
| `remove-preset-password-warning` | `OPTION_REMOVE_PRESET_PASSWORD_WARNING` |
| `display-name` / `avatar` | `OPTION_DISPLAY_NAME` / `OPTION_AVATAR` |
| `sync-ab-with-recent-sessions` | `OPTION_SYNC_AB_WITH_RECENT_SESSIONS` |
| `sync-ab-tags` | `OPTION_SYNC_AB_TAGS` |
| `filter-ab-by-intersection` | `OPTION_FILTER_AB_BY_INTERSECTION` |
| `hideAbTagsPanel` | `OPTION_HIDE_AB_TAGS_PANEL` *(note: camelCase, inconsistent with the rest of this list — verbatim from source)* |

### Misc / App-Level

| Key | Constant |
|---|---|
| `theme` | `OPTION_THEME` |
| `lang` | `OPTION_LANGUAGE` |
| `enable-check-update` | `OPTION_ENABLE_CHECK_UPDATE` |
| `allow-auto-update` | `OPTION_ALLOW_AUTO_UPDATE` |
| `enable-confirm-closing-tabs` | `OPTION_ENABLE_CONFIRM_CLOSING_TABS` |
| `enable-open-new-connections-in-tabs` | `OPTION_ENABLE_OPEN_NEW_CONNECTIONS_IN_TABS` |
| `remote-menubar-drag-left` / `-right` | `OPTION_REMOTE_MENUBAR_DRAG_LEFT` / `_RIGHT` |
| `enable-flutter-http-on-rust` | `OPTION_ENABLE_FLUTTER_HTTP_ON_RUST` |
| `allow-ask-for-note` | `OPTION_ALLOW_ASK_FOR_NOTE` |
| `show_monitors_toolbar` / `collapse_toolbar` | `OPTION_SHOW_MONITORS_TOOLBAR` / `OPTION_COLLAPSE_TOOLBAR` |
| `view_only` | `OPTION_VIEW_ONLY` |
| `enable-perm-change-in-accept-window` | `OPTION_ENABLE_PERM_CHANGE_IN_ACCEPT_WINDOW` |
| `remoteMenubarState` / `peer-sorting` / `peer-tab-index` / `peer-tab-order` / `peer-tab-visible` / `peer-card-ui-type` / `current-ab-name` | Flutter-UI-state-only keys (`OPTION_FLUTTER_*`) — not behavioral config |

---

## 5. Not Investigated / Not Verified This Pass

- **Exact default value for every key** — `DEFAULT_SETTINGS` is populated at runtime (partly from
  compiled-in defaults, partly from `option2bool`'s own fallback), not from a single static table
  in this file; confirming each key's true default would require tracing each read call site
  individually. Not done for all 130+ keys given the scope of this pass.
- **Cross-referencing official RustDesk documentation/wiki** — this reference is built entirely
  from source code (`libs/hbb_common/src/config.rs`), which is authoritative for *this exact
  version* of the codebase. Upstream's own docs/wiki may describe additional context (recommended
  values, deployment guides) not reproduced here; recommend checking
  `https://rustdesk.com/docs/` and the upstream project's own wiki for operator-facing guidance,
  since this document's job is to be accurate to *this repository's actual code*, not to
  duplicate external documentation.

---

## 6. Related Files

- `configs/local.toml`, `configs/remote.toml`, `configs/all-options-reference.toml` — sample files using a subset of the keys above, plus every `direct-ip-*` key
- `docs/CONFIG_REFERENCE.md` — the fork's own `direct-ip-*` keys within this same file
- `docs/CONFIG_FEATURE_VALIDATION.md` — behavioral validation of `whitelist`,
  `enable-lan-discovery`, and other options this fork's audit already touched
