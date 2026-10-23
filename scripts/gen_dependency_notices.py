#!/usr/bin/env python3
"""Writes crates/app/licenses/THIRD_PARTY_DEPENDENCIES.txt: the licence notices of the Rust crates and npm packages
that ship in Pyxee. Generated, never hand-edited.

  python scripts/gen_dependency_notices.py          write the file (needs cargo-about 0.9.x and `npm ci --prefix ui`)
  python scripts/gen_dependency_notices.py --check  fail if the file is out of date (the same tools, then a diff)
  python scripts/gen_dependency_notices.py --locks  fail if Cargo.lock or ui/package-lock.json changed since the file
                                                    was written (no tools needed; scripts/tests runs this)

Rust: `cargo about` (about.toml, about.hbs) for the Windows build's crates. npm: the packages the UI is built from at
run time, found by walking ui/node_modules from `@tauri-apps/api` (its only runtime dependency) and `svelte` (a dev
dependency in package.json, but its runtime is compiled into the UI bundle); dev-only tooling is not listed.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "crates" / "app" / "licenses" / "THIRD_PARTY_DEPENDENCIES.txt"
UI = ROOT / "ui"
LOCKS = {"Cargo.lock": ROOT / "Cargo.lock", "ui/package-lock.json": UI / "package-lock.json"}
# Packages whose code ends up in the UI bundle although package.json lists them as development dependencies.
BUNDLED_DEV_PACKAGES = ["svelte"]
LICENSE_FILE = re.compile(r"^(licen[sc]e|copying|notice)(\..*)?$", re.IGNORECASE)
HASH_LINE = re.compile(r"^(Cargo\.lock|ui/package-lock\.json) sha256: ([0-9a-f]{64})$", re.MULTILINE)
RULE = "=" * 80


def lock_hash(path: Path) -> str:
    """SHA-256 of a lock file with Windows line endings folded to \\n, so a checkout with autocrlf agrees."""
    return hashlib.sha256(path.read_bytes().replace(b"\r\n", b"\n")).hexdigest()


def recorded_hashes(text: str) -> dict[str, str]:
    return {name: digest for name, digest in HASH_LINE.findall(text)}


def stale_locks(text: str) -> list[str]:
    """The lock files whose current hash differs from the one recorded in `text`."""
    recorded = recorded_hashes(text)
    return [name for name, path in LOCKS.items() if recorded.get(name) != lock_hash(path)]


def rust_notices() -> str:
    out = subprocess.run(
        ["cargo", "about", "generate", "--locked", "--manifest-path", str(ROOT / "crates/app/Cargo.toml"),
         str(ROOT / "about.hbs")],
        cwd=ROOT, check=True, capture_output=True, text=True,
    )
    return out.stdout.replace("\r\n", "\n").rstrip() + "\n"


def _find_package(name: str, from_dir: Path) -> Path | None:
    """Node's resolution: node_modules/<name> in this folder or any parent up to ui/."""
    here = from_dir
    while True:
        candidate = here / "node_modules" / name
        if (candidate / "package.json").is_file():
            return candidate
        if here == UI or here == here.parent:
            return None
        here = here.parent


def _license_field(meta: dict) -> str:
    value = meta.get("license") or meta.get("licenses") or "UNKNOWN"
    if isinstance(value, dict):
        return str(value.get("type", "UNKNOWN"))
    if isinstance(value, list):
        return " OR ".join(str(v.get("type", v)) if isinstance(v, dict) else str(v) for v in value)
    return str(value)


def npm_packages() -> list[dict]:
    package_json = json.loads((UI / "package.json").read_text(encoding="utf-8"))
    roots = sorted(set(package_json.get("dependencies", {})) | set(BUNDLED_DEV_PACKAGES))
    found: dict[str, dict] = {}
    todo: list[tuple[str, Path]] = [(name, UI) for name in roots]
    while todo:
        name, from_dir = todo.pop()
        folder = _find_package(name, from_dir)
        if folder is None:
            raise SystemExit(f"{name} is not installed under ui/node_modules; run `npm ci --prefix ui` first")
        meta = json.loads((folder / "package.json").read_text(encoding="utf-8"))
        key = f"{meta['name']}@{meta['version']}"
        if key in found:
            continue
        texts = []
        for f in sorted(folder.iterdir()):
            if f.is_file() and LICENSE_FILE.match(f.name):
                texts.append((f.name, f.read_text(encoding="utf-8", errors="replace").replace("\r\n", "\n").strip()))
        found[key] = {"key": key, "license": _license_field(meta), "homepage": meta.get("homepage", ""), "texts": texts}
        for dep in meta.get("dependencies", {}):
            todo.append((dep, folder))
    return [found[k] for k in sorted(found)]


def npm_notices() -> str:
    lines = []
    for pkg in npm_packages():
        lines += [RULE, f"{pkg['key']}  (license: {pkg['license']})"]
        if pkg["homepage"]:
            lines.append(pkg["homepage"])
        lines.append("")
        if pkg["texts"]:
            for name, text in pkg["texts"]:
                lines += [f"[{name}]", text, ""]
        else:
            lines += ["(the package ships no licence file; the licence named above is what its package.json declares)", ""]
    return "\n".join(lines).rstrip() + "\n"


def build() -> str:
    hashes = "\n".join(f"{name} sha256: {lock_hash(path)}" for name, path in LOCKS.items())
    return (
        "THIRD-PARTY DEPENDENCY NOTICES\n"
        "Pyxee ships the Rust crates and the npm packages listed below. This file is generated by\n"
        "scripts/gen_dependency_notices.py (cargo-about for the crates, a walk of ui/node_modules for npm); do not edit it.\n"
        "The program, its own crates and the other bundled components are described in THIRD_PARTY.txt.\n"
        "Generated from these lock files (a change to either means this file must be regenerated):\n"
        f"{hashes}\n\n"
        f"{RULE}\nPART 1: RUST CRATES (Windows build)\n{RULE}\n\n"
        f"{rust_notices()}\n"
        f"{RULE}\nPART 2: NPM PACKAGES IN THE USER INTERFACE (@tauri-apps/api and the Svelte runtime with its dependencies)\n{RULE}\n\n"
        f"{npm_notices()}"
    )


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--check", action="store_true", help="regenerate in memory and fail if the file differs")
    ap.add_argument("--locks", action="store_true", help="only compare the recorded lock-file hashes")
    args = ap.parse_args()
    if args.locks:
        if not OUT.is_file():
            print(f"{OUT.relative_to(ROOT)} is missing; run scripts/gen_dependency_notices.py", file=sys.stderr)
            return 1
        stale = stale_locks(OUT.read_text(encoding="utf-8"))
        if stale:
            print(f"{', '.join(stale)} changed since {OUT.name} was generated; run scripts/gen_dependency_notices.py",
                  file=sys.stderr)
            return 1
        return 0
    text = build()
    if args.check:
        if not OUT.is_file() or OUT.read_text(encoding="utf-8").replace("\r\n", "\n") != text:
            print(f"{OUT.relative_to(ROOT)} is out of date; run scripts/gen_dependency_notices.py", file=sys.stderr)
            return 1
        return 0
    OUT.write_text(text, encoding="utf-8", newline="\n")
    print(f"wrote {OUT.relative_to(ROOT)} ({len(text.splitlines())} lines)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
