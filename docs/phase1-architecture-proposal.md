# Phase 1 — Architecture Proposal (pre-implementation, subject to revision after Phase 2)

This proposal targets the acceptance criteria in `CLAUDE_MASTER_PROMPT.md`:

> local initiates only; remote accepts authenticated direct-IP only; no relay/rendezvous; mandatory first-run password; one Start Session button launches camera+audio and desktop when enabled.

It is based on general public knowledge of RustDesk's architecture (Rust core + Flutter UI, `hbbs`/`hbbc` rendezvous/relay servers, libsodium-based encrypted transport, vpx/av1 video, opus audio). **It has not yet been checked against the actual cloned source** — that verification is the explicit purpose of Phase 2 (`docs/upstream-analysis.md`), and this proposal will be corrected there if any assumption is wrong. No source exists locally yet to verify against.

## A. Networking / transport

- Drop the rendezvous-server (`hbbs`) discovery path and the relay-server (`hbbr`) fallback path from the client's connection logic entirely. Those two binaries are not built or packaged for this fork.
- **Local (controlling) role:** initiates an outbound TCP connection directly to a user-entered `host:port`. No ID lookup, no NAT punch-through via a third party.
- **Remote (controlled) role:** listens on a fixed local port for inbound direct connections only; performs the mandatory authentication handshake before accepting a session; refuses to accept any session if no password has been configured yet (see Authentication below).
- RustDesk's existing end-to-end encrypted channel (session-key exchange over the raw socket, independent of the rendezvous server for the actual data channel) is expected to be reusable as-is for the direct-IP link — the rendezvous server in upstream RustDesk is primarily used for peer discovery/NAT traversal, not for the encryption itself. Phase 2 must confirm the key exchange has no hidden dependency on the rendezvous server (e.g. for initial key/id exchange) before this is finalized.

## B. Authentication

- First-run gate: if the remote (controlled) side has no password configured, the app forces a "set a password" step and the listener stays disabled/refuses connections until one is set. This is a hard precondition, not a skippable prompt.
- Reuse and harden RustDesk's existing password storage rather than invent a new mechanism — confirm in Phase 2 whether it's already salted+hashed at rest or needs strengthening.
- Expose a password-change flow in the remote-side settings UI.
- Because removing the rendezvous server also removes whatever coarse abuse-prevention it provided, the direct listener needs its own brute-force protection (rate limiting / lockout after repeated failures) — flagged as a hard requirement for the Authentication phase, not optional hardening.

## C. Media

- Camera streaming and two-way audio reuse RustDesk's existing capture/encode/decode pipeline, transported over the direct-IP channel instead of via relay.
- Desktop sharing becomes an explicit, off-by-default configuration toggle on the remote side rather than a separate connection type.

## D. UI

- **Local (controller) app:** a hostname/IP field and a single "Start Session" button. No server list, no ID/relay concepts, no address book.
- **Remote (controlled) app:** connection status, permission prompts (camera/mic/screen as applicable), and the password-change setting. No ID display, no relay/rendezvous configuration screens.
- One "Start Session" action on the controller side always requests camera+audio, and additionally requests desktop sharing if the remote has that toggle enabled — a single button, not a menu of session types.

## E. Removed subsystems

- `hbbs` (ID/rendezvous server) and `hbbr` (relay server) crates/binaries — excluded from the build target and from packaging.
- NAT traversal / hole-punching logic that depends on the rendezvous server.

## Open items to re-verify in Phase 2

1. Does the initial key/session handshake depend on the rendezvous server in any way, or is it purely peer-to-peer already?
2. Exact current password storage mechanism and whether it meets "secure storage" as worded in the master spec.
3. How cleanly `hbbs`/`hbbr` are separated in the Cargo workspace (own crates vs. intertwined modules) — determines whether exclusion is a build-target change or requires source edits.
4. Target platform(s) for this fork — see Risk Assessment, open question.
