// main window right pane
//
// Fork: minimal UI. Deliberately does not include RustDesk's peer list, peer
// history, autocomplete, address book, public-server messaging, ID-based
// workflows (file transfer/terminal/bare view-camera via a peer menu), or
// relay/rendezvous UI — see docs/FORK_PROFILE_SPEC.md and docs/DECISIONS.md.
// The only inputs are a hostname/IP field and the Support/Desktop buttons
// (each independently gated by the direct-ip-* options in config.toml, translated by
// src/fork_config.rs — see connection_page.dart's _supportEnabled/_desktopShareEnabled below).

import 'package:flutter/material.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:get/get.dart';
import 'package:window_manager/window_manager.dart';

import '../../common.dart';
import '../../models/platform_model.dart';

/// Connection page for connecting to a remote peer.
class ConnectionPage extends StatefulWidget {
  const ConnectionPage({Key? key}) : super(key: key);

  @override
  State<ConnectionPage> createState() => _ConnectionPageState();
}

/// State for the connection page.
class _ConnectionPageState extends State<ConnectionPage>
    with SingleTickerProviderStateMixin, WindowListener {
  /// Controller for the hostname/IP input field. Deliberately a plain
  /// `TextEditingController` (not RustDesk's `IDTextEditingController`) —
  /// this field takes a hostname or IP, not a RustDesk ID, and is not
  /// registered with GetX, since nothing needs to look it up externally
  /// (`connect()` in common.dart only does so defensively, behind an
  /// `isRegistered` check).
  final TextEditingController _hostController = TextEditingController();

  bool isWindowMinimized = false;

  @override
  void initState() {
    super.initState();
    windowManager.addListener(this);
  }

  @override
  void dispose() {
    _hostController.dispose();
    windowManager.removeListener(this);
    super.dispose();
  }

  @override
  void onWindowEvent(String eventName) {
    super.onWindowEvent(eventName);
    if (eventName == 'minimize') {
      isWindowMinimized = true;
    } else if (eventName == 'maximize' || eventName == 'restore') {
      if (isWindowMinimized && isWindows) {
        // windows can't update when minimized.
        Get.forceAppUpdate();
      }
      isWindowMinimized = false;
    }
  }

  @override
  void onWindowEnterFullScreen() {
    // Remove edge border by setting the value to zero.
    stateGlobal.resizeEdgeSize.value = 0;
  }

  @override
  void onWindowLeaveFullScreen() {
    // Restore edge border to default edge size.
    stateGlobal.resizeEdgeSize.value = stateGlobal.isMaximized.isTrue
        ? kMaximizeEdgeSize
        : windowResizeEdgeSize;
  }

  @override
  void onWindowClose() {
    super.onWindowClose();
    bind.mainOnMainWindowClose();
  }

  @override
  Widget build(BuildContext context) {
    // Fork config: a "remote" role (role, translated to upstream's own
    // conn-type=incoming by src/fork_config.rs) may only ACCEPT inbound sessions - it can never
    // initiate one. The IP field and Support/Desktop buttons below all initiate an outbound
    // connect, so a remote-role instance must not show them at all; showing controls that would
    // always be rejected is confusing, not merely inert.
    if (bind.isIncomingOnly()) {
      return Center(child: _buildRemoteModeStatus(context));
    }
    return Center(child: _buildConnectPanel(context));
  }

  /// Shown instead of the connect panel when this instance is restricted to "remote" (incoming
  /// -only) role: there is nothing here to connect *from*, so no IP field or buttons are shown.
  Widget _buildRemoteModeStatus(BuildContext context) {
    return Container(
      width: 320 + 20 * 2,
      padding: const EdgeInsets.fromLTRB(20, 24, 20, 22),
      decoration: BoxDecoration(
          borderRadius: const BorderRadius.all(Radius.circular(13)),
          border: Border.all(color: Theme.of(context).colorScheme.background)),
      child: Text(
        translate("Waiting for incoming connections"),
        textAlign: TextAlign.center,
        style: const TextStyle(fontFamily: 'WorkSans', fontSize: 16),
      ),
    );
  }

  /// Callback shared by the Support, Desktop and Transfer file buttons. Connects to the
  /// host/IP entered above.
  ///
  /// Fork (2026-10-09): every click starts a brand-new session in its own window, also when
  /// a session to the same host is already open (`forceNewWindow`). Upstream's connect panel
  /// would instead focus the existing tab/window for that id, so a second Desktop session
  /// to the same host (e.g. to put a second monitor in a separate window) was impossible
  /// from here; product decision is one click = one new session, like opening a monitor in
  /// a new window. The "Open new connections in tabs" setting therefore no longer applies to
  /// these buttons.
  void onConnect(
      {bool isFileTransfer = false,
      bool isViewCamera = false,
      bool isTerminal = false}) {
    var id = _hostController.text.trim();
    connect(context, id,
        isFileTransfer: isFileTransfer,
        isViewCamera: isViewCamera,
        isTerminal: isTerminal,
        forceNewWindow: true);
  }

  /// Fork config: shows/hides the Support button. Also gates VIEW_CAMERA/Voice Call
  /// acceptance on the remote side, via the existing upstream "enable-camera" permission
  /// (see src/fork_config.rs). Defaults to shown if the fork config is absent/invalid.
  bool get _supportEnabled => mainGetBoolOptionSync("enable-camera");

  /// Fork config: shows/hides the Desktop button. Local UI only — see
  /// docs/FORK_PROFILE_SPEC.md for why this has no remote-side enforcement.
  bool get _desktopShareEnabled => mainGetBoolOptionSync("desktop-share-enabled");

  /// Fork config: shows/hides the Transfer file button (`file-transfer-enabled`, mapped by
  /// src/fork_config.rs onto upstream's "enable-file-transfer" permission, which also makes
  /// the remote reject file-transfer logins). Defaults to shown if the config is absent.
  bool get _fileTransferEnabled => mainGetBoolOptionSync("enable-file-transfer");

  /// Callback for the Support button. Opens *only* a VIEW_CAMERA session (which starts a
  /// Voice Call on it once connected — see ViewCameraPage.initState()).
  ///
  /// Previously this also opened a second, plain DEFAULT_CONN (desktop) session whenever
  /// desktop-share-enabled was also true - two independent sessions dialing out at once,
  /// each producing its own accept/approval prompt on the remote side. Found via real testing:
  /// this caused synchronization issues and confusing double prompts when both Support and
  /// Desktop buttons were enabled together. Support and Desktop are now fully independent -
  /// Support opens only a camera/voice-call session, Desktop (below) opens only a plain
  /// desktop session; neither triggers the other.
  void onSupport() {
    onConnect(isViewCamera: true);
  }

  void _onSubmit() {
    // Mirrors whichever button(s) are actually shown (fork_config guarantees
    // at least one of the two is enabled).
    if (_supportEnabled) {
      onSupport();
    } else {
      onConnect();
    }
  }

  /// Minimal connect panel: a hostname/IP field plus the Support/Desktop
  /// buttons, per docs/FORK_PROFILE_SPEC.md's "Local Client" UI.
  Widget _buildConnectPanel(BuildContext context) {
    return Container(
      width: 320 + 20 * 2,
      padding: const EdgeInsets.fromLTRB(20, 24, 20, 22),
      decoration: BoxDecoration(
          borderRadius: const BorderRadius.all(Radius.circular(13)),
          border: Border.all(color: Theme.of(context).colorScheme.background)),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          TextField(
            controller: _hostController,
            autocorrect: false,
            enableSuggestions: false,
            keyboardType: TextInputType.visiblePassword,
            style: const TextStyle(
              fontFamily: 'WorkSans',
              fontSize: 22,
              height: 1.4,
            ),
            maxLines: 1,
            cursorColor: Theme.of(context).textTheme.titleLarge?.color,
            decoration: InputDecoration(
                filled: false,
                counterText: '',
                hintText: translate('Enter Hostname or IP'),
                contentPadding:
                    const EdgeInsets.symmetric(horizontal: 15, vertical: 13)),
            onSubmitted: (_) => _onSubmit(),
          ).workaroundFreezeLinuxMint(),
          Padding(
            padding: const EdgeInsets.only(top: 13.0),
            // Wrap, not Row: with three buttons a long translation would overflow the
            // 320 px panel; wrapping to a second line is the harmless outcome.
            child: Wrap(
                alignment: WrapAlignment.end,
                spacing: 8,
                runSpacing: 8,
                children: [
                  if (_supportEnabled)
                    SizedBox(
                      height: 28.0,
                      child: ElevatedButton(
                        onPressed: () {
                          onSupport();
                        },
                        child: Text(translate("Support")),
                      ),
                    ),
                  if (_desktopShareEnabled)
                    SizedBox(
                      height: 28.0,
                      child: ElevatedButton(
                        onPressed: () {
                          onConnect();
                        },
                        child: Text(translate("Desktop")),
                      ),
                    ),
                  // Fork (2026-10-09): upstream offers "Transfer file" in the dropdown next
                  // to its Connect button; this fork's panel replaced that dropdown with the
                  // Support/Desktop buttons and lost the option. Restored as a plain button,
                  // gated by config.toml file-transfer-enabled like the other two.
                  if (_fileTransferEnabled)
                    SizedBox(
                      height: 28.0,
                      child: OutlinedButton(
                        onPressed: () {
                          onConnect(isFileTransfer: true);
                        },
                        child: Text(translate("Transfer file")),
                      ),
                    ),
                ]),
          ),
        ],
      ),
    );
  }
}
