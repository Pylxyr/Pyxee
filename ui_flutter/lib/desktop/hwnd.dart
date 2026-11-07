import 'dart:ffi';
import 'dart:io';

import 'package:ffi/ffi.dart';

typedef _FindNative = IntPtr Function(Pointer<Utf16>, Pointer<Utf16>);
typedef _Find = int Function(Pointer<Utf16>, Pointer<Utf16>);

/// The native handle of the app's window on Windows (0 elsewhere), which the system media controls need. The runner
/// registers its window class as `FLUTTER_RUNNER_WIN32_WINDOW` (windows/runner/win32_window.cpp).
int mainWindowHandle() {
  if (!Platform.isWindows) return 0;
  final find = DynamicLibrary.open('user32.dll').lookupFunction<_FindNative, _Find>('FindWindowW');
  final className = 'FLUTTER_RUNNER_WIN32_WINDOW'.toNativeUtf16();
  try {
    return find(className, nullptr);
  } finally {
    malloc.free(className);
  }
}
