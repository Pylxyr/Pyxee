import 'dart:convert';
import 'dart:io';

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
    ..maximumSizeBytes = 48 << 20;

  Bridge bridge;
  try {
    bridge = Bridge.open();
  } on Object catch (e) {
    runApp(_StartupError('$e'));
    return;
  }

  if (Platform.isWindows) await acrylic.Window.initialize();
  await windowManager.ensureInitialized();
  // ignore: prefer_const_constructors
  final options = WindowOptions(
    size: const Size(1100, 720),
    minimumSize: const Size(420, 560),
    center: true,
    title: 'Pyxee',
    titleBarStyle: TitleBarStyle.hidden,
    backgroundColor: const Color(0x00000000),
  );
  await windowManager.waitUntilReadyToShow(options, () async {
    await windowManager.show();
    await windowManager.focus();
  });

  var prefs = const Prefs();
  try {
    final text = bridge.call('prefsGet');
    if (text is String) prefs = Prefs.fromJson(_decode(text));
  } on Object {
    // Unreadable settings: start with the defaults.
  }
  bridge.call('setVolume', {'percent': prefs.volume});

  runApp(
    ProviderScope(
      overrides: [
        bridgeProvider.overrideWithValue(bridge),
        initialPrefsProvider.overrideWithValue(prefs),
      ],
      child: const PyxeeApp(),
    ),
  );
}

Json _decode(String text) => jsonDecode(text) as Json;

class _StartupError extends StatelessWidget {
  const _StartupError(this.message);

  final String message;

  @override
  Widget build(BuildContext context) => MaterialApp(
    debugShowCheckedModeBanner: false,
    home: Scaffold(body: Center(child: Padding(padding: const EdgeInsets.all(24), child: Text('Pyxee could not start the core.\n\n$message')))),
  );
}
