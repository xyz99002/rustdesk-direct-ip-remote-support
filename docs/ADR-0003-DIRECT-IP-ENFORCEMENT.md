# ADR-0003: Direct-IP Enforcement — Removing Rendezvous Registration, Relay Participation, and LAN Discovery

**Status:** Accepted, implemented 2026-08-29
**Supersedes/refines:** the "operational assumption, not a code-level guarantee" caveat recorded against `role=remote` in `docs/FEATURE_ENFORCEMENT_MATRIX.md` prior to this ADR.

## Context

`docs/DECISIONS.md` and `docs/FORK_PROFILE_SPEC.md` state, as a permanent product decision, that this fork is direct-IP only: no public IDs, no relay, no rendezvous, no discovery. Phase 3 (role enforcement) and the Minimal UI phase implemented this at the **config** and **UI** layers respectively — `role=local`/`role=remote` gate outbound/inbound capability via `HARD_SETTINGS["conn-type"]`, and the Dart UI never surfaces ID/relay/rendezvous/account controls.

A dedicated investigation ("Direct-IP Enforcement Analysis," prior turn) traced every rendezvous-registration, relay-registration, and public-ID path in the upstream source and found a real gap neither of those phases closed:

- `RendezvousMediator::start_all()` (`src/rendezvous_mediator.rs:123-198`, pre-ADR) ran `crate::hbbs_http::sync::start()` and a full rendezvous-registration loop **unconditionally** for `role=remote` (the `is_outgoing_only()` guard at the top of the function only ever short-circuits for `role=local`).
- `Config::get_rendezvous_servers()` (`libs/hbb_common/src/config.rs:936-961`) **never returns an empty list** — it falls through several config overrides to a hardcoded `RENDEZVOUS_SERVERS` constant (RustDesk's own public servers) as the final default.
- **Consequence:** a `role=remote` instance of this fork, run with only `fork_config.toml` and no further action, actively registered its public ID and public key with RustDesk's default public rendezvous server, and would participate in relay if that server ever forwarded a `RequestRelay` message to it.
- Separately, `src/lan.rs`'s passive LAN-discovery listener responds to broadcast `ping` messages with this host's ID/hostname/username/MAC whenever the existing upstream option `enable-lan-discovery` is not explicitly disabled — which is the default state.

This directly contradicts the product's core direct-IP-only claim at the protocol level, not just the UI level.

## Decision

1. **Remove rendezvous-server registration and relay participation** from `src/rendezvous_mediator.rs::start_all()`. Two changes, both marked inline with `--- BEGIN/END DIRECT-IP FORK ---` comment blocks referencing this ADR:
   - The `crate::hbbs_http::sync::start()` call is removed (account/sysinfo "calling home," unrelated to accounts this fork doesn't have).
   - The per-server registration loop (`for host in Config::get_rendezvous_servers() { ... RegisterPk/RegisterPeer ... }`) is replaced with `loop { sleep(1.).await; }` — an inert park, using the **exact same pattern already used** by the pre-existing `is_outgoing_only()` guard two lines above it in the same function.
2. **Disable LAN discovery's ID exposure** by setting the existing upstream option `enable-lan-discovery` to `"N"` unconditionally in `src/fork_config.rs::apply()` — zero source changes to `src/lan.rs` itself; the listener keeps running but the code path that would reply with the ID (`src/lan.rs:37-47`) is gated on this option and never fires.
3. **Preserve everything else in `start_all()` exactly as upstream implements it:** `direct_server(...)` (the direct-IP TCP listener — already an independent `tokio::spawn`ed task, structurally separable from the registration loop), LAN listening itself (only its ID-reply behavior is suppressed, via decision 2, not the listener), Linux xdesktop startup, and the AV1 codec self-test.
4. **No relay-specific hook was needed.** `handle_request_relay()`/`create_relay()` are only ever reached from `handle_resp()`, itself only reachable from the registration session established by the loop removed in decision 1 — removing registration removes relay participation as a structural consequence, not a separate fix.

## What was explicitly NOT touched

- `direct_server` and the direct-IP listener/dial paths (`src/client.rs`'s `is_ip_str`/`is_domain_port_str` handling).
- Any authentication code (`src/server/connection.rs`'s login/approval logic, `approve-mode` mapping).
- Any encryption/transport framing.
- Voice Call (`session_request_voice_call`, `VoiceCallRequest`/`VoiceCallResponse`) — entirely independent of rendezvous; it's a message-level feature on an already-established `VIEW_CAMERA`/`DEFAULT_CONN` session (see `docs/session-orchestration-analysis.md` §9-10).
- `VIEW_CAMERA` session establishment or the `enable-camera` permission mapping from `support_enabled`.
- The rendezvous-mediator functions themselves (`start_udp`, `start_tcp`, `register_pk`, `register_peer`, `handle_resp`, `create_relay`, etc.) are **left in place, unmodified, just unreachable** — deliberately not deleted. Rationale: deleting hundreds of lines of protocol-handling code would be a much larger, riskier diff for no functional benefit (dead code costs nothing at runtime and is a warning, not an error), and keeping it verbatim makes the removal trivially reversible and the diff trivially reviewable (a future maintainer sees exactly what upstream does and exactly where this fork stops calling it, rather than having to reconstruct removed logic from git history).

## Rationale

- **Matches the product's actual claim.** "Direct-IP only, no relay/rendezvous" was already stated as a hard product decision (`docs/DECISIONS.md`); this ADR closes the gap between that stated decision and what the code actually did.
- **Smallest correct hook point.** The investigation considered and rejected relying on making `Config::get_rendezvous_servers()` return an empty list (fragile — upstream's hardcoded fallback always wins over an absent config) in favor of an explicit, permanent code path removal mirroring a pattern upstream itself already uses (`is_outgoing_only()`'s early-return).
- **No new config flag.** Unlike `support_enabled`/`desktop_share_enabled` (per-deployment toggles), "no rendezvous, ever" is unconditional for this whole product — consistent with how `disable-account`/`hide-network-settings` were already made unconditional in the Minimal UI phase.
- **`enable-lan-discovery=N` is pure configuration reuse** — an existing upstream option, not new code, exactly matching this project's standing preference order (existing upstream functionality > existing upstream permissions > existing upstream configuration > new code).

## Upgrade considerations

- **`src/rendezvous_mediator.rs` is now an upgrade-sensitive file.** It was previously listed in `docs/FORK_AUTOMATION.md` under "Files Expected To Remain Stable" (grouped with "direct-IP implementation") — that listing is updated by this ADR's companion doc changes. A future upstream release that restructures `start_all()` requires re-locating the `is_outgoing_only()` guard (the anchor point) and re-applying both `--- BEGIN/END DIRECT-IP FORK ---` blocks.
- **Re-verify after every upstream upgrade:**
  1. `Config::get_rendezvous_servers()` still has a non-empty hardcoded fallback (if upstream removes it, the urgency of this ADR's fix changes, though the fix remains correct either way).
  2. `direct_server(...)` and LAN listening remain structurally independent tokio tasks, spawned before the removed loop — a refactor that couples them to the registration loop would require redesigning this fix.
  3. `enable-lan-discovery`'s semantics in `src/lan.rs` haven't changed (still gates the ID-bearing `pong` response specifically).
  4. `RendezvousMediator::restart()` (called from several UI/IPC sites to force rendezvous reconnection after a settings change) is now an inert no-op for this fork — confirm no new call site starts depending on it actually doing something.
- See `docs/UPSTREAM_UPGRADE_GUIDE.md`'s regression checklist and `docs/HOOK_POINTS.md` for the mechanical verification steps.

## Verification performed

- `rustfmt --check` on `src/rendezvous_mediator.rs`: clean (syntactically valid Rust, no formatting diff).
- Confirmed via `grep` across `src/` that `RendezvousMediator::start()`/`start_udp()`/`start_tcp()`/`register_pk()`/`register_peer()` have no callers outside the removed loop itself (i.e., no other code path could still trigger registration).
- Confirmed `RendezvousMediator::restart()`'s several call sites (`flutter_ffi.rs`, `ipc.rs`, `ui_interface.rs`) still compile-reference a valid function; its effect is now inert (harmless, not an error).
- Removed now-genuinely-unused imports (`std::sync::RwLock`, `hbb_common::futures::future::join_all`) surfaced by this change.
- `src/fork_config.rs`: 17/17 tests pass (isolated verification crate), `cargo fmt`/`clippy` clean, including a new test asserting `enable-lan-discovery` is set to `"N"` unconditionally across every valid role/mode/flag combination.
- **Not verified:** a full `cargo build`/`cargo test` of the real binary, which remains blocked by the pre-existing, unrelated vcpkg/aom/NASM issue documented in `docs/UPSTREAM_UPGRADE_GUIDE.md`'s "Known Build Environment Issue" section (tracked separately, not caused by this change).
