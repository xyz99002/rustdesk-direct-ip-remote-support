//! Configuration and role-restriction layer for the RustDesk direct-IP fork.
//!
//! This module owns a small, versioned TOML file (`fork_config.toml`, looked up next to the
//! executable using the same convention as the existing `load_custom_client()`/`custom.txt`
//! mechanism in `src/common.rs`) and translates it into upstream RustDesk's own, unmodified
//! mechanisms:
//!
//! - `role` -> `hbb_common::config::HARD_SETTINGS["conn-type"]` (`"outgoing"` / `"incoming"`),
//!   which upstream's own `is_incoming_only()`/`is_outgoing_only()` already gate outbound
//!   connects (`src/client.rs`) and the inbound listener (`src/rendezvous_mediator.rs`) on.
//! - `authentication.mode` -> `hbb_common::config::Config::set_option("approve-mode", ...)`,
//!   which upstream's own `password_security::approve_mode()` already reads.
//! - `support_enabled` -> `hbb_common::config::Config::set_option("enable-camera", ...)`, which
//!   upstream's own login handler (`src/server/connection.rs:2544-2551`) already reads to
//!   accept/reject `VIEW_CAMERA` (and therefore Voice Call, which rides on it) connections.
//! - Minimal UI (unconditional, not config-driven): `HARD_SETTINGS["disable-account"]` and
//!   `BUILTIN_SETTINGS["hide-network-settings"]` are set so the Flutter UI's own existing
//!   conditionals (`DesktopSettingPage.tabKeys` in `flutter/lib/desktop/pages/
//!   desktop_setting_page.dart`) hide the Account and Network (relay/rendezvous server
//!   address) settings tabs — reusing upstream's own custom-client hiding mechanism, the
//!   same one `is_disable_account()`/`get_builtin_option()` already read for any other
//!   RustDesk custom client build. No new Dart-side logic was needed for this.
//!
//! - Direct-IP enforcement (unconditional): `Config::set_option("enable-lan-discovery", "N")`
//!   closes the LAN-broadcast public-ID exposure path in `src/lan.rs` (existing upstream
//!   option). This complements the rendezvous-registration/relay-participation removal in
//!   `src/rendezvous_mediator.rs::start_all()` — search that file for "DIRECT-IP FORK". See
//!   `docs/ADR-0003-DIRECT-IP-ENFORCEMENT.md`.
//!
//! No authentication, transport, encryption, password storage, or Voice Call/VIEW_CAMERA code
//! is modified or reimplemented here. Rendezvous registration and relay participation *are*
//! removed (not modified — the client-side registration loop is deleted), by explicit,
//! documented decision — see `docs/ADR-0003-DIRECT-IP-ENFORCEMENT.md`. See also
//! `docs/architecture.md` and `docs/upstream-analysis.md`.

use hbb_common::config::{Config, BUILTIN_SETTINGS, HARD_SETTINGS};
use hbb_common::log;
use serde_derive::Deserialize;
use std::path::PathBuf;

/// The only configuration schema version understood today. A future incompatible schema
/// change must bump this and add explicit migration/rejection logic rather than silently
/// reinterpreting old files.
pub const SUPPORTED_CONFIG_VERSION: u32 = 1;

const CONFIG_FILE_NAME: &str = "fork_config.toml";

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
/// validated here (so the file format is stable and won't need a version bump later) but are
/// **not yet** wired to any behavior — that happens in the phases that own them (Direct-IP
/// transport, minimal UI).
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
    // Parsed and validated now so the file format is stable across phases; not read by any
    // caller yet. Each will lose this `allow` when its owning phase wires it up:
    // Direct-IP transport (listen_address, listen_port), Media (video_quality, audio_quality),
    // minimal UI (log_level).
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

/// Raw, unvalidated shape of `fork_config.toml`. Every field is optional at the parse layer so
/// that a missing/invalid field is reported explicitly during validation, rather than silently
/// substituted by `#[serde(default)]` or a blanket `Default` impl.
#[derive(Debug, Deserialize, Default)]
struct RawForkConfig {
    version: Option<u32>,
    role: Option<String>,
    authentication: Option<RawAuthConfig>,
    support_enabled: Option<bool>,
    desktop_share_enabled: Option<bool>,
    listen_address: Option<String>,
    listen_port: Option<u16>,
    video_quality: Option<String>,
    audio_quality: Option<String>,
    log_level: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct RawAuthConfig {
    mode: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ConfigError {
    Io(String),
    Parse(String),
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
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "failed to read fork config file: {e}"),
            ConfigError::Parse(e) => write!(f, "failed to parse fork config as TOML: {e}"),
            ConfigError::UnsupportedVersion(v) => write!(
                f,
                "unsupported fork config version {v} (supported: {SUPPORTED_CONFIG_VERSION})"
            ),
            ConfigError::MissingField(field) => write!(f, "missing required field '{field}'"),
            ConfigError::InvalidValue { field, value } => {
                write!(f, "invalid value for '{field}': '{value}'")
            }
            ConfigError::NoConnectionModeEnabled => write!(
                f,
                "at least one of 'support_enabled' or 'desktop_share_enabled' must be true"
            ),
        }
    }
}

fn parse_role(s: &str) -> Result<Role, ConfigError> {
    match s {
        "local" => Ok(Role::Local),
        "remote" => Ok(Role::Remote),
        _ => Err(ConfigError::InvalidValue {
            field: "role",
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
            field: "authentication.mode",
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
            field: "log_level",
            value: s.to_owned(),
        }),
    }
}

fn validate_listen_address(s: &str) -> Result<(), ConfigError> {
    if s.parse::<std::net::IpAddr>().is_ok() {
        Ok(())
    } else {
        Err(ConfigError::InvalidValue {
            field: "listen_address",
            value: s.to_owned(),
        })
    }
}

/// Validate a raw, parsed TOML document into a [`ForkConfig`]. Every field is required; there
/// are no implicit defaults at this layer (deployments must state their configuration
/// explicitly). Returns the first validation error encountered.
fn validate(raw: RawForkConfig) -> Result<ForkConfig, ConfigError> {
    let version = raw.version.ok_or(ConfigError::MissingField("version"))?;
    if version != SUPPORTED_CONFIG_VERSION {
        return Err(ConfigError::UnsupportedVersion(version));
    }

    let role = raw.role.ok_or(ConfigError::MissingField("role"))?;
    let role = parse_role(&role)?;

    let auth = raw
        .authentication
        .ok_or(ConfigError::MissingField("authentication"))?;
    let mode = auth
        .mode
        .ok_or(ConfigError::MissingField("authentication.mode"))?;
    let auth_mode = parse_auth_mode(&mode)?;

    let support_enabled = raw
        .support_enabled
        .ok_or(ConfigError::MissingField("support_enabled"))?;
    let desktop_share_enabled = raw
        .desktop_share_enabled
        .ok_or(ConfigError::MissingField("desktop_share_enabled"))?;
    if !support_enabled && !desktop_share_enabled {
        return Err(ConfigError::NoConnectionModeEnabled);
    }

    let listen_address = raw
        .listen_address
        .ok_or(ConfigError::MissingField("listen_address"))?;
    validate_listen_address(&listen_address)?;

    let listen_port = raw
        .listen_port
        .ok_or(ConfigError::MissingField("listen_port"))?;
    if listen_port == 0 {
        return Err(ConfigError::InvalidValue {
            field: "listen_port",
            value: "0".to_owned(),
        });
    }

    let video_quality = raw
        .video_quality
        .ok_or(ConfigError::MissingField("video_quality"))?;
    let video_quality = parse_quality("video_quality", &video_quality)?;

    let audio_quality = raw
        .audio_quality
        .ok_or(ConfigError::MissingField("audio_quality"))?;
    let audio_quality = parse_quality("audio_quality", &audio_quality)?;

    let log_level = raw
        .log_level
        .ok_or(ConfigError::MissingField("log_level"))?;
    let log_level = parse_log_level(&log_level)?;

    Ok(ForkConfig {
        version,
        role,
        auth_mode,
        support_enabled,
        desktop_share_enabled,
        listen_address,
        listen_port,
        video_quality,
        audio_quality,
        log_level,
    })
}

/// Parse and validate a fork config file's raw TOML text. Exposed separately from filesystem
/// lookup so it's directly unit-testable.
pub fn parse_str(text: &str) -> Result<ForkConfig, ConfigError> {
    let raw: RawForkConfig = toml::from_str(text).map_err(|e| ConfigError::Parse(e.to_string()))?;
    validate(raw)
}

/// Locate `fork_config.toml` using the same convention as `common::load_custom_client()`:
/// `./fork_config.toml` in debug builds, `<exe_dir>/fork_config.toml` otherwise.
fn config_path() -> Option<PathBuf> {
    #[cfg(debug_assertions)]
    {
        let debug_path = PathBuf::from(format!("./{CONFIG_FILE_NAME}"));
        if debug_path.is_file() {
            return Some(debug_path);
        }
    }
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let path = dir.join(CONFIG_FILE_NAME);
    if path.is_file() {
        Some(path)
    } else {
        None
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
    // therefore Voice Call, which rides on it — when support_enabled is false. This is
    // configuration reuse, not a new authentication code path.
    Config::set_option(
        "enable-camera".to_owned(),
        if config.support_enabled { "Y" } else { "N" }.to_owned(),
    );

    // No existing upstream permission rejects DEFAULT_CONN outright, so desktop_share_enabled
    // has no remote-side enforcement (see docs/FORK_PROFILE_SPEC.md). This option exists solely
    // for the local UI to read via the existing main_get_option_sync bridge function, mirroring
    // how the Dart side already reads other string options — no new FFI surface.
    Config::set_option(
        "desktop-share-enabled".to_owned(),
        if config.desktop_share_enabled {
            "Y"
        } else {
            "N"
        }
        .to_owned(),
    );

    // Minimal UI (unconditional — not gated on any config field, since this is a permanent
    // product decision per docs/FORK_PROFILE_SPEC.md, not a runtime toggle): hide the Account
    // and Network (relay/rendezvous server address) settings tabs by reusing the exact
    // mechanism upstream already provides for any custom-client build. Read by
    // `hbb_common::config::is_disable_account()` and `common::get_builtin_option()`
    // respectively; both are plain `pub static` maps upstream already exposes, same pattern as
    // `HARD_SETTINGS`. Note: like everything else in `apply()`, this only takes effect when a
    // valid `fork_config.toml` is present — an absent/invalid file falls back to pure upstream
    // behavior (including the Account/Network tabs being visible), consistent with
    // `load_and_apply()`'s existing fail-safe fallback documented below.
    HARD_SETTINGS
        .write()
        .unwrap()
        .insert("disable-account".to_owned(), "Y".to_owned());
    BUILTIN_SETTINGS
        .write()
        .unwrap()
        .insert("hide-network-settings".to_owned(), "Y".to_owned());

    // Direct-IP enforcement (unconditional — see docs/ADR-0003-DIRECT-IP-ENFORCEMENT.md).
    // `enable-lan-discovery` is an existing upstream option (src/lan.rs) gating whether this
    // host responds to LAN-broadcast discovery pings with its public ID/hostname/username/MAC.
    // Setting it to "N" here closes that exposure path with zero source changes — the listener
    // in `src/rendezvous_mediator.rs::start_all()` keeps running but never replies. This
    // complements, but is independent of, that same file's removal of rendezvous-server
    // registration and relay participation.
    Config::set_option("enable-lan-discovery".to_owned(), "N".to_owned());

    log::info!(
        "fork_config: applied role={:?} authentication.mode={:?} support_enabled={} desktop_share_enabled={} \
         (conn-type={conn_type}, approve-mode={approve_mode:?})",
        config.role,
        config.auth_mode,
        config.support_enabled,
        config.desktop_share_enabled,
    );
}

/// Load, validate, and apply the fork configuration. Must be called once, early in startup
/// (`src/core_main.rs`, immediately after `crate::load_custom_client()`), before the inbound
/// listener or any outbound-connect capability is reachable.
///
/// A missing config file is not an error: the app runs with pure upstream behavior (no role
/// restriction, upstream's own default authentication). A present-but-invalid file is logged
/// loudly and falls back the same way, never leaving the app in a partial/inconsistent state.
pub fn load_and_apply() {
    let Some(path) = config_path() else {
        log::warn!(
            "fork_config: '{CONFIG_FILE_NAME}' not found next to the executable; role restriction \
             and authentication-mode mapping will not be applied (upstream default behavior in effect)"
        );
        return;
    };

    let text = match std::fs::read_to_string(&path).map_err(|e| ConfigError::Io(e.to_string())) {
        Ok(text) => text,
        Err(e) => {
            log::error!(
                "fork_config: failed to read '{}': {e}; falling back to upstream default behavior",
                path.display()
            );
            return;
        }
    };

    match parse_str(&text) {
        Ok(config) => apply(&config),
        Err(e) => {
            log::error!(
                "fork_config: invalid config at '{}': {e}; falling back to upstream default behavior",
                path.display()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_toml(role: &str, mode: &str) -> String {
        valid_toml_with_modes(role, mode, true, true)
    }

    fn valid_toml_with_modes(
        role: &str,
        mode: &str,
        support_enabled: bool,
        desktop_share_enabled: bool,
    ) -> String {
        // NOTE: `[authentication]` must come LAST. In TOML, every `key = value` line after a
        // `[table]` header belongs to that table, not to the top level — putting scalar keys
        // after `[authentication]` would silently nest them under it instead of at the root.
        format!(
            r#"
version = 1
role = "{role}"

support_enabled = {support_enabled}
desktop_share_enabled = {desktop_share_enabled}

listen_address = "0.0.0.0"
listen_port = 21118

video_quality = "medium"
audio_quality = "medium"

log_level = "info"

[authentication]
mode = "{mode}"
"#
        )
    }

    #[test]
    fn parses_all_role_and_mode_combinations() {
        for role in ["local", "remote"] {
            for mode in ["ask", "password", "ask_and_password"] {
                let cfg = parse_str(&valid_toml(role, mode))
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
        let text = valid_toml("local", "ask").replace("version = 1", "version = 2");
        assert_eq!(
            parse_str(&text).unwrap_err(),
            ConfigError::UnsupportedVersion(2)
        );
    }

    #[test]
    fn rejects_invalid_role() {
        let text = valid_toml("sideways", "ask");
        assert_eq!(
            parse_str(&text).unwrap_err(),
            ConfigError::InvalidValue {
                field: "role",
                value: "sideways".to_owned()
            }
        );
    }

    #[test]
    fn rejects_invalid_auth_mode() {
        let text = valid_toml("local", "maybe");
        assert_eq!(
            parse_str(&text).unwrap_err(),
            ConfigError::InvalidValue {
                field: "authentication.mode",
                value: "maybe".to_owned()
            }
        );
    }

    #[test]
    fn rejects_missing_required_field() {
        let text = "version = 1\nrole = \"local\"\n";
        assert_eq!(
            parse_str(text).unwrap_err(),
            ConfigError::MissingField("authentication")
        );
    }

    #[test]
    fn rejects_invalid_listen_address() {
        let text = valid_toml("local", "ask").replace(
            "listen_address = \"0.0.0.0\"",
            "listen_address = \"not-an-ip\"",
        );
        assert_eq!(
            parse_str(&text).unwrap_err(),
            ConfigError::InvalidValue {
                field: "listen_address",
                value: "not-an-ip".to_owned()
            }
        );
    }

    #[test]
    fn rejects_zero_listen_port() {
        let text = valid_toml("local", "ask").replace("listen_port = 21118", "listen_port = 0");
        assert_eq!(
            parse_str(&text).unwrap_err(),
            ConfigError::InvalidValue {
                field: "listen_port",
                value: "0".to_owned()
            }
        );
    }

    #[test]
    fn rejects_invalid_quality_and_log_level() {
        let bad_video = valid_toml("local", "ask")
            .replace("video_quality = \"medium\"", "video_quality = \"ultra\"");
        assert_eq!(
            parse_str(&bad_video).unwrap_err(),
            ConfigError::InvalidValue {
                field: "video_quality",
                value: "ultra".to_owned()
            }
        );

        let bad_log =
            valid_toml("local", "ask").replace("log_level = \"info\"", "log_level = \"verbose\"");
        assert_eq!(
            parse_str(&bad_log).unwrap_err(),
            ConfigError::InvalidValue {
                field: "log_level",
                value: "verbose".to_owned()
            }
        );
    }

    #[test]
    fn rejects_both_support_and_desktop_share_disabled() {
        let text = valid_toml_with_modes("local", "ask", false, false);
        assert_eq!(
            parse_str(&text).unwrap_err(),
            ConfigError::NoConnectionModeEnabled
        );
    }

    #[test]
    fn accepts_either_flag_alone() {
        assert!(parse_str(&valid_toml_with_modes("local", "ask", true, false)).is_ok());
        assert!(parse_str(&valid_toml_with_modes("local", "ask", false, true)).is_ok());
    }

    #[test]
    fn rejects_malformed_toml() {
        let text = "this is not valid toml {{{";
        assert!(matches!(parse_str(text), Err(ConfigError::Parse(_))));
    }

    // `cargo test` runs tests in parallel by default; every test below mutates the same
    // process-global `HARD_SETTINGS`/`Config` options, so without serializing them, one test's
    // `apply()` can race another's assertion. `GlobalStateGuard` holds this lock for its whole
    // lifetime (in addition to snapshotting/restoring state) so at most one such test runs at a
    // time; `Mutex` is deliberately not `parking_lot` — a poisoned lock (from a panicking test)
    // should surface as failures in subsequent tests, not be silently ignored.
    static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Serializes access to, and restores, `HARD_SETTINGS`, `BUILTIN_SETTINGS`, and the
    /// `approve-mode`/`enable-camera`/`desktop-share-enabled` CONFIG2 options around each test
    /// that calls `apply()`, so tests don't race or leak global state into each other or into
    /// other test modules in the same process.
    struct GlobalStateGuard<'a> {
        _lock: std::sync::MutexGuard<'a, ()>,
        original_hard_settings: std::collections::HashMap<String, String>,
        original_builtin_settings: std::collections::HashMap<String, String>,
        original_approve_mode: String,
        original_enable_camera: String,
        original_desktop_share_enabled: String,
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
                original_enable_lan_discovery: Config::get_option("enable-lan-discovery"),
            }
        }
    }

    impl Drop for GlobalStateGuard<'_> {
        fn drop(&mut self) {
            *HARD_SETTINGS.write().unwrap() = self.original_hard_settings.clone();
            *BUILTIN_SETTINGS.write().unwrap() = self.original_builtin_settings.clone();
            Config::set_option(
                "approve-mode".to_owned(),
                self.original_approve_mode.clone(),
            );
            Config::set_option(
                "enable-camera".to_owned(),
                self.original_enable_camera.clone(),
            );
            Config::set_option(
                "desktop-share-enabled".to_owned(),
                self.original_desktop_share_enabled.clone(),
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
        let cfg = parse_str(&valid_toml("local", "ask")).unwrap();
        apply(&cfg);
        assert!(hbb_common::config::is_outgoing_only());
        assert!(!hbb_common::config::is_incoming_only());
    }

    #[test]
    fn apply_sets_incoming_only_for_remote_role() {
        let _guard = GlobalStateGuard::new();
        let cfg = parse_str(&valid_toml("remote", "ask")).unwrap();
        apply(&cfg);
        assert!(hbb_common::config::is_incoming_only());
        assert!(!hbb_common::config::is_outgoing_only());
    }

    #[test]
    fn apply_maps_authentication_modes_to_approve_mode_option() {
        let _guard = GlobalStateGuard::new();

        let cfg = parse_str(&valid_toml("local", "ask")).unwrap();
        apply(&cfg);
        assert_eq!(Config::get_option("approve-mode"), "click");

        let cfg = parse_str(&valid_toml("local", "password")).unwrap();
        apply(&cfg);
        assert_eq!(Config::get_option("approve-mode"), "password");

        let cfg = parse_str(&valid_toml("local", "ask_and_password")).unwrap();
        apply(&cfg);
        assert_eq!(Config::get_option("approve-mode"), "");
    }

    #[test]
    fn apply_maps_support_enabled_to_enable_camera_permission() {
        let _guard = GlobalStateGuard::new();

        let cfg = parse_str(&valid_toml_with_modes("local", "ask", true, true)).unwrap();
        apply(&cfg);
        assert_eq!(Config::get_option("enable-camera"), "Y");

        let cfg = parse_str(&valid_toml_with_modes("local", "ask", false, true)).unwrap();
        apply(&cfg);
        assert_eq!(Config::get_option("enable-camera"), "N");
    }

    #[test]
    fn apply_maps_desktop_share_enabled_to_local_option() {
        let _guard = GlobalStateGuard::new();

        let cfg = parse_str(&valid_toml_with_modes("local", "ask", true, true)).unwrap();
        apply(&cfg);
        assert_eq!(Config::get_option("desktop-share-enabled"), "Y");

        let cfg = parse_str(&valid_toml_with_modes("local", "ask", true, false)).unwrap();
        apply(&cfg);
        assert_eq!(Config::get_option("desktop-share-enabled"), "N");
    }

    #[test]
    fn apply_hides_account_network_and_lan_discovery_unconditionally() {
        let _guard = GlobalStateGuard::new();

        // Unconditional means unconditional: verify it holds for every role/mode/flag
        // combination, not just one.
        for role in ["local", "remote"] {
            for mode in ["ask", "password", "ask_and_password"] {
                for support in [true, false] {
                    for desktop in [true, false] {
                        if !support && !desktop {
                            continue; // invalid combination, rejected by validate()
                        }
                        let cfg = parse_str(&valid_toml_with_modes(role, mode, support, desktop))
                            .unwrap();
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
