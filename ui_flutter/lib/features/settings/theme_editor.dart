import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../app/state/ui_state.dart';
import '../../app/theme/colors.dart';
import '../../app/theme/px_theme.dart';
import '../../core/theme/prefs.dart';
import '../../shared/sheets.dart';
import '../../shared/widgets.dart';

/// Customize theme: the name, a dark or light base, the seven colours as hex, readability warnings, and Import and
/// Export as text. A built-in theme is never edited itself: the draft is a new custom theme made from it. The app
/// shows the draft live while the sheet is open and goes back to the saved theme when it closes.
Future<void> showThemeEditor(BuildContext context, ThemeDef start) => showPxSheet<void>(
  context,
  title: 'Customize theme',
  builder: (sheet) => _ThemeEditor(start: start),
);

class _ThemeEditor extends ConsumerStatefulWidget {
  const _ThemeEditor({required this.start});

  final ThemeDef start;

  @override
  ConsumerState<_ThemeEditor> createState() => _ThemeEditorState();
}

class _ThemeEditorState extends ConsumerState<_ThemeEditor> {
  late ThemeDef _draft;
  late ThemeDef _origin;
  late final ThemePreviewNotifier _preview;
  final _name = TextEditingController();
  final _import = TextEditingController();
  final _hex = <String, TextEditingController>{};
  String? _error;
  bool _importing = false;

  @override
  void initState() {
    super.initState();
    _preview = ref.read(themePreviewProvider.notifier);
    for (final f in colorFields) {
      _hex[f.$1] = TextEditingController();
    }
    _begin(widget.start);
  }

  @override
  void dispose() {
    _name.dispose();
    _import.dispose();
    for (final c in _hex.values) {
      c.dispose();
    }
    // Back to the saved theme; after this frame, because the tree is being taken apart right now.
    Future.microtask(() => _preview.set(null));
    super.dispose();
  }

  void _begin(ThemeDef def) {
    _draft = def.builtin ? ThemeDef('', '${def.name} (custom)', def.dark, 'Custom theme', def.colors) : def;
    _origin = _draft;
    _syncFields();
    _error = null;
    _publish();
  }

  void _syncFields() {
    _name.text = _draft.name;
    for (final f in colorFields) {
      _hex[f.$1]!.text = colorToHex(_draft.colors.get(f.$1));
    }
  }

  void _publish() => Future.microtask(() => _preview.set(_draft));

  void _setDraft(ThemeDef next) {
    setState(() => _draft = next);
    _publish();
  }

  void _save() {
    try {
      final saved = ref.read(prefsProvider.notifier).saveCustom(_draft);
      ref.read(prefsProvider.notifier).select(saved.id);
      Navigator.of(context).pop();
      ref.read(toastProvider.notifier).push('Saved theme “${saved.name}”');
    } on StateError catch (e) {
      setState(() => _error = e.message);
    }
  }

  void _delete() {
    final name = _draft.name;
    ref.read(prefsProvider.notifier).deleteCustom(_draft.id);
    Navigator.of(context).pop();
    ref.read(toastProvider.notifier).push('Deleted theme “$name”');
  }

  Future<void> _copyExport() async {
    await Clipboard.setData(ClipboardData(text: exportTheme(_draft)));
    if (mounted) ref.read(toastProvider.notifier).push('Theme copied. Paste it anywhere to share it.');
  }

  void _doImport() {
    try {
      final def = parseThemeText(_import.text);
      setState(() {
        _draft = def;
        _origin = def;
        _error = null;
        _importing = false;
        _import.clear();
      });
      _syncFields();
      _publish();
    } on FormatException catch (e) {
      setState(() => _error = e.message);
    }
  }

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    final saved = _draft.id.isNotEmpty && ref.watch(prefsProvider).custom.any((t) => t.id == _draft.id);
    final warnings = contrastChecks(_draft.colors).where((c) => !c.ok).toList();
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text('Name', style: PxText.caption(p.muted)),
          const SizedBox(height: 4),
          PxTextField(controller: _name, maxLength: 40, onChanged: (v) => _setDraft(_draft.copyWith(name: v))),
          const SizedBox(height: 12),
          Align(
            alignment: Alignment.centerLeft,
            child: PxSegmented<bool>(
              value: _draft.dark,
              onSelect: (dark) => _setDraft(_draft.copyWith(dark: dark)),
              items: const [
                (id: true, label: 'Dark base', disabled: false, tooltip: null),
                (id: false, label: 'Light base', disabled: false, tooltip: null),
              ],
            ),
          ),
          Padding(padding: const EdgeInsets.only(top: 4), child: Text('The base sets form controls, scroll bars and the window frame.', style: PxText.caption(p.muted))),
          const SizedBox(height: 12),
          for (final f in colorFields)
            Padding(
              padding: const EdgeInsets.only(bottom: 8),
              child: Row(
                children: [
                  Container(
                    width: 48,
                    height: 36,
                    decoration: BoxDecoration(color: _draft.colors.get(f.$1), borderRadius: BorderRadius.circular(rSm), border: Border.all(color: p.line)),
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(f.$2, style: PxText.body(p.fg)),
                        Text(f.$3, style: PxText.caption(p.muted), maxLines: 1, overflow: TextOverflow.ellipsis),
                      ],
                    ),
                  ),
                  const SizedBox(width: 12),
                  SizedBox(
                    width: 96,
                    child: PxTextField(
                      controller: _hex[f.$1]!,
                      mono: true,
                      onChanged: (v) {
                        final c = normalizeHex(v);
                        if (c != null) _setDraft(_draft.copyWith(colors: _draft.colors.withKey(f.$1, c)));
                      },
                    ),
                  ),
                ],
              ),
            ),
          if (warnings.isNotEmpty)
            Container(
              margin: const EdgeInsets.only(top: 4),
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(borderRadius: BorderRadius.circular(rMd), border: Border.all(color: p.rule)),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [for (final c in warnings) Text('${c.label} is hard to read (${c.ratio.toStringAsFixed(1)}:1, aim for ${c.min == c.min.roundToDouble() ? c.min.round() : c.min}:1 or more).', style: PxText.caption(p.fg))],
              ),
            ),
          const SizedBox(height: 12),
          Container(
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(borderRadius: BorderRadius.circular(rMd), border: Border.all(color: p.rule)),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                PxPress(
                  onTap: () => setState(() => _importing = !_importing),
                  builder: (context, _, _) => Text('${_importing ? '▾' : '▸'} Import a theme', style: PxText.body(p.fg)),
                ),
                if (_importing) ...[
                  const SizedBox(height: 8),
                  PxTextField(controller: _import, hint: 'Paste theme text here', mono: true, lines: 5, onChanged: (_) => setState(() {})),
                  const SizedBox(height: 8),
                  Align(alignment: Alignment.centerLeft, child: PxButton(label: 'Import', onPressed: _import.text.trim().isEmpty ? null : _doImport)),
                ],
              ],
            ),
          ),
          if (_error != null) Padding(padding: const EdgeInsets.only(top: 12), child: Text(_error!, style: PxText.strong(p.accent))),
          const SizedBox(height: 16),
          Row(
            children: [
              Expanded(
                child: Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: [
                    PxButton(label: 'Save', kind: PxButtonKind.primary, onPressed: _save),
                    PxButton(label: 'Cancel', onPressed: () => Navigator.of(context).pop()),
                    PxButton(
                      label: 'Reset',
                      onPressed: () {
                        setState(() {
                          _draft = _origin;
                          _error = null;
                        });
                        _syncFields();
                        _publish();
                      },
                    ),
                    PxButton(label: 'Export', onPressed: () => unawaited(_copyExport())),
                  ],
                ),
              ),
              if (saved) PxButton(label: 'Delete', onPressed: _delete),
            ],
          ),
        ],
      ),
    );
  }
}
