import 'dart:async';

import 'package:material_ui/material_ui.dart';
import 'package:window_manager/window_manager.dart';

import '../../shared/icons.dart';
import '../../shared/logo.dart';
import '../../shared/widgets.dart';
import '../theme/px_theme.dart';

/// The window's own 32 px title bar (`TitleBar.svelte`): the empty part drags the window (a double click maximizes it)
/// and the three buttons call the window. Close goes through the window manager, so close-to-tray applies.
class PxTitleBar extends StatefulWidget {
  const PxTitleBar({super.key, required this.showBrand});

  /// The tabs already show the name in the wide layout.
  final bool showBrand;

  @override
  State<PxTitleBar> createState() => _PxTitleBarState();
}

class _PxTitleBarState extends State<PxTitleBar> with WindowListener {
  bool _maximized = false;

  @override
  void initState() {
    super.initState();
    windowManager.addListener(this);
    unawaited(windowManager.isMaximized().then((m) {
      if (mounted) setState(() => _maximized = m);
    }));
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

  void _toggleMaximize() => unawaited(_maximized ? windowManager.unmaximize() : windowManager.maximize());

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    return Container(
      height: 32,
      color: p.bg,
      child: Row(
        children: [
          Expanded(
            child: GestureDetector(
              behavior: HitTestBehavior.opaque,
              onDoubleTap: _toggleMaximize,
              child: DragToMoveArea(
                child: Align(
                  alignment: Alignment.centerLeft,
                  child: widget.showBrand
                      ? Padding(
                          padding: const EdgeInsets.only(left: 12),
                          child: Row(mainAxisSize: MainAxisSize.min, children: [const PxLogo(size: 14), const SizedBox(width: 8), Text('Pyxee', style: PxText.caption(p.muted))]),
                        )
                      : const SizedBox.expand(),
                ),
              ),
            ),
          ),
          _CaptionButton(icon: 'minimize', label: 'Minimize', onTap: () => unawaited(windowManager.minimize())),
          _CaptionButton(icon: _maximized ? 'restore' : 'maximize', label: _maximized ? 'Restore' : 'Maximize', onTap: _toggleMaximize),
          _CaptionButton(icon: 'close', label: 'Close', danger: true, onTap: () => unawaited(windowManager.close())),
        ],
      ),
    );
  }
}

class _CaptionButton extends StatelessWidget {
  const _CaptionButton({required this.icon, required this.label, required this.onTap, this.danger = false});

  final String icon;
  final String label;
  final VoidCallback onTap;
  final bool danger;

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    return PxPress(
      onTap: onTap,
      semanticLabel: label,
      builder: (context, hovered, pressed) {
        final red = danger && (hovered || pressed);
        return Container(
          width: 46,
          height: 32,
          color: red ? const Color(0xFFC42B1C) : (pressed ? p.press : (hovered ? p.hover : Colors.transparent)),
          alignment: Alignment.center,
          child: IconTheme(data: IconThemeData(color: red ? const Color(0xFFFFFFFF) : p.fg), child: PxIcon(icon, size: 14)),
        );
      },
    );
  }
}
