// original cm window in Sciter version.
//
// Fork (2026-10-07): the connection manager is a *list*, one row per connection, grouped by
// the connecting local's "Name (id)", instead of upstream's one-tab-per-connection card. Every
// row shows the connection type, the monitors/cameras it currently receives (server-reported,
// see ServerModel.updateVideoSources), its state (pending / connected / in call / incoming
// call / disconnected) and its actions inline. Chat and the file-transfer log use upstream's
// side panel (toggleCMSidePage). Cameras get a pop-up preview window (ServerModel.
// openCameraPreview). See docs/UPSTREAM_UPGRADE_GUIDE.md "Connection Manager List View".

import 'dart:async';
import 'dart:convert';
import 'dart:math';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common/widgets/audio_input.dart';
import 'package:flutter_hbb/common/widgets/chat_page.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/models/chat_model.dart';
import 'package:flutter_hbb/models/cm_file_model.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:flutter_hbb/utils/platform_channel.dart';
import 'package:get/get.dart';
import 'package:percent_indicator/linear_percent_indicator.dart';
import 'package:provider/provider.dart';
import 'package:window_manager/window_manager.dart';
import 'package:flutter_svg/flutter_svg.dart';

import '../../common.dart';
import '../../models/file_model.dart';
import '../../models/platform_model.dart';
import '../../models/server_model.dart';

class DesktopServerPage extends StatefulWidget {
  const DesktopServerPage({Key? key}) : super(key: key);

  @override
  State<DesktopServerPage> createState() => _DesktopServerPageState();
}

class _DesktopServerPageState extends State<DesktopServerPage>
    with WindowListener, AutomaticKeepAliveClientMixin {
  final tabController = gFFI.serverModel.tabController;

  _DesktopServerPageState() {
    gFFI.ffiModel.updateEventListener(gFFI.sessionId, "");
    Get.put<DesktopTabController>(tabController);
    tabController.onRemoved = (_, id) {
      onRemoveId(id);
    };
  }

  @override
  void initState() {
    windowManager.addListener(this);
    // Fork: camera preview pop-ups report their close back to this (main) window.
    rustDeskWinManager.setMethodHandler((call, fromWindowId) async {
      if (call.method == kWindowEventCameraPreviewClosed) {
        try {
          final args = call.arguments is String
              ? jsonDecode(call.arguments)
              : call.arguments;
          final index = args['index'] as int?;
          if (index != null) {
            gFFI.serverModel.onCameraPreviewClosed(index);
          }
        } catch (e) {
          debugPrint("camera preview closed: bad args: $e");
        }
      }
      return null;
    });
    super.initState();
  }

  @override
  void dispose() {
    windowManager.removeListener(this);
    super.dispose();
  }

  @override
  void onWindowClose() {
    Future.wait([gFFI.serverModel.closeAll(), gFFI.close()]).then((_) {
      if (isMacOS) {
        RdPlatformChannel.instance.terminate();
      } else {
        windowManager.setPreventClose(false);
        windowManager.close();
      }
    });
    super.onWindowClose();
  }

  void onRemoveId(String id) {
    if (tabController.state.value.tabs.isEmpty) {
      windowManager.close();
    }
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return MultiProvider(
      providers: [
        ChangeNotifierProvider.value(value: gFFI.serverModel),
        ChangeNotifierProvider.value(value: gFFI.chatModel),
      ],
      child: Consumer<ServerModel>(
        builder: (context, serverModel, child) {
          final body = Scaffold(
            backgroundColor: Theme.of(context).colorScheme.background,
            body: ConnectionManager(),
          );
          return isLinux
              ? buildVirtualWindowFrame(context, body)
              : workaroundWindowBorder(
                  context,
                  Container(
                    decoration: BoxDecoration(
                        border:
                            Border.all(color: MyTheme.color(context).border!)),
                    child: body,
                  ));
        },
      ),
    );
  }

  @override
  bool get wantKeepAlive => true;
}

class ConnectionManager extends StatefulWidget {
  @override
  State<StatefulWidget> createState() => ConnectionManagerState();
}

class ConnectionManagerState extends State<ConnectionManager>
    with WidgetsBindingObserver {
  final RxBool _controlPageBlock = false.obs;
  final RxBool _sidePageBlock = false.obs;
  final ScrollController _scrollController = ScrollController();
  double _lastFittedHeight = 0;
  bool _startupFitScheduled = false;

  ConnectionManagerState() {
    gFFI.serverModel.tabController.onSelected = (client_id_str) {
      final client_id = int.tryParse(client_id_str);
      if (client_id != null) {
        final client =
            gFFI.serverModel.clients.firstWhereOrNull((e) => e.id == client_id);
        if (client != null) {
          gFFI.chatModel.changeCurrentKey(MessageKey(client.peerId, client.id));
          if (client.unreadChatMessageCount.value > 0) {
            WidgetsBinding.instance.addPostFrameCallback((_) {
              client.unreadChatMessageCount.value = 0;
              gFFI.chatModel.showChatPage(MessageKey(client.peerId, client.id));
            });
          }
          windowManager.setTitle(getWindowNameWithId(client.peerId));
          gFFI.cmFileModel.updateCurrentClientId(client.id);
        }
      }
    };
    gFFI.chatModel.isConnManager = true;
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    super.didChangeAppLifecycleState(state);
    if (state == AppLifecycleState.resumed) {
      if (!allowRemoteCMModification()) {
        shouldBeBlocked(_controlPageBlock, null);
        shouldBeBlocked(_sidePageBlock, null);
      }
    }
  }

  @override
  void initState() {
    gFFI.serverModel.updateClientState();
    WidgetsBinding.instance.addObserver(this);
    super.initState();
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _scrollController.dispose();
    super.dispose();
  }

  // Fork: group clients by the connecting local (peer id), keeping first-seen order; groups
  // with a pending request come first so an accept prompt is never below the fold.
  List<List<Client>> _groupClients(List<Client> clients) {
    final groups = <String, List<Client>>{};
    for (final c in clients) {
      groups.putIfAbsent(c.peerId, () => []).add(c);
    }
    final list = groups.values.toList();
    list.sort((a, b) {
      final ap = a.any((c) => !c.authorized) ? 0 : 1;
      final bp = b.any((c) => !c.authorized) ? 0 : 1;
      return ap - bp;
    });
    return list;
  }

  // Fork: grow the window with its rows (never shrink it under the user), up to a cap;
  // past the cap the list scrolls. The window is resizable, so a manual resize is respected
  // until the next time more rows need room.
  void _fitWindowHeight(int groupCount, int rowCount) {
    const titleBar = kDesktopRemoteTabBarHeight;
    const groupHeader = 30.0;
    const row = 78.0;
    const padding = 24.0;
    final desired = (titleBar + groupHeader * groupCount + row * rowCount + padding)
        .clamp(kConnectionManagerWindowSizeClosedChat.height,
            kConnectionManagerWindowMaxAutoHeight)
        .toDouble();
    if ((desired - _lastFittedHeight).abs() < 1) return;
    _lastFittedHeight = desired;
    () async {
      try {
        final size = await windowManager.getSize();
        if (desired > size.height + 1) {
          await windowManager.setSize(Size(size.width, desired));
        }
      } catch (e) {
        debugPrint("cm: failed to fit window height: $e");
      }
    }();
  }

  @override
  Widget build(BuildContext context) {
    final serverModel = Provider.of<ServerModel>(context);
    // Subscribe to the side-panel flag too: the LayoutBuilder below only re-runs on a
    // window-size change, which does not happen when the window is already wide enough.
    final chatModel = Provider.of<ChatModel>(context);
    pointerHandler(PointerEvent e) {
      if (serverModel.cmHiddenTimer != null) {
        serverModel.cmHiddenTimer!.cancel();
        serverModel.cmHiddenTimer = null;
        debugPrint("CM hidden timer has been canceled");
      }
    }

    final groups = _groupClients(serverModel.clients);
    if (serverModel.clients.isNotEmpty) {
      WidgetsBinding.instance.addPostFrameCallback(
          (_) => _fitWindowHeight(groups.length, serverModel.clients.length));
      // The startup show/resize in showCmWindow() may land after the first frame and reset
      // the size; fit once more shortly after.
      if (!_startupFitScheduled) {
        _startupFitScheduled = true;
        Future.delayed(const Duration(milliseconds: 800), () {
          if (mounted) {
            _lastFittedHeight = 0;
            final m = gFFI.serverModel;
            _fitWindowHeight(_groupClients(m.clients).length, m.clients.length);
          }
        });
      }
    }

    final list = serverModel.clients.isEmpty
        ? Center(child: Text(translate("Waiting")))
        : ListView(
            controller: _scrollController,
            padding: const EdgeInsets.fromLTRB(8, 4, 8, 8),
            children: [
              for (final group in groups) ...[
                _GroupHeader(client: group.first),
                for (final client in group)
                  _ClientRow(
                    key: ValueKey(client.id),
                    client: client,
                    index: serverModel.clients.indexOf(client),
                  ),
              ],
            ],
          );

    return Listener(
      onPointerDown: pointerHandler,
      onPointerMove: pointerHandler,
      child: Column(
        children: [
          buildTitleBar(serverModel),
          Expanded(
            child: LayoutBuilder(
              builder: (context, constrains) {
                var borderWidth = 0.0;
                if (constrains.maxWidth >
                    kConnectionManagerWindowSizeClosedChat.width) {
                  borderWidth = kConnectionManagerWindowSizeOpenChat.width -
                      constrains.maxWidth;
                } else {
                  borderWidth = kConnectionManagerWindowSizeClosedChat.width -
                      constrains.maxWidth;
                }
                if (borderWidth < 0 || borderWidth > 50) {
                  borderWidth = 0;
                }
                final sideOpen = chatModel.isShowCMSidePage &&
                    constrains.maxWidth >
                        kConnectionManagerWindowSizeClosedChat.width;
                final realClosedWidth =
                    kConnectionManagerWindowSizeClosedChat.width - borderWidth;
                final sideWidth = sideOpen
                    ? min(constrains.maxWidth - realClosedWidth,
                        kConnectionManagerWindowSizeOpenChat.width -
                            kConnectionManagerWindowSizeClosedChat.width)
                    : 0.0;
                final listWidget = allowRemoteCMModification()
                    ? list
                    : buildRemoteBlock(
                        child: _buildKeyEventBlock(list),
                        block: _controlPageBlock,
                        mask: false,
                      );
                return Container(
                  color: Theme.of(context).scaffoldBackgroundColor,
                  child: Row(children: [
                    if (sideOpen)
                      Consumer<ChatModel>(
                          builder: (_, model, child) => SizedBox(
                                width: sideWidth,
                                child: allowRemoteCMModification()
                                    ? buildSidePage()
                                    : buildRemoteBlock(
                                        child: buildSidePage(),
                                        block: _sidePageBlock,
                                        mask: true),
                              )),
                    Expanded(child: listWidget),
                  ]),
                );
              },
            ),
          ),
        ],
      ),
    );
  }

  // Upstream's side panel (chat or file-transfer log for the selected connection), with a
  // fork-added header naming the connection as "Name (id)" plus a close button.
  Widget buildSidePage() {
    final selected = gFFI.serverModel.tabController.state.value.selected;
    if (selected < 0 || selected >= gFFI.serverModel.clients.length) {
      return Offstage();
    }
    final client = gFFI.serverModel.clients[selected];
    final isFile = client.type_() == ClientType.file;
    return Column(
      children: [
        Container(
          height: 36,
          padding: const EdgeInsets.only(left: 12, right: 4),
          decoration: BoxDecoration(
            border: Border(
                bottom: BorderSide(color: MyTheme.color(context).border!)),
          ),
          child: Row(
            children: [
              Icon(isFile ? Icons.folder_outlined : Icons.chat_outlined,
                  size: 16),
              const SizedBox(width: 6),
              Expanded(
                child: Text(
                  "${translate(isFile ? 'File Transfer' : 'Chat')} · ${client.displayName}",
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(fontWeight: FontWeight.bold),
                ),
              ),
              IconButton(
                tooltip: translate('Close'),
                icon: const Icon(Icons.close, size: 16),
                splashRadius: kDesktopIconButtonSplashRadius,
                onPressed: () => gFFI.chatModel.toggleCMSidePage(),
              ),
            ],
          ),
        ),
        Expanded(
          child: isFile
              ? _FileTransferLogPage()
              : ChatPage(type: ChatPageType.desktopCM),
        ),
      ],
    );
  }

  Widget _buildKeyEventBlock(Widget child) {
    return ExcludeFocus(child: child, excluding: true);
  }

  Widget buildTitleBar(ServerModel serverModel) {
    final count = serverModel.clients.length;
    final title = count == 0
        ? translate("Waiting")
        : "${translate('Connections')}: $count";
    return SizedBox(
      height: kDesktopRemoteTabBarHeight,
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          const _AppIcon(),
          Expanded(
            child: GestureDetector(
              onPanStart: (d) {
                windowManager.startDragging();
              },
              onDoubleTap: () {},
              child: Container(
                color: Theme.of(context).colorScheme.background,
                alignment: Alignment.centerLeft,
                child: Text(
                  title,
                  style: const TextStyle(
                      fontSize: 12, fontWeight: FontWeight.bold),
                ),
              ),
            ),
          ),
          if (!isMacOS)
            ActionIcon(
              message: 'Minimize',
              icon: IconFont.min,
              onTap: () => windowManager.minimize(),
              isClose: false,
            ),
          if (!isMacOS)
            ActionIcon(
              message: 'Close',
              icon: IconFont.close,
              onTap: () async {
                if (await handleWindowCloseButton()) {
                  windowManager.close();
                }
              },
              isClose: true,
            ),
        ],
      ),
    );
  }

  Future<bool> handleWindowCloseButton() async {
    var tabController = gFFI.serverModel.tabController;
    final connLength = tabController.length;
    if (connLength <= 1) {
      return true;
    } else {
      if (!option2bool(kOptionEnableConfirmClosingTabs,
          bind.mainGetLocalOption(key: kOptionEnableConfirmClosingTabs))) {
        return true;
      }
      return await closeConfirmDialog();
    }
  }
}

class _AppIcon extends StatelessWidget {
  const _AppIcon({Key? key}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: EdgeInsets.symmetric(horizontal: 4.0),
      child: loadIcon(30),
    );
  }
}

// Fork: one header per connecting local: avatar, "Name (id)".
class _GroupHeader extends StatelessWidget {
  final Client client;

  const _GroupHeader({Key? key, required this.client}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    final avatar = buildAvatarWidget(
          avatar: client.avatar,
          size: 22,
          borderRadius: 6,
          fallback: _initialAvatar(),
        ) ??
        _initialAvatar();
    return Padding(
      padding: const EdgeInsets.only(top: 8, bottom: 2, left: 2),
      child: Row(
        children: [
          avatar,
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              client.displayName,
              overflow: TextOverflow.ellipsis,
              style: const TextStyle(fontWeight: FontWeight.bold, fontSize: 13),
            ),
          ),
        ],
      ),
    );
  }

  Widget _initialAvatar() {
    return Container(
      width: 22,
      height: 22,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: str2color(client.name),
        borderRadius: BorderRadius.circular(6),
      ),
      child: Text(
        client.name.isNotEmpty ? client.name[0] : '?',
        style: const TextStyle(
            fontWeight: FontWeight.bold, color: Colors.white, fontSize: 13),
      ),
    );
  }
}

// Fork: one row per connection - type, sources, state and actions.
class _ClientRow extends StatefulWidget {
  final Client client;
  final int index;

  const _ClientRow({Key? key, required this.client, required this.index})
      : super(key: key);

  @override
  State<_ClientRow> createState() => _ClientRowState();
}

class _ClientRowState extends State<_ClientRow> {
  Client get client => widget.client;

  final _time = 0.obs;
  Timer? _timer;

  @override
  void initState() {
    super.initState();
    _timer = Timer.periodic(const Duration(seconds: 1), (_) {
      if (client.authorized && !client.disconnected) {
        _time.value = _time.value + 1;
      }
    });
    // Like upstream's tab card: a newly listed connection becomes the selected one, which
    // sets the chat key, window title and file-transfer log (tabController.onSelected).
    WidgetsBinding.instance.addPostFrameCallback((_) {
      gFFI.serverModel.tabController.jumpToByKey(client.id.toString());
    });
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  String _typeLabel() {
    switch (client.type_()) {
      case ClientType.file:
        return translate("File Transfer");
      case ClientType.camera:
        return translate("Camera");
      case ClientType.terminal:
        return translate("Terminal");
      case ClientType.portForward:
        return "${translate('Port Forward')}: ${client.portForward}";
      case ClientType.remote:
        return translate("Desktop");
    }
  }

  IconData _typeIcon() {
    switch (client.type_()) {
      case ClientType.file:
        return Icons.folder_outlined;
      case ClientType.camera:
        return Icons.videocam_outlined;
      case ClientType.terminal:
        return Icons.terminal;
      case ClientType.portForward:
        return Icons.swap_horiz;
      case ClientType.remote:
        return Icons.desktop_windows_outlined;
    }
  }

  @override
  Widget build(BuildContext context) {
    final serverModel = Provider.of<ServerModel>(context);
    final pending = !client.authorized;
    return Obx(() {
      final selected =
          serverModel.tabController.state.value.selected == widget.index;
      final borderColor = pending
          ? Colors.orange
          : selected
              ? MyTheme.accent
              : MyTheme.color(context).border!;
      return Container(
        margin: const EdgeInsets.only(bottom: 6),
        decoration: BoxDecoration(
          color: Theme.of(context).colorScheme.background,
          borderRadius: BorderRadius.circular(8),
          border: Border.all(
              color: borderColor, width: pending || selected ? 1.5 : 1),
        ),
        child: InkWell(
          borderRadius: BorderRadius.circular(8),
          onTap: () => serverModel.tabController.jumpTo(widget.index),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.center,
              children: [
                Icon(_typeIcon(), size: 22).marginOnly(right: 8),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      _buildTypeAndSources(serverModel),
                      const SizedBox(height: 4),
                      _buildStateLine(),
                    ],
                  ),
                ),
                const SizedBox(width: 6),
                _buildActions(context, serverModel),
              ],
            ),
          ),
        ),
      );
    });
  }

  // "Desktop · Monitor 1, Monitor 2" / "Camera · [USB Camera 👁]".
  Widget _buildTypeAndSources(ServerModel serverModel) {
    final pending = !client.authorized;
    final children = <Widget>[
      // A pending request is the one thing the operator must read before clicking Accept,
      // so its type is larger and orange until it is accepted or rejected.
      Text(_typeLabel(),
          style: TextStyle(
              fontWeight: FontWeight.bold,
              fontSize: pending ? 15 : 13,
              color: pending ? Colors.orange : null)),
    ];
    for (final source in client.sources) {
      children.add(Text(" · ", style: TextStyle(color: MyTheme.darkGray)));
      children.add(Flexible(
        child: Text(
          source.name,
          overflow: TextOverflow.ellipsis,
          style: const TextStyle(fontSize: 13),
        ),
      ));
      if (source.isCamera) {
        final open = serverModel.isCameraPreviewOpen(source.index);
        children.add(Tooltip(
          message: translate("Show the camera feed being sent"),
          child: InkWell(
            borderRadius: BorderRadius.circular(10),
            onTap: () => serverModel.openCameraPreview(source.index, source.name),
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 4),
              child: Icon(
                open ? Icons.visibility : Icons.visibility_outlined,
                size: 16,
                color: open ? MyTheme.accent : null,
              ),
            ),
          ),
        ));
      }
    }
    return Row(mainAxisSize: MainAxisSize.min, children: children);
  }

  Widget _chip(String text, Color color, {IconData? icon}) {
    return Container(
      margin: const EdgeInsets.only(right: 6),
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 1),
      decoration: BoxDecoration(
        color: color.withOpacity(0.15),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          if (icon != null) Icon(icon, size: 12, color: color).marginOnly(right: 3),
          Text(text, style: TextStyle(fontSize: 11, color: color)),
        ],
      ),
    );
  }

  Widget _buildStateLine() {
    final chips = <Widget>[];
    if (!client.authorized) {
      // Say what is being asked for, not just that something is: the operator decides
      // between "Desktop", "Camera", "File Transfer", ... from this line.
      chips.add(_chip(
          "${translate('Requesting')}: ${_typeLabel()}", Colors.orange,
          icon: Icons.hourglass_top));
    } else if (client.disconnected) {
      chips.add(_chip(translate("Disconnected"), Colors.grey,
          icon: Icons.link_off));
    } else {
      chips.add(Obx(() => _chip(
          "${translate('Connected')} ${formatDurationToTime(Duration(seconds: _time.value))}",
          Colors.green,
          icon: Icons.link)));
      if (client.inVoiceCall) {
        chips.add(_chip(translate("Voice call"), MyTheme.accent,
            icon: Icons.call));
      } else if (client.incomingVoiceCall) {
        chips.add(_chip(
            "${translate('Requesting')}: ${translate('Voice call')}",
            Colors.orange,
            icon: Icons.ring_volume));
      }
      if (client.privacyMode) {
        chips.add(_chip(translate("Privacy mode"), Colors.purple,
            icon: Icons.visibility_off));
      }
    }
    return Wrap(runSpacing: 2, children: chips);
  }

  Widget _smallButton(
    BuildContext context, {
    required String text,
    required Color color,
    IconData? icon,
    GestureTapCallback? onTap,
    GestureTapDownCallback? onTapDown,
    String? tooltip,
  }) {
    final btn = Container(
      height: 26,
      margin: const EdgeInsets.only(left: 4),
      decoration: BoxDecoration(
          color: color, borderRadius: BorderRadius.circular(6)),
      child: InkWell(
        borderRadius: BorderRadius.circular(6),
        onTap: onTap == null ? null : () => checkClickTime(client.id, onTap),
        onTapDown: onTapDown == null
            ? null
            : (d) => checkClickTime(client.id, () => onTapDown(d)),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 8),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              if (icon != null)
                Icon(icon, size: 14, color: Colors.white).marginOnly(right: 4),
              Text(translate(text),
                  style: const TextStyle(color: Colors.white, fontSize: 12)),
            ],
          ),
        ),
      ),
    );
    return tooltip == null
        ? btn
        : Tooltip(message: translate(tooltip), child: btn);
  }

  Widget _iconButton(IconData icon, String tooltip, VoidCallback onTap,
      {Color? color, Widget? badge}) {
    Widget btn = IconButton(
      tooltip: translate(tooltip),
      icon: Icon(icon, size: 18, color: color),
      splashRadius: kDesktopIconButtonSplashRadius,
      constraints: const BoxConstraints(minWidth: 30, minHeight: 30),
      padding: EdgeInsets.zero,
      onPressed: () => checkClickTime(client.id, onTap),
    );
    if (badge != null) {
      btn = Stack(clipBehavior: Clip.none, children: [
        btn,
        Positioned(right: 0, top: 0, child: badge),
      ]);
    }
    return btn;
  }

  Widget _buildActions(BuildContext context, ServerModel model) {
    final canElevate = bind.cmCanElevate();
    final showElevation = canElevate &&
        model.showElevation &&
        client.type_() == ClientType.remote;
    final buttons = <Widget>[];

    if (!client.authorized) {
      final showAccept = model.approveMode != 'password';
      if (showAccept && showElevation) {
        buttons.add(_smallButton(context,
            text: 'Accept and Elevate',
            color: Colors.green[700]!,
            icon: Icons.security_rounded,
            tooltip: 'accept_and_elevate_btn_tooltip', onTap: () {
          _accept(model);
          _elevate(model);
          windowManager.minimize();
        }));
      }
      if (showAccept) {
        buttons.add(_smallButton(context,
            text: 'Accept', color: MyTheme.accent, icon: Icons.check,
            onTap: () {
          _accept(model);
          windowManager.minimize();
        }));
      }
      buttons.add(_smallButton(context,
          text: 'Reject',
          color: Colors.redAccent,
          icon: Icons.close,
          onTap: _disconnect));
      return Row(mainAxisSize: MainAxisSize.min, children: buttons);
    }

    if (client.disconnected) {
      buttons.add(_smallButton(context,
          text: 'Close', color: MyTheme.accent, onTap: _close));
      return Row(mainAxisSize: MainAxisSize.min, children: buttons);
    }

    if (client.incomingVoiceCall) {
      // "Accept call" / "Decline call", not the bare "Accept" / "Dismiss" a session request
      // uses - the operator must not confuse the two.
      buttons.add(_smallButton(context,
          text: 'Accept call',
          color: Colors.green[700]!,
          icon: Icons.call_rounded,
          onTap: () => _handleVoiceCall(true)));
      buttons.add(_smallButton(context,
          text: 'Decline call',
          color: Colors.red,
          icon: Icons.phone_disabled_rounded,
          onTap: () => _handleVoiceCall(false)));
    } else if (client.inVoiceCall) {
      // The audio-input chooser needs the tap position for its menu (upstream's "Audio
      // input" button), hence a GestureDetector rather than _iconButton.
      buttons.add(GestureDetector(
        onTapDown: (details) => checkClickTime(
            client.id, () => _showAudioInputMenu(context, details)),
        child: Tooltip(
          message: translate('Audio input'),
          child: Padding(
            padding: const EdgeInsets.all(6),
            child: Icon(Icons.mic, size: 18, color: MyTheme.accent),
          ),
        ),
      ));
      buttons.add(_smallButton(context,
          text: 'Stop voice call',
          color: Colors.red,
          icon: Icons.call_end_rounded,
          onTap: _closeVoiceCall));
    }
    if (client.fromSwitch) {
      buttons.add(_smallButton(context,
          text: 'Switch Sides',
          color: Colors.purple,
          icon: Icons.reply,
          onTap: _switchBack));
    }
    if (showElevation) {
      buttons.add(_smallButton(context,
          text: 'Elevate',
          color: MyTheme.accent,
          icon: Icons.security_rounded, onTap: () {
        _elevate(model);
        windowManager.minimize();
      }));
    }
    final type = client.type_();
    if (type == ClientType.remote ||
        type == ClientType.camera ||
        type == ClientType.file) {
      final isFile = type == ClientType.file;
      buttons.add(_iconButton(
        isFile ? Icons.folder_outlined : Icons.chat_outlined,
        isFile ? 'File Transfer' : 'Chat',
        () {
          model.tabController.jumpTo(widget.index);
          if (isFile) {
            gFFI.chatModel.toggleCMFilePage();
          } else {
            gFFI.chatModel
                .toggleCMChatPage(MessageKey(client.peerId, client.id));
          }
        },
        badge: isFile
            ? null
            : unreadMessageCountBuilder(client.unreadChatMessageCount,
                size: 12, fontSize: 8),
      ));
    }
    if (type == ClientType.remote || type == ClientType.camera) {
      buttons.add(_buildPermissionsMenu());
    }
    buttons.add(_iconButton(Icons.link_off_rounded, 'Disconnect', _disconnect,
        color: Colors.redAccent));
    return Row(mainAxisSize: MainAxisSize.min, children: buttons);
  }

  // Upstream's permission board, folded into a "⋯" menu per row.
  Widget _buildPermissionsMenu() {
    final canModify =
        bind.mainGetBuildinOption(key: kOptionEnablePermChangeInAcceptWindow) !=
            'N';
    final items = <_PermItem>[];
    if (client.type_() == ClientType.camera) {
      items.add(_PermItem('audio', 'Enable audio', Icons.volume_up_rounded,
          () => client.audio, (v) => client.audio = v));
      items.add(_PermItem('recording', 'Enable recording session',
          Icons.videocam_rounded, () => client.recording, (v) => client.recording = v));
    } else {
      items.add(_PermItem('keyboard', 'Enable keyboard/mouse', Icons.keyboard,
          () => client.keyboard, (v) => client.keyboard = v));
      items.add(_PermItem('clipboard', 'Enable clipboard',
          Icons.assignment_rounded, () => client.clipboard, (v) => client.clipboard = v));
      items.add(_PermItem('audio', 'Enable audio', Icons.volume_up_rounded,
          () => client.audio, (v) => client.audio = v));
      items.add(_PermItem('file', 'Enable file copy and paste',
          Icons.upload_file_rounded, () => client.file, (v) => client.file = v));
      items.add(_PermItem('restart', 'Enable remote restart',
          Icons.restart_alt_rounded, () => client.restart, (v) => client.restart = v));
      items.add(_PermItem('recording', 'Enable recording session',
          Icons.videocam_rounded, () => client.recording, (v) => client.recording = v));
      if (isWindows) {
        items.add(_PermItem('block_input', 'Enable blocking user input',
            Icons.block, () => client.blockInput, (v) => client.blockInput = v));
      }
      if (bind.mainSupportedPrivacyModeImpls() != '[]') {
        items.add(_PermItem('privacy_mode', 'Enable privacy mode',
            Icons.visibility_off, () => client.privacyMode, (v) => client.privacyMode = v));
      }
    }
    return PopupMenuButton<_PermItem>(
      tooltip: translate('Permissions'),
      icon: const Icon(Icons.more_horiz, size: 18),
      splashRadius: kDesktopIconButtonSplashRadius,
      padding: EdgeInsets.zero,
      constraints: const BoxConstraints(minWidth: 30, minHeight: 30),
      enabled: canModify,
      itemBuilder: (context) => items
          .map((item) => CheckedPopupMenuItem<_PermItem>(
                value: item,
                checked: item.get(),
                child: Row(children: [
                  Icon(item.icon, size: 16).marginOnly(right: 8),
                  Text(translate(item.label)),
                ]),
              ))
          .toList(),
      onSelected: (item) => checkClickTime(client.id, () {
        final enabled = !item.get();
        bind.cmSwitchPermission(
            connId: client.id, name: item.name, enabled: enabled);
        setState(() => item.set(enabled));
      }),
    );
  }

  void _showAudioInputMenu(BuildContext context, TapDownDetails details) async {
    final devicesInfo = await AudioInput.getDevicesInfo(true, true);
    List<String> devices = devicesInfo['devices'] as List<String>;
    if (devices.isEmpty) {
      msgBox(
        gFFI.sessionId,
        'custom-nocancel-info',
        'Prompt',
        'no_audio_input_device_tip',
        '',
        gFFI.dialogManager,
      );
      return;
    }
    String currentDevice = devicesInfo['current'] as String;
    final x = details.globalPosition.dx;
    final y = details.globalPosition.dy;
    final position = RelativeRect.fromLTRB(x, y, x, y);
    if (!context.mounted) return;
    showMenu(
      context: context,
      position: position,
      items: devices
          .map((d) => PopupMenuItem<String>(
                value: d,
                height: 18,
                padding: EdgeInsets.zero,
                onTap: () => AudioInput.setDevice(d, true, true),
                child: IgnorePointer(
                    child: RadioMenuButton(
                  value: d,
                  groupValue: currentDevice,
                  onChanged: (v) {
                    if (v != null) AudioInput.setDevice(v, true, true);
                  },
                  child: Container(
                    child: Text(
                      d,
                      overflow: TextOverflow.ellipsis,
                      maxLines: 1,
                    ),
                    constraints: BoxConstraints(
                        maxWidth:
                            kConnectionManagerWindowSizeClosedChat.width - 80),
                  ),
                )),
              ))
          .toList(),
    );
  }

  void _disconnect() {
    bind.cmCloseConnection(connId: client.id);
  }

  void _accept(ServerModel model) {
    model.sendLoginResponse(client, true);
  }

  void _elevate(ServerModel model) {
    model.setShowElevation(false);
    bind.cmElevatePortable(connId: client.id);
  }

  void _close() async {
    await bind.cmRemoveDisconnectedConnection(connId: client.id);
    if (await bind.cmGetClientsLength() == 0) {
      windowManager.close();
    }
  }

  void _switchBack() {
    bind.cmSwitchBack(connId: client.id);
  }

  void _handleVoiceCall(bool accept) {
    bind.cmHandleIncomingVoiceCall(id: client.id, accept: accept);
  }

  void _closeVoiceCall() {
    bind.cmCloseVoiceCall(id: client.id);
  }
}

class _PermItem {
  final String name;
  final String label;
  final IconData icon;
  final bool Function() get;
  final void Function(bool) set;

  _PermItem(this.name, this.label, this.icon, this.get, this.set);
}

void checkClickTime(int id, Function() callback) async {
  if (allowRemoteCMModification()) {
    callback();
    return;
  }
  var clickCallbackTime = DateTime.now().millisecondsSinceEpoch;
  await bind.cmCheckClickTime(connId: id);
  Timer(const Duration(milliseconds: 120), () async {
    var d = clickCallbackTime - await bind.cmGetClickTime();
    if (d > 120) callback();
  });
}

bool allowRemoteCMModification() {
  return option2bool(kOptionAllowRemoteCmModification,
      bind.mainGetLocalOption(key: kOptionAllowRemoteCmModification));
}

class _FileTransferLogPage extends StatefulWidget {
  _FileTransferLogPage({Key? key}) : super(key: key);

  @override
  State<_FileTransferLogPage> createState() => __FileTransferLogPageState();
}

class __FileTransferLogPageState extends State<_FileTransferLogPage> {
  @override
  Widget build(BuildContext context) {
    return statusList();
  }

  Widget generateCard(Widget child) {
    return Container(
      decoration: BoxDecoration(
        color: Theme.of(context).cardColor,
        borderRadius: BorderRadius.all(
          Radius.circular(15.0),
        ),
      ),
      child: child,
    );
  }

  iconLabel(CmFileLog item) {
    switch (item.action) {
      case CmFileAction.none:
        return Container();
      case CmFileAction.localToRemote:
      case CmFileAction.remoteToLocal:
        return Column(
          children: [
            Transform.rotate(
              angle: item.action == CmFileAction.remoteToLocal ? 0 : pi,
              child: SvgPicture.asset(
                "assets/arrow.svg",
                colorFilter: svgColor(Theme.of(context).tabBarTheme.labelColor),
              ),
            ),
            Text(item.action == CmFileAction.remoteToLocal
                ? translate('Send')
                : translate('Receive'))
          ],
        );
      case CmFileAction.remove:
        return Column(
          children: [
            Icon(
              Icons.delete,
              color: Theme.of(context).tabBarTheme.labelColor,
            ),
            Text(translate('Delete'))
          ],
        );
      case CmFileAction.createDir:
        return Column(
          children: [
            Icon(
              Icons.create_new_folder,
              color: Theme.of(context).tabBarTheme.labelColor,
            ),
            Text(translate('Create Folder'))
          ],
        );
      case CmFileAction.rename:
        return Column(
          children: [
            Icon(
              Icons.drive_file_move_outlined,
              color: Theme.of(context).tabBarTheme.labelColor,
            ),
            Text(translate('Rename'))
          ],
        );
    }
  }

  Widget statusList() {
    return PreferredSize(
      preferredSize: const Size(200, double.infinity),
      child: Container(
          padding: const EdgeInsets.all(12.0),
          child: Obx(
            () {
              final jobTable = gFFI.cmFileModel.currentJobTable;
              statusListView(List<CmFileLog> jobs) => ListView.builder(
                    controller: ScrollController(),
                    itemBuilder: (BuildContext context, int index) {
                      final item = jobs[index];
                      return Padding(
                        padding: const EdgeInsets.only(bottom: 5),
                        child: generateCard(
                          Column(
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              Row(
                                crossAxisAlignment: CrossAxisAlignment.center,
                                children: [
                                  SizedBox(
                                    width: 50,
                                    child: iconLabel(item),
                                  ).paddingOnly(left: 15),
                                  const SizedBox(
                                    width: 16.0,
                                  ),
                                  Expanded(
                                    child: Column(
                                      mainAxisSize: MainAxisSize.min,
                                      crossAxisAlignment:
                                          CrossAxisAlignment.start,
                                      children: [
                                        Text(
                                          item.fileName,
                                        ).paddingSymmetric(vertical: 10),
                                        if (item.totalSize > 0)
                                          Text(
                                            '${translate("Total")} ${readableFileSize(item.totalSize.toDouble())}',
                                            style: TextStyle(
                                              fontSize: 12,
                                              color: MyTheme.darkGray,
                                            ),
                                          ),
                                        if (item.totalSize > 0)
                                          Offstage(
                                            offstage: item.state !=
                                                JobState.inProgress,
                                            child: Text(
                                              '${translate("Speed")} ${readableFileSize(item.speed)}/s',
                                              style: TextStyle(
                                                fontSize: 12,
                                                color: MyTheme.darkGray,
                                              ),
                                            ),
                                          ),
                                        Offstage(
                                          offstage: !(item.isTransfer() &&
                                              item.state !=
                                                  JobState.inProgress),
                                          child: Text(
                                            translate(
                                              item.display(),
                                            ),
                                            style: TextStyle(
                                              fontSize: 12,
                                              color: MyTheme.darkGray,
                                            ),
                                          ),
                                        ),
                                        if (item.totalSize > 0)
                                          Offstage(
                                            offstage: item.state !=
                                                JobState.inProgress,
                                            child: LinearPercentIndicator(
                                              padding:
                                                  EdgeInsets.only(right: 15),
                                              animateFromLastPercent: true,
                                              center: Text(
                                                '${(item.finishedSize / item.totalSize * 100).toStringAsFixed(0)}%',
                                              ),
                                              barRadius: Radius.circular(15),
                                              percent: item.finishedSize /
                                                  item.totalSize,
                                              progressColor: MyTheme.accent,
                                              backgroundColor:
                                                  Theme.of(context).hoverColor,
                                              lineHeight:
                                                  kDesktopFileTransferRowHeight,
                                            ).paddingSymmetric(vertical: 15),
                                          ),
                                      ],
                                    ),
                                  ),
                                  Row(
                                    mainAxisAlignment: MainAxisAlignment.end,
                                    children: [],
                                  ),
                                ],
                              ),
                            ],
                          ).paddingSymmetric(vertical: 10),
                        ),
                      );
                    },
                    itemCount: jobTable.length,
                  );

              return jobTable.isEmpty
                  ? generateCard(
                      Center(
                        child: Column(
                          mainAxisAlignment: MainAxisAlignment.center,
                          children: [
                            SvgPicture.asset(
                              "assets/transfer.svg",
                              colorFilter: svgColor(
                                  Theme.of(context).tabBarTheme.labelColor),
                              height: 40,
                            ).paddingOnly(bottom: 10),
                            Text(
                              translate("No transfers in progress"),
                              textAlign: TextAlign.center,
                              textScaler: TextScaler.linear(1.20),
                              style: TextStyle(
                                  color:
                                      Theme.of(context).tabBarTheme.labelColor),
                            ),
                          ],
                        ),
                      ),
                    )
                  : statusListView(jobTable);
            },
          )),
    );
  }
}
