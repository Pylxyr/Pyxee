import 'dart:async';
import 'dart:convert';
import 'dart:io';
// ignore: unnecessary_import
import 'dart:ui' as ui;

// ignore: unnecessary_import
import 'package:flutter/rendering.dart';
import 'package:flutter_acrylic/flutter_acrylic.dart' as acrylic;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';
import 'package:window_manager/window_manager.dart';

import 'app/app.dart';
import 'core/bridge/bridge.dart';
import 'core/models/models.dart';
import 'core/state/providers.dart';
import 'core/theme/prefs.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  // Covers are small and a screen shows a few dozen: a modest cache keeps memory flat.
  PaintingBinding.instance.imageCache
    ..maximumSize = 150
    ..maximumSizeBytes = 96 << 20;

  Bridge bridge;
  try {
    bridge = Bridge.open();
  } on Object catch (e) {
    runApp(_StartupError('$e'));
    return;
  }

  debugPrint('[pyxee] bridge open');
  if (Platform.isWindows && !featureOff('acrylic')) await acrylic.Window.initialize();
  await windowManager.ensureInitialized();
  // `PYXEE_OFF=chrome` keeps the system title bar (and the app then draws none of its own).
  // ignore: prefer_const_constructors
  final options = WindowOptions(
    size: const Size(1100, 720),
    minimumSize: const Size(420, 560),
    center: true,
    title: 'Pyxee',
    titleBarStyle: featureOff('chrome') ? null : TitleBarStyle.hidden,
    backgroundColor: featureOff('chrome') ? null : const Color(0xFF0D0E11), // the host makes it transparent only for Mica
  );
  await windowManager.waitUntilReadyToShow(options, () async {
    await windowManager.show();
    await windowManager.focus();
  });

  debugPrint('[pyxee] window shown');
  var prefs = const Prefs();
  try {
    final text = bridge.call('prefsGet');
    if (text is String) prefs = Prefs.fromJson(_decode(text));
  } on Object {
    // Unreadable settings: start with the defaults.
  }
  bridge.call('setVolume', {'percent': prefs.volume});
  try {
    // Discord Rich Presence follows the setting (it stays off in a build without a Discord application ID).
    bridge.call('discordSet', {'on': prefs.discordPresence});
  } on Object {
    // Not worth stopping the start for.
  }

  var rasterized = 0;
  WidgetsBinding.instance.addTimingsCallback((timings) {
    if (rasterized < 6) {
      rasterized += timings.length;
      debugPrint('[pyxee] rasterized ${timings.length} frame(s)');
    }
  });
  WidgetsBinding.instance.addPostFrameCallback((_) => debugPrint('[pyxee] first frame built'));
  debugPrint('[pyxee] runApp');
  // `PYXEE_DUMP=1` writes the widget and render trees to %TEMP%\\pyxee_dump.txt three seconds after start.
  if (Platform.environment['PYXEE_DUMP'] == '1') {
    Timer(const Duration(seconds: 3), () {
      final file = File('${Directory.systemTemp.path}${Platform.pathSeparator}pyxee_dump.txt');
      final out = StringBuffer()
        ..writeln('== elements')
        ..writeln(WidgetsBinding.instance.rootElement?.toStringDeep())
        ..writeln('== render tree');
      for (final view in RendererBinding.instance.renderViews) {
        out.writeln(view.toStringDeep());
      }
      file.writeAsStringSync(out.toString());
      debugPrint('[pyxee] dump written to ${file.path}');
      unawaited(_capture());
    });
  }
  // `PYXEE_OFF=ui` shows a bare page instead of the app, to tell a drawing problem from an app problem.
  if (featureOff('ui')) {
    runApp(const _Probe());
    return;
  }
  runApp(
    ProviderScope(
      overrides: [
        bridgeProvider.overrideWithValue(bridge),
        initialPrefsProvider.overrideWithValue(prefs),
      ],
      child: Platform.environment['PYXEE_DUMP'] == '1'
          ? RepaintBoundary(key: _captureKey, child: const PyxeeApp())
          : const PyxeeApp(),
    ),
  );
}

final _captureKey = GlobalKey();

/// Renders the app to a picture off screen, to tell "the app draws nothing" from "the window does not show it".
Future<void> _capture() async {
  final boundary = _captureKey.currentContext?.findRenderObject() as RenderRepaintBoundary?;
  if (boundary == null) {
    debugPrint('[pyxee] capture: no boundary');
    return;
  }
  final image = await boundary.toImage(pixelRatio: 0.5);
  final png = await image.toByteData(format: ui.ImageByteFormat.png);
  final raw = await image.toByteData();
  if (png == null || raw == null) return;
  final colours = <int>{};
  for (var i = 0; i + 3 < raw.lengthInBytes; i += 4) {
    colours.add(raw.getUint32(i));
  }
  final file = File('${Directory.systemTemp.path}${Platform.pathSeparator}pyxee_capture.png');
  file.writeAsBytesSync(png.buffer.asUint8List());
  debugPrint('[pyxee] capture ${image.width}x${image.height}, ${colours.length} distinct colours: ${file.path}');
}

Json _decode(String text) => jsonDecode(text) as Json;

class _Probe extends StatelessWidget {
  const _Probe();

  @override
  Widget build(BuildContext context) => const MaterialApp(
    debugShowCheckedModeBanner: false,
    home: Scaffold(
      backgroundColor: Color(0xFF204060),
      body: Center(child: Text('Pyxee probe: if you can read this, Flutter draws.', style: TextStyle(fontSize: 28))),
    ),
  );
}

class _StartupError extends StatelessWidget {
  const _StartupError(this.message);

  final String message;

  @override
  Widget build(BuildContext context) => MaterialApp(
    debugShowCheckedModeBanner: false,
    home: Scaffold(body: Center(child: Padding(padding: const EdgeInsets.all(24), child: Text('Pyxee could not start the core.\n\n$message')))),
  );
}
