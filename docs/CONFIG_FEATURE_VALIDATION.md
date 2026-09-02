# Configuration Feature Validation — Support/Desktop & Related Options

Source-verified investigation of where fork-specific and fork-adjacent options are read, where
they are applied, and whether they actually affect runtime behavior.

---

## Summary Table

| Option | Expected Behavior | Actual Behavior | Root Cause | Recommended Fix |
|---|---|---|---|---|
| `support_enabled` | Gates Support button locally AND blocks VIEW_CAMERA/Voice Call remotely when off | **Matches expectation** on both counts | N/A | None needed |
| `desktop_share_enabled` | Gates Desktop button locally AND blocks DEFAULT_CONN remotely when off | Gates the button locally only; **DEFAULT_CONN is NOT blocked remotely** | No upstream permission exists that rejects a bare `DEFAULT_CONN` login the way `OPTION_ENABLE_CAMERA` rejects `VIEW_CAMERA` | See Section 2 below — three options presented |
| IP allow/deny list | Assumed to be a fork-specific `allow_client_ips`-style option | **No such fork option exists.** Upstream already has an unrelated, more general `whitelist` option (CIDR list, `Config::get_option("whitelist")`) checked in `on_open()` | Upstream feature, not a fork gap | Document and reuse (Section 3) — do not build a duplicate mechanism |
| IP filtering (fork-specific) | Same as above | Same as above | Same as above | Same as above |
| `listen_address` / `listen_port` | Assumed to control the direct-IP listener bind address/port | **Confirmed not wired** (see `CONFIG_REFERENCE.md` 4.7-4.8) — the actual inbound listener bind address/port comes from upstream's own existing listener setup, untouched by this fork | Fields were added to stabilize the file schema ahead of a transport phase that was never started | Either wire them up in a scoped follow-up, or remove from the schema if no longer planned (decision needed, not a bug) |

---

## 1. `support_enabled` — Validated Working

**Read at (local UI):** `flutter/lib/desktop/pages/connection_page.dart:110`
```dart
bool get _supportEnabled => mainGetBoolOptionSync("enable-camera");
```

**Applied at (fork config layer):** `src/fork_config.rs:361-364`
```rust
Config::set_option(
    "enable-camera".to_owned(),
    if config.support_enabled { "Y" } else { "N" }.to_owned(),
);
```

**Enforced at (remote, protocol level):** `src/server/connection.rs:2544-2551`
```rust
Some(login_request::Union::ViewCamera(_vc)) => {
    if !Self::permission(keys::OPTION_ENABLE_CAMERA, &self.control_permissions) {
        self.send_login_error("No permission of viewing camera").await;
        sleep(1.).await;
        return false;
    }
    self.view_camera = true;
}
```

**Conclusion:** This is a complete, closed loop. Setting `support_enabled = false` on a
"remote"-role host both hides the Support button locally (irrelevant for a remote-only host, but
consistent) and causes any incoming `VIEW_CAMERA` login attempt to be rejected at the protocol
level, before any camera/voice-call resource is touched. **No fix needed.**

---

## 2. `desktop_share_enabled` — Local-Only, No Remote Enforcement

**Read at (local UI):** `flutter/lib/desktop/pages/connection_page.dart:114`
```dart
bool get _desktopShareEnabled => mainGetBoolOptionSync("desktop-share-enabled");
```

**Applied at (fork config layer):** `src/fork_config.rs:370-378` — sets
`Config::set_option("desktop-share-enabled", "Y"/"N")`.

**Searched for remote enforcement:** `src/server/connection.rs` login handler
(`Some(login_request::Union::...)` match arms, lines ~2530-2588) has explicit arms for
`FileTransfer`, `ViewCamera`, `Terminal`, and `PortForward` — each checks a permission
(`OPTION_ENABLE_FILE_TRANSFER`, `OPTION_ENABLE_CAMERA`, `OPTION_ENABLE_TERMINAL`,
`OPTION_ENABLE_TUNNEL` respectively) and rejects the login if unset. **There is no match arm for
a bare `DEFAULT_CONN`** — it falls into the `_ =>` catch-all (line 2583), which only runs a
privacy-mode check, not a `desktop-share-enabled` check.

**Root cause:** This is documented as a known, deliberate gap in `docs/FORK_PROFILE_SPEC.md` —
upstream RustDesk has no permission key that gates a plain remote-control (`DEFAULT_CONN`)
session the way `enable-camera` gates `VIEW_CAMERA`. Adding one would require new server-side
logic, which was explicitly out of scope for the original fork work (ADR-0003 "What was
explicitly NOT touched").

**Practical impact:** On a "remote"-role host with `desktop_share_enabled = false`, the Desktop
button is hidden in *this fork's own UI*, but a client using any other RustDesk-compatible client
(or a raw protocol client) can still open a `DEFAULT_CONN` session against that host, subject only
to whatever password/approval the host has configured. The local UI toggle is not a security
boundary.

### Recommended Fix Options (not yet implemented — decision needed)

| Option | Description | Effort | Risk |
|---|---|---|---|
| **A. Document only** | Keep current behavior, update `FORK_PROFILE_SPEC.md` to state explicitly that `desktop_share_enabled` is cosmetic-only, not a security control | None | None — but leaves the gap live |
| **B. Add a new permission check** | Add a `desktop-share-enabled` check in the `_ =>` catch-all arm of the login match in `connection.rs`, mirroring the existing `OPTION_ENABLE_CAMERA` pattern | Small, localized (~10 lines) | Touches `connection.rs`, which ADR-0003 flagged as sensitive; needs careful review to avoid breaking non-DEFAULT_CONN paths that also fall into `_ =>` |
| **C. Reuse an existing upstream permission** | Investigate whether `enable-keyboard`/`enable-clipboard`-style permissions could be repurposed, or whether upstream has added a more direct control since this fork's baseline was cut | Unknown until investigated | Unknown |

**This document does not implement any of the above — per Workstream 2 scope, investigation and
root-cause only.**

---

## 3. IP Filtering / Allow-Client-Lists — Not a Fork Gap, Upstream Already Has This

Investigated for a fork-specific "allow client list" or "IP filter" option. **None exists in
`fork_config.rs` or anywhere in the fork-authored code.**

What *does* exist is fully upstream, unrelated to `fork_config.toml`:

**Option:** `whitelist` (`OPTION_WHITELIST` constant, `libs/hbb_common/src/config.rs:2920`)

**Enforced at:** `src/server/connection.rs:1347-1373`, `check_whitelist()`:
```rust
async fn check_whitelist(&mut self, addr: &SocketAddr) -> bool {
    let whitelist: Vec<String> = Config::get_option("whitelist")
        .split(",")
        .filter(|x| !x.is_empty())
        .map(|x| x.to_owned())
        .collect();
    if !whitelist.is_empty()
        && whitelist.iter().filter(|x| x == &"0.0.0.0").next().is_none()
        && whitelist.iter().filter(|x| IpCidr::from_str(x).map_or(false, |y| y.contains(addr.ip()))).next().is_none()
    {
        self.send_login_error("Your ip is blocked by the peer").await;
        // ... alarm audit logged ...
        return false;
    }
    // ...
}
```
Called from `on_open()` (line 1378) — checked before any login processing.

**UI:** Already exposed in `flutter/lib/desktop/pages/desktop_setting_page.dart:1405-1450`,
labeled "Use IP Whitelisting" under the Safety tab, backed by upstream's own
`changeWhiteList()`/`whitelistNotEmpty()` helpers.

**Conclusion:** A comma-separated CIDR allow-list already exists, is already enforced at
connection-open time, and is already exposed in the UI. **No fork work is needed here** — this
satisfies the "IP filtering" requirement from Workstream 2 using an existing upstream mechanism.
The only fork-relevant question is whether this control should remain visible in a simplified
Setup UI (see `docs/SETUP_UI_AUDIT.md`, Workstream 3) — it is a legitimate security control, not
RustDesk-cloud-account cruft, so the working recommendation is **KEEP**.

---

## 4. `listen_address` / `listen_port` — Confirmed Dead Fields

Cross-referenced against the actual inbound listener setup. The real bind address/port for
incoming connections comes from upstream's own existing listener code
(`src/rendezvous_mediator.rs` / `src/server/mod.rs`), which this fork does not modify for
listener binding. `fork_config.toml`'s `listen_address`/`listen_port` fields are validated at
parse time but never read afterward (`#[allow(dead_code)]` in `src/fork_config.rs:104-106`).

**This is not a bug** — the module doc comment explicitly states these are "parsed and validated
now so the file format is stable across phases; not read by any caller yet," reserved for a
Direct-IP transport phase that has not been started. Flagging here per Workstream 2's request to
determine "whether options actually affect behavior" — they do not, yet, by design.

---

## Next Steps (Decisions Needed, Not Implemented Here)

1. Decide on Section 2's Option A/B/C for `desktop_share_enabled` remote enforcement.
2. Decide whether `listen_address`/`listen_port` should be wired to an actual future transport
   phase or removed from the schema if no longer planned.
3. No action needed for IP filtering — already solved upstream.
