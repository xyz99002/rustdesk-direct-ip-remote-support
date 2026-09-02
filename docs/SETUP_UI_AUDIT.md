# Setup UI Audit — Direct-IP Fork

Source: `flutter/lib/desktop/pages/desktop_setting_page.dart` (desktop Settings dialog) and
`flutter/lib/mobile/pages/settings_page.dart` (mobile).

**Update (2026-09-02): Implemented.** Per explicit follow-up instruction, five items were
removed from both desktop and mobile settings UIs (not just documented):

| Item | Desktop | Mobile | Reason |
|---|---|---|---|
| 2FA section | ✅ Removed | ✅ Removed | Depends on the upstream account system this fork doesn't use (`disable-account=Y`) |
| Change ID section | ✅ Removed | N/A (not present on mobile) | ID is never registered anywhere per ADR-0003 — changing it is meaningless in this fork |
| "Check for software update on startup" | ✅ Removed | ✅ Removed | Calls an external update server; only gated by `isCustomClient()`, which this fork never triggers (never renames `APP_NAME`) — was visible by default before this fix |
| "Auto update" (Windows installed builds) | ✅ Removed | N/A (Windows-only) | Same category — no fork gate existed at all before this fix |
| "Deny LAN discovery" checkbox | **Kept, by explicit decision** | — | Confirmed behavior: checked = `enable-lan-discovery=N` (matches fork default); unchecked = `Y` (re-enables discovery). This is a deliberate, understood escape hatch, not a bug — left in place |

The two update-check items were found during a follow-up sweep (prompted by "check for similar
other options" after sign-in/relay-server were confirmed already covered by existing
`disable-account`/`hide-network-settings` gates) — they were previously unflagged because they
sit outside both of those gates entirely, controlled only by `isCustomClient()` (which checks
`get_app_name() != "RustDesk"` — always `false` for this fork) or no gate at all.

The remaining audit below (KEEP/NEEDS REVIEW items) is unchanged from the original
investigation-only pass and still reflects recommendations, not implemented changes.

---

## Tab-Level Visibility (Already Enforced)

`DesktopSettingPage.tabKeys` ([desktop_setting_page.dart:66-84](../flutter/lib/desktop/pages/desktop_setting_page.dart)):

| Tab | Shown when | Fork-relevant? |
|---|---|---|
| `general` | Always | Contains mixed relevant/irrelevant settings — see below |
| `safety` | `!isOutgoingOnly() && !isDisableSettings() && hideSecuritySetting != 'Y'` | Auto-hidden for "local" role (outgoing-only) by upstream's own logic — **not fork code**, but works in our favor |
| `network` | `!isDisableSettings() && hideNetworkSetting != 'Y'` | **Already hidden** — fork sets `hide-network-settings=Y` unconditionally (`fork_config.rs:394-397`) |
| `display` | `!isIncomingOnly()` | Auto-hidden for "remote" role by upstream logic |
| `plugin` | `!isWeb && !isIncomingOnly() && pluginFeatureIsEnabled()` | Depends on plugin feature flag; not fork-controlled |
| `account` | `!isDisableAccount()` | **Already hidden** — fork sets `disable-account=Y` unconditionally (`fork_config.rs:390-393`) |
| `printer` | `isWindows && hideRemotePrinterSetting != 'Y'` | Windows-only, not fork-controlled |
| `about` | Always | Generic, low-relevance |

**Two of eight tabs are already fully removed by existing fork enforcement** (Network, Account).
The remaining audit below covers what's left inside the tabs that DO render.

---

## Per-Section Audit

### General Tab

| Section (`_Card` title) | Widget/Option | Reason it exists | Recommendation |
|---|---|---|---|
| Language | Language picker | Generic i18n, harmless | **KEEP** |
| Theme | Light/Dark/Auto | Generic UI preference | **KEEP** |
| Service | Start on boot, run as service, etc. (upstream) | Controls the RustDesk background service | **NEEDS REVIEW** — some sub-options may reference relay/rendezvous connectivity status that no longer applies |
| Other | `Confirm before closing multiple tabs`, `Adaptive bitrate` (`kOptionEnableAbr`), misc | Adaptive bitrate assumes a variable-quality relay/internet link; direct-IP LAN links may not need it | **NEEDS REVIEW** per-item |
| Hardware Codec | Encoder selection | Legitimate performance setting | **KEEP** |
| Audio Input Device | Mic selection | Directly relevant to Support Mode's Voice Call feature (Workstream 5) | **KEEP** |
| Recording | `Automatically record incoming/outgoing sessions` | Legitimate feature, no cloud dependency | **KEEP** |

### Safety Tab (auto-hidden for "local" role already)

| Section | Widget/Option | Reason it exists | Recommendation |
|---|---|---|---|
| 2FA | Two-factor auth setup | Upstream account-security feature; this fork has no account system (`disable-account=Y`) elsewhere | **REMOVE** — inconsistent with "no account" positioning; likely dead/confusing without an account |
| ID | Change ID | RustDesk ID is tied to the rendezvous-registration system this fork removes (ADR-0003) | **REMOVE** — changing an ID that's never registered anywhere is meaningless in this fork |
| Permissions | `Enable keyboard/mouse`, `clipboard`, `file transfer`, `audio`, `camera`, `terminal`, `TCP tunneling`, `remote restart`, `recording session`, `block input`, `privacy mode`, `remote configuration modification` | Legitimate per-session permission toggles for a "remote"-role host | **KEEP**, all of them — directly relevant to what a Remote-role host allows an incoming Support/Desktop session to do |
| Password | One-time / permanent / both | Directly backs `authentication.mode` | **KEEP** |
| Security → "Enable RDP session sharing" | Windows-only RDP passthrough | Niche upstream feature, no direct-IP conflict | **NEEDS REVIEW** — likely fine to keep, low usage |
| Security → **"Deny LAN discovery"** | `_OptionCheckBox(..., 'enable-lan-discovery', reverse: true)` | Toggles the same option the fork sets unconditionally to "N" (`fork_config.rs:406`) | **KEPT — explicit decision (2026-09-02).** Confirmed behavior: checked = `enable-lan-discovery=N` (matches fork default, i.e. the checkbox's default/unmodified state is already consistent with fork enforcement); unchecked = `Y`. This is an understood, deliberate escape hatch rather than a silent bug, and was kept by explicit instruction rather than removed. |
| Security → "Enable direct IP access" (`kOptionDirectServer`) | Upstream optional direct-TCP-listener toggle, originally meant to run *alongside* rendezvous/relay | Given this fork removes rendezvous/relay entirely, direct IP is now the *only* path — this toggle being off would break connectivity | **NEEDS REVIEW — potential issue.** If a user unchecks this, does the host stop listening entirely? If so, this is a second enforcement-bypass risk. Needs runtime verification (not done here — investigation only). |
| Security → "Use IP Whitelisting" | Upstream CIDR allow-list (see `CONFIG_FEATURE_VALIDATION.md` Section 3) | Legitimate, useful security control; the closest thing to a fork-relevant "IP filtering" feature | **KEEP** |
| Security → auto-disconnect, keep-awake, allow-only-conn-window-open, unlock PIN | Misc upstream session-management options | No cloud/account dependency | **KEEP** |

### Display Tab (auto-hidden for "remote" role already)

| Section | Widget/Option | Reason it exists | Recommendation |
|---|---|---|---|
| Default View Style / Scroll Style / Image Quality / trackpad speed / Codec | Per-session display defaults | Legitimate, affects the local "local"-role viewer experience | **KEEP** |
| Other Default Options | Misc | Not yet itemized in this pass | **NEEDS REVIEW** |

### Account Tab — Already Removed

Fully hidden by `disable-account=Y`. No action needed.

### Printer Tab (Windows only)

| Section | Recommendation |
|---|---|
| Outgoing/Incoming Print Jobs | **NEEDS REVIEW** — niche feature, not investigated further this pass |

### About Tab

| Section | Recommendation |
|---|---|
| About RustDesk | **KEEP** — but should be checked for any upstream branding/links inconsistent with the fork's own identity (not verified this pass) |

### Network Tab — Already Removed

Fully hidden by `hide-network-settings=Y`. No action needed.

---

## Findings Requiring Attention (Not Just Cleanup — Possible Enforcement Gaps)

1. **"Deny LAN discovery" checkbox — resolved by decision, not by removal.** Confirmed the
   checkbox's checked state matches the fork's own default (`enable-lan-discovery=N`); unchecking
   it is a known, accepted escape hatch. Kept in place per explicit instruction (2026-09-02) —
   no longer an open item.

2. **"Enable direct IP access" toggle's effect when unchecked is unverified.** Needs a runtime
   check: does unchecking this stop the inbound listener from accepting direct-IP connections at
   all? If yes, this is a user-facing footgun in a fork where direct IP is the *only* transport.
   **Still open** — not addressed in the 2026-09-02 implementation pass.

3. **Sign-in and ID/Relay Server dialog — verified already covered on both platforms (2026-09-02
   follow-up check).** `!bind.isDisableAccount()` gates login/logout on both desktop (Account tab
   removed from `tabKeys`) and mobile (`settings_page.dart:685`), and also gates the "Note"
   feature in the remote-session toolbar (`toolbar.dart:490`) — not just the Settings page.
   "ID/Relay Server" is gated by `hide-network-settings` on both platforms too
   (`settings_page.dart:715` on mobile; nested inside the already-removed `network()` tab
   function on desktop). No gap found — these depend on `fork_config.toml` being present and
   valid at runtime, same as everything else in this document.

---

## Categorization Summary

| Category | Count |
|---|---|
| KEEP | 16 items (includes "Deny LAN discovery", kept by decision) |
| **REMOVED (implemented 2026-09-02)** | **5 items: 2FA, Change ID, "Check for software update on startup", "Auto update", across desktop and mobile** |
| NEEDS REVIEW | 5 items ("Enable direct IP access" runtime verification still open; others unchanged from original pass) |

This audit covers General, Safety, Display, Printer, and About tabs' major sections on desktop,
plus a targeted mobile sweep for the five implemented removals and the sign-in/relay-server
verification above. A full, independent mobile audit (beyond these five items) has still not been
done and should be a separate pass if further mobile-specific settings need review.
