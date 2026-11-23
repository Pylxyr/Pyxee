#!/usr/bin/env python3
"""Overlays must be opaque in every theme, also under Windows 11 Mica (where --surface and --layer are see-through tints).

Opens the queue drawer (a 1286x830 window, where there is no room for the lane column), the pop-up song menu and the
Settings dialog against the vite dev server (mock backend, `?backdrop=mica` makes it wear the Mica tints) and fails when
one of them is not fully opaque, is covered by something, or does not close with Escape / a click outside.

Needs: `pip install playwright && playwright install chromium`, and the dev server running:
  cd ui && npm run dev            # in one shell
  python3 scripts/ui-check/overlays.py [--url http://localhost:5173] [--themes dark,oled,light] [--shots DIR]
Exit code 1 when something fails."""
import argparse, json, re, sys
from playwright.sync_api import sync_playwright

ALL = "dark light oled deepblue sky graphitegold turquoise ember paper amethyst forest crimson synthwave steel midnightsun abyss mint".split()
ALPHA = re.compile(r"/\s*([\d.]+)\s*\)$")


def alpha(css: str) -> float:
    """Alpha of a computed colour: rgb(...) / rgba(...) / color(srgb r g b / a)."""
    m = ALPHA.search(css)
    if m:
        return float(m.group(1))
    if css.startswith("rgba("):
        return float(css.rstrip(")").split(",")[-1])
    return 1.0


def settings_for(theme: str, dynamic: bool) -> str:
    system = theme in ("system-dark", "system-light")
    return json.dumps({"followSystem": system, "themeId": "dark" if system else theme, "darkId": "dark", "lightId": "light", "dynamicColor": dynamic})


def run(url: str, themes: list[str], shots: str | None) -> int:
    failures: list[str] = []

    def check(ok: bool, what: str):
        if not ok:
            failures.append(what)
            print("FAIL", what)

    with sync_playwright() as p:
        browser = p.chromium.launch()
        for theme, dynamic in [(t, False) for t in themes] + [("dark", True), ("light", True)]:
            for backdrop in ("mica", "none"):
                label = f"{theme}{'+art' if dynamic else ''}/{backdrop}"
                ctx = browser.new_context(viewport={"width": 1286, "height": 830}, color_scheme="light" if theme == "system-light" else "dark")
                ctx.add_init_script(f"localStorage.setItem('pyxee.theme', JSON.stringify({settings_for(theme, dynamic)}))")
                page = ctx.new_page()
                page.goto(f"{url}/?home&backdrop={backdrop}")
                page.wait_for_selector(".mix-card")
                page.locator(".mix-card").first.click()
                page.wait_for_selector("[data-testid=player-bar]")
                page.wait_for_timeout(500)

                # 1. The queue drawer.
                page.get_by_label("Show the queue").click()
                page.wait_for_selector(".lane[data-drawer]")
                page.wait_for_timeout(300)
                drawer = page.locator(".lane[data-drawer]")
                bg = drawer.evaluate("e => getComputedStyle(e).backgroundColor")
                check(alpha(bg) == 1.0, f"{label}: the drawer's colour {bg} is see-through")
                box = drawer.bounding_box()
                top = page.evaluate(
                    "([x, y]) => { const e = document.elementFromPoint(x, y); return !!e && !!e.closest('.lane[data-drawer]'); }",
                    [box["x"] + box["width"] / 2, box["y"] + box["height"] / 2],
                )
                check(top, f"{label}: something covers the middle of the drawer")
                # Every row of the drawer is the thing on top at its own centre (nothing from the page draws over it).
                rows = page.locator(".lane[data-drawer] .lane-list li")
                for i in range(min(rows.count(), 8)):
                    rb = rows.nth(i).bounding_box()
                    ok = page.evaluate(
                        "([x, y]) => { const e = document.elementFromPoint(x, y); return !!e && !!e.closest('.lane[data-drawer]'); }",
                        [rb["x"] + rb["width"] / 2, rb["y"] + rb["height"] / 2],
                    )
                    check(ok, f"{label}: row {i} of the drawer is covered")
                if shots:
                    page.screenshot(path=f"{shots}/drawer-{theme}{'-art' if dynamic else ''}-{backdrop}.png")

                # 2. The pop-up song menu from a drawer row.
                rows.nth(2).click(button="right")
                page.wait_for_selector("[data-testid=ctx-menu]")
                page.wait_for_timeout(200)
                menu = page.locator("[data-testid=ctx-menu]")
                mbg = menu.evaluate("e => getComputedStyle(e).backgroundColor")
                check(alpha(mbg) == 1.0, f"{label}: the song menu's colour {mbg} is see-through")
                page.keyboard.press("Escape")
                page.wait_for_timeout(200)
                check(page.locator("[data-testid=ctx-menu]").count() == 0, f"{label}: Escape leaves the song menu open")
                check(page.locator(".lane[data-drawer]").count() == 1, f"{label}: Escape on the menu also closed the drawer")

                # 3. Escape, then a click outside, close the drawer.
                page.keyboard.press("Escape")
                page.wait_for_timeout(200)
                check(page.locator(".lane[data-drawer]").count() == 0, f"{label}: Escape does not close the drawer")
                page.get_by_label("Show the queue").click()
                page.wait_for_selector(".lane[data-drawer]")
                page.mouse.click(200, 300)
                page.wait_for_timeout(200)
                check(page.locator(".lane[data-drawer]").count() == 0, f"{label}: a click outside does not close the drawer")

                # 4. The Settings dialog.
                page.get_by_label("Settings").first.click()
                page.wait_for_selector(".sheet-panel")
                page.wait_for_timeout(300)
                panel = page.locator(".sheet-panel").first
                pbg = panel.evaluate("e => getComputedStyle(e).backgroundColor")
                check(alpha(pbg) == 1.0, f"{label}: the Settings dialog's colour {pbg} is see-through")
                if shots:
                    page.screenshot(path=f"{shots}/settings-{theme}{'-art' if dynamic else ''}-{backdrop}.png")
                ctx.close()
                print("ok  " if not any(f.startswith(label) for f in failures) else "BAD ", label)
        browser.close()
    print(f"{len(failures)} failure(s)")
    return 1 if failures else 0


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--url", default="http://localhost:5173")
    ap.add_argument("--themes", default=",".join(ALL + ["system-dark", "system-light"]))
    ap.add_argument("--shots")
    a = ap.parse_args()
    sys.exit(run(a.url.rstrip("/"), a.themes.split(","), a.shots))
