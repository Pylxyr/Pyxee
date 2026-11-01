# Performance: what keeps the window busy, and how to measure it

A music player should be quiet while a song plays. In Pyxee the page draws a frame only when something visibly
changes: no animation runs without end, nothing is blurred at run time on a large area, the position is reported once
a second, and the page under Now Playing is not drawn while Now Playing covers it. These are guarded by
`ui/src/lib/css-budget.test.ts` (never-ending animations and large blurs), `ui/src/lib/player-reactivity.svelte.test.ts`
(a position report re-runs only what shows the position) and the engine tests (one position report per second).

## Measuring on your PC

**Task Manager, Details tab.** Right-click a column header, *Select columns*, and add **CPU**, **GPU**, **GPU engine**,
**Power usage** and **Command line**. Pyxee's web content runs in several `msedgewebview2.exe` processes started by
`pyxee-app.exe`; the command line tells them apart: `--type=renderer` is the page (JavaScript, style, layout, paint),
`--type=gpu-process` is drawing and compositing, the one without `--type` is the browser process. Watch a process for a
minute while a song plays with the window visible. The *Processes* tab, under *Pyxee*, shows them grouped.

**Pyxee's own count (`PYXEE_PERF=1`).** Turn on *Performance log* in Settings > Advanced (or put `PYXEE_PERF=1` in
`.env`) and restart. Every ten seconds the log (Settings > Advanced shows its folder) gets a line like

    perf: 10s state=0 position=10 other=0 dom-changes=30 animations=0 long-tasks=0 long-frames=0 hidden=false

`position` is the number of position reports the window received (about one a second while playing),
`dom-changes` how many times the page was changed, `animations` how many animations are running right now (it should
be 0 while a song plays), and `long-tasks`/`long-frames` how many tasks or frames took over 50 ms. A window that is
asleep (minimised, in the tray) should show `hidden=true` and `position=0`. The log counts what makes the page busy,
not frames: counting frames needs a `requestAnimationFrame` loop, which would make the page draw sixty a second by
itself.

**DevTools.** Close Pyxee, then in a terminal:

    set WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222
    pyxee-app.exe

Open `edge://inspect` in Edge, *Configure...*, add `localhost:9222`, and click *inspect* under Pyxee's page. In the
console, `document.getAnimations().filter(a => a.playState === 'running')` lists what runs (it should be empty while
a song plays). In *Performance*, record ten seconds of playback: the frames lane should show one frame a second, not
sixty. *More tools > Rendering* has *Paint flashing* (the areas that are drawn flash green; only the clock and the
seek bar should flash, once a second) and *Frame Rendering Stats*.

## The measurement harness (Linux, relative numbers)

`scripts/perf/measure.py` drives the built UI (`npm run build` in `ui/`) in WebKitGTK under Xvfb and reports the CPU of
the web process per view while a demo song plays. It is software-rendered, so use it to compare a change with the one
before, not as what WebView2 will use on a GPU. See the script's header for what it needs.
