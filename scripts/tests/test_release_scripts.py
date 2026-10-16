"""Tests for scripts/bump_version.py and scripts/release_notes.py. Run: python -m unittest discover -s scripts/tests"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import bump_version as bv  # noqa: E402
import release_notes as rn  # noqa: E402


class NextVersion(unittest.TestCase):
    def test_patch_minor_major(self):
        self.assertEqual(bv.next_version("1.2.3", "patch"), "1.2.4")
        self.assertEqual(bv.next_version("1.2.3", "minor"), "1.3.0")
        self.assertEqual(bv.next_version("1.2.3", "major"), "2.0.0")

    def test_prerelease_suffix_is_dropped(self):
        self.assertEqual(bv.next_version("1.2.0-rc1", "patch"), "1.2.1")

    def test_bad_input_is_refused(self):
        with self.assertRaises(ValueError):
            bv.next_version("1.2", "patch")
        with self.assertRaises(ValueError):
            bv.next_version("1.2.3", "huge")

    def test_semver_pattern(self):
        for ok in ("0.0.1", "1.2.3", "1.2.3-rc1", "1.2.3-beta.2"):
            self.assertTrue(bv.SEMVER.match(ok), ok)
        for bad in ("1.2", "v1.2.3", "1.2.3.4", "1.2.x", ""):
            self.assertFalse(bv.SEMVER.match(bad), bad)

    def test_the_repo_files_agree_with_each_other(self):
        versions = set(bv.read_versions().values())
        self.assertEqual(len(versions), 1, versions)


class LockfileRegex(unittest.TestCase):
    LOCK = 'name = "pyxee-core"\nversion = "0.0.1"\n\nname = "serde"\nversion = "1.0.0"\n\nname = "xtask"\nversion = "0.0.1"\n'

    def test_only_our_own_crates_change(self):
        out = bv.LOCK_ENTRY.sub(lambda m: f"{m.group(1)}9.9.9{m.group(3)}", self.LOCK)
        self.assertIn('name = "pyxee-core"\nversion = "9.9.9"', out)
        self.assertIn('name = "xtask"\nversion = "9.9.9"', out)
        self.assertIn('name = "serde"\nversion = "1.0.0"', out)


class Classify(unittest.TestCase):
    def test_words_and_prefixes(self):
        self.assertEqual(rn.classify("Add sleep timer"), ("Added", "Add sleep timer"))
        self.assertEqual(rn.classify("Fix 403 on playback")[0], "Fixed")
        self.assertEqual(rn.classify("feat(ui): themes")[0], "Added")
        self.assertEqual(rn.classify("Refactor queue")[0], "Changed")

    def test_work_package_labels_are_dropped(self):
        self.assertEqual(rn.classify("WP4: synced lyrics"), ("Changed", "Synced lyrics"))
        self.assertEqual(rn.classify("WP4 part 1: lyrics crate")[1], "Lyrics crate")

    def test_noise_and_generic_messages(self):
        self.assertIsNone(rn.classify("Release 1.2.0"))
        self.assertIsNone(rn.classify("Merge branch 'x'"))
        self.assertTrue(rn.is_generic("Update main.rs"))
        self.assertTrue(rn.is_generic("Add files via upload"))
        self.assertFalse(rn.is_generic("Fix queue reordering"))

    def test_sections_dedupe_and_skip_generic(self):
        commits = [("a1", "Fix crash"), ("b2", "Fix crash"), ("c3", "Update lib.rs"), ("d4", "Add themes")]
        sections = rn.build_sections(commits)
        self.assertEqual(sections["Fixed"], ["- Fix crash (a1)"])
        self.assertEqual(sections["Added"], ["- Add themes (d4)"])
        self.assertNotIn("Changed", sections)


class CodeAnalysis(unittest.TestCase):
    DIFF = (
        "+++ b/crates/app/src/main.rs\n+            commands::set_volume,\n"
        "+++ b/crates/shell/src/settings.rs\n+    pub close_to_tray: bool,\n"
        "+++ b/crates/audio/src/volume.rs\n+pub struct Volume {\n+pub fn gain() -> f32 {\n-pub fn old_gain() -> f32 {\n"
    )

    def test_detects_commands_settings_and_public_items(self):
        seen = rn.detect_logic(self.DIFF)
        self.assertEqual(seen["commands"]["added"], {"set_volume"})
        self.assertEqual(seen["settings"]["added"], {"close_to_tray"})
        self.assertEqual(seen["code"]["added"], {"Volume", "gain()"})
        self.assertEqual(seen["code"]["removed"], {"old_gain()"})

    def test_new_files_are_not_itemised(self):
        seen = rn.detect_logic(self.DIFF, frozenset({"crates/audio/src/volume.rs"}))
        self.assertEqual(seen["code"]["added"], set())

    def test_an_edited_signature_counts_as_neither(self):
        seen = rn.detect_logic("+++ b/a.rs\n-pub fn go(a: i32) {\n+pub fn go(a: i64) {\n")
        self.assertEqual(seen["code"], {"added": set(), "removed": set()})

    def test_areas_and_labels(self):
        self.assertEqual(rn.area_of("crates/audio/src/pipeline.rs"), "Audio engine")
        self.assertEqual(rn.area_of("crates/app/binaries/README.md"), "Documentation")
        self.assertEqual(rn.area_of("ui/src/App.test.ts"), "Tests")
        self.assertEqual(rn.area_of("somewhere/else.txt"), "Other")
        group = ["crates/app/src/main.rs", "xtask/src/main.rs", "ui/src/App.svelte"]
        self.assertNotEqual(rn._label(group[0], group), rn._label(group[1], group))
        self.assertEqual(rn._label(group[2], group), "App.svelte")

    def test_net_file_state(self):
        net, counts = rn.merge_file_states(
            [({"a": "A", "b": "M"}, {"a": (5, 0), "b": (1, 1)}), ({"a": "D", "b": "M", "c": "D"}, {"b": (2, 0)})]
        )
        self.assertEqual(net, {"b": "M", "c": "D"})
        self.assertEqual(counts["b"], (3, 1))


class AiPlumbing(unittest.TestCase):
    def test_provider_order(self):
        self.assertIsNone(rn.pick_provider({}))
        both = {"ANTHROPIC_API_KEY": "k", "AI_API_KEY": "x", "AI_BASE_URL": "https://h", "AI_MODEL": "m", "GITHUB_TOKEN": "t"}
        self.assertEqual(rn.pick_provider(both).name, "Claude")
        del both["ANTHROPIC_API_KEY"]
        self.assertEqual(rn.pick_provider(both).name, "AI")
        self.assertEqual(rn.pick_provider({"GITHUB_TOKEN": "t"}).name, "GitHub Models")

    def test_openai_compatible_needs_a_model(self):
        self.assertIsNone(rn.pick_provider({"AI_API_KEY": "x", "AI_BASE_URL": "https://h"}))

    def test_prompt_stays_in_budget_and_drops_lockfiles(self):
        diff = "diff --git a/Cargo.lock b/Cargo.lock\n+x\n" + "".join(
            f"diff --git a/ui/src/F{i}.svelte b/ui/src/F{i}.svelte\n+" + "y" * 2000 + "\n" for i in range(30)
        )
        prompt = rn.build_prompt("v1.0.0", "v0.9.0", [("a", "Add x")], "stat", diff, 10_000)
        self.assertLess(len(prompt), 10_000)
        self.assertNotIn("Cargo.lock", prompt.split("## Diff")[1].split("[files changed")[0])
        self.assertIn("files changed but not shown", prompt)

    def test_output_token_limit_is_clamped(self):
        self.assertEqual(rn._max_output_tokens({}), 3000)
        self.assertEqual(rn._max_output_tokens({"AI_MAX_OUTPUT_TOKENS": "5"}), 256)
        self.assertEqual(rn._max_output_tokens({"AI_MAX_OUTPUT_TOKENS": "999999"}), 16000)
        self.assertEqual(rn._max_output_tokens({"AI_MAX_OUTPUT_TOKENS": "abc"}), 3000)


class Render(unittest.TestCase):
    def test_first_release(self):
        self.assertIn("First release.", rn.render("v0.1.0", None, "o/r", []))

    def test_ai_text_is_labelled(self):
        out = rn.render("v0.2.0", "v0.1.0", "o/r", [], ai_text="### Added\n- A thing")
        self.assertIn("Summarised automatically by AI", out)
        self.assertIn("compare/v0.1.0...v0.2.0", out)

    def test_empty_release(self):
        self.assertIn("Maintenance release", rn.render("v0.2.0", "v0.1.0", "", []))


if __name__ == "__main__":
    unittest.main()
