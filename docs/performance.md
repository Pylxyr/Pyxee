# Performance: what keeps the window busy, and how to measure it

A music player should be quiet while a song plays. The engine reports the position once a second (the engine tests guard
that), and the Flutter app should draw only what visibly changes. Two parts of the UI have never been profiled and are
the first places to look: the blurred backdrop (`PxBackdrop`) and the seek bar (`PxSeek`), which rebuilds four times a
second while a song plays.

## Measuring on your PC

**Task Manager, Details tab.** Pyxee is one process now (`Pyxee.exe`), with no web-view helper processes. Add the
columns **CPU**, **GPU**, **GPU engine**, **Power usage** and **Memory (active private working set)**: right-click a
column header, *Select columns*. Watch the process for a minute while a song plays with the window visible, then again
with the window minimised and with it closed to the tray. A window that is not visible should cost close to nothing.

**Flutter profile mode.** A debug build is far slower than a release build, so never measure in debug. In `ui_flutter`:

    flutter run -d windows --profile

then open DevTools (the link `flutter run` prints) and record in **Performance** for ten seconds of playback. While a
song plays with nothing else going on, the frame chart should show few frames, not sixty a second. Turn on *Track widget
rebuilds* to see which widgets rebuild, and *Repaint rainbow* to see which areas repaint (only the clock and the seek bar
should, a few times a second). Check a long list (a large playlist or the history) as well, since lists are where a
blurred or rebuilt area costs most.

Change something only when a measurement shows it is a problem.

## History

The Tauri version had its own measurements for WebView2: a performance log, a Linux measurement harness and a
step-by-step idle-use guide. They measured web-view work that no longer exists. They are in the `tauri-final` tag
(`docs/performance.md`, `docs/measuring.md`, `scripts/perf/measure.py`).
