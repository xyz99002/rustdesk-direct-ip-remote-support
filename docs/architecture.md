# Architecture (Final — requirements frozen)

**Status:** Frozen 2026-08-28, per direct user instruction plus `docs/DECISIONS.md`. This document is now authoritative and **supersedes** the earlier `docs/phase1-architecture-proposal.md` draft it replaces (that content is gone; see git history if the earlier proposal is needed for reference). Read `docs/upstream-analysis.md` alongside this for the source citations the decisions below are checked against.

## Authentication — no redesign, expose existing upstream modes as-is

RustDesk 1.4.9 already implements exactly three approval modes via the `approve-mode` config option (`libs/hbb_common/src/password_security.rs:77-86`):

| Fork-facing name | Config value | Upstream enum | Behavior |
|---|---|---|---|
| `ask` | `click` | `ApproveMode::Click` | remote user manually approves each incoming session; no password required |
| `password` | `password` | `ApproveMode::Password` | session requires a valid password (temporary and/or permanent, per `verification-method`) |
| `ask_and_password` | *(default/empty)* | `ApproveMode::Both` | either a manual click-approve or a valid password is sufficient |

Decisions (frozen):

- **No authentication redesign.** No changes to `src/server/connection.rs`'s login/approval logic, password hashing/storage, or lockout/brute-force handling. Upstream behavior is preserved exactly.
- **No mandatory first-run password creation.** The app does not force a password to be set before the listener works.
- **No password complexity/policy enforcement.** Whatever upstream accepts today (including an empty password under `ask`/`ask_and_password`) continues to be accepted.
- The only in-scope work is making the existing `approve-mode` (and `verification-method`) options selectable through this fork's own configuration surface, using `ask` / `password` / `ask_and_password` as the fork's vocabulary for the three upstream values above. This is configuration wiring, not new authentication logic.

This reverses two items that were previously planned: the "mandatory first-run password gate" and "harden default-password lockout" work is now explicitly out of scope.

## Connectivity — direct-IP only, enforced at the UI/config layer, not the protocol layer

- **No transport redesign.** `src/rendezvous_mediator.rs`, `src/client.rs`'s connection paths, and all relay/rendezvous logic are preserved unchanged. Upstream transport behavior — including the rendezvous discovery and relay fallback code paths — is not removed, rewritten, or gated behind a new "direct-IP-only" enforcement flag.
- The direct-IP dial path already exists in upstream (`is_ip_str` / `is_domain_port_str` handling in `src/client.rs`, confirmed in `docs/upstream-analysis.md` §1) and is what this fork's UI exclusively drives.
- "Direct-IP only" is achieved by **not building UI or configuration surface** for anything else, per `docs/DECISIONS.md`:
  - No public ID display or ID-based connect.
  - No relay configuration UI.
  - No rendezvous-server configuration UI.
  - No account-system UI.
  - The only connect input is a hostname/IP field.
- **Local client:** outbound-only in its UI — it only ever initiates a direct-IP connection; there is no UI path for it to accept inbound sessions.
- **Remote client:** inbound-only in its UI — it only listens for and accepts incoming direct-IP sessions; there is no UI path for it to initiate an outbound session.
- Net effect: the upstream engine is capable of more than this (rendezvous, relay, bidirectional roles), but this fork's UI/config never exposes or exercises any of that — direct-IP-only is a product-level restriction, not a protocol change.

## Session startup — one button, two possible connections underneath

Per `docs/DECISIONS.md` and consistent with the `ConnType` findings in `docs/upstream-analysis.md` §4 (camera-view and desktop-control are mutually exclusive connection types upstream):

- A single **Start Session** action on the local client always starts:
  - camera (a `VIEW_CAMERA` connection)
  - audio (the `Permission::Audio` bit on that connection)
- The same action additionally starts a desktop/control connection (`DEFAULT_CONN`) when the remote side has `desktop_enabled=true` configured.
- This does not require a new combined `ConnType` in the protocol — the local client opens the camera+audio connection always, and conditionally opens a second desktop connection, both behind the one button, per the "smallest valid diff" principle in `AGENTS.md`.

## Upstream baseline

- RustDesk `1.4.9`, commit `6c578292e8ebbbec708b76986ba8c4bc7c509747` (already merged into `main`; work happens on `feature/direct-ip-fork`).

## Traceability note — conflict with `CLAUDE_MASTER_PROMPT.md`

`CLAUDE_MASTER_PROMPT.md`'s original acceptance criteria ("remote accepts authenticated direct-IP only," "no relay/rendezvous," "mandatory first-run password") reads, taken literally, as a transport/auth *code* change. The frozen decision above achieves the product-level intent (direct-IP-only experience) through UI/config curation instead, and explicitly drops the mandatory-password requirement. This document reflects the user's direct chat instruction, which takes precedence over the checked-in project file. `CLAUDE_MASTER_PROMPT.md` itself has not been edited to match this — noting the discrepancy here so it isn't lost; whether to amend that file is left to the user.
