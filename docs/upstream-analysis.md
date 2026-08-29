# Phase 2 — Upstream Analysis

Scope: `PROMPTS/PHASE_02_UPSTREAM_ANALYSIS.md` — "Clone official RustDesk. Record tag/commit. Analyze networking, media, auth, packaging, licensing."

## Baseline

- Source: `https://github.com/rustdesk/rustdesk.git`
- Tag: `1.4.9`
- Commit: `6c578292e8ebbbec708b76986ba8c4bc7c509747` (2026-07-06)
- Submodule `libs/hbb_common`: `https://github.com/rustdesk/hbb_common`, pinned at `7e1c392c62d39c364127307cd408421dd5f8cfb0`
- Both are checked out locally: `main` holds this exact baseline (merged with our Phase 1 planning docs); `feature/direct-ip-fork` (current branch) is where all subsequent modifications will happen. Neither branch has been modified yet beyond the merge itself and a `.gitignore` fix.

The findings below come from reading the actual checked-out source (not from general/training knowledge), and they **correct** several assumptions in the Phase 1 architecture proposal.

## 1. Networking — direct-IP path already exists, but isn't authenticated the way rendezvous connections are

`Client::_start` (`src/client.rs:238`) branches before touching any rendezvous server:
- `hbb_common::is_ip_str(peer)` → `connect_tcp_local(...)`, returns immediately (`src/client.rs:259-271`).
- `hbb_common::is_domain_port_str(peer)` (i.e. `host:port`) → `connect_tcp_local(...)`, returns immediately (`src/client.rs:274-286`).
- Only otherwise does it fall through to `get_rendezvous_server` / UDP-TCP hole-punching / relay-request logic (`src/client.rs:288+`, `src/rendezvous_mediator.rs`).

So a "connect directly by IP" mode is not something we need to build from scratch — it already exists as a fallback path. However:

- The peer-to-peer end-to-end key exchange (`Client::secure_connection`, `src/client.rs:760-836`) authenticates the *peer's identity* by verifying `signed_id_pk` against the rendezvous server's public key (`decode_id_pk(&signed_id_pk, &rs_pk)`, line 775). On the direct-IP path, no `signed_id_pk` is ever obtained (there's no rendezvous round-trip), so this identity check never happens.
- `src/client/io_loop.rs:187-193` normally warns the user when a connection isn't secured (`!is_secured`) — but `is_direct_ip_access` (`src/common.rs:2622-2624`) explicitly **exempts** direct-IP connections from that warning. In other words, upstream already treats direct-IP as "expected to be unauthenticated at the identity layer" and silently accepts that.
- Separately, `secure_tcp`/`secure_tcp_impl` (`src/common.rs:1938-1990`) only encrypts the client↔rendezvous-server control link, not the peer data channel — irrelevant to the direct-IP case.

**Consequence for our fork:** we cannot simply "keep the existing direct-IP path as-is" and call it authenticated. The actual authentication boundary we need is the password challenge-response in `src/server/connection.rs` (see §3), which is independent of rendezvous — that's what "remote accepts authenticated direct-IP only" must be built on, not the rendezvous-signed identity mechanism (which requires infrastructure we're deliberately removing). The channel is still encrypted (libsodium box/sign key exchange happens regardless), just without third-party identity attestation — acceptable for this fork's threat model (a user manually entering a trusted IP), but worth stating explicitly in the security docs rather than leaving implicit.

## 2. No embedded rendezvous/relay server in this repo

Root `Cargo.toml` workspace `members` (line 204) lists only `libs/scrap, hbb_common, enigo, clipboard, virtual_display, virtual_display/dylib, portable, remote_printer` — no `hbbs`/`hbbr` crate. `hbbs`/`hbbr` live in the separate `rustdesk/rustdesk-server` repository; this repo only contains the *client-side* mediator that talks to that external server (`src/rendezvous_mediator.rs`, confirmed by `AGENTS.md:18` and multiple localized READMEs).

**Consequence:** "no relay/rendezvous" for this fork is a client-side change only — removing/bypassing `rendezvous_mediator.rs`'s discovery logic and the fallback branch in `client.rs`. There is no server crate to exclude from the build; we were never going to build/package `rustdesk-server` in the first place, so this scope is smaller than Phase 1 assumed.

## 3. Authentication — no mandatory password today; this is the main gap to close

`src/server/connection.rs`:
- Password is verified via challenge-response, not plaintext-over-wire: `validate_password_plain` (line 2207) computes `SHA256(password + salt)`; `verify_h1` (line 2196) computes `SHA256(h1 + challenge)` and compares with `constant_time_eq`. Reasonable as-is.
- On-disk storage (`decode_permanent_password_h1_from_storage`, from `hbb_common`) supports a hashed format **with a legacy plaintext fallback** — needs to be closed off (hashed-only) for this fork.
- **`ApproveMode::Click` requires no password at all** — an incoming connection just triggers a manual-accept prompt via `try_start_cm` (line 2676-2693).
- **Even in password mode, an empty local password is accepted** into the manual-approval flow (`lr.password.is_empty()` branch, lines 2705-2721).
- There is no first-run "you must set a password before the listener works" gate anywhere in this flow.

Brute-force protection exists but is narrower than it looks: a per-IP/IPv6-prefix lockout (`LOGIN_FAILURES`, lines 77 & 3969-4110, 30-attempt threshold) covers the default password path. A second mechanism (`src/server/login_failure_check.rs`, `FailureScope::TerminalOsLogin`, exponential backoff 15s→30min) applies only to OS-credential/terminal logins; `FailureScope::Default` always allows (lines 113-114) — i.e. it doesn't add backoff to the ordinary remote-desktop password path beyond the 30-attempt counter. There's also a temp-password-specific counter that rotates the temporary password after 10 wrong attempts (lines 2249-2294).

**Consequence:** Phase 4/5 (Configuration/Authentication) must (a) eliminate `ApproveMode::Click` and the empty-password acceptance path, (b) add a genuine first-run "must set a password before the listener activates" gate, (c) drop the legacy plaintext-password storage fallback, and (d) consider strengthening the default-password lockout with real backoff (reusing the pattern already written for `TerminalOsLogin`, rather than inventing a new one).

## 4. Media / session model — camera and desktop are separate connection types today

`ConnType` (defined in `hbb_common`, used throughout `src/client.rs` and `src/client/io_loop.rs`) distinguishes `VIEW_CAMERA`, `DEFAULT_CONN` (desktop control), `FILE_TRANSFER`, `TERMINAL`, `PORT_FORWARD` as separate connection types (`src/client.rs:2745-2751`, `src/client/io_loop.rs:162-170`) — each is its own TCP session/handshake. Audio is not a `ConnType`; it's an orthogonal per-session permission bit (`Permission::Audio` / `OPTION_ENABLE_AUDIO`, gating `audio_enabled()` at runtime — `src/server/connection.rs:522, 2078, 2399`), independent of `privacy_mode` (screen-blanking, a desktop-only toggle at lines 327/528).

**Consequence:** the spec's "one Start Session button launches camera+audio and desktop when enabled" cannot be a single existing connection type — camera-view and desktop-control are mutually exclusive `ConnType`s in the current protocol. The lowest-risk way to satisfy the requirement (per `AGENTS.md`'s "smallest valid diff" rule) is to have the *local* controller app open two connections behind one button when desktop is enabled — a `VIEW_CAMERA` connection (with the audio permission bit set) plus a `DEFAULT_CONN` connection — rather than inventing a new combined connection type in the protocol. This is an implementation detail to finalize in Phase 6 (Direct-IP transport) once the connection-establishment code is being touched anyway.

## 5. Packaging & licensing

- `LICENCE` (repo root) is the standard GNU AGPL-3.0 text.
- No per-file SPDX/copyright header convention exists — sampled files (`src/main.rs`, `src/client.rs`, `src/server/connection.rs`, `src/rendezvous_mediator.rs`) start directly with code, no header block. Nothing to preserve/replicate per-file.
- The only copyright string in the tree is packaging metadata: `Cargo.toml` → `[package.metadata.winres] LegalCopyright = "Copyright © 2026 Purslane Tech Pte. Ltd. All rights reserved."` — leave this as-is (it documents upstream's copyright on the code we're building from); our modifications are a derivative work under the same AGPL-3.0 terms, to be documented explicitly in Phase 11 packaging docs.

## Updated architecture proposal

`docs/phase1-architecture-proposal.md` is superseded by these findings on the points above (transport/auth boundary, no server crate to remove, no mandatory password today, camera/desktop as separate connection types). The proposal document has not been deleted — it's kept for the record of what was assumed going in — but implementation in Phases 4-9 should follow this document, not it, wherever they disagree.
