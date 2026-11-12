import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:material_ui/material_ui.dart';

import '../../app/theme/px_theme.dart';
import '../../core/models/system.dart';
import '../../core/state/providers.dart';
import '../../shared/sheets.dart';
import '../../shared/widgets.dart';
import 'env_vars.dart';

/// Settings > Advanced: the environment variables Pyxee reads, kept in a `.env` file in the data folder.
Future<void> showAdvancedSheet(BuildContext context) => showPxSheet<void>(
  context,
  title: 'Advanced settings',
  right: true,
  builder: (_) => const _Advanced(),
);

class _Advanced extends ConsumerStatefulWidget {
  const _Advanced();

  @override
  ConsumerState<_Advanced> createState() => _AdvancedState();
}

class _AdvancedState extends ConsumerState<_Advanced> {
  EnvSettings? _env;
  String? _error;
  bool _saving = false;
  bool _saved = false;
  final Map<String, TextEditingController> _controllers = {};
  final Map<String, String> _values = {};

  @override
  void initState() {
    super.initState();
    unawaited(_load());
  }

  @override
  void dispose() {
    for (final c in _controllers.values) {
      c.dispose();
    }
    super.dispose();
  }

  Future<void> _load() async {
    setState(() => _error = null);
    try {
      final env = await ref.read(apiProvider).envGet();
      if (!mounted) return;
      setState(() {
        _env = env;
        _fill(env);
      });
    } on Object catch (e) {
      if (mounted) setState(() => _error = errorText(e));
    }
  }

  /// Puts what the file holds into the fields.
  void _fill(EnvSettings env) {
    for (final v in env.vars) {
      _values[v.name] = v.value;
      final c = _controllers[v.name];
      if (c == null) {
        _controllers[v.name] = TextEditingController(text: v.value);
      } else if (c.text != v.value) {
        c.text = v.value;
      }
    }
  }

  void _edit(String name, String value) {
    _values[name] = value;
    if (_saved) setState(() => _saved = false);
  }

  Future<void> _save() async {
    final env = _env;
    if (env == null) return;
    setState(() {
      _saving = true;
      _saved = false;
      _error = null;
    });
    try {
      // Every setting is sent, the ones this page does not show as they are now.
      final next = await ref.read(apiProvider).envSet([for (final v in env.vars) EnvVar(v.name, _values[v.name] ?? v.value)]);
      if (!mounted) return;
      setState(() {
        _env = next;
        _fill(next);
        _saving = false;
        _saved = true;
      });
    } on Object catch (e) {
      if (mounted) {
        setState(() {
          _saving = false;
          _error = errorText(e);
        });
      }
    }
  }

  Widget _field(BuildContext context, EnvVarInfo info, EnvSettings env) {
    final p = context.px;
    final fromWindows = env.vars.any((v) => v.name == info.name && v.fromWindows);
    final choices = info.choices;
    final Widget input = choices != null
        ? Align(
            alignment: Alignment.centerLeft,
            child: PxSegmented<String>(
              value: _values[info.name] ?? '',
              onSelect: (c) => setState(() {
                _values[info.name] = c;
                _saved = false;
              }),
              items: [
                for (final c in choices) (id: c, label: c.isEmpty ? 'Default' : c, disabled: false, tooltip: null),
              ],
            ),
          )
        : PxTextField(controller: _controllers[info.name]!, hint: info.placeholder, mono: true, onChanged: (v) => _edit(info.name, v));
    return Padding(
      padding: const EdgeInsets.fromLTRB(12, 12, 12, 4),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(info.title, style: PxText.strong(p.fg)),
          const SizedBox(height: 2),
          Text(info.about, style: PxText.caption(p.muted)),
          const SizedBox(height: 8),
          input,
          const SizedBox(height: 4),
          Text(info.fill, style: PxText.caption(p.muted)),
          if (fromWindows)
            Padding(
              padding: const EdgeInsets.only(top: 4),
              child: Text('Windows also sets this variable, and that value is the one used.', style: PxText.caption(p.accent)),
            ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final p = context.px;
    final env = _env;
    if (env == null) {
      return Padding(
        padding: const EdgeInsets.all(12),
        child: _error == null
            ? Text('Loading…', style: PxText.body(p.muted))
            : Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(_error!, style: PxText.body(p.accent)),
                  const SizedBox(height: 8),
                  PxButton(label: 'Try again', onPressed: () => unawaited(_load())),
                ],
              ),
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(12, 0, 12, 4),
          child: Text(
            env.file == null
                ? 'Pyxee has no data folder, so there is nowhere to save these.'
                : 'Kept in ${env.file}. A change is used from the next start of Pyxee, and an empty field removes the setting.',
            style: PxText.caption(p.muted),
          ),
        ),
        for (final info in envVars) _field(context, info, env),
        Padding(
          padding: const EdgeInsets.fromLTRB(12, 16, 12, 8),
          child: Row(
            spacing: 12,
            children: [
              PxButton(label: _saving ? 'Saving…' : 'Save', kind: PxButtonKind.primary, onPressed: _saving || env.file == null ? null : () => unawaited(_save())),
              Expanded(
                child: _error != null
                    ? Text(_error!, style: PxText.caption(p.accent))
                    : _saved
                    ? Text('Saved. The new values are used from the next start.', style: PxText.caption(p.muted))
                    : const SizedBox.shrink(),
              ),
            ],
          ),
        ),
      ],
    );
  }
}
