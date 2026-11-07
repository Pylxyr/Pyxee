import 'dart:async';
import 'dart:io';

import 'package:flutter_acrylic/flutter_acrylic.dart' as acrylic;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';
import 'package:tray_manager/tray_manager.dart' as tray;
import 'package:window_manager/window_manager.dart';

import '../core/state/providers.dart';
import '../core/theme/prefs.dart';
import 'hwnd.dart';

/// Window and system integration around the app: close to tray, the tray icon and its menu, the window material
/// (Mica), the media keys, and quitting. None of it paints anything or runs a timer.
class DesktopHost extends ConsumerStatefulWidget {
  const DesktopHost({super.key, required this.child});

  final Widget child;

  @override
  ConsumerState<DesktopHost> createState() => _DesktopHostState();
}

class _DesktopHostState extends ConsumerState<DesktopHost> with WindowListener, tray.TrayListener {
  bool? _dark;
  bool? _mica;

  @override
  void initState() {
    super.initState();
    windowManager.addListener(this);
    tray.trayManager.addListener(this);
    unawaited(_init());
  }

  @override
  void dispose() {
    windowManager.removeListener(this);
    tray.trayManager.removeListener(this);
    super.dispose();
  }

  Future<void> _init() async {
    await windowManager.setPreventClose(true);
    try {
      await tray.trayManager.setIcon(Platform.isWindows ? 'assets/tray.ico' : 'assets/tray.png');
      await _menu(playing: ref.read(isPlayingProvider));
    } on Object {
      // No tray on this desktop (GNOME without the AppIndicator extension): closing then really closes.
    }
    // The system media controls need the window handle (Windows) and exist from here on.
    try {
      ref.read(bridgeProvider).call('attachMedia', {'hwnd': mainWindowHandle()});
    } on Object {
      // Media keys are a convenience; the core logs why they are missing.
    }
  }

  Future<void> _menu({required bool playing}) => tray.trayManager.setContextMenu(
    tray.Menu(
      items: [
        tray.MenuItem(key: 'show', label: 'Open Pyxee'),
        tray.MenuItem.separator(),
        tray.MenuItem(key: 'toggle', label: playing ? 'Pause' : 'Play'),
        tray.MenuItem(key: 'next', label: 'Next'),
        tray.MenuItem(key: 'previous', label: 'Previous'),
        tray.MenuItem.separator(),
        tray.MenuItem(key: 'quit', label: 'Quit'),
      ],
    ),
  );

  Future<void> _show() async {
    await windowManager.show();
    if (await windowManager.isMinimized()) await windowManager.restore();
    await windowManager.focus();
  }

  Future<void> _quit() async {
    await ref.read(apiProvider).quit();
    try {
      await tray.trayManager.destroy();
    } on Object {
      // Nothing to remove.
    }
    await windowManager.destroy();
  }

  @override
  void onWindowClose() {
    if (ref.read(prefsProvider).closeToTray) {
      unawaited(windowManager.hide());
    } else {
      unawaited(_quit());
    }
  }

  @override
  void onTrayIconMouseDown() => unawaited(_show());

  @override
  void onTrayIconRightMouseDown() => unawaited(tray.trayManager.popUpContextMenu());

  @override
  void onTrayMenuItemClick(tray.MenuItem menuItem) {
    final actions = ref.read(actionsProvider);
    switch (menuItem.key) {
      case 'show':
        unawaited(_show());
      case 'toggle':
        actions.toggle();
      case 'next':
        actions.next();
      case 'previous':
        actions.previous();
      case 'quit':
        unawaited(_quit());
    }
  }

  /// Mica follows the theme's brightness and the setting; elsewhere the window stays solid.
  void _syncMaterial() {
    if (!Platform.isWindows) return;
    final dark = Theme.of(context).brightness == Brightness.dark;
    final mica = ref.read(prefsProvider).mica && micaSupported;
    if (dark == _dark && mica == _mica) return;
    _dark = dark;
    _mica = mica;
    unawaited(
      acrylic.Window.setEffect(
        effect: mica ? acrylic.WindowEffect.mica : acrylic.WindowEffect.disabled,
        dark: dark,
      ),
    );
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _syncMaterial();
  }

  @override
  Widget build(BuildContext context) {
    ref.listen(isPlayingProvider, (_, playing) {
      unawaited(_menu(playing: playing).catchError((Object _) {}));
    });
    ref.listen(prefsProvider.select((p) => p.mica), (_, _) => _syncMaterial());
    ref.listen(windowRequestProvider, (_, next) {
      next.whenData((action) => unawaited(action == 'quit' ? _quit() : _show()));
    });
    return widget.child;
  }
}
