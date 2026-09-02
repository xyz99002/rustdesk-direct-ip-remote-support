// main window right pane
//
// Fork: minimal UI. Deliberately does not include RustDesk's peer list, peer
// history, autocomplete, address book, public-server messaging, ID-based
// workflows (file transfer/terminal/bare view-camera via a peer menu), or
// relay/rendezvous UI — see docs/FORK_PROFILE_SPEC.md and docs/DECISIONS.md.
// The only inputs are a hostname/IP field and the Support/Desktop buttons
// (each independently gated by the direct-ip-* options in RustDesk2.toml, translated by
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
    return Center(child: _buildConnectPanel(context));
  }

  /// Callback shared by the Support and Desktop buttons. Connects to the
  /// host/IP entered above. Unchanged from the Connection Workflow phase.
  void onConnect(
      {bool isFileTransfer = false,
      bool isViewCamera = false,
      bool isTerminal = false}) {
    var id = _hostController.text.trim();
    connect(context, id,
        isFileTransfer: isFileTransfer,
        isViewCamera: isViewCamera,
        isTerminal: isTerminal);
  }

  /// Fork config: shows/hides the Support button. Also gates VIEW_CAMERA/Voice Call
  /// acceptance on the remote side, via the existing upstream "enable-camera" permission
  /// (see src/fork_config.rs). Defaults to shown if the fork config is absent/invalid.
  bool get _supportEnabled => mainGetBoolOptionSync("enable-camera");

  /// Fork config: shows/hides the Desktop button. Local UI only — see
  /// docs/FORK_PROFILE_SPEC.md for why this has no remote-side enforcement.
  bool get _desktopShareEnabled => mainGetBoolOptionSync("desktop-share-enabled");

  /// Callback for the Support button. Always opens a VIEW_CAMERA session (which starts a
  /// Voice Call on it once connected — see ViewCameraPage.initState()); additionally opens a
  /// plain DEFAULT_CONN session when desktop sharing is enabled. Both reuse the existing
  /// `connect()` call unmodified.
  void onSupport() {
    onConnect(isViewCamera: true);
    if (_desktopShareEnabled) {
      onConnect();
    }
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
            child: Row(mainAxisAlignment: MainAxisAlignment.end, children: [
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
              if (_supportEnabled && _desktopShareEnabled)
                const SizedBox(width: 8),
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
            ]),
          ),
        ],
      ),
    );
  }
}
