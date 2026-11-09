import 'dart:async';
import 'dart:math' as math;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../app/state/ui_state.dart';
import '../app/theme/px_theme.dart';
import '../core/models/models.dart';
import '../core/state/providers.dart';
import 'icons.dart';
import 'widgets.dart';

/// Sheets (`Sheet.svelte`): never a native dialog. `bottom` is a card that rises from the bottom (centred on wide
/// windows); `right` is a tab that slides in from the right edge and takes half the window (all of it when narrow).
Future<T?> showPxSheet<T>(
  BuildContext context, {
  required String title,
  required Widget Function(BuildContext context) builder,
  bool right = false,
}) {
  final titleBar = ownTitleBar ? 32.0 : 0.0;
  return showGeneralDialog<T>(
    context: context,
    barrierDismissible: true,
    barrierLabel: 'Close',
    barrierColor: Colors.black.withValues(alpha: 0.5),
    transitionDuration: const Duration(milliseconds: 280),
    pageBuilder: (context, _, _) {
      final p = context.px;
      final size = MediaQuery.sizeOf(context);
      final narrow = size.width < 640;
      final panelWidth = right ? math.min(size.width, math.max(416.0, size.width / 2)) : 448.0;
      final panel = Material(
        color: Colors.transparent,
        child: Container(
          width: right ? panelWidth : (narrow ? size.width : 448),
          constraints: BoxConstraints(maxHeight: right ? double.infinity : size.height * 0.92),
          decoration: BoxDecoration(
            color: p.solid2,
            border: Border.all(color: p.rule),
            borderRadius: right
                ? const BorderRadius.horizontal(left: Radius.circular(12))
                : (narrow ? const BorderRadius.vertical(top: Radius.circular(12)) : BorderRadius.circular(12)),
            boxShadow: [BoxShadow(color: Colors.black.withValues(alpha: p.dark ? 0.3 : 0.16), blurRadius: 30, offset: const Offset(0, 10))],
          ),
          child: right
              ? Column(
                  children: [
                    Container(
                      padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 12),
                      decoration: BoxDecoration(border: Border(bottom: BorderSide(color: p.line))),
                      child: Row(
                        children: [
                          Expanded(child: Text(title, style: PxText.subtitle(p.fg))),
                          PxIconButton('close', label: 'Close $title', onPressed: () => Navigator.of(context).maybePop()),
                        ],
                      ),
                    ),
                    Expanded(child: SingleChildScrollView(padding: const EdgeInsets.fromLTRB(16, 8, 16, 24), child: builder(context))),
                  ],
                )
              : SingleChildScrollView(
                  padding: const EdgeInsets.fromLTRB(16, 16, 16, 24),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Padding(
                        padding: const EdgeInsets.fromLTRB(12, 4, 12, 8),
                        child: Text(title, style: PxText.button(p.fg, FontWeight.w600), maxLines: 1, overflow: TextOverflow.ellipsis),
                      ),
                      builder(context),
                    ],
                  ),
                ),
        ),
      );
      return Padding(
        padding: EdgeInsets.only(top: titleBar),
        child: Align(
          alignment: right ? Alignment.centerRight : (narrow ? Alignment.bottomCenter : Alignment.center),
          child: panel,
        ),
      );
    },
    transitionBuilder: (context, animation, _, child) {
      final curved = CurvedAnimation(parent: animation, curve: Curves.easeOutCubic, reverseCurve: Curves.easeInCubic);
      return FadeTransition(
        opacity: curved,
        child: SlideTransition(
          position: Tween(begin: right ? const Offset(0.3, 0) : const Offset(0, 0.08), end: Offset.zero).animate(curved),
          child: child,
        ),
      );
    },
  );
}

/// One row of a sheet's list: a menu entry, a sleep-timer length, a playlist to add to.
class PxMenuItem extends StatelessWidget {
  const PxMenuItem({super.key, required this.onTap, required this.child});

  final VoidCallback onTap;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    return PxPress(
      onTap: onTap,
      builder: (context, hovered, pressed) => Container(
        constraints: const BoxConstraints(minHeight: 40),
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
        decoration: BoxDecoration(
          color: pressed ? p.press : (hovered ? p.hover : Colors.transparent),
          borderRadius: BorderRadius.circular(rSm),
        ),
        child: DefaultTextStyle(
          style: PxText.body(p.fg),
          child: Row(spacing: 12, children: [Expanded(child: child)]),
        ),
      ),
    );
  }
}

// ----------------------------------------------------------------------------------------------- track menu

/// The menu a row's "More options" button (or a right click) opens.
Future<void> showTrackMenu(BuildContext context, WidgetRef ref, Song song) {
  final actions = ref.read(actionsProvider);
  final toast = ref.read(toastProvider.notifier);
  final liked = ref.read(libraryProvider).likedKeys.contains(song.key);
  return showPxSheet<void>(
    context,
    title: song.title,
    builder: (sheet) {
      void done(String label, VoidCallback run) {
        run();
        Navigator.of(sheet).pop();
        toast.push(label);
      }

      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          PxMenuItem(onTap: () => done('Playing now', () => actions.play([song])), child: const Text('Play now')),
          PxMenuItem(onTap: () => done('Will play next', () => actions.playNext(song)), child: const Text('Play next')),
          PxMenuItem(onTap: () => done('Added to queue', () => actions.addToQueue(song)), child: const Text('Add to queue')),
          PxMenuItem(
            onTap: () {
              Navigator.of(sheet).pop();
              unawaited(ref.read(apiProvider).setLiked(song, !liked));
            },
            child: Text(liked ? 'Unlike' : 'Like'),
          ),
          PxMenuItem(
            onTap: () {
              Navigator.of(sheet).pop();
              if (context.mounted) unawaited(showPlaylistPicker(context, ref, song));
            },
            child: const Text('Add to playlist'),
          ),
          PxMenuItem(
            onTap: () {
              Navigator.of(sheet).pop();
              ref.read(searchRequestProvider.notifier).run(song.artist, kind: SearchKind.songs);
            },
            child: const Text('Go to artist'),
          ),
        ],
      );
    },
  );
}

// ---------------------------------------------------------------------------------------- playlist picker

Future<void> showPlaylistPicker(BuildContext context, WidgetRef ref, Song song) {
  final playlists = ref.read(libraryProvider).playlists;
  final api = ref.read(apiProvider);
  final toast = ref.read(toastProvider.notifier);
  return showPxSheet<void>(
    context,
    title: 'Add to playlist',
    builder: (sheet) {
      final p = sheet.px;
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          PxMenuItem(
            onTap: () {
              Navigator.of(sheet).pop();
              if (!context.mounted) return;
              unawaited(
                showNameSheet(
                  context,
                  title: 'New playlist',
                  confirm: 'Create',
                  run: (name) async {
                    final id = await api.createPlaylist(name.trim(), [song]);
                    toast.push('Added to ${name.trim()}', actionLabel: 'Open', onAction: () => ref.read(navProvider.notifier).openPlaylist(id));
                  },
                ),
              );
            },
            child: Row(
              children: [
                Container(
                  width: 40,
                  height: 40,
                  decoration: BoxDecoration(color: p.surface, borderRadius: BorderRadius.circular(rSm)),
                  child: Center(child: IconTheme(data: IconThemeData(color: p.fg), child: const PxIcon('plus', size: 18))),
                ),
                const SizedBox(width: 12),
                const Text('New playlist…'),
              ],
            ),
          ),
          ConstrainedBox(
            constraints: const BoxConstraints(maxHeight: 288),
            child: ListView(
              shrinkWrap: true,
              children: [
                for (final pl in playlists)
                  PxMenuItem(
                    onTap: () {
                      Navigator.of(sheet).pop();
                      unawaited(api.addToPlaylist(pl.id, [song]).then((_) => toast.push('Added to ${pl.name}')));
                    },
                    child: Row(
                      children: [
                        PxThumb(url: pl.thumbnail, seed: 'pl:${pl.id}', size: 40),
                        const SizedBox(width: 12),
                        Expanded(
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Text(pl.name, style: PxText.body(p.fg), maxLines: 1, overflow: TextOverflow.ellipsis),
                              Text(songsCount(pl.trackCount), style: PxText.caption(p.muted)),
                            ],
                          ),
                        ),
                      ],
                    ),
                  ),
              ],
            ),
          ),
        ],
      );
    },
  );
}

// ------------------------------------------------------------------------------------------------ name sheet

/// An in-page prompt for a name (new playlist, rename, save queue). [run] throws to show a sentence under the field.
Future<void> showNameSheet(
  BuildContext context, {
  required String title,
  required String confirm,
  required Future<void> Function(String name) run,
  String initial = '',
}) {
  return showPxSheet<void>(
    context,
    title: title,
    builder: (sheet) => _NameForm(initial: initial, confirm: confirm, run: run),
  );
}

class _NameForm extends StatefulWidget {
  const _NameForm({required this.initial, required this.confirm, required this.run});

  final String initial;
  final String confirm;
  final Future<void> Function(String name) run;

  @override
  State<_NameForm> createState() => _NameFormState();
}

class _NameFormState extends State<_NameForm> {
  late final TextEditingController _c = TextEditingController(text: widget.initial);
  String _error = '';
  bool _busy = false;

  @override
  void dispose() {
    _c.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    if (_busy || _c.text.trim().isEmpty) return;
    setState(() {
      _busy = true;
      _error = '';
    });
    try {
      await widget.run(_c.text);
      if (mounted) Navigator.of(context).pop();
    } on Object catch (e) {
      if (mounted) setState(() => _error = errorText(e));
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 4),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          PxTextField(controller: _c, hint: 'Name', autofocus: true, maxLength: 100, onSubmitted: (_) => _submit()),
          if (_error.isNotEmpty)
            Padding(
              padding: const EdgeInsets.only(top: 12),
              child: Text(_error, style: PxText.strong(p.accent)),
            ),
          const SizedBox(height: 12),
          ListenableBuilder(
            listenable: _c,
            builder: (context, _) => Row(
              mainAxisAlignment: MainAxisAlignment.end,
              spacing: 8,
              children: [
                PxButton(label: 'Cancel', kind: PxButtonKind.quiet, onPressed: () => Navigator.of(context).pop()),
                PxButton(label: widget.confirm, kind: PxButtonKind.primary, onPressed: _busy || _c.text.trim().isEmpty ? null : _submit),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// `.field`: a text field with a hairline frame and a stronger line under it that turns accent when focused.
class PxTextField extends StatefulWidget {
  const PxTextField({super.key, required this.controller, this.hint, this.autofocus = false, this.maxLength, this.onSubmitted});

  final TextEditingController controller;
  final String? hint;
  final bool autofocus;
  final int? maxLength;
  final ValueChanged<String>? onSubmitted;

  @override
  State<PxTextField> createState() => _PxTextFieldState();
}

class _PxTextFieldState extends State<PxTextField> {
  final _focus = FocusNode();

  @override
  void initState() {
    super.initState();
    _focus.addListener(() => setState(() {}));
  }

  @override
  void dispose() {
    _focus.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    final focused = _focus.hasFocus;
    return Container(
      constraints: const BoxConstraints(minHeight: 32),
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
      decoration: BoxDecoration(
        color: p.surface,
        borderRadius: BorderRadius.circular(rSm),
        border: Border(
          left: BorderSide(color: focused ? p.accent : p.rule),
          right: BorderSide(color: focused ? p.accent : p.rule),
          top: BorderSide(color: focused ? p.accent : p.rule),
          bottom: BorderSide(color: focused ? p.accent : p.muted, width: focused ? 2 : 1),
        ),
      ),
      child: TextField(
        controller: widget.controller,
        focusNode: _focus,
        autofocus: widget.autofocus,
        maxLength: widget.maxLength,
        onSubmitted: widget.onSubmitted,
        cursorColor: p.accent,
        style: PxText.body(p.fg),
        decoration: InputDecoration.collapsed(hintText: widget.hint, hintStyle: PxText.body(p.muted)),
        buildCounter: (context, {required currentLength, required isFocused, maxLength}) => null,
      ),
    );
  }
}
