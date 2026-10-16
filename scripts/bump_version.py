#!/usr/bin/env python3
"""Keeps the app version in one step across every file that carries it.

python scripts/bump_version.py 1.1.0          set the version everywhere, print next steps
python scripts/bump_version.py --check v1.1.0 fail unless every file equals the tag (used by CI)
python scripts/bump_version.py --next patch   print the next version (patch, minor or major)
python scripts/bump_version.py --current      print the current version

Files: Cargo.toml ([workspace.package]), Cargo.lock (the pyxee-* and xtask entries),
ui/package.json, ui/package-lock.json and crates/app/tauri.conf.json.
Standard library only.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CARGO_TOML = ROOT / "Cargo.toml"
CARGO_LOCK = ROOT / "Cargo.lock"
PACKAGE = ROOT / "ui" / "package.json"
PACKAGE_LOCK = ROOT / "ui" / "package-lock.json"
TAURI_CONF = ROOT / "crates" / "app" / "tauri.conf.json"

SEMVER = re.compile(r"^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$")
# [workspace.package] table: the first `version = "..."` after the header.
WORKSPACE_VERSION = re.compile(r'(\[workspace\.package\][^\[]*?^version = ")([^"]*)(")', re.MULTILINE | re.DOTALL)
# Cargo.lock entries for our own crates (pyxee-* and xtask); third-party crates are never touched.
LOCK_ENTRY = re.compile(r'(^name = "(?:pyxee-[\w-]+|xtask)"\nversion = ")([^"]*)(")', re.MULTILINE)


def _read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def _write(path: Path, text: str) -> None:
    # newline="" keeps the file's own line endings (the repo is edited on Windows).
    with path.open("w", encoding="utf-8", newline="") as handle:
        handle.write(text)


def _json(path: Path) -> dict:
    return json.loads(_read(path))


def _write_json(path: Path, data: dict) -> None:
    _write(path, json.dumps(data, indent=2, ensure_ascii=False) + "\n")


def read_versions() -> dict[str, str | None]:
    """{label: version} for every place that carries the version."""
    found: dict[str, str | None] = {}
    match = WORKSPACE_VERSION.search(_read(CARGO_TOML))
    found["Cargo.toml [workspace.package]"] = match.group(2) if match else None
    if CARGO_LOCK.exists():
        for entry in LOCK_ENTRY.finditer(_read(CARGO_LOCK)):
            name = entry.group(0).split('"')[1]
            found[f"Cargo.lock {name}"] = entry.group(2)
    found["ui/package.json"] = _json(PACKAGE).get("version")
    if PACKAGE_LOCK.exists():
        lock = _json(PACKAGE_LOCK)
        found["ui/package-lock.json"] = lock.get("version")
        found["ui/package-lock.json (root package)"] = (lock.get("packages", {}).get("") or {}).get("version")
    found["crates/app/tauri.conf.json"] = _json(TAURI_CONF).get("version")
    return found


def current_version() -> str:
    return _json(PACKAGE)["version"]


def next_version(current: str, kind: str) -> str:
    """The version after `current`. A pre-release suffix is dropped first (1.2.0-rc1 + patch = 1.2.1);
    to promote a release candidate to 1.2.0, give the exact version instead."""
    if not SEMVER.match(current):
        raise ValueError(f"'{current}' is not MAJOR.MINOR.PATCH[-suffix].")
    major, minor, patch = (int(part) for part in current.split("-", 1)[0].split("."))
    if kind == "major":
        return f"{major + 1}.0.0"
    if kind == "minor":
        return f"{major}.{minor + 1}.0"
    if kind == "patch":
        return f"{major}.{minor}.{patch + 1}"
    raise ValueError(f"Unknown bump kind '{kind}' (use patch, minor or major).")


def check(tag: str) -> int:
    expected = tag.removeprefix("v")
    if not SEMVER.match(expected):
        print(f"'{tag}' is not a vMAJOR.MINOR.PATCH[-suffix] tag.", file=sys.stderr)
        return 1
    versions = read_versions()
    for label, found in versions.items():
        print(f"{'ok  ' if found == expected else 'FAIL'} {label}: {found}")
    if any(found != expected for found in versions.values()):
        print(
            f"\nThe tag says {expected} but the files above disagree. Run: python scripts/bump_version.py {expected}",
            file=sys.stderr,
        )
        return 1
    return 0


def bump(version: str) -> int:
    if not SEMVER.match(version):
        print(f"'{version}' is not MAJOR.MINOR.PATCH (optionally -rc1 etc.).", file=sys.stderr)
        return 1
    text, count = WORKSPACE_VERSION.subn(lambda m: f"{m.group(1)}{version}{m.group(3)}", _read(CARGO_TOML), count=1)
    if not count:
        print("[workspace.package] version not found in Cargo.toml", file=sys.stderr)
        return 1
    _write(CARGO_TOML, text)
    if CARGO_LOCK.exists():
        _write(CARGO_LOCK, LOCK_ENTRY.sub(lambda m: f"{m.group(1)}{version}{m.group(3)}", _read(CARGO_LOCK)))
    for path in (PACKAGE, PACKAGE_LOCK):
        if path.exists():
            data = _json(path)
            data["version"] = version
            if "" in data.get("packages", {}):
                data["packages"][""]["version"] = version
            _write_json(path, data)
    conf = _json(TAURI_CONF)
    conf["version"] = version
    _write_json(TAURI_CONF, conf)
    print(f"Version set to {version} in Cargo.toml, Cargo.lock, ui/package.json, ui/package-lock.json and tauri.conf.json.")
    return 0


def main(argv: list[str]) -> int:
    try:
        if len(argv) == 3 and argv[1] == "--check":
            return check(argv[2])
        if len(argv) == 3 and argv[1] == "--next":
            print(next_version(current_version(), argv[2]))
            return 0
        if len(argv) == 2 and argv[1] == "--current":
            print(current_version())
            return 0
        if len(argv) == 2 and not argv[1].startswith("-"):
            return bump(argv[1].removeprefix("v"))
    except ValueError as exc:
        print(exc, file=sys.stderr)
        return 1
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
