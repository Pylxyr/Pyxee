"""FFmpeg is linked statically under the LGPL: the build must stay LGPL-only and the notices must match what is built.

These tests make the release checklist's licence items automatic where they can be (docs/windows-checklist.md).
"""
import json
import re
import sys
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


YTDLP_FILES = ["GPL-3.0.txt", "yt-dlp-THIRD_PARTY_LICENSES.txt"]
XTASK = (ROOT / "xtask/src/main.rs").read_text(encoding="utf-8")


class YtDlpLicence(unittest.TestCase):
    """yt-dlp.exe is a PyInstaller build that upstream licenses as a GPLv3+ combined work."""

    def test_the_notice_describes_the_gpl_status_and_the_source_offer(self):
        section = NOTICES[NOTICES.index("yt-dlp (bundled program"):]
        self.assertNotIn("License: The Unlicense (public domain", section, "the executable is not public domain")
        for needed in ["GPLv3+", "separate process", "not link", "Where to get the source code"]:
            self.assertIn(needed, section)
        for promised in ["three years", "will provide", "Source code offer"]:
            self.assertNotIn(promised, section, "no personal promise to supply source")
        for needed in []:
            self.assertIn(needed, section)
        version = re.search(r'YTDLP_VERSION: &str = "([^"]+)"', XTASK).group(1)
        self.assertIn(f"Version: {version}", section, "the notice must name the yt-dlp release that is bundled")
        self.assertIn(f"https://github.com/yt-dlp/yt-dlp/archive/refs/tags/{version}.tar.gz", section)

    def test_the_gpl_files_exist_are_named_in_the_notice_and_are_the_real_texts(self):
        for name in YTDLP_FILES:
            self.assertTrue((ROOT / "crates/app/licenses" / name).is_file(), f"{name} is missing; run `cargo xtask tools`")
            self.assertIn(name, NOTICES)
        gpl = (ROOT / "crates/app/licenses/GPL-3.0.txt").read_text(encoding="utf-8")
        self.assertIn("GNU GENERAL PUBLIC LICENSE", gpl)
        self.assertIn("Version 3, 29 June 2007", gpl)
        self.assertGreater(len(gpl.splitlines()), 600)
        third = (ROOT / "crates/app/licenses/yt-dlp-THIRD_PARTY_LICENSES.txt").read_text(encoding="utf-8")
        self.assertGreater(len(third.splitlines()), 1000, "upstream's full list, not a stub")

    def test_xtask_fetches_the_files_and_the_release_uploads_them(self):
        for name in YTDLP_FILES:
            self.assertIn(name, XTASK, "cargo xtask tools must fetch it")
            self.assertIn(name, RELEASE_YML, "the release must upload it next to THIRD_PARTY_NOTICES.txt")


KIT_RS = (ROOT / "xtask/src/kit.rs").read_text(encoding="utf-8")
CHECKLIST = (ROOT / "docs/windows-checklist.md").read_text(encoding="utf-8")


class RelinkKit(unittest.TestCase):
    """The statically linked LGPL FFmpeg must come with what is needed to relink Pyxee against another build."""

    def test_the_notice_describes_the_kit_and_the_relink_steps(self):
        for needed in ["ffmpeg-build-kit.zip", "prefix/lib", "prefix/include", "ffmpeg-source/FFmpeg-<commit>.tar.gz",
                       "build-inputs/build-ffmpeg.sh", "PYXEE_FFMPEG_DIR", "cargo xtask installer"]:
            self.assertIn(needed, NOTICES)

    def test_the_checklist_describes_the_relink_procedure(self):
        for needed in ["ffmpeg-build-kit.zip", "PYXEE_FFMPEG_DIR", "cargo xtask ffmpeg-kit"]:
            self.assertIn(needed, CHECKLIST)

    def test_the_release_workflow_builds_and_uploads_the_kit(self):
        self.assertIn("cargo xtask ffmpeg-kit", RELEASE_YML)
        staging = RELEASE_YML[RELEASE_YML.index("$files += @(Get-ChildItem \"target/portable"):]
        self.assertIn("ffmpeg-build-kit.zip", staging.split("Copy-Item $f.FullName")[0],
                      "the kit must be in the list of files copied into release/")

    def test_the_kit_is_made_from_the_pin_and_stays_static(self):
        # The pin comes from build-ffmpeg.sh (one place); the installer assertions about static linking are untouched.
        self.assertIn("FFMPEG_COMMIT", KIT_RS)
        self.assertIn("libav DLL is in the installed app", RELEASE_YML)
        self.assertIn("--disable-shared --enable-static", BUILD_SCRIPT)

    def test_the_readme_in_the_kit_matches_the_notice(self):
        for needed in ["PYXEE_FFMPEG_DIR", "cargo xtask installer", "build-inputs/build-ffmpeg.sh", "ffmpeg-source/"]:
            self.assertIn(needed, KIT_RS)


README = (ROOT / "README.md").read_text(encoding="utf-8")


class ReadmeLegalText(unittest.TestCase):
    def test_the_top_says_pyxee_is_not_affiliated_and_may_conflict_with_the_terms(self):
        top = README.split("## Install")[0]
        self.assertIn("not affiliated with", top)
        self.assertIn("YouTube or Google", top)
        self.assertIn("Terms of Service", top)

    def test_cookie_advice_warns_about_the_account_and_suggests_a_throwaway_and_a_profile(self):
        section = README[README.index("## If something goes wrong"):README.index("## Advanced settings")]
        for needed in ["restricted", "throwaway account", "separate browser profile"]:
            self.assertIn(needed, section)

    def test_the_licences_text_matches_the_yt_dlp_notice(self):
        licences = README[README.index("## Licences"):]
        self.assertNotIn("Unlicense", licences, "the bundled yt-dlp executable is a GPLv3+ combined work")
        self.assertIn("GPLv3+", licences)

    def test_there_is_a_takedown_and_contact_section_pointing_to_github_issues(self):
        section = README[README.index("## Takedown requests and contact"):]
        self.assertIn("https://github.com/Pylxyr/Pyxee/issues", section)


class DependencyNotices(unittest.TestCase):
    """THIRD_PARTY_DEPENDENCIES.txt is generated by scripts/gen_dependency_notices.py, never written by hand."""

    @classmethod
    def setUpClass(cls):
        sys.path.insert(0, str(ROOT / "scripts"))
        import gen_dependency_notices as gen

        cls.gen = gen
        cls.path = ROOT / "crates/app/licenses/THIRD_PARTY_DEPENDENCIES.txt"

    def test_the_file_exists_and_the_notice_and_the_release_name_it(self):
        self.assertTrue(self.path.is_file(), "run python scripts/gen_dependency_notices.py")
        self.assertIn("THIRD_PARTY_DEPENDENCIES.txt", NOTICES)
        self.assertIn("gen_dependency_notices.py", NOTICES)
        self.assertIn("THIRD_PARTY_DEPENDENCIES.txt", RELEASE_YML)

    def test_it_is_generated_and_covers_both_halves(self):
        text = self.path.read_text(encoding="utf-8")
        self.assertIn("generated by", text.lower())
        for needed in ["PART 1: RUST CRATES", "PART 2: NPM PACKAGES", "serde ", "svelte@", "@tauri-apps/api@"]:
            self.assertIn(needed, text)

    def test_it_is_not_older_than_the_lock_files(self):
        stale = self.gen.stale_locks(self.path.read_text(encoding="utf-8"))
        self.assertEqual(stale, [], f"{stale} changed: run python scripts/gen_dependency_notices.py and commit the result")

    def test_the_recorded_hashes_ignore_line_endings(self):
        import tempfile

        with tempfile.TemporaryDirectory() as d:
            lf, crlf = Path(d, "lf"), Path(d, "crlf")
            lf.write_bytes(b"a\nb\n")
            crlf.write_bytes(b"a\r\nb\r\n")
            self.assertEqual(self.gen.lock_hash(lf), self.gen.lock_hash(crlf))


if __name__ == "__main__":
    unittest.main()
