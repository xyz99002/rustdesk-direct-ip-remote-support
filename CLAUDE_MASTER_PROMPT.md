\# RustDesk Direct-IP Fork

\## Master Implementation Specification



This document is the authoritative implementation specification for this project.



Claude Code must:



1\. Read this document completely before modifying files.

2\. Begin with environment discovery and repository inspection.

3\. Present a written implementation plan before major file changes.

4\. Ask before:

&#x20;  - administrator/root installs

&#x20;  - firewall changes

&#x20;  - service installation

&#x20;  - destructive edits

&#x20;  - global tooling changes

5\. Maintain CHANGELOG\_IMPLEMENTATION.md.

6\. Stop on failed tests.

7\. Preserve upstream license notices.

8\. Use the smallest maintainable fork possible.

9\. Reuse upstream RustDesk functionality whenever practical.

10\. Report exact commands executed and results.



\---



\# Product Overview



Build two executables:



\## Local Client



Purpose:



\- Initiates direct-IP sessions only.

\- Never accepts inbound sessions.

\- Never exposes listening controls.

\- Presents a simplified connection workflow.



UI:



```text

\[ Hostname / IP Address ] \[ Start Session ]

```



No:



\- Public IDs

\- Relay controls

\- Rendezvous controls

\- Account controls

\- Discovery controls



\---



\## Remote Client



Purpose:



\- Accepts inbound direct-IP sessions.

\- Never initiates outbound sessions.

\- Presents authentication and status information only.



Expose:



\- Authentication mode

\- Password management

\- Permission status

\- Connection status



Do not expose:



\- Outbound connection controls

\- Public IDs

\- Relay configuration

\- Rendezvous configuration

\- Discovery configuration

\- Account workflows



\---



\# Upstream Baseline



RustDesk Release:



```text

1.4.9

```



Commit:



```text

6c578292e

```



Claude Code must verify the selected upstream revision before implementation.



Reuse upstream:



\- Direct-IP transport

\- Authentication

\- Encryption

\- Session establishment

\- Camera support

\- Audio support

\- Desktop support



Avoid redesigning upstream functionality unless required.



\---



\# Connectivity Model



The product is direct-IP only from the user's perspective.



Users connect only by entering:



```text

IP Address

or

Hostname

```



Direct-IP-only behavior is achieved through:



\- UI restrictions

\- Product workflow restrictions

\- Configuration restrictions



NOT through:



\- transport rewrites

\- custom protocols

\- networking redesign



The following upstream components may remain if required by shared dependencies:



\- rendezvous\_mediator

\- relay infrastructure

\- account infrastructure



However they must not appear in the user experience.



\---



\# Authentication



Preserve upstream RustDesk authentication behavior.



Do NOT redesign:



\- password handling

\- challenge-response logic

\- credential storage

\- identity management

\- transport security



Do NOT implement:



\- mandatory first-run password creation

\- custom password policies

\- password complexity rules

\- custom lockout systems

\- custom authentication mechanisms



Supported authentication modes:



```text

ask

password

ask\_and\_password

```



These must map directly to validated upstream RustDesk approval modes.



Example (illustrative; actual implementation uses TOML — see `docs/architecture.md`):



```toml

[authentication]

mode = "ask"

```



Allowed values:



```text

ask

password

ask\_and\_password

```



\---



\# Session Startup



The Local Client exposes one button:



```text

Start Session

```



When pressed:



Always launch:



\- Camera session

\- Two-way audio session



When enabled:



Examples are illustrative; actual implementation uses TOML.



```toml

desktop_enabled = true

```



also launch:



\- Desktop session



Multiple RustDesk session types may be created internally.



The user should experience a single action.



\---



\# Configuration



All behavior must be configuration-driven.



Provide:



Examples are illustrative; actual implementation uses TOML.



```toml

version = 1

role = "local"



camera_enabled = true

audio_enabled = true

desktop_enabled = false



listen_address = "0.0.0.0"

listen_port = 21118



video_quality = "medium"

audio_quality = "medium"



log_level = "info"



[authentication]

mode = "ask"

```

Note: `[authentication]` must be the last section in the file — in TOML, every `key = value` line after a `[table]` header belongs to that table, not the top level. This ordering was verified by `src/fork_config.rs`'s own test suite.



Documentation must describe:



\- type

\- default

\- allowed values

\- validation

\- restart requirements



Do NOT include:



```toml

direct_ip_only = true

```



because direct-IP is an architectural constraint, not a runtime mode.



\---



\# Roles



\## Local Role



Must:



\- initiate sessions

\- provide hostname/IP input

\- provide Start Session button



Must not:



\- accept sessions

\- expose listening controls

\- expose inbound connection workflows



\---



\## Remote Role



Must:



\- accept inbound sessions

\- expose authentication settings

\- expose password management

\- expose status



Must not:



\- initiate sessions

\- expose outbound connection workflows



\---



\# Upstream Investigation Requirements



Before implementation:



Create:



```text

docs/upstream-analysis.md

```



Document:



\- Exact RustDesk revision

\- Relevant modules

\- Authentication paths

\- Direct-IP paths

\- Camera implementation

\- Audio implementation

\- Desktop implementation

\- Licensing considerations

\- Packaging considerations



Do not make assumptions when upstream source can be inspected.



\---



\# Architecture Documentation



Create:



```text

docs/architecture.md

```



Include:



\## Component Mapping



\- Direct-IP transport

\- Authentication

\- Camera

\- Audio

\- Desktop

\- Configuration



For each:



\- Source modules

\- Reused paths

\- Required changes

\- Risks



\---



\# Implementation Phases



\## Phase 1



Environment Discovery



Deliver:



```text

docs/environment.md

```



Document:



\- OS

\- Architecture

\- Rust toolchain

\- Flutter toolchain

\- Build dependencies



\---



\## Phase 2



Upstream Analysis



Deliver:



```text

docs/upstream-analysis.md

```



\---



\## Phase 3



Configuration



Implement:



\- Versioned configuration

\- Validation

\- Sample configuration generation



Create tests.



\---



\## Phase 4



Role Restriction



Implement:



\### Local



Outbound-only behavior.



\### Remote



Inbound-only behavior.



Create tests validating role enforcement.



\---



\## Phase 5



Authentication Mapping



Implement configuration support for:



```text

ask

password

ask\_and\_password

```



Map directly to upstream RustDesk approval modes.



No authentication redesign.



Create tests.



\---



\## Phase 6



Session Orchestration



Implement:



\- Camera startup

\- Audio startup

\- Optional desktop startup



from one button press.



Create integration tests.



\---



\## Phase 7



Minimal UI



Replace user experience with:



\### Local



```text

\[ Host/IP ] \[ Start Session ]

```



\### Remote



```text

Authentication

Password Settings

Status

Permissions

```



Remove from the user experience:



\- Public IDs

\- Relay configuration

\- Rendezvous configuration

\- Discovery

\- Accounts



\---



\## Phase 8



Packaging



Produce:



```text

local-client

remote-client

```



and documentation:



```text

docs/build.md

docs/deployment.md

docs/security.md

docs/troubleshooting.md

docs/license-review.md

```



\---



\# Quality Gates



After each phase:



Run:



```bash

cargo fmt

cargo clippy

```



Run:



\- unit tests

\- integration tests

\- clean builds



If tests fail:



STOP.



Provide:



\- root cause

\- affected files

\- remediation plan



Do not continue.



\---



\# Acceptance Criteria



Verify:



✅ Local initiates sessions.



✅ Local cannot accept sessions.



✅ Remote accepts sessions.



✅ Remote cannot initiate sessions.



✅ User enters only hostname/IP.



✅ Authentication modes function:



\- ask

\- password

\- ask\_and\_password



✅ Start Session launches:



\- Camera

\- Audio



✅ Desktop launches when (illustrative; actual implementation uses TOML):



```toml

desktop_enabled = true

```



✅ Public IDs are absent from user experience.



✅ Relay settings are absent from user experience.



✅ Rendezvous settings are absent from user experience.



✅ Account workflows are absent from user experience.



✅ Configuration controls all supported behavior.



✅ Builds remain compatible with the selected upstream revision.



\---



\# Final Deliverables



Provide:



\## Artifacts



Paths to:



\- executables

\- configs

\- tests

\- documentation



\## Upstream Revision



Exact:



\- release

\- tag

\- commit



\## Risks



Remaining known limitations.



\## Requirements Traceability Matrix



Requirement → Test mapping for all implemented functionality.



\---



\# Start Here



1\. Inspect repository state.

2\. Verify environment.

3\. Verify upstream RustDesk revision.

4\. Present implementation plan.

5\. Do not edit source files before the plan is approved.

