"""The release pipeline is made of files that name each other: workflows call `cargo xtask` commands and scripts, read
files by path, and the packaging code lists the files it expects. Nothing else notices when one of them is renamed or
removed (the Tauri removal left them all pointing at folders that were gone), so this does."""
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
XTASK = (ROOT / "xtask/src/main.rs").read_text(encoding="utf-8")
RELEASE_RS = (ROOT / "xtask/src/release.rs").read_text(encoding="utf-8")

# Everything that runs or documents the pipeline.
PIPELINE_FILES = sorted(
    [*ROOT.glob(".github/workflows/*.yml"), *ROOT.glob(".github/actions/*/*.yml"), *ROOT.glob("scripts/*.ps1"),
     *ROOT.glob("scripts/*.cmd"), *ROOT.glob("scripts/*.py"), *ROOT.glob("scripts/*.sh"),
     *ROOT.glob("packaging/**/*.nsi"), ROOT / "docs/releasing.md", ROOT / "docs/development.md"]
)
# The folders of the old shell. A mention in a sentence about history is fine; a path a script would use is not.
GONE = re.compile(r"crates[\\/]app\b|\bpyxee-app\b|[\\\s\"'(]ui[\\/](?:src|dist|node_modules|package)|tauri\.(?:conf|bundle)|\bnpm (?:ci|run|install)\b")


def xtask_commands() -> set[str]:
    """The command names `cargo xtask` dispatches on (the `Some("name") =>` arms of main)."""
    return set(re.findall(r'Some\("([a-z-]+)"\)\s*=>', XTASK))


class Alignment(unittest.TestCase):
    def test_xtask_has_the_packaging_commands(self):
        for name in ("tools", "ffmpeg", "stage", "installer", "portable", "smoke", "sign-file", "sign-sums", "update-keygen"):
            self.assertIn(name, xtask_commands())

    def test_every_xtask_command_that_is_called_exists(self):
        known = xtask_commands()
        for f in PIPELINE_FILES:
            if f.name == "test_pipeline_alignment.py":
                continue
            for m in re.finditer(r"cargo xtask ([a-z][a-z-]*)", f.read_text(encoding="utf-8", errors="replace")):
                self.assertIn(m.group(1), known, f"{f.relative_to(ROOT)} calls `cargo xtask {m.group(1)}`, which does not exist")

    def test_nothing_points_at_the_removed_tauri_shell(self):
        for f in PIPELINE_FILES:
            if f.name == "test_pipeline_alignment.py":
                continue
            text = f.read_text(encoding="utf-8", errors="replace")
            hit = GONE.search(text)
            self.assertIsNone(hit, f"{f.relative_to(ROOT)} still mentions the removed shell: {hit.group(0).strip() if hit else ''}")
        self.assertFalse((ROOT / "crates/app").exists(), "crates/app is gone")
        self.assertFalse((ROOT / "ui").exists(), "ui/ is gone")

    def test_files_the_packaging_expects_exist(self):
        shipped = re.findall(r'"(licenses/[^"]+)",\s*"[^"]+"\s*\)?', RELEASE_RS)
        self.assertGreaterEqual(len(shipped), 4, "release.rs lists the licence texts it ships")
        for rel in shipped:
            self.assertTrue((ROOT / rel).is_file(), f"release.rs ships {rel} but it is not in the repository")
        for rel in ("assets/icons/icon.ico", "assets/icons/installer/header.bmp", "assets/icons/installer/sidebar.bmp"):
            self.assertTrue((ROOT / rel).is_file(), rel)
        self.assertTrue((ROOT / "packaging/windows/installer.nsi").is_file())

    def test_the_workflow_copies_only_files_that_exist(self):
        text = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
        for rel in re.findall(r"Copy-Item (licenses/\S+)", text):
            self.assertTrue((ROOT / rel).is_file(), f"release.yml copies {rel}")

    def test_the_installer_script_gets_every_define_xtask_passes_and_needs(self):
        nsi = (ROOT / "packaging/windows/installer.nsi").read_text(encoding="utf-8")
        passed = set(re.findall(r'/D([A-Z_]+)=', XTASK))
        needed = set(re.findall(r"!ifndef ([A-Z_]+)\n\s+!error", nsi))
        self.assertTrue(needed, "the script checks its required defines")
        self.assertLessEqual(needed, passed, "the script requires a define that xtask does not pass")
        for name in passed:
            self.assertIn(name, nsi, f"xtask passes /D{name} but the script never uses it")

    def test_the_installer_is_named_the_way_the_updater_and_the_workflow_look_for_it(self):
        self.assertIn("_x64-setup.exe", RELEASE_RS)
        wf = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
        self.assertIn("target/installer/*-setup.exe", wf)
        self.assertIn("target/portable/*_x64-portable.zip", wf)
        self.assertIn('create_dir_all(&out_dir)', XTASK)
        self.assertIn('"target/installer"', XTASK)
        self.assertIn('"target/portable"', XTASK)

    def test_the_stage_folder_is_the_one_the_installer_and_the_smoke_test_use(self):
        self.assertEqual(XTASK.count('"target/stage/Pyxee"'), 3)  # stage, installer define, smoke default

    def test_the_flutter_exe_name_matches_what_the_runner_builds(self):
        cmake = (ROOT / "ui_flutter/windows/CMakeLists.txt").read_text(encoding="utf-8")
        name = re.search(r'set\(BINARY_NAME "([^"]+)"\)', cmake).group(1)
        self.assertIn(f'FLUTTER_EXE: &str = "{name}.exe"', RELEASE_RS)

    def test_the_bridge_library_name_matches_the_hook(self):
        hook = (ROOT / "ui_flutter/hook/build.dart").read_text(encoding="utf-8")
        dll = re.search(r"OS\.windows => '([^']+)'", hook).group(1)
        self.assertIn(f'BRIDGE_DLL: &str = "{dll}"', RELEASE_RS)

    def test_ci_builds_the_flutter_app_the_way_the_release_does(self):
        ci = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        wf = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
        for step in ("cargo build -p pyxee-bridge --release --locked", "flutter build windows --release"):
            self.assertIn(step, ci)
            self.assertIn(step, wf)
        self.assertIn("flutter", re.search(r"needs: \[([^\]]+)\]\n\s+runs-on: ubuntu-latest\n\s+steps:\n\s+- name: All jobs passed", ci).group(1))

    def test_release_notes_know_the_current_layout(self):
        import sys
        sys.path.insert(0, str(ROOT / "scripts"))
        import release_notes as rn

        for path in sorted(ROOT.glob("crates/*/src/lib.rs")) + sorted(ROOT.glob("ui_flutter/lib/main.dart")):
            self.assertNotEqual(rn.area_of(str(path.relative_to(ROOT)).replace("\\", "/")), "Other", path)


if __name__ == "__main__":
    unittest.main()
