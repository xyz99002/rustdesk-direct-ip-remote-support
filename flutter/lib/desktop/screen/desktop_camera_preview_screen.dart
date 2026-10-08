// Fork: the connection manager's pop-up camera preview window.
//
// The remote operator can see their own monitors on screen, but not what a camera is sending
// to a connected local. This window shows exactly that: the frames the server is already
// capturing for the camera's video service, downscaled and JPEG-encoded on the server side
// (src/server/video_service.rs `maybe_send_camera_preview`), relayed over the CM IPC pipe and
// then forwarded here by the CM main window (ServerModel.onCameraPreviewFrame) as
// kWindowEventCameraPreviewFrame method calls. No second camera capture is ever opened.
//
// Lifecycle: created by ServerModel.openCameraPreview via rustDeskWinManager.newCameraPreview
// (one window per camera index). Closing it (title-bar X) notifies the CM main window with
// kWindowEventCameraPreviewClosed so it can tell the server to stop sending frames.

import 'dart:convert';
import 'dart:typed_data';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/main.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';

class DesktopCameraPreviewScreen extends StatefulWidget {
  final Map<String, dynamic> params;

  const DesktopCameraPreviewScreen({Key? key, required this.params})
      : super(key: key);

  @override
  State<DesktopCameraPreviewScreen> createState() =>
      _DesktopCameraPreviewScreenState();
}

class _DesktopCameraPreviewScreenState extends State<DesktopCameraPreviewScreen>
    with MultiWindowListener {
  late final int _cameraIndex;
  late final String _cameraName;
  Uint8List? _frame;
  int _frameWidth = 0;
  int _frameHeight = 0;
  DateTime? _lastFrameAt;
  bool _closing = false;

  @override
  void initState() {
    super.initState();
    _cameraIndex = widget.params['camera_index'] as int? ?? 0;
    _cameraName = widget.params['camera_name'] as String? ?? '';
    DesktopMultiWindow.addListener(this);
    rustDeskWinManager.setMethodHandler((call, fromWindowId) async {
      if (call.method == kWindowEventCameraPreviewFrame) {
        _onFrame(call.arguments);
      }
      return null;
    });
  }

  @override
  void dispose() {
    DesktopMultiWindow.removeListener(this);
    super.dispose();
  }

  void _onFrame(dynamic arguments) {
    try {
      final Map<String, dynamic> evt = arguments is String
          ? jsonDecode(arguments)
          : Map<String, dynamic>.from(arguments as Map);
      if ((evt['index'] as int? ?? -1) != _cameraIndex) return;
      final data = evt['data'] as String? ?? '';
      if (data.isEmpty) return;
      final bytes = base64Decode(data);
      if (!mounted) return;
      setState(() {
        _frame = bytes;
        _frameWidth = evt['width'] as int? ?? 0;
        _frameHeight = evt['height'] as int? ?? 0;
        _lastFrameAt = DateTime.now();
      });
    } catch (e) {
      debugPrint('camera preview: bad frame: $e');
    }
  }

  @override
  void onWindowClose() async {
    if (_closing) return;
    _closing = true;
    try {
      await DesktopMultiWindow.invokeMethod(kMainWindowId,
          kWindowEventCameraPreviewClosed, jsonEncode({'index': _cameraIndex}));
    } catch (e) {
      debugPrint('camera preview: failed to notify CM: $e');
    }
    final controller = WindowController.fromWindowId(kWindowId!);
    await controller.setPreventClose(false);
    await controller.close();
    super.onWindowClose();
  }

  @override
  Widget build(BuildContext context) {
    final frame = _frame;
    final stale = _lastFrameAt != null &&
        DateTime.now().difference(_lastFrameAt!) > const Duration(seconds: 3);
    return Scaffold(
      backgroundColor: Colors.black,
      body: Stack(
        fit: StackFit.expand,
        children: [
          Center(
            child: frame == null
                ? Text(
                    translate('Waiting for the camera stream'),
                    style: const TextStyle(color: Colors.white70),
                  )
                : Image.memory(
                    frame,
                    gaplessPlayback: true,
                    fit: BoxFit.contain,
                    filterQuality: FilterQuality.medium,
                  ),
          ),
          Positioned(
            left: 8,
            top: 8,
            child: Container(
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
              decoration: BoxDecoration(
                color: Colors.black54,
                borderRadius: BorderRadius.circular(6),
              ),
              child: Text(
                _frameWidth > 0
                    ? '$_cameraName  ·  ${_frameWidth}x$_frameHeight'
                    : _cameraName,
                style: const TextStyle(color: Colors.white, fontSize: 12),
              ),
            ),
          ),
          if (stale)
            Positioned(
              left: 8,
              bottom: 8,
              child: Container(
                padding:
                    const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
                decoration: BoxDecoration(
                  color: Colors.red.withOpacity(0.7),
                  borderRadius: BorderRadius.circular(6),
                ),
                child: Text(
                  translate('No frames received recently'),
                  style: const TextStyle(color: Colors.white, fontSize: 12),
                ),
              ),
            ),
        ],
      ),
    );
  }
}
