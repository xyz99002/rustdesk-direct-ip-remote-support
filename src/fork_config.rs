//! Configuration and role-restriction layer for the RustDesk direct-IP fork.
//!
//! **2026-09-02: consolidated from a separate `fork_config.toml` file into upstream's own
//! `RustDesk2.toml` (`hbb_common::config::Config2`).** This module no longer parses or looks up
//! any file of its own — every value it needs is read via the existing
//! `Config::get_option()`/`Config::set_option()` mechanism, under a set of `direct-ip-*`-prefixed
//! keys stored in `Config2`'s own `options` table (the same table that holds `approve-mode`,
//! `enable-camera`, and every other upstream option). This eliminates the second config file
//! entirely, along with the "which file wins" precedence question that existed while there were
//! two files. See `docs/CONFIG_REFERENCE.md` and `docs/UPSTREAM_CONFIG_REFERENCE.md`.
//!
//! The `direct-ip-*` keys below are **inputs** this module reads; they are translated into
//! upstream's own, unmodified mechanisms via a **different** set of keys (never the same name,
//! so there is no key-level collision even though everything now lives in one file):
//!
//! - `direct-ip-role` -> `hbb_common::config::HARD_SETTINGS["conn-type"]` (`"outgoing"` /
//!   `"incoming"`), which upstream's own `is_incoming_only()`/`is_outgoing_only()` already gate
//!   outbound connects (`src/client.rs`) and the inbound listener
//!   (`src/rendezvous_mediator.rs`) on. `HARD_SETTINGS` is in-memory only, never persisted to
//!   `RustDesk2.toml`.
//! - `direct-ip-auth-mode` -> `Config::set_option("approve-mode", ...)`, which upstream's own
//!   `password_security::approve_mode()` already reads.
//! - `direct-ip-support-enabled` -> `Config::set_option("enable-camera", ...)`, which upstream's
//!   own login handler (`src/server/connection.rs:2544-2551`) already reads to accept/reject
//!   `VIEW_CAMERA` (and therefore Voice Call, which rides on it) connections.
//! - `direct-ip-desktop-share-enabled` -> `Config::set_option("desktop-share-enabled", ...)` —
//!   still a fork-specific key with no upstream meaning (see `docs/CONFIG_FEATURE_VALIDATION.md`
//!   Section 2 for why this has no remote-side enforcement).
//! - `direct-ip-show-setup-ui` -> `Config::set_option("show-setup-ui", ...)` — new (see
//!   `docs/GUI_CONFIGURATION_CONTROL.md`). **Optional**, defaults to `"Y"` (shown) if absent —
//!   a deliberate, documented exception to this module's usual "every field required" rule,
//!   since this field is UI convenience, not a security-relevant setting, and a missing key
//!   should not regress an existing deployment's Settings visibility.
//!
//! Minimal UI (unconditional, not config-driven): `HARD_SETTINGS["disable-account"]` and
//! `BUILTIN_SETTINGS["hide-network-settings"]` are set so the Flutter UI's own existing
//! conditionals (`DesktopSettingPage.tabKeys` in
//! `flutter/lib/desktop/pages/desktop_setting_page.dart`) hide the Account and Network
//! (relay/rendezvous server address) settings tabs — reusing upstream's own custom-client hiding
//! mechanism. Direct-IP enforcement (also unconditional): `Config::set_option("enable-lan-discovery", "N")`
//! closes the LAN-broadcast public-ID exposure path in `src/lan.rs`. See
//! `docs/ADR-0003-DIRECT-IP-ENFORCEMENT.md`.
//!
//! No authentication, transport, encryption, password storage, or Voice Call/VIEW_CAMERA code is
//! modified or reimplemented here.

use hbb_common::config::{Config, BUILTIN_SETTINGS, HARD_SETTINGS};
use hbb_common::log;
use std::path::{Path, PathBuf};

/// The only configuration schema version understood today. A future incompatible schema change
/// must bump this and add explicit migration/rejection logic rather than silently reinterpreting
/// old values.
pub const SUPPORTED_CONFIG_VERSION: u32 = 1;

/// `direct-ip-*` option keys read from `Config2.options` (i.e. `RustDesk2.toml`'s `[options]`
/// table). Every key here is a distinct string from any upstream `OPTION_*` constant in
/// `libs/hbb_common/src/config.rs` — verified by grep against that file at the time this was
/// written, to guarantee no collision with the ~130 existing upstream keys.
mod keys {
    pub const CONFIG_VERSION: &str = "direct-ip-config-version";
    pub const ROLE: &str = "direct-ip-role";
    pub const AUTH_MODE: &str = "direct-ip-auth-mode";
    pub const SUPPORT_ENABLED: &str = "direct-ip-support-enabled";
    pub const DESKTOP_SHARE_ENABLED: &str = "direct-ip-desktop-share-enabled";
    pub const LISTEN_ADDRESS: &str = "direct-ip-listen-address";
    pub const LISTEN_PORT: &str = "direct-ip-listen-port";
    pub const VIDEO_QUALITY: &str = "direct-ip-video-quality";
    pub const AUDIO_QUALITY: &str = "direct-ip-audio-quality";
    pub const LOG_LEVEL: &str = "direct-ip-log-level";
    /// Optional; see module doc comment for the default-value rationale.
    pub const SHOW_SETUP_UI: &str = "direct-ip-show-setup-ui";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// May only initiate outbound direct-IP connections; never accepts inbound sessions.
    Local,
    /// May only accept inbound direct-IP connections; never initiates outbound sessions.
    Remote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    /// Maps to upstream `approve-mode = "click"` (`ApproveMode::Click`).
    Ask,
    /// Maps to upstream `approve-mode = "password"` (`ApproveMode::Password`).
    Password,
    /// Maps to upstream `approve-mode` unset/empty (`ApproveMode::Both`, upstream's own default).
    AskAndPassword,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

/// Fully parsed and validated fork configuration.
///
/// `listen_address`, `listen_port`, `video_quality`, `audio_quality`, and `log_level` are
/// validated here (so the schema is stable and won't need a version bump later) but are **not
/// yet** wired to any behavior — that happens in the phases that own them (Direct-IP transport,
/// minimal UI).
#[derive(Debug, Clone)]
pub struct ForkConfig {
    pub version: u32,
    pub role: Role,
    pub auth_mode: AuthMode,
    /// Gates the Support button (local UI) and, via [`apply`], the remote's acceptance of
    /// `VIEW_CAMERA`/Voice Call connections (existing upstream `enable-camera` permission).
    pub support_enabled: bool,
    /// Gates the Desktop button (local UI only — no existing upstream permission rejects
    /// `DEFAULT_CONN` outright, so this cannot be enforced remotely; see `docs/FORK_PROFILE_SPEC.md`).
    pub desktop_share_enabled: bool,
    /// Gates whether the Settings UI entry point is reachable at all. Defaults to `true` if the
    /// `direct-ip-show-setup-ui` key is absent. See `docs/GUI_CONFIGURATION_CONTROL.md`.
    pub show_setup_ui: bool,
    // Parsed and validated now so the schema is stable across phases; not read by any caller
    // yet. Each will lose this `allow` when its owning phase wires it up: Direct-IP transport
    // (listen_address, listen_port), Media (video_quality, audio_quality), minimal UI (log_level).
    #[allow(dead_code)]
    pub listen_address: String,
    #[allow(dead_code)]
    pub listen_port: u16,
    #[allow(dead_code)]
    pub video_quality: Quality,
    #[allow(dead_code)]
    pub audio_quality: Quality,
    #[allow(dead_code)]
    pub log_level: LogLevel,
}

/// Raw, unvalidated values looked up from `Config2.options`. Every field is optional at this
/// layer (`None` means "key absent") so that a missing/invalid value is reported explicitly
/// during validation, rather than silently substituted — except `show_setup_ui`, which is a
/// deliberate, documented exception (see module doc comment).
#[derive(Debug, Default)]
struct RawForkConfig {
    version: Option<u32>,
    role: Option<String>,
    auth_mode: Option<String>,
    support_enabled: Option<bool>,
    desktop_share_enabled: Option<bool>,
    show_setup_ui: Option<bool>,
    listen_address: Option<String>,
    listen_port: Option<u16>,
    video_quality: Option<String>,
    audio_quality: Option<String>,
    log_level: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ConfigError {
    UnsupportedVersion(u32),
    MissingField(&'static str),
    InvalidValue {
        field: &'static str,
        value: String,
    },
    /// Neither `support_enabled` nor `desktop_share_enabled` is true — no button would ever be
    /// shown, so the configuration is rejected outright rather than silently producing a
    /// connection screen with nothing on it.
    NoConnectionModeEnabled,
    /// No `direct-ip-*` keys are present at all — treated the same as "no config supplied" by
    /// the caller, not a hard error (see `load_and_apply()`).
    NotConfigured,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::UnsupportedVersion(v) => write!(
                f,
                "unsupported direct-ip config version {v} (supported: {SUPPORTED_CONFIG_VERSION})"
            ),
            ConfigError::MissingField(field) => write!(f, "missing required option '{field}'"),
            ConfigError::InvalidValue { field, value } => {
                write!(f, "invalid value for '{field}': '{value}'")
            }
            ConfigError::NoConnectionModeEnabled => write!(
                f,
                "at least one of 'direct-ip-support-enabled' or 'direct-ip-desktop-share-enabled' must be true"
            ),
            ConfigError::NotConfigured => write!(f, "no direct-ip-* options configured"),
        }
    }
}

fn parse_role(s: &str) -> Result<Role, ConfigError> {
    match s {
        "local" => Ok(Role::Local),
        "remote" => Ok(Role::Remote),
        _ => Err(ConfigError::InvalidValue {
            field: "direct-ip-role",
            value: s.to_owned(),
        }),
    }
}

fn parse_auth_mode(s: &str) -> Result<AuthMode, ConfigError> {
    match s {
        "ask" => Ok(AuthMode::Ask),
        "password" => Ok(AuthMode::Password),
        "ask_and_password" => Ok(AuthMode::AskAndPassword),
        _ => Err(ConfigError::InvalidValue {
            field: "direct-ip-auth-mode",
            value: s.to_owned(),
        }),
    }
}

fn parse_quality(field: &'static str, s: &str) -> Result<Quality, ConfigError> {
    match s {
        "low" => Ok(Quality::Low),
        "medium" => Ok(Quality::Medium),
        "high" => Ok(Quality::High),
        _ => Err(ConfigError::InvalidValue {
            field,
            value: s.to_owned(),
        }),
    }
}

fn parse_log_level(s: &str) -> Result<LogLevel, ConfigError> {
    match s {
        "error" => Ok(LogLevel::Error),
        "warn" => Ok(LogLevel::Warn),
        "info" => Ok(LogLevel::Info),
        "debug" => Ok(LogLevel::Debug),
        "trace" => Ok(LogLevel::Trace),
        _ => Err(ConfigError::InvalidValue {
            field: "direct-ip-log-level",
            value: s.to_owned(),
        }),
    }
}

fn validate_listen_address(s: &str) -> Result<(), ConfigError> {
    if s.parse::<std::net::IpAddr>().is_ok() {
        Ok(())
    } else {
        Err(ConfigError::InvalidValue {
            field: "direct-ip-listen-address",
            value: s.to_owned(),
        })
    }
}

/// Validate raw, looked-up option values into a [`ForkConfig`]. Every field is required except
/// `show_setup_ui` — there are no other implicit defaults at this layer (deployments must state
/// their configuration explicitly). Returns the first validation error encountered.
fn validate(raw: RawForkConfig) -> Result<ForkConfig, ConfigError> {
    let version = raw.version.ok_or(ConfigError::MissingField(keys::CONFIG_VERSION))?;
    if version != SUPPORTED_CONFIG_VERSION {
        return Err(ConfigError::UnsupportedVersion(version));
    }

    let role = raw.role.ok_or(ConfigError::MissingField(keys::ROLE))?;
    let role = parse_role(&role)?;

    let mode = raw
        .auth_mode
        .ok_or(ConfigError::MissingField(keys::AUTH_MODE))?;
    let auth_mode = parse_auth_mode(&mode)?;

    let support_enabled = raw
        .support_enabled
        .ok_or(ConfigError::MissingField(keys::SUPPORT_ENABLED))?;
    let desktop_share_enabled = raw
        .desktop_share_enabled
        .ok_or(ConfigError::MissingField(keys::DESKTOP_SHARE_ENABLED))?;
    if !support_enabled && !desktop_share_enabled {
        return Err(ConfigError::NoConnectionModeEnabled);
    }

    // Documented exception: defaults to true (shown) rather than erroring when absent.
    let show_setup_ui = raw.show_setup_ui.unwrap_or(true);

    let listen_address = raw
        .listen_address
        .ok_or(ConfigError::MissingField(keys::LISTEN_ADDRESS))?;
    validate_listen_address(&listen_address)?;

    let listen_port = raw
        .listen_port
        .ok_or(ConfigError::MissingField(keys::LISTEN_PORT))?;
    if listen_port == 0 {
        return Err(ConfigError::InvalidValue {
            field: keys::LISTEN_PORT,
            value: "0".to_owned(),
        });
    }

    let video_quality = raw
        .video_quality
        .ok_or(ConfigError::MissingField(keys::VIDEO_QUALITY))?;
    let video_quality = parse_quality(keys::VIDEO_QUALITY, &video_quality)?;

    let audio_quality = raw
        .audio_quality
        .ok_or(ConfigError::MissingField(keys::AUDIO_QUALITY))?;
    let audio_quality = parse_quality(keys::AUDIO_QUALITY, &audio_quality)?;

    let log_level = raw
        .log_level
        .ok_or(ConfigError::MissingField(keys::LOG_LEVEL))?;
    let log_level = parse_log_level(&log_level)?;

    Ok(ForkConfig {
        version,
        role,
        auth_mode,
        support_enabled,
        desktop_share_enabled,
        show_setup_ui,
        listen_address,
        listen_port,
        video_quality,
        audio_quality,
        log_level,
    })
}

fn bool_from_yn(s: &str) -> Option<bool> {
    match s {
        "Y" => Some(true),
        "N" => Some(false),
        _ => None,
    }
}

/// Look up every `direct-ip-*` key from `Config2.options` (via `Config::get_option`, which
/// already applies upstream's own `OVERWRITE_SETTINGS` > `Config2.options` > `DEFAULT_SETTINGS`
/// resolution) and build a [`RawForkConfig`]. `Config::get_option` returns `""` for a fully
/// absent key, which every field below treats as `None` — none of the valid values for any
/// field is ever the empty string.
fn read_raw_from_config2() -> RawForkConfig {
    let get = |k: &str| -> Option<String> {
        let v = Config::get_option(k);
        if v.is_empty() {
            None
        } else {
            Some(v)
        }
    };

    RawForkConfig {
        version: get(keys::CONFIG_VERSION).and_then(|v| v.parse::<u32>().ok()),
        role: get(keys::ROLE),
        auth_mode: get(keys::AUTH_MODE),
        support_enabled: get(keys::SUPPORT_ENABLED).and_then(|v| bool_from_yn(&v)),
        desktop_share_enabled: get(keys::DESKTOP_SHARE_ENABLED).and_then(|v| bool_from_yn(&v)),
        show_setup_ui: get(keys::SHOW_SETUP_UI).and_then(|v| bool_from_yn(&v)),
        listen_address: get(keys::LISTEN_ADDRESS),
        listen_port: get(keys::LISTEN_PORT).and_then(|v| v.parse::<u16>().ok()),
        video_quality: get(keys::VIDEO_QUALITY),
        audio_quality: get(keys::AUDIO_QUALITY),
        log_level: get(keys::LOG_LEVEL),
    }
}

/// Translate a validated [`ForkConfig`] into upstream RustDesk's existing, unmodified
/// role/authentication mechanisms. Does not touch `src/server/connection.rs`,
/// `src/rendezvous_mediator.rs`, `src/client.rs`, or any password/encryption code — only sets
/// values those already read.
pub fn apply(config: &ForkConfig) {
    let conn_type = match config.role {
        Role::Local => "outgoing",
        Role::Remote => "incoming",
    };
    HARD_SETTINGS
        .write()
        .unwrap()
        .insert("conn-type".to_owned(), conn_type.to_owned());

    let approve_mode = match config.auth_mode {
        AuthMode::Ask => "click",
        AuthMode::Password => "password",
        // Empty string clears/omits the option; `approve_mode()` then falls through to its own
        // `ApproveMode::Both` default. See `libs/hbb_common/src/password_security.rs:77-86`.
        AuthMode::AskAndPassword => "",
    };
    Config::set_option("approve-mode".to_owned(), approve_mode.to_owned());

    // Reuses the existing upstream `enable-camera` permission (read at login time by
    // `src/server/connection.rs:2544-2551`) so the remote side rejects VIEW_CAMERA — and
    // therefore Voice Call, which rides on it — when support_enabled is false.
    Config::set_option(
        "enable-camera".to_owned(),
        if config.support_enabled { "Y" } else { "N" }.to_owned(),
    );

    // No existing upstream permission rejects DEFAULT_CONN outright, so desktop_share_enabled
    // has no remote-side enforcement (see docs/FORK_PROFILE_SPEC.md). This option exists solely
    // for the local UI to read via the existing main_get_option_sync bridge function.
    Config::set_option(
        "desktop-share-enabled".to_owned(),
        if config.desktop_share_enabled {
            "Y"
        } else {
            "N"
        }
        .to_owned(),
    );

    // Gates the Settings UI entry point (DesktopSettingPage.switch2page()). See
    // docs/GUI_CONFIGURATION_CONTROL.md.
    Config::set_option(
        "show-setup-ui".to_owned(),
        if config.show_setup_ui { "Y" } else { "N" }.to_owned(),
    );

    // Minimal UI (unconditional — a permanent product decision per docs/FORK_PROFILE_SPEC.md,
    // not a runtime toggle): hide the Account and Network (relay/rendezvous server address)
    // settings tabs by reusing the exact mechanism upstream already provides for any
    // custom-client build.
    HARD_SETTINGS
        .write()
        .unwrap()
        .insert("disable-account".to_owned(), "Y".to_owned());
    BUILTIN_SETTINGS
        .write()
        .unwrap()
        .insert("hide-network-settings".to_owned(), "Y".to_owned());

    // Direct-IP enforcement (unconditional — see docs/ADR-0003-DIRECT-IP-ENFORCEMENT.md).
    Config::set_option("enable-lan-discovery".to_owned(), "N".to_owned());

    log::info!(
        "fork_config: applied role={:?} auth_mode={:?} support_enabled={} desktop_share_enabled={} show_setup_ui={} \
         (conn-type={conn_type}, approve-mode={approve_mode:?})",
        config.role,
        config.auth_mode,
        config.support_enabled,
        config.desktop_share_enabled,
        config.show_setup_ui,
    );
}

/// Get the directory where the rustdesk executable is located.
fn get_executable_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|parent| parent.to_path_buf()))
}

/// Check if `RustDesk2.toml` exists in the executable directory (first-run detection).
/// Returns `true` if config exists, `false` if it should be created.
pub fn config_exists() -> bool {
    if let Some(exe_dir) = get_executable_dir() {
        exe_dir.join("RustDesk2.toml").exists()
    } else {
        // If we can't determine the executable directory, assume config exists
        // (fallback to normal behavior)
        true
    }
}

/// Get the full path to `RustDesk2.toml` in the executable directory.
pub fn get_config_path() -> Option<PathBuf> {
    get_executable_dir().map(|dir| dir.join("RustDesk2.toml"))
}

/// Copy a bundled sample TOML file to become `RustDesk2.toml`.
/// `sample_name` should be "local" or "remote" (without .toml extension).
/// Returns `true` on success, `false` on failure.
pub fn copy_sample_config(sample_name: &str) -> bool {
    let exe_dir = match get_executable_dir() {
        Some(d) => d,
        None => {
            log::error!("fork_config: cannot determine executable directory");
            return false;
        }
    };

    let sample_file = exe_dir.join(format!("{}.toml", sample_name));
    let target_file = exe_dir.join("RustDesk2.toml");

    if !sample_file.exists() {
        log::error!(
            "fork_config: bundled sample file not found at {}",
            sample_file.display()
        );
        return false;
    }

    match std::fs::copy(&sample_file, &target_file) {
        Ok(_) => {
            log::info!(
                "fork_config: copied {} to {}",
                sample_file.display(),
                target_file.display()
            );
            true
        }
        Err(e) => {
            log::error!(
                "fork_config: failed to copy {} to {}: {}",
                sample_file.display(),
                target_file.display(),
                e
            );
            false
        }
    }
}

/// Load, validate, and apply the fork configuration from `Config2.options` (i.e.
/// `RustDesk2.toml`'s `[options]` table — the same file/table upstream RustDesk already uses for
/// every other option). Must be called once, early in startup (`src/core_main.rs`, immediately
/// after `crate::load_custom_client()`), before the inbound listener or any outbound-connect
/// capability is reachable.
///
/// No `direct-ip-*` keys present at all is not an error: the app runs with pure upstream
/// behavior (no role restriction, upstream's own default authentication). Keys present but
/// invalid are logged loudly and fall back the same way, never leaving the app in a
/// partial/inconsistent state.
pub fn load_and_apply() {
    let raw = read_raw_from_config2();

    // Distinguish "nothing configured" (silent, expected fallback) from "configured but
    // invalid" (loud fallback) the same way the old file-based version distinguished "file not
    // found" from "file present but invalid" — using the presence of `direct-ip-role` as the
    // sentinel, since it's required in every valid configuration.
    if raw.role.is_none() {
        log::warn!(
            "fork_config: no 'direct-ip-*' options configured in RustDesk2.toml; role \
             restriction and authentication-mode mapping will not be applied (upstream default \
             behavior in effect)"
        );
        return;
    }

    match validate(raw) {
        Ok(config) => apply(&config),
        Err(e) => {
            log::error!(
                "fork_config: invalid direct-ip-* configuration: {e}; falling back to upstream \
                 default behavior"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_raw(role: &str, mode: &str) -> RawForkConfig {
        valid_raw_with_modes(role, mode, true, true)
    }

    fn valid_raw_with_modes(
        role: &str,
        mode: &str,
        support_enabled: bool,
        desktop_share_enabled: bool,
    ) -> RawForkConfig {
        RawForkConfig {
            version: Some(1),
            role: Some(role.to_owned()),
            auth_mode: Some(mode.to_owned()),
            support_enabled: Some(support_enabled),
            desktop_share_enabled: Some(desktop_share_enabled),
            show_setup_ui: None,
            listen_address: Some("0.0.0.0".to_owned()),
            listen_port: Some(21118),
            video_quality: Some("medium".to_owned()),
            audio_quality: Some("medium".to_owned()),
            log_level: Some("info".to_owned()),
        }
    }

    #[test]
    fn parses_all_role_and_mode_combinations() {
        for role in ["local", "remote"] {
            for mode in ["ask", "password", "ask_and_password"] {
                let cfg = validate(valid_raw(role, mode))
                    .unwrap_or_else(|e| panic!("role={role} mode={mode}: {e}"));
                assert_eq!(cfg.version, 1);
                assert_eq!(
                    cfg.role,
                    if role == "local" {
                        Role::Local
                    } else {
                        Role::Remote
                    }
                );
                assert_eq!(
                    cfg.auth_mode,
                    match mode {
                        "ask" => AuthMode::Ask,
                        "password" => AuthMode::Password,
                        _ => AuthMode::AskAndPassword,
                    }
                );
            }
        }
    }

    #[test]
    fn rejects_unsupported_version() {
        let mut raw = valid_raw("local", "ask");
        raw.version = Some(2);
        assert_eq!(validate(raw).unwrap_err(), ConfigError::UnsupportedVersion(2));
    }

    #[test]
    fn rejects_invalid_role() {
        let mut raw = valid_raw("local", "ask");
        raw.role = Some("sideways".to_owned());
        assert_eq!(
            validate(raw).unwrap_err(),
            ConfigError::InvalidValue {
                field: "direct-ip-role",
                value: "sideways".to_owned()
            }
        );
    }

    #[test]
    fn rejects_invalid_auth_mode() {
        let mut raw = valid_raw("local", "ask");
        raw.auth_mode = Some("maybe".to_owned());
        assert_eq!(
            validate(raw).unwrap_err(),
            ConfigError::InvalidValue {
                field: "direct-ip-auth-mode",
                value: "maybe".to_owned()
            }
        );
    }

    #[test]
    fn rejects_missing_required_field() {
        let mut raw = valid_raw("local", "ask");
        raw.auth_mode = None;
        assert_eq!(
            validate(raw).unwrap_err(),
            ConfigError::MissingField("direct-ip-auth-mode")
        );
    }

    #[test]
    fn rejects_invalid_listen_address() {
        let mut raw = valid_raw("local", "ask");
        raw.listen_address = Some("not-an-ip".to_owned());
        assert_eq!(
            validate(raw).unwrap_err(),
            ConfigError::InvalidValue {
                field: "direct-ip-listen-address",
                value: "not-an-ip".to_owned()
            }
        );
    }

    #[test]
    fn rejects_zero_listen_port() {
        let mut raw = valid_raw("local", "ask");
        raw.listen_port = Some(0);
        assert_eq!(
            validate(raw).unwrap_err(),
            ConfigError::InvalidValue {
                field: "direct-ip-listen-port",
                value: "0".to_owned()
            }
        );
    }

    #[test]
    fn rejects_invalid_quality_and_log_level() {
        let mut bad_video = valid_raw("local", "ask");
        bad_video.video_quality = Some("ultra".to_owned());
        assert_eq!(
            validate(bad_video).unwrap_err(),
            ConfigError::InvalidValue {
                field: "direct-ip-video-quality",
                value: "ultra".to_owned()
            }
        );

        let mut bad_log = valid_raw("local", "ask");
        bad_log.log_level = Some("verbose".to_owned());
        assert_eq!(
            validate(bad_log).unwrap_err(),
            ConfigError::InvalidValue {
                field: "direct-ip-log-level",
                value: "verbose".to_owned()
            }
        );
    }

    #[test]
    fn rejects_both_support_and_desktop_share_disabled() {
        let raw = valid_raw_with_modes("local", "ask", false, false);
        assert_eq!(validate(raw).unwrap_err(), ConfigError::NoConnectionModeEnabled);
    }

    #[test]
    fn accepts_either_flag_alone() {
        assert!(validate(valid_raw_with_modes("local", "ask", true, false)).is_ok());
        assert!(validate(valid_raw_with_modes("local", "ask", false, true)).is_ok());
    }

    #[test]
    fn show_setup_ui_defaults_to_true_when_absent() {
        let raw = valid_raw("local", "ask");
        assert_eq!(raw.show_setup_ui, None);
        let cfg = validate(raw).unwrap();
        assert!(cfg.show_setup_ui);
    }

    #[test]
    fn show_setup_ui_respects_explicit_false() {
        let mut raw = valid_raw("local", "ask");
        raw.show_setup_ui = Some(false);
        let cfg = validate(raw).unwrap();
        assert!(!cfg.show_setup_ui);
    }

    #[test]
    fn bool_from_yn_only_accepts_y_and_n() {
        assert_eq!(bool_from_yn("Y"), Some(true));
        assert_eq!(bool_from_yn("N"), Some(false));
        assert_eq!(bool_from_yn("yes"), None);
        assert_eq!(bool_from_yn(""), None);
    }

    // `cargo test` runs tests in parallel by default; every test below that calls `apply()`
    // mutates the same process-global `HARD_SETTINGS`/`Config` options, so without serializing
    // them, one test's `apply()` can race another's assertion. `GlobalStateGuard` holds this
    // lock for its whole lifetime (in addition to snapshotting/restoring state) so at most one
    // such test runs at a time.
    static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct GlobalStateGuard<'a> {
        _lock: std::sync::MutexGuard<'a, ()>,
        original_hard_settings: std::collections::HashMap<String, String>,
        original_builtin_settings: std::collections::HashMap<String, String>,
        original_approve_mode: String,
        original_enable_camera: String,
        original_desktop_share_enabled: String,
        original_show_setup_ui: String,
        original_enable_lan_discovery: String,
    }

    impl GlobalStateGuard<'_> {
        fn new() -> Self {
            let lock = TEST_MUTEX
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            Self {
                _lock: lock,
                original_hard_settings: HARD_SETTINGS.read().unwrap().clone(),
                original_builtin_settings: BUILTIN_SETTINGS.read().unwrap().clone(),
                original_approve_mode: Config::get_option("approve-mode"),
                original_enable_camera: Config::get_option("enable-camera"),
                original_desktop_share_enabled: Config::get_option("desktop-share-enabled"),
                original_show_setup_ui: Config::get_option("show-setup-ui"),
                original_enable_lan_discovery: Config::get_option("enable-lan-discovery"),
            }
        }
    }

    impl Drop for GlobalStateGuard<'_> {
        fn drop(&mut self) {
            *HARD_SETTINGS.write().unwrap() = self.original_hard_settings.clone();
            *BUILTIN_SETTINGS.write().unwrap() = self.original_builtin_settings.clone();
            Config::set_option("approve-mode".to_owned(), self.original_approve_mode.clone());
            Config::set_option(
                "enable-camera".to_owned(),
                self.original_enable_camera.clone(),
            );
            Config::set_option(
                "desktop-share-enabled".to_owned(),
                self.original_desktop_share_enabled.clone(),
            );
            Config::set_option(
                "show-setup-ui".to_owned(),
                self.original_show_setup_ui.clone(),
            );
            Config::set_option(
                "enable-lan-discovery".to_owned(),
                self.original_enable_lan_discovery.clone(),
            );
        }
    }

    #[test]
    fn apply_sets_outgoing_only_for_local_role() {
        let _guard = GlobalStateGuard::new();
        let cfg = validate(valid_raw("local", "ask")).unwrap();
        apply(&cfg);
        assert!(hbb_common::config::is_outgoing_only());
        assert!(!hbb_common::config::is_incoming_only());
    }

    #[test]
    fn apply_sets_incoming_only_for_remote_role() {
        let _guard = GlobalStateGuard::new();
        let cfg = validate(valid_raw("remote", "ask")).unwrap();
        apply(&cfg);
        assert!(hbb_common::config::is_incoming_only());
        assert!(!hbb_common::config::is_outgoing_only());
    }

    #[test]
    fn apply_maps_authentication_modes_to_approve_mode_option() {
        let _guard = GlobalStateGuard::new();

        let cfg = validate(valid_raw("local", "ask")).unwrap();
        apply(&cfg);
        assert_eq!(Config::get_option("approve-mode"), "click");

        let cfg = validate(valid_raw("local", "password")).unwrap();
        apply(&cfg);
        assert_eq!(Config::get_option("approve-mode"), "password");

        let cfg = validate(valid_raw("local", "ask_and_password")).unwrap();
        apply(&cfg);
        assert_eq!(Config::get_option("approve-mode"), "");
    }

    #[test]
    fn apply_maps_support_enabled_to_enable_camera_permission() {
        let _guard = GlobalStateGuard::new();

        let cfg = validate(valid_raw_with_modes("local", "ask", true, true)).unwrap();
        apply(&cfg);
        assert_eq!(Config::get_option("enable-camera"), "Y");

        let cfg = validate(valid_raw_with_modes("local", "ask", false, true)).unwrap();
        apply(&cfg);
        assert_eq!(Config::get_option("enable-camera"), "N");
    }

    #[test]
    fn apply_maps_desktop_share_enabled_to_local_option() {
        let _guard = GlobalStateGuard::new();

        let cfg = validate(valid_raw_with_modes("local", "ask", true, true)).unwrap();
        apply(&cfg);
        assert_eq!(Config::get_option("desktop-share-enabled"), "Y");

        let cfg = validate(valid_raw_with_modes("local", "ask", true, false)).unwrap();
        apply(&cfg);
        assert_eq!(Config::get_option("desktop-share-enabled"), "N");
    }

    #[test]
    fn apply_maps_show_setup_ui_to_local_option() {
        let _guard = GlobalStateGuard::new();

        let mut raw = valid_raw("local", "ask");
        raw.show_setup_ui = Some(false);
        let cfg = validate(raw).unwrap();
        apply(&cfg);
        assert_eq!(Config::get_option("show-setup-ui"), "N");

        let cfg = validate(valid_raw("local", "ask")).unwrap(); // absent -> default true
        apply(&cfg);
        assert_eq!(Config::get_option("show-setup-ui"), "Y");
    }

    #[test]
    fn apply_hides_account_network_and_lan_discovery_unconditionally() {
        let _guard = GlobalStateGuard::new();

        for role in ["local", "remote"] {
            for mode in ["ask", "password", "ask_and_password"] {
                for support in [true, false] {
                    for desktop in [true, false] {
                        if !support && !desktop {
                            continue; // invalid combination, rejected by validate()
                        }
                        let cfg =
                            validate(valid_raw_with_modes(role, mode, support, desktop)).unwrap();
                        apply(&cfg);
                        assert_eq!(
                            HARD_SETTINGS.read().unwrap().get("disable-account"),
                            Some(&"Y".to_owned())
                        );
                        assert_eq!(
                            BUILTIN_SETTINGS
                                .read()
                                .unwrap()
                                .get("hide-network-settings"),
                            Some(&"Y".to_owned())
                        );
                        assert_eq!(Config::get_option("enable-lan-discovery"), "N");
                    }
                }
            }
        }
    }
}
