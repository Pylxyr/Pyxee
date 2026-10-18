"""FFmpeg is linked statically under the LGPL: the build must stay LGPL-only and the notices must match what is built.

These tests make the release checklist's licence items automatic where they can be (docs/windows-checklist.md).
"""
import json
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BUILD_SCRIPT = (ROOT / "scripts/build-ffmpeg.sh").read_text(encoding="utf-8")
NOTICES = (ROOT / "crates/app/licenses/THIRD_PARTY.txt").read_text(encoding="utf-8")
LGPL_TEXT = ROOT / "crates/app/licenses/FFmpeg-LGPL-2.1.txt"
RELEASE_YML = (ROOT / ".github/workflows/release.yml").read_text(encoding="utf-8")
BUNDLE_CONF = json.loads((ROOT / "crates/app/tauri.bundle.conf.json").read_text(encoding="utf-8"))


def pin(name):
    m = re.search(rf'^{name}="([^"]+)"', BUILD_SCRIPT, re.M)
    assert m, f"{name} is not set in scripts/build-ffmpeg.sh"
    return m.group(1)


def code_lines():
    return [l for l in BUILD_SCRIPT.splitlines() if not l.strip().startswith("#")]


class FFmpegLicence(unittest.TestCase):
    def test_the_build_enables_nothing_that_is_not_lgpl(self):
        # --enable-gpl / --enable-version3 / --enable-nonfree change the licence; any external library
        # (--enable-libx264, --enable-libfdk-aac...) brings its own terms, so none is allowed without a review.
        bad = [l.strip() for l in code_lines() if re.search(r"--enable-(gpl|version3|nonfree|lib\w+)", l)]
        self.assertEqual(bad, [], "this would change the licence FFmpeg is used under")

    def test_the_build_script_refuses_a_gpl_or_nonfree_result(self):
        self.assertIn("CONFIG_GPL", BUILD_SCRIPT)
        self.assertIn("CONFIG_NONFREE", BUILD_SCRIPT)

    def test_the_notices_name_the_version_that_is_built(self):
        self.assertIn(pin("FFMPEG_COMMIT"), NOTICES, "update crates/app/licenses/THIRD_PARTY.txt with the new commit")
        self.assertIn(pin("FFMPEG_TAG"), NOTICES, "update crates/app/licenses/THIRD_PARTY.txt with the new tag")

    def test_the_notices_carry_what_the_lgpl_asks_for(self):
        for needed in ["LGPL", "FFmpeg-LGPL-2.1.txt", "scripts/build-ffmpeg.sh", "github.com/FFmpeg/FFmpeg", "Pyxee is MIT"]:
            self.assertIn(needed, NOTICES)

    def test_the_licence_text_is_shipped_complete(self):
        text = LGPL_TEXT.read_text(encoding="utf-8")
        self.assertIn("GNU LESSER GENERAL PUBLIC LICENSE", text)
        self.assertIn("Version 2.1, February 1999", text)
        self.assertGreater(len(text.splitlines()), 400, "the full licence text, not a summary")

    def test_the_installer_and_the_release_both_carry_it(self):
        self.assertIn("licenses/*", BUNDLE_CONF["bundle"]["resources"], "the installer bundles crates/app/licenses/*")
        self.assertIn("FFmpeg-LGPL-2.1.txt", RELEASE_YML, "the release must upload the licence text next to the notices")
        self.assertIn("THIRD_PARTY_NOTICES.txt", RELEASE_YML)


if __name__ == "__main__":
    unittest.main()
