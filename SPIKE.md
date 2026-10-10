# Pyxee milestone 1: spike

## Run it on Windows
1. Install Rust (rustup, stable, MSVC toolchain), Visual Studio Build Tools (C++), yt-dlp.exe and ffmpeg.exe (on PATH or pass paths).
2. `cargo test --workspace` (gapless tests use committed fixtures; the Opus test needs ffmpeg on PATH and skips otherwise).
3. Offline check: `cargo run --release -p pyxee-spike -- files crates/audio/tests/fixtures/a_44k.mp3 crates/audio/tests/fixtures/b_48k.mp3`
4. Real playback: `cargo run --release -p pyxee-spike -- play "artist song one" "artist song two" --ytdlp C:\path\yt-dlp.exe --ffmpeg C:\path\ffmpeg.exe`
5. Paste the whole output (the REPORT block especially) back into the chat.

## Findings so far (Linux sandbox, fixtures, no network)
| Codec | Decoder | Joined length | Silence at boundary |
|---|---|---|---|
| WAV, FLAC, MP3 320, Vorbis | symphonia | exact | none |
| AAC (m4a) | symphonia + own edge trim | within 1 frame | none |
| Opus | ffmpeg pipe (symphonia has no Opus decoder) | exact | none |

- symphonia 0.5.5 trims MP3/Vorbis itself but exposes no delay/padding for AAC-in-MP4, so quiet edges (max 2112 frames) are trimmed in `SymphoniaSource`.
- rubato 0.16 output is already time-aligned: skipping `output_delay()` shifted audio ~69 frames (measured in `examples/lag.rs`).

## Not verified yet (needs your machine)
WASAPI output, real yt-dlp streams (Opus vs AAC, URL expiry, whether yt-dlp needs a JS runtime), fragmented MP4 from YouTube, device underruns.

## Review fixes (second pass)
- Expired or rejected stream URLs (HTTP 401/403/410) now surface as `AudioError::UrlExpired` so the caller can re-resolve; transient 5xx/timeouts are retried twice.
- ffmpeg pipe: a non-zero exit is an error with ffmpeg's last stderr line, not a silent "track ended"; only needed protocols are whitelisted and HTTP inputs reconnect.
- A track that decodes to zero frames emits `Failed` instead of `Finished`.
- The spike's producer thread no longer quits after a 6 s idle timer; it stops when the caller calls `close()`.

## Known limitations (to fix in later milestones)
- (Fixed in milestone 2, see docs/core.md) In the spike, `Finished`/`Started` events fire when audio is *produced*, up to ~2 s before it is *heard*. The core's `HeardTracker` turns them into heard-time events. The spike binary itself is not wired to it yet; that happens in the app crate (milestone 3+).
- Surround (>2 channels) keeps only front-left/right; a proper downmix is needed.
- No volume, seek, or device-change handling yet.
- Quiet-edge trim for AAC assumes exact digital silence below 1e-3 for up to 2112 frames.
