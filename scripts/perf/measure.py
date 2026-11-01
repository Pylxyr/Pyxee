#!/usr/bin/env python3
"""CPU of the web process per view while a demo song plays: WebKitGTK under Xvfb, driving the built UI (ui/dist) that
runs against its mock backend. Software-rendered on whatever cores the machine has, so the numbers are RELATIVE: use
them to compare a change with the one before it, not as what WebView2 will use on a real GPU.

Needs: xvfb, webkit2gtk-driver, `pip install selenium`, and `npm run build` in ui/ first.
  python3 scripts/perf/measure.py [--window 12] [--views home,queue,now-playing,lyrics] [--shots DIR] [--label NAME]
Prints one line per view and, with --json, a JSON object (for diffing runs)."""
import argparse, json, os, signal, subprocess, sys, time

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
DIST = os.path.join(ROOT, "ui", "dist")
MINI = "/usr/lib/x86_64-linux-gnu/webkit2gtk-4.1/MiniBrowser"
CLK = os.sysconf("SC_CLK_TCK")


def procs(prefix):
    out = []
    for pid in filter(str.isdigit, os.listdir("/proc")):
        try:
            with open(f"/proc/{pid}/stat") as f:
                s = f.read()
            comm = s[s.index("(") + 1 : s.rindex(")")]
            if comm.startswith(prefix):
                rest = s[s.rindex(")") + 2 :].split()
                out.append((int(pid), int(rest[11]) + int(rest[12])))
        except (OSError, ValueError):
            pass
    return out


def cpu_percent(prefix, seconds):
    a = dict(procs(prefix))
    t0 = time.time()
    time.sleep(seconds)
    b = dict(procs(prefix))
    dt = time.time() - t0
    return 100.0 * sum(max(0, b[p] - a.get(p, b[p])) for p in b) / CLK / dt


def start(cmd, **kw):
    return subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, stdin=subprocess.DEVNULL, start_new_session=True, **kw)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--window", type=float, default=12)
    ap.add_argument("--views", default="home,queue,now-playing,lyrics")
    ap.add_argument("--shots", default=None)
    ap.add_argument("--label", default="run")
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--size", default="1280,800")
    ap.add_argument("--query", default="", help="query string for the mock page, e.g. seek=pixel (a variant switch)")
    args = ap.parse_args()
    w, h = (int(x) for x in args.size.split(","))
    kids = []
    env = dict(os.environ, DISPLAY=":99")
    try:
        kids.append(start(["Xvfb", ":99", "-screen", "0", f"{w}x{h}x24", "-nolisten", "tcp"]))
        kids.append(start([sys.executable, "-m", "http.server", "8765", "--bind", "127.0.0.1", "--directory", DIST]))
        kids.append(start(["WebKitWebDriver", "--port=4444"], env=env))
        time.sleep(2.5)
        from selenium import webdriver
        from selenium.webdriver.webkitgtk.options import Options

        o = Options()
        o.binary_location = MINI
        o.add_argument("--automation")
        o.set_capability("webkitgtk:browserOptions", {"binary": MINI, "args": ["--automation"]})
        d = webdriver.Remote("http://127.0.0.1:4444", options=o)
        d.set_window_size(w, h)
        d.get("http://127.0.0.1:8765/" + ("?" + args.query.lstrip("?") if args.query else ""))
        time.sleep(2)
        click = """const t=arguments[0];const els=[...document.querySelectorAll('button,[role=tab],a')];
          const el=els.find(e=>(e.getAttribute('aria-label')||e.textContent||'').trim()===t);
          if(!el)return false; el.click(); return true;"""
        # start a song: the first row of the first list
        d.execute_script("document.querySelector('.trow-main').click()")
        time.sleep(2.5)
        # What the page changes per second (DOM mutations): shows what each position event makes Svelte touch.
        d.execute_script(
            "window.__mut=0;new MutationObserver(r=>{window.__mut+=r.length}).observe(document.body,"
            "{subtree:true,childList:true,attributes:true,characterData:true});"
        )
        result, order = {}, []
        for view in args.views.split(","):
            if view == "home":
                d.execute_script(click, "Home")
            elif view == "queue":
                d.execute_script(click, "Queue")
            elif view == "now-playing":
                d.execute_script(click, "Expand Now Playing") or d.execute_script(click, "Open Now Playing")
            elif view == "lyrics":
                d.execute_script(click, "Lyrics")
            time.sleep(2.5)
            m0 = d.execute_script("return window.__mut")
            cpu = cpu_percent("WebKitWebProces", args.window)
            mut = (d.execute_script("return window.__mut") - m0) / args.window
            anim = d.execute_script("return document.getAnimations().filter(a=>a.playState==='running').length")
            # the seek bars on screen: their width and what a song of this length makes them move, in pixels a second
            bars = d.execute_script(
                "return [...document.querySelectorAll('input.slider[aria-label=Seek]')].map(e=>[e.getBoundingClientRect().width,+e.max])"
            )
            pxs = [round(wd / (mx / 1000), 1) for wd, mx in bars if mx > 0]
            result[view] = {"cpu": round(cpu, 1), "running_animations": anim, "dom_mutations_per_s": round(mut, 1), "seek_px_per_s": pxs}
            if args.shots:
                os.makedirs(args.shots, exist_ok=True)
                d.get_screenshot_as_file(os.path.join(args.shots, f"{args.label}-{view}.png"))
            print(f"{args.label:>10} {view:<12} cpu={cpu:5.1f}%  running animations={anim}  dom mutations/s={mut:5.1f}  seek bar px/s={pxs}", flush=True)
        if args.json:
            print(json.dumps({args.label: result}))
        d.quit()
    finally:
        for k in kids:
            try:
                os.killpg(k.pid, signal.SIGTERM)
            except OSError:
                pass


if __name__ == "__main__":
    main()
