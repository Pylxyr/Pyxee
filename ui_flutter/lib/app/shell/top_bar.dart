import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../features/settings/settings_sheet.dart';
import '../../shared/icons.dart';
import '../../shared/widgets.dart';
import '../state/ui_state.dart';
import '../theme/px_theme.dart';

/// The search box and, in a narrow window (which has no tabs), the Settings button. The suggestions drop down under
/// the box as you type.
class PxTopBar extends ConsumerStatefulWidget {
  const PxTopBar({super.key, this.showSettings = false});

  final bool showSettings;

  @override
  ConsumerState<PxTopBar> createState() => _PxTopBarState();
}

class _PxTopBarState extends ConsumerState<PxTopBar> {
  final _text = TextEditingController();
  final _focus = FocusNode();
  final _portal = OverlayPortalController();
  final _link = LayerLink();
  String _typed = '';
  int _active = -1;
  bool _dismissed = false;

  @override
  void initState() {
    super.initState();
    _focus.addListener(() {
      if (!_focus.hasFocus) {
        // The list goes a moment after the box loses focus, so a click on a suggestion still lands.
        Timer(const Duration(milliseconds: 150), () {
          if (mounted && !_focus.hasFocus) setState(() => _dismissed = true);
        });
      } else {
        setState(() => _dismissed = false);
      }
    });
  }

  @override
  void dispose() {
    _text.dispose();
    _focus.dispose();
    super.dispose();
  }

  void _go(String q) {
    _text.text = q;
    _text.selection = TextSelection.collapsed(offset: q.length);
    _active = -1;
    _dismissed = true;
    ref.read(searchRequestProvider.notifier).run(q);
    setState(() {});
  }

  KeyEventResult _key(FocusNode node, KeyEvent e, List<String> items) {
    if (e is! KeyDownEvent) return KeyEventResult.ignored;
    if (e.logicalKey == LogicalKeyboardKey.arrowDown && items.isNotEmpty) {
      setState(() => _active = (_active + 1) % items.length);
      return KeyEventResult.handled;
    }
    if (e.logicalKey == LogicalKeyboardKey.arrowUp && items.isNotEmpty) {
      setState(() => _active = (_active - 1 + items.length) % items.length);
      return KeyEventResult.handled;
    }
    if (e.logicalKey == LogicalKeyboardKey.escape && items.isNotEmpty) {
      setState(() {
        _dismissed = true;
        _active = -1;
      });
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    final width = MediaQuery.sizeOf(context).width;
    final suggestions = _dismissed ? const <String>[] : (ref.watch(suggestionsProvider(_typed)).value ?? const <String>[]);
    final open = suggestions.isNotEmpty;
    if (open && !_portal.isShowing) {
      WidgetsBinding.instance.addPostFrameCallback((_) => mounted && !_portal.isShowing ? _portal.show() : null);
    } else if (!open && _portal.isShowing) {
      WidgetsBinding.instance.addPostFrameCallback((_) => mounted && _portal.isShowing ? _portal.hide() : null);
    }
    final focused = _focus.hasFocus;
    final box = ConstrainedBox(
      constraints: BoxConstraints(maxWidth: width >= 1536 ? 768 : 576),
      child: CompositedTransformTarget(
        link: _link,
        child: OverlayPortal(
          controller: _portal,
          overlayChildBuilder: (context) => CompositedTransformFollower(
            link: _link,
            targetAnchor: Alignment.bottomLeft,
            followerAnchor: Alignment.topLeft,
            offset: const Offset(0, 8),
            child: Align(
              alignment: Alignment.topLeft,
              child: SizedBox(
                width: _link.leaderSize?.width,
                child: Material(
                  color: Colors.transparent,
                  child: Container(
                    padding: const EdgeInsets.symmetric(vertical: 4),
                    decoration: BoxDecoration(
                      color: p.solid2,
                      border: Border.all(color: p.line),
                      borderRadius: BorderRadius.circular(rMd),
                      boxShadow: [BoxShadow(color: Colors.black.withValues(alpha: p.dark ? 0.3 : 0.16), blurRadius: 30, offset: const Offset(0, 10))],
                    ),
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        for (var i = 0; i < suggestions.length; i++)
                          PxPress(
                            onTap: () => _go(suggestions[i]),
                            builder: (context, hovered, _) => Container(
                              color: hovered || _active == i ? p.hover : Colors.transparent,
                              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
                              child: Row(
                                children: [
                                  IconTheme(data: IconThemeData(color: p.muted), child: const PxIcon('search', size: 16)),
                                  const SizedBox(width: 12),
                                  Expanded(child: Text(suggestions[i], style: PxText.body(p.fg), maxLines: 1, overflow: TextOverflow.ellipsis)),
                                ],
                              ),
                            ),
                          ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          ),
          child: Container(
            height: 36,
            padding: const EdgeInsets.symmetric(horizontal: 12),
            decoration: BoxDecoration(
              color: p.surface,
              borderRadius: BorderRadius.circular(rMd),
              border: Border(
                left: BorderSide(color: focused ? p.accent : p.line),
                right: BorderSide(color: focused ? p.accent : p.line),
                top: BorderSide(color: focused ? p.accent : p.line),
                bottom: BorderSide(color: focused ? p.accent : p.muted, width: focused ? 2 : 1),
              ),
            ),
            child: Row(
              children: [
                IconTheme(data: IconThemeData(color: focused ? p.fg : p.muted), child: const PxIcon('search', size: 18)),
                const SizedBox(width: 12),
                Expanded(
                  child: Focus(
                    onKeyEvent: (n, e) => _key(n, e, suggestions),
                    child: TextField(
                      controller: _text,
                      focusNode: _focus,
                      cursorColor: p.accent,
                      style: PxText.body(p.fg),
                      textInputAction: TextInputAction.search,
                      decoration: InputDecoration.collapsed(hintText: 'Search songs, albums, artists', hintStyle: PxText.body(p.muted)),
                      onChanged: (v) => setState(() {
                        _typed = v;
                        _active = -1;
                        _dismissed = false;
                      }),
                      onSubmitted: (v) => _go(_active >= 0 && _active < suggestions.length ? suggestions[_active] : v),
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
    return Row(
      children: [
        Expanded(child: Align(alignment: Alignment.centerLeft, child: box)),
        if (widget.showSettings) ...[
          const SizedBox(width: 8),
          PxIconButton('settings', label: 'Settings', large: true, onPressed: () => unawaited(showSettingsSheet(context)), iconSize: 20),
        ],
      ],
    );
  }
}
