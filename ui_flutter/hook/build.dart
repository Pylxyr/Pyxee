import 'dart:io';

import 'package:code_assets/code_assets.dart';
import 'package:hooks/hooks.dart';

/// Bundles the Rust bridge library that `cargo build -p pyxee-bridge --release` leaves in the workspace's target
/// directory. It does not run cargo: hooks run with a stripped environment, and which Rust toolchain builds the
/// library (and where FFmpeg is) is up to whoever runs the build.
void main(List<String> args) async {
  await build(args, (input, output) async {
    if (!input.config.buildCodeAssets) return;
    final fileName = switch (input.config.code.targetOS) {
      OS.windows => 'pyxee_bridge.dll',
      OS.linux => 'libpyxee_bridge.so',
      final os => throw UnsupportedError('Pyxee has no bridge library for $os'),
    };
    final library = input.packageRoot.resolve('../target/release/$fileName');
    if (!File.fromUri(library).existsSync()) {
      throw StateError(
        'Rust bridge not found at ${library.toFilePath()}. '
        'Run: cargo build -p pyxee-bridge --release',
      );
    }
    output.assets.code.add(
      CodeAsset(
        package: input.packageName,
        name: 'core/bridge/bridge_native.dart',
        linkMode: DynamicLoadingBundled(),
        file: library,
      ),
    );
    output.dependencies.add(library);
  });
}
