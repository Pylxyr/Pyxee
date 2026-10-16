"""The yt-dlp format selector the app sends must be accepted by the real yt-dlp, and must pick what the decoder can play.

A bad selector (it once shipped as `audio_channels<=2?`; yt-dlp wants `<=?2`) makes yt-dlp die with a SyntaxError on
every play, before any network call, and no Rust test can see that. This reads FORMAT_SELECTOR out of the Rust source
and runs it through yt-dlp's own parser. CI installs yt-dlp and sets REQUIRE_YT_DLP=1; without that, a machine that has
no yt-dlp module skips these tests instead of failing.
"""

from __future__ import annotations

import os
import re
import unittest
from pathlib import Path

SOURCE = Path(__file__).resolve().parents[2] / "crates" / "sources" / "src" / "ytdlp.rs"
_CONSTANT = re.compile(r'FORMAT_SELECTOR: &str =\s*"([^"]+)"')

try:
    from yt_dlp import YoutubeDL
except ImportError:  # pragma: no cover - depends on the machine
    YoutubeDL = None


def selector_from_source() -> str:
    match = _CONSTANT.search(SOURCE.read_text(encoding="utf-8"))
    if not match:
        raise AssertionError(f"FORMAT_SELECTOR not found in {SOURCE}")
    return match.group(1)


def fmt(format_id: str, *, ext="m4a", acodec="mp4a.40.2", protocol="https", channels=2, abr=128) -> dict:
    out = {"format_id": format_id, "ext": ext, "acodec": acodec, "vcodec": "none", "protocol": protocol,
           "abr": abr, "tbr": abr, "url": f"https://example.invalid/{format_id}"}
    if channels is not None:
        out["audio_channels"] = channels
    return out


class FormatSelector(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if YoutubeDL is None:
            if os.environ.get("REQUIRE_YT_DLP") == "1":
                raise AssertionError("REQUIRE_YT_DLP=1 but the yt_dlp module is not installed")
            raise unittest.SkipTest("yt_dlp is not installed (pip install yt-dlp)")
        cls.ydl = YoutubeDL({"quiet": True})
        cls.selector = selector_from_source()

    def pick(self, formats: list[dict]) -> list[str]:
        chosen = self.ydl.build_format_selector(self.selector)({"formats": formats, "incomplete_formats": False})
        return [f["format_id"] for f in chosen]

    def test_yt_dlp_accepts_the_selector(self):
        # Raises SyntaxError("Invalid filter specification ...") for a malformed filter.
        self.ydl.build_format_selector(self.selector)

    def test_the_old_broken_spelling_really_is_rejected(self):
        # If yt-dlp ever starts accepting this, the guard above is no longer proving what we think it proves.
        with self.assertRaises(SyntaxError):
            self.ydl.build_format_selector("bestaudio[audio_channels<=2?]")

    def test_picks_the_best_stereo_aac_lc_over_https(self):
        formats = [fmt("139", acodec="mp4a.40.5", abr=48), fmt("140", abr=129), fmt("141", abr=256),
                   fmt("256", channels=6, abr=192), fmt("251", ext="webm", acodec="opus", abr=160),
                   fmt("hls", protocol="m3u8_native", abr=300)]
        self.assertEqual(self.pick(formats), ["141"])

    def test_skips_surround_and_he_aac(self):
        self.assertEqual(self.pick([fmt("139", acodec="mp4a.40.5"), fmt("258", channels=6), fmt("140")]), ["140"])

    def test_keeps_a_format_that_does_not_report_its_channel_count(self):
        self.assertEqual(self.pick([fmt("140", channels=None)]), ["140"])

    def test_nothing_playable_selects_nothing(self):
        self.assertEqual(self.pick([fmt("258", channels=6), fmt("251", ext="webm", acodec="opus")]), [])


if __name__ == "__main__":
    unittest.main()
