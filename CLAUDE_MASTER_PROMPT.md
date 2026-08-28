# RustDesk Direct-IP Master Specification

This is the authoritative implementation specification.

Claude Code must: read completely before modifying files, perform discovery, inspect upstream RustDesk, present plan, ask before admin actions, stop on failed tests, maintain CHANGELOG_IMPLEMENTATION.md, preserve licenses, implement only direct-IP workflows.

## Phases
1. Environment discovery
2. Upstream analysis
3. Architecture design
4. Configuration
5. Authentication
6. Direct-IP transport
7. Camera & audio
8. Optional desktop
9. Minimal UI
10. Verification
11. Packaging

Acceptance criteria: local initiates only; remote accepts authenticated direct-IP only; no relay/rendezvous; mandatory first-run password; one Start Session button launches camera+audio and desktop when enabled.
