# Pyxee milestone 1: spike

## Run it on Windows
1. Install Rust (rustup, stable, MSVC toolchain), Visual Studio Build Tools (C++), yt-dlp.exe (on PATH or pass its path). There is no ffmpeg: symphonia is the only decoder.
2. `cargo test --workspace` (tests use committed fixtures).
3. Offline check: `cargo run --release -p pyxee-spike -- files crates/audio/tests/fixtures/a_44k.mp3 crates/audio/tests/fixtures/b_48k.mp3`
4. Real playback: `cargo run --release -p pyxee-spike -- play "artist song one" "artist song two" --ytdlp C:\path\yt-dlp.exe`
5. Paste the whole output (the REPORT block especially) back into the chat.

## Findings so far (Linux sandbox, fixtures, no network)
| Codec | Decoder | Joined length | Silence at boundary |
|---|---|---|---|
| WAV, FLAC, MP3 320, Vorbis | symphonia | exact | none |
| AAC (m4a) | symphonia + own edge trim | within 1 frame | none |
| Opus | not decodable (symphonia has no Opus decoder); refused as unsupported, never requested from YouTube | n/a | n/a |
| AAC (m4a), fragmented like YouTube | symphonia (+ edge trim) | within 1 frame | none |

- symphonia 0.5.5 trims MP3/Vorbis itself but exposes no delay/padding for AAC-in-MP4, so quiet edges (max 2112 frames) are trimmed in `SymphoniaSource`.
- rubato 0.16 output is already time-aligned: skipping `output_delay()` shifted audio ~69 frames (measured in `examples/lag.rs`).

## Not verified yet (needs your machine)
WASAPI output, real yt-dlp streams (Opus vs AAC, URL expiry, whether yt-dlp needs a JS runtime), fragmented MP4 from live YouTube (fragmented layout is now covered by fixtures; the real service has not been tried), device underruns.

## Review fixes (second pass)
- Expired or rejected stream URLs (HTTP 401/403/410) now surface as `AudioError::UrlExpired` so the caller can re-resolve; transient 5xx/timeouts are retried twice.
- *(Historical)* an ffmpeg pipe existed in this review round; it has since been removed.
- A track that decodes to zero frames emits `Failed` instead of `Finished`.
- The spike's producer thread no longer quits after a 6 s idle timer; it stops when the caller calls `close()`.

## Known limitations (to fix in later milestones)
- (Fixed in milestone 2, see docs/core.md) In the spike, `Finished`/`Started` events fire when audio is *produced*, up to ~2 s before it is *heard*. The core's `HeardTracker` turns them into heard-time events. The spike binary itself is not wired to it yet; that happens in the app crate (milestone 3+).
- Surround (>2 channels) keeps only front-left/right; a proper downmix is needed.
- No volume, seek, or device-change handling yet.
- Quiet-edge trim for AAC assumes exact digital silence below 1e-3 for up to 2112 frames.
