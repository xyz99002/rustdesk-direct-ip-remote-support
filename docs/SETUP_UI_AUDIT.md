# Setup UI Audit — Direct-IP Fork

Source: `flutter/lib/desktop/pages/desktop_setting_page.dart` (desktop Settings dialog).
Investigation only — **no changes implemented**, per Workstream 3 scope.

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
| Security → **"Deny LAN discovery"** | `_OptionCheckBox(..., 'enable-lan-discovery', reverse: true)` | **Directly toggles the same option the fork sets unconditionally to "N" (`fork_config.rs:406`)** | **REMOVE — flagged as a real issue.** This checkbox lets a user manually re-enable LAN discovery (the ID-exposure path ADR-0003 explicitly closes). Currently nothing prevents an operator from unchecking this and undoing the fork's own Direct-IP enforcement via the UI. See "Findings Requiring Attention" below. |
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

1. **"Deny LAN discovery" checkbox can undo fork enforcement.** `fork_config.rs` sets
   `enable-lan-discovery=N` unconditionally on every valid config load, but the same underlying
   option is exposed as a live, user-toggleable checkbox in Settings → Safety → Security. Nothing
   re-applies the fork's value after the user changes it in the UI. **This should be treated as a
   priority follow-up**, separate from cosmetic cleanup — recommend either hiding this specific
   checkbox unconditionally (mirroring how Account/Network tabs are hidden) or making the option
   read-only when a fork config is active (`isOptionFixed`-style lock, a pattern already used
   elsewhere in this same file for `kOptionWhitelist` and `kOptionAccessMode`).

2. **"Enable direct IP access" toggle's effect when unchecked is unverified.** Needs a runtime
   check: does unchecking this stop the inbound listener from accepting direct-IP connections at
   all? If yes, this is a user-facing footgun in a fork where direct IP is the *only* transport.
   Recommend verifying before Workstream 3 cleanup implementation.

---

## Categorization Summary

| Category | Count (this pass) |
|---|---|
| KEEP | 16 items |
| REMOVE | 3 items (2FA, Change ID, Deny LAN discovery checkbox) |
| NEEDS REVIEW | 6 items |

This audit covers General, Safety, Display, Printer, and About tabs' major sections. Network and
Account are already fully removed by existing fork code. Mobile settings pages
(`flutter/lib/mobile/pages/settings_page.dart`) and the Server page were not covered in this pass
and should be audited separately if mobile/Android is a supported deployment target.

**No implementation performed.** This is a removal plan for review, per Workstream 3 instructions.
