# Test Matrix — Install Separator + Settings UI Work (2026-09)

**Status:** Ready for manual testing. Nothing in this matrix has been executed on a real machine —
everything below has only been verified by reading the code and (where noted) confirming it
compiles in CI. See `docs/PLAN-install-separator.md` and `docs/UPSTREAM_UPGRADE_GUIDE.md` for the
full design rationale behind each item.

**Scope:** Everything implemented in this development pass — the `APP_NAME` identity fix, the
Local-mode server/IPC removal, the two-MSI-installer build, and the Safety/Display/Network/Printer
settings-tab visibility work (`--advance-setup`/`--printer-setup`).

**Out of scope for this matrix:** Linux/macOS/Android identity separation (not started), the Fork
Peer Marker (blocked, not implemented — see `docs/DECISIONS.md`).

**How to use this**: each row has an ID you can reference back in bug reports. Test artifacts
needed: the portable exe, the Local MSI, and the Remote MSI (all built by the same CI run once
`docs/PLAN-install-separator.md` Phase 1 is CI-verified).

---

## 1. App Identity (runtime — already shipped, `53c4fb71c`)

| ID | Precondition | Steps | Expected result |
|---|---|---|---|
| AI-1 | Portable exe, no real RustDesk installed | Launch the exe | Window title reads "RustDesk-DirectIP-RemoteSupport", not "RustDesk" |
| AI-2 | Same | Check `%APPDATA%\` after launch | A `RustDesk-DirectIP-RemoteSupport\` folder exists; no new files appear under `%APPDATA%\RustDesk\` |
| AI-3 | Real RustDesk installed and its service running | Launch the portable exe | App starts normally; Settings/options reflect *this* config.toml, not stale/foreign values (this is the original bug — confirm it's gone) |
| AI-4 | Same as AI-3 | Check log file location | Logs appear under `%APPDATA%\RustDesk-DirectIP-RemoteSupport\log\`, not `%APPDATA%\RustDesk\log\` |
| AI-5 | Same as AI-3 | Check Task Manager / Resource Monitor for named pipes (or just confirm AI-3 passes) | No evidence this fork's IPC talked to the real RustDesk's service |
| AI-6 | Real RustDesk installed on the machine | Portable exe → click "Install" | Installation dialog's path field defaults to `C:\Program Files\RustDesk-DirectIP-RemoteSupport`, **not** `C:\Program Files\RustDesk` (regression test for the `get_valid_subkey()`/`IS1` bug fixed 2026-09-12) |

## 2. No Server/IPC for Local Mode (`3f4751628`)

| ID | Precondition | Steps | Expected result |
|---|---|---|---|
| L-1 | `role = "local"` in config.toml | Launch the app | Settings changes (any toggle) persist correctly across an app restart |
| L-2 | Same | Launch the app, then check Task Manager | No unexpected extra background thread/process behavior compared to before (informal — mainly confirm nothing crashes or hangs) |
| L-3 | Same | Open Settings > About tab | Fingerprint field is blank — **expected, not a bug** (documented trade-off) |
| L-4 | Same | Use the connect panel (hostname/IP + Support/Desktop) to connect to a real Remote instance | Outgoing connection still works normally (video, input, clipboard, etc.) |
| L-5 | Same | Open the tray icon (if applicable) | Tray icon behaves normally, no error/crash |
| R-1 | `role = "remote"` in config.toml | Launch the app | Unaffected by the above — server thread and IPC still start (this is the control case) |
| R-2 | Same | Connect to this instance from a Local peer | Incoming connection still works normally |

## 3. Install Separator Phase 1 — Two MSI Installers (`dff2372fc`, not yet CI-verified — see §6)

| ID | Precondition | Steps | Expected result |
|---|---|---|---|
| MSI-1 | Local MSI built | Install it | Installs to `C:\Program Files\RustDesk-DirectIP-RemoteSupport\`, not `...\RustDesk\` |
| MSI-2 | Same | Check `services.msc` after install | **No** service is registered/running |
| MSI-3 | Same | Launch the installed app | Opens straight to the connect panel — **no first-run Local/Remote picker dialog** (config.toml was pre-baked) |
| MSI-4 | Same | Check the installed `config.toml` | `role = "local"` |
| MSI-5 | Remote MSI built | Install it | Installs to the same distinct folder name as MSI-1 |
| MSI-6 | Same | Check `services.msc` | Service **is** registered and running |
| MSI-7 | Same | Launch the installed app | Opens straight to the "waiting for incoming connections" panel, no first-run dialog |
| MSI-8 | Same | Check the installed `config.toml` | `role = "remote"` |
| MSI-9 | Machine with a **real RustDesk MSI already installed** | Install the Local MSI | Install succeeds; real RustDesk's install folder/files are untouched; no shared service name collision in `services.msc`; real RustDesk still opens and works normally afterward |
| MSI-10 | Same, real RustDesk installed | Install the Remote MSI | Same checks as MSI-9 |
| MSI-11 | Local MSI installed | Uninstall it | Real RustDesk (if present) is unaffected; no leftover `rustdesk.exe --service` process for this fork |
| MSI-12 | Remote MSI installed | Uninstall it | Service is fully stopped and removed; no orphaned process (this exercises the untouched `CustomActions.cpp` `TryStopDeleteService` path — the one part of the C++ layer we deliberately didn't change) |
| MSI-13 | Older build of the Remote MSI installed | Install a newer build of the same variant over it (upgrade) | Upgrade completes cleanly; old service stopped/replaced without leaving a duplicate or orphaned process |
| MSI-14 | Either MSI | During interactive install, click "Change Folder" and pick a custom path | Install succeeds to the custom path (confirms the folder-picker still works after the `--app-name` change) |

## 4. Settings Tab Visibility

| ID | Precondition | Steps | Expected result |
|---|---|---|---|
| TAB-1 | Any role, launch normally (no flags) | Open Settings | Safety, Display, and Network tabs are **all absent** from the tab list |
| TAB-2 | `role = "remote"`, launch as `rustdesk.exe --advance-setup` | Open Settings | Safety tab **present**; Display tab still absent (role-gated) |
| TAB-3 | `role = "local"`, launch as `rustdesk.exe --advance-setup` | Open Settings | Display tab **present**; Safety tab still absent (role-gated) |
| TAB-4 | Either role, launch as `rustdesk.exe --advance-setup` | Open Settings > Network | Tab is present; "ID/Relay Server" and "Use WebSocket" rows are hidden; Proxy, TLS-fallback, Disable-UDP rows are visible |
| TAB-5 | Launch with `--advance-setup`, then relaunch without it | Reopen Settings | Safety/Display/Network are hidden again immediately — confirms no persistence to `config.toml` |
| TAB-6 | Any role, launch normally (no flags) | Open Settings | Printer tab absent (Windows) |
| TAB-7 | `role = "local"`, launch as `rustdesk.exe --printer-setup` | Open Settings > Printer | Tab present; only the "outgoing" (printer-redirect driver) section shown; "Incoming Print Jobs" section absent |
| TAB-8 | `role = "remote"`, launch as `rustdesk.exe --printer-setup` | Open Settings > Printer | Tab present; only "Incoming Print Jobs" section shown; "outgoing" section absent |
| TAB-9 | Either role, launch with **both** `--advance-setup --printer-setup` | Open Settings | Safety/Display/Network (per role) and Printer (per role) all present together — confirms the two flags don't interfere with each other |
| TAB-10 | Any role, any flags | Open Settings | Account tab always absent (unconditional, unaffected by any flag) |

## 5. Existing/Baseline Behavior (regression — confirm nothing already-working broke)

| ID | Steps | Expected result |
|---|---|---|
| BASE-1 | `role = "local"`, `auth-mode = "ask"` | Support/Desktop buttons connect; no temporary password shown anywhere in the connecting UI |
| BASE-2 | `role = "remote"`, `auth-mode = "ask"` | "Your Desktop" panel shows **no** one-time password field |
| BASE-3 | `role = "remote"`, `auth-mode = "password"` | "Your Desktop" panel **does** show a one-time password |
| BASE-4 | `role = "remote"`, `desktop-share-enabled = "N"` | Desktop button absent on the connecting Local peer's screen |
| BASE-5 | `role = "remote"`, `support-enabled = "N"` | Support button absent on the connecting Local peer's screen |
| BASE-6 | `role = "remote"`, `show-setup-ui = "N"` | Settings gear icon and "Change Password" pencil icon both fully hidden (not just unclickable) |
| BASE-7 | Any role | `enable-record-session` behaves as `"N"` (off) per the shipped sample config — confirm recording is not silently allowed |
| BASE-8 | `role = "remote"` | Monitor network traffic during idle | No outbound traffic to any rendezvous/relay server (ADR-0003) |
| BASE-9 | `role = "remote"` | Send a LAN broadcast discovery ping | No response with this instance's ID |

## 6. Build/CI Prerequisites (must pass before any of §3 can be tested)

| ID | Steps | Expected result |
|---|---|---|
| CI-1 | Trigger CI for the current `master` (push-triggered, since normal commits touch `src/`/`flutter/`, or manual `workflow_dispatch` if only `.github/**`/`docs/**` changed) | "Full Flutter CI" run completes |
| CI-2 | Check the `x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc` jobs' "Build msi" step specifically | Both produce **two** `.msi` files each (`...-local.msi`, `...-remote.msi`) without error |
| CI-3 | Check `i686-pc-windows-msvc` | May still fail at "Install Rust toolchain" — this is pre-existing, unrelated flakiness (confirmed across multiple prior commits); not a regression to chase |

---

## Notes for whoever runs this

- Sections 1-2 have already been reasoned through carefully and are lower-risk; sections 3-4 are
  the least-verified (no WiX/MSBuild/install environment was available during implementation) and
  deserve the most attention, especially MSI-9/MSI-10/MSI-12/MSI-13 (the real-RustDesk-coexistence
  and upgrade/uninstall cases — these are exactly the scenarios that could cause the worst outcomes
  if something's wrong).
- If any MSI-series test fails, check `docs/PLAN-install-separator.md` Phase 1 and
  `docs/UPSTREAM_UPGRADE_GUIDE.md`'s "App Identity (MSI)" hook point for the exact mechanism before
  guessing at a fix — the `--app-name`/`--conn-type` wiring is subtle enough that a symptom in one
  area (e.g. service not created) usually traces back to one specific flag not being threaded
  through correctly.
- Please report back pass/fail per ID rather than a general "it worked" — several of these are
  easy to eyeball-pass without actually confirming the specific thing being tested (e.g. TAB-5's
  "no persistence" is easy to miss if you don't specifically relaunch without the flag).
