Requirement | Test | Status
---|---|---
Versioned configuration loading | `src/fork_config.rs::tests::parses_all_role_and_mode_combinations`, `rejects_unsupported_version`, `rejects_missing_required_field`, `rejects_malformed_toml` | Implemented (Phase 3)
Configuration validation (role, authentication.mode, listen_address, listen_port, video_quality, audio_quality, log_level) | `src/fork_config.rs::tests::rejects_invalid_role`, `rejects_invalid_auth_mode`, `rejects_invalid_listen_address`, `rejects_zero_listen_port`, `rejects_invalid_quality_and_log_level` | Implemented (Phase 3)
`role = local` -> outbound-only (upstream `is_outgoing_only()`) | `src/fork_config.rs::tests::apply_sets_outgoing_only_for_local_role` | Implemented (Phase 3)
`role = remote` -> inbound-only (upstream `is_incoming_only()`) | `src/fork_config.rs::tests::apply_sets_incoming_only_for_remote_role` | Implemented (Phase 3)
`authentication.mode` (ask/password/ask_and_password) -> upstream `approve-mode` (click/password/default) | `src/fork_config.rs::tests::apply_maps_authentication_modes_to_approve_mode_option` | Implemented (Phase 3)
Local initiates only; remote accepts sessions only | N/A yet — UI/product-level restriction (`docs/architecture.md` "Connectivity"), depends on the minimal-UI phase | Planned (later phase)
No relay/rendezvous in the user experience | N/A yet — UI-level restriction, upstream transport code untouched by design | Planned (later phase)
Camera/audio always start, desktop only when `desktop_enabled = true` | N/A yet — depends on Media and Direct-IP transport phases | Planned (later phase)
Clean build of the full `rustdesk` binary | Blocked — see `docs/UPSTREAM_UPGRADE_GUIDE.md` "Known build environment issue" (pre-existing vcpkg/aom/nasm incompatibility, unrelated to this phase's code) | Blocked (environment, not code)
