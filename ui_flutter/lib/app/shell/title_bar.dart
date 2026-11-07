import 'dart:async';

import 'package:material_ui/material_ui.dart';
import 'package:window_manager/window_manager.dart';

import '../theme/metrics.dart';

/// The window's own title bar: drag area, minimise, maximise and close. Close goes through the window manager, so
/// close-to-tray applies (see `DesktopHost`).
class TitleBar extends StatefulWidget {
  const TitleBar({super.key});

  @override
  State<TitleBar> createState() => _TitleBarState();
}

class _TitleBarState extends State<TitleBar> with WindowListener {
  bool _maximized = false;

  @override
  void initState() {
    super.initState();
    windowManager.addListener(this);
    unawaited(windowManager.isMaximized().then((m) => mounted ? setState(() => _maximized = m) : null));
  }

  @override
  void dispose() {
    windowManager.removeListener(this);
    super.dispose();
  }

  @override
  void onWindowMaximize() => setState(() => _maximized = true);

  @override
  void onWindowUnmaximize() => setState(() => _maximized = false);

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return SizedBox(
      height: Metrics.titleBarHeight,
      child: Row(
        children: [
          Expanded(
            child: DragToMoveArea(
              child: Align(
                alignment: Alignment.centerLeft,
                child: Padding(
                  padding: const EdgeInsets.only(left: 16),
                  child: Text(
                    'Pyxee',
                    style: Theme.of(context).textTheme.labelLarge?.copyWith(color: scheme.onSurfaceVariant),
                  ),
                ),
              ),
            ),
          ),
          _Button(Icons.remove, 'Minimise', () => unawaited(windowManager.minimize())),
          _Button(
            _maximized ? Icons.filter_none : Icons.crop_square,
            _maximized ? 'Restore' : 'Maximise',
            () => unawaited(_maximized ? windowManager.unmaximize() : windowManager.maximize()),
          ),
          _Button(Icons.close, 'Close', () => unawaited(windowManager.close())),
        ],
      ),
    );
  }
}

class _Button extends StatelessWidget {
  const _Button(this.icon, this.tooltip, this.onPressed);

  final IconData icon;
  final String tooltip;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) => SizedBox(
    width: 46,
    height: Metrics.titleBarHeight,
    child: IconButton(
      onPressed: onPressed,
      tooltip: tooltip,
      iconSize: 16,
      style: IconButton.styleFrom(shape: const RoundedRectangleBorder()),
      icon: Icon(icon),
    ),
  );
}
