"""The yt-dlp format selector the app sends must be accepted by the real yt-dlp, and must pick Opus, then AAC-LC, then only what libav can decode.

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

    def test_prefers_opus_over_every_aac(self):
        # build_format_selector takes the last match: real yt-dlp has already sorted worst to best.
        formats = [fmt("139", acodec="mp4a.40.5", abr=48), fmt("250", ext="webm", acodec="opus", abr=70),
                   fmt("140", abr=129), fmt("141", abr=256), fmt("251", ext="webm", acodec="opus", abr=160)]
        self.assertEqual(self.pick(formats), ["251"])

    def test_falls_back_to_the_best_stereo_aac_lc_when_there_is_no_opus(self):
        formats = [fmt("139", acodec="mp4a.40.5", abr=48), fmt("140", abr=129), fmt("141", abr=256),
                   fmt("256", channels=6, abr=192), fmt("hls", protocol="m3u8_native", abr=300)]
        self.assertEqual(self.pick(formats), ["141"])

    def test_skips_surround_and_streaming_manifests_even_for_opus(self):
        formats = [fmt("338", ext="webm", acodec="opus", channels=4, abr=480),
                   fmt("251-hls", ext="webm", acodec="opus", protocol="m3u8_native", abr=160), fmt("140")]
        self.assertEqual(self.pick(formats), ["140"])

    def test_skips_surround_and_he_aac(self):
        self.assertEqual(self.pick([fmt("139", acodec="mp4a.40.5"), fmt("258", channels=6), fmt("140")]), ["140"])

    def test_keeps_a_format_that_does_not_report_its_channel_count(self):
        self.assertEqual(self.pick([fmt("251", ext="webm", acodec="opus", channels=None)]), ["251"])
        self.assertEqual(self.pick([fmt("140", channels=None)]), ["140"])

    def test_nothing_playable_selects_nothing(self):
        # AC-3 and E-AC-3 are not built into libav here; HE-AAC and surround are excluded; none may be picked.
        self.assertEqual(self.pick([fmt("258", channels=6), fmt("139", acodec="mp4a.40.5"),
                                    fmt("ac3", ext="m4a", acodec="ac-3"), fmt("eac3", ext="m4a", acodec="ec-3"),
                                    fmt("wma", ext="wma", acodec="wmav2"), fmt("sorenson", ext="flv", acodec="mp3")]), [])

    def test_the_catch_all_takes_other_decodable_codecs_in_decodable_containers(self):
        for codec, ext in [("vorbis", "webm"), ("vorbis", "ogg"), ("opus", "ogg"), ("opus", "opus"), ("flac", "flac"),
                           ("mp3", "mp3"), ("alac", "m4a"), ("opus", "mka")]:
            with self.subTest(codec=codec, ext=ext):
                self.assertEqual(self.pick([fmt("x", ext=ext, acodec=codec, abr=96)]), ["x"])

    def test_the_catch_all_is_still_plain_https_and_stereo(self):
        self.assertEqual(self.pick([fmt("v", ext="webm", acodec="vorbis", protocol="m3u8_native")]), [])
        self.assertEqual(self.pick([fmt("v", ext="ogg", acodec="vorbis", protocol="http_dash_segments")]), [])
        self.assertEqual(self.pick([fmt("v", ext="webm", acodec="vorbis", channels=6)]), [])
        self.assertEqual(self.pick([fmt("v", ext="webm", acodec="vorbis", channels=None)]), ["v"])

    def test_the_catch_all_never_beats_opus_or_aac_lc(self):
        formats = [fmt("flac", ext="flac", acodec="flac", abr=900), fmt("v", ext="webm", acodec="vorbis", abr=200),
                   fmt("140", abr=129)]
        self.assertEqual(self.pick(formats), ["140"])
        formats.append(fmt("251", ext="webm", acodec="opus", abr=160))
        self.assertEqual(self.pick(formats), ["251"])

    def test_the_selector_is_one_plain_literal_the_release_workflow_can_extract(self):
        text = SOURCE.read_text(encoding="utf-8")
        # Same pattern as release.yml: no escapes, no concatenation, no double quotes inside.
        match = re.search(r'FORMAT_SELECTOR: &str =\s*"([^"]+)"\s*;', text)
        self.assertIsNotNone(match)
        self.assertNotIn("\\", match.group(1))
        self.assertNotIn("\n", match.group(1))


if __name__ == "__main__":
    unittest.main()
