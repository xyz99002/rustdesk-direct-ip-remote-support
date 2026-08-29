# Final Product Decisions

## Connectivity

- Direct-IP only
- No public IDs
- No relay UI
- No rendezvous UI
- No account system UI
- IP/Hostname entry only

## Authentication

Preserve upstream RustDesk authentication.

Supported modes:

- ask
- password
- ask_and_password

Configured via configuration file.

No mandatory first-run password creation.
No password complexity policy.
No authentication redesign.

## Session Startup

One Start Session button.

Always starts:

- camera
- audio

Also starts:

- desktop

when desktop_enabled=true.

## Local Client

- Outbound only
- Cannot accept inbound connections

## Remote Client

- Inbound only
- Cannot initiate outbound sessions

## Upstream Base

RustDesk 1.4.9
Commit 6c578292e