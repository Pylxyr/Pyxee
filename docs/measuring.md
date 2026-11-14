# Measuring Pyxee's idle resource use

How to find out what Task Manager really charges to Pyxee while it sits hidden, and which process is responsible. Do this
before and after any change that is meant to lower it, with the same song and the same machine state.

## Setup

1. Start Pyxee with debug logging, so the page power diagnostics are written: `set PYXEE_LOG=debug`, then run `pyxee.exe`
   from the same prompt. The log is `%LOCALAPPDATA%\dev.pyxee.app\logs\pyxee.log`.
2. Play a long song (or a playlist on repeat) at a low volume, so music is playing during every measurement.
3. Open Task Manager > **Details**. Add the columns **CPU**, **GPU**, **GPU engine** and **Memory (active private working
   set)**: right-click a column header > *Select columns*. Sort by name.
4. Pyxee shows up as `Pyxee.exe` plus several `msedgewebview2.exe` children. Add the **Command line** column: a WebView2
   process of Pyxee has `--webview-exe-name=Pyxee.exe` in it. Other apps that use WebView2 have their own
   `msedgewebview2.exe` rows; leave those out. Group the rows so `Pyxee.exe` and its children are read together.
5. Tell the five WebView2 roles apart by their `--type=` argument:

   | Role | Command line | What it does for Pyxee |
   | --- | --- | --- |
   | browser | no `--type` | one per user data folder; the manager of all the others |
   | renderer | `--type=renderer` | runs the page (Svelte, CSS animations, the event handlers) |
   | GPU | `--type=gpu-process` | paints the page, the backdrop and the blur |
   | utility | `--type=utility` | network and storage services |
   | crashpad | `--type=crashpad-handler` | crash reporting, idle |

## The four states

Measure each for **60 seconds** with music playing. Read the values at the end, and note the highest CPU seen in the
minute. CPU is Task Manager's per-process percentage (of all cores); GPU is the highest of the engine percentages.

1. **Window open**: the main window visible, on Home, not minimized.
2. **Minimized**: the main window minimized to the taskbar.
3. **Hidden in tray**: the main window closed to the tray (close button), flyout closed.
4. **Mini player only**: the main window closed to the tray, the mini player open.

Optionally a fifth: the tray **flyout** open and nothing else.

## Record

Fill one table per run. Write the Pyxee version and the date on top; a short note on the machine (CPU, Windows version,
WebView2 runtime version from `edge://version` or *Apps > Microsoft Edge WebView2 Runtime*).

| State | Process | CPU % (avg / max) | GPU % | Working set (MB) | Notes (is it busy? which one?) |
| --- | --- | --- | --- | --- | --- |
| Window open | Pyxee.exe | | | | |
| | browser | | | | |
| | GPU | | | | |
| | renderer | | | | |
| | utility | | | | |
| | crashpad | | | | |
| | **total** | | | | |
| Minimized | (same rows) | | | | |
| Hidden in tray | (same rows) | | | | |
| Mini player only | (same rows) | | | | |

Which process is busy decides what is worth fixing:

* **renderer or GPU busy while hidden**: the page is not asleep, or woke up. See the log lines below.
* **browser, utility or crashpad hold the memory, renderer and GPU are near zero**: the page sleeps, but the WebView2
  processes themselves stay. Only closing the window (destroying the webview) frees that.
* **`Pyxee.exe` busy**: the Rust side (timers, threads, events) is waking up for nobody.

## What the log says about a hidden page

With `PYXEE_LOG=debug`, hiding or minimizing a window writes, per window (`main`, `mini`, `flyout`):

* `<label> page: asleep`, or a warning `WebView2 declined to suspend it` / `could not be suspended: ...`: the answer of
  `TrySuspend`;
* `<label> page: IsSuspended = true|false after TrySuspend`: what WebView2 reports right after;
* about 5 seconds later, either `<label> page: still asleep after 5 s`, or the warning
  `<label> page: awake again 5 s after it was put to sleep (visible: ...); sent to it meanwhile: ...`. The last part lists
  the events the app sent to every window in that time (notices, update progress). `nothing the app sent` means the wake
  came from WebView2 itself, a request made by the page, or a window event.

`visible: Some(true)` in that warning means something made the page visible again (the window was shown, or restored);
`visible: Some(false)` with `IsSuspended` false means it woke while hidden.

Search the log with: `findstr /C:"page:" pyxee.log`.
