# Milestone 4: real engine, sources, search, queue

## What changed
- `crates/sources`: `MusicSource` (search, resolve_stream, related, suggest, supported_filters) with a yt-dlp
  implementation, a local-files implementation and a router by key prefix (`yt:` / `file:`). yt-dlp runs through one
  wrapper with a timeout, parallel pipe draining, and error classification (offline, rate limited, unavailable,
  tool missing) so the UI can explain failures.
- `crates/audio`: `Pipeline` (producer thread -> small buffer -> device `Consumer`) with instant flush, pause, heard
  position, and the `HeardTracker` fed from real consumption; in-place seek; typed track failures; `output` (cpal/WASAPI).
- `crates/app/src/engine.rs`: turns the player's effects into resolve -> open -> prefetch -> play and the pipeline's reports
  into engine events. No thread of its own: driven by `handle(effect)` and `poll()`, so it is tested step by step.
  `runner.rs` runs it on a thread, or a stand-in that fails honestly when no audio device exists, or the simulator
  (`PYXEE_SIM=1`) for UI work.
- UI: search (live suggestions, filters, loading/empty/error states with retry), row menus, and the queue screen
  (drag to reorder, move up/down buttons, remove and clear with Undo).

## How playback works now
1. `StartTrack` flushes the pipeline, resolves the stream (URLs cached 15 minutes), opens it and queues it.
   libav (a minimal static FFmpeg linked into the app; no ffmpeg process) is the only decoder. yt-dlp is asked for Opus in
   WebM first and AAC-LC in M4A second; a stream it cannot read fails with the format named in the log and the song is
   skipped (see "In-process libav" below).
2. While a track plays, the next one is resolved, opened and decoded ahead; the pipeline joins them with no gap.
3. "Now playing" changes only when the device has consumed the audio (see docs/core.md).
4. A stream URL that is rejected mid-song (403/410) makes the track stop producing audio; when the listener reaches that
   point the engine reports `UrlExpired` at the exact frame it died, the player re-resolves, and playback resumes there.
5. Seek re-opens the current track at the offset; the pre-decoded next track is re-queued.

## Verified here (automated, no network)
Gapless order and zero silence, skip, pause (clock and audio freeze), seek, unplayable track skipped, offline stays on the
track with Retry, radio continues after the queue ends, repeat-one loops seamlessly, and the expired-URL recovery against a
local HTTP server that starts returning 403 mid-song.

## NOT verified yet: needs your PC (please run these and paste the output)
Nothing in the sandbox can reach YouTube or a sound card, so these shapes come from what I know of yt-dlp, not from live output:
- `spike search "lowtide" --filter all|songs|videos|artists|playlists` and `spike related yt:<id>`: do the results parse?
  Does the Songs search URL (see `search_target`) work in your yt-dlp? Does the `list=RD<id>` mix return tracks?
  If yt-dlp needs a JavaScript runtime for YouTube it will say so on stderr.
- The suggestions endpoint (`suggestqueries.google.com`).
- Real WASAPI output: gapless on a real device, device latency compensation, underruns.
- Albums search is not supported yet (needs a dedicated albums API); the filter is shown disabled.
- Instant skip to a prefetched track is not implemented in the real engine (the sim has it); a skip re-opens the track, using
  the cached URL, so it is fast but not instant.

## Volume
The slider (0-100) is sent to the engine at start-up and on every change (`set_volume`). The audio callback applies it as a
gain of percent squared (50% is -12 dB, which sounds like "half as loud") and ramps from the previous gain across each buffer,
so changing it never clicks. While nothing is playing the gain simply follows the setting, so a quiet setting never blips loudly
at the start of the next track (this bug was caught by a test). Muting keeps tracks advancing and announced; only the sound stops.

## Seeking
A seek re-opens the current track at the offset. Over a network that is slow, while the already-known next track is prepared
quickly; the first version let the next track win that race, so a forward seek played the next song instead. A prefetched
track now waits until the track being re-opened is queued, then follows it. `seeking_never_lets_the_next_track_play_first`
reproduces this with a slow local HTTP server (0.02 s of the seeked song before the next one cut in, before the fix).

## Logs and messages (added after the first Windows run)
`crates/app/src/logging.rs` writes `pyxee.log` (rotated at 2 MB, Taipei-time timestamps (fixed UTC+8, `+08:00`), panics included); the engine, sources and
audio crates log through the standard `log` facade. A song's life is traceable: `start`, `resolved` (format, codec, bitrate and
the host the stream comes from, never the full URL), `ready`, `audible`, `ended ... after N ms (expected M ms)`, and for failures
`could not start ...: <reason>` plus yt-dlp's own error output.

Skips are no longer silent. When a song is skipped because it cannot be played, or its stream ends long before its known length,
the UI shows a message naming the song and the reason ("... can't be played, so it was skipped: it isn't available ..."). Being
offline or rate limited is not announced (the player bar already shows it with a Retry).

## Fixes from the second Windows run
- Seeking after a gapless handover re-opened the song the session *started* with, so seeking in song 2 played song 1. A seek now
  re-opens the song that is audible (`seeking_after_a_gapless_advance_stays_on_the_song_that_is_playing`).
- If a stream cannot seek to a position, or a server ignores range requests, the engine used ffmpeg instead of treating the
  song as unplayable. *(Superseded: no ffmpeg process is ever started. A stream that cannot seek is re-opened and decoded up
  to the offset; a server that ignores range requests fails the song with a notice. See "In-process libav".)*

## WP1 fixes (third Windows run)

- **403 / stale URLs.** yt-dlp's `http_headers` travel with the stream (`StreamInfo.headers`) to `HttpRangeSource`
  (then also to ffmpeg, since removed). A 401/403/410 becomes `AudioError::UrlExpired` (at start and mid-play), so the usual re-resolve and resume-at-position path applies. A URL is cached only after it
  opened; a refused URL is evicted, re-resolved once, and never served from the cache again (Retry resolves fresh).
  The message is "YouTube refused to send it (HTTP 403)" (`Failure::Expired(Option<u16>)`).
- **Stream buffer.** `HttpRangeSource` downloads the whole song in the background (512 KiB range requests, retried with a short back-off), so the decoder reads from memory and a song that is fully downloaded finishes with no network. If the buffer runs dry the reader waits up to 45 s while the download keeps retrying; after that the song fails with a timeout. Files over 64 MiB are only buffered in a window around the playhead. A reply that is not a real range answer (not 206, or a `Content-Range` that does not start where asked) counts as a transient failure, so a server that ignores ranges cannot hand over the wrong bytes. A 4xx stops the download (the URL is expired or limited) and surfaces as before once the reader reaches the missing part. A read that is waiting for data gives up when the pipeline is flushed (`Pipeline::cancel_token`), so a stalled stream cannot freeze a skip or a seek; `push` and `clear_queued` only leave a note for the producer thread, so they never wait for it either.
- **Track details.** Resolving also reports artist, length and the largest cover. `EngineEvent::TrackDetails` fills an
  unknown artist, a missing length and the cover in the queue entry (a known artist/length is never overwritten) and
  `Library::refresh_track` updates a saved copy. An "Unknown artist" never erases a stored artist.
- **Logs** use Taipei time (fixed UTC+8, written `2026-10-12 21:30:00.123+08:00`).
- **Queue.** `Command::MoveUpcoming` reorders Up next in play order (under shuffle the list the user arranged is left alone).

Needs your PC: that yt-dlp's headers really cure the 403 for `yt:Ni5jPjvGxtw`; that songs-only search results show
the artist after the song starts.

## Rate-limit guard and in-process libav

**Why 429s happened.** Search (`spawn_blocking`) and the engine (one `thread::spawn` per resolve, prefetch and radio) all
called one shared `run_tool`, which started yt-dlp immediately. Skipping through songs or searching while a song loaded ran
several yt-dlp processes at once, each making several requests of its own.

**The gate** (`crates/sources/src/gate.rs`, one per `YtDlpSource`, so it covers search, resolve, prefetch and radio):
- one yt-dlp run at a time; a 1 s quiet gap after each run ends; the line is ordered by urgency (`gate::Priority`): searches and the song being played go before background work (shelves, similar songs, the next song's address), newest first within each kind, with the pacing itself unchanged;
- a search is answered from a 10-minute session cache when the same query and filter are asked again (no run), and after the songs page has answered with bare titles once, Songs searches skip it for 30 minutes (one run instead of two);
- after a rate-limit answer (HTTP 429, HTTP 403 from YouTube, "too many requests", "Sign in to confirm you're not a bot"), new runs are refused at once with
  `SourceError::RateLimited` for 2 minutes, including callers already waiting, so there is no retry storm. The player
  already treats `RateLimited` as transient (it stops with a Retry; a failed prefetch is not retried);
- the newest waiter goes first: the engine never cancels the worker of a song you skipped, so without this the song you
  landed on would wait behind every stale resolve (those still run afterwards).
Log lines: `yt-dlp <op> waited ...` (waits over 250 ms), `yt-dlp was rate limited: new runs are refused for ...`, `yt-dlp
<op> refused: rate-limit cooldown, ... left`, and at start `yt-dlp: using the cookie file ...` / `reading cookies from the
<browser> browser` (never the path or profile). Cookies: see the README table (`PYXEE_COOKIES`, `PYXEE_COOKIES_FROM_BROWSER`).
Cookies do not help if the IP address itself is blocked.

**Transient or permanent** (`classify_stderr`, `AudioError`, `LyricsError`). A transient failure keeps the song selected with a
Retry and burns no tracks; a permanent one skips (at most 3 in a row). Transient: yt-dlp 429 / 403 / bot check (`RateLimited`),
yt-dlp 5xx and a yt-dlp run that hits its 40 s timeout (`Offline`), a stream server's HTTP 429 (`AudioError::RateLimited`),
network errors. Permanent: unavailable, age-restricted (the age gate is *not* a rate limit), a refused stream URL after its
re-resolves, undecodable media. An out-of-date yt-dlp is named as such. A stream that fails mid-song says why in words
(`stopped_early_notice`).
Lyrics (`crates/lyrics`): 429 is `RateLimited` (honours `Retry-After`, 1 s to 10 min, default 60 s), 401/403 `Blocked`, 5xx
`Unavailable`; each pauses lookups (`Cooldown`: 60 s default, 5 min blocked, 30 s failing) so flipping songs cannot hammer a
service that refused the app. Lyrics never affect playback.

**In-process libav.** No `ffmpeg` process is started, ever: the build, the installer and `cargo xtask tools` carry no ffmpeg
executable (`check-windows.ps1` proves it with a fake `ffmpeg.exe` first on `PATH`). Decoding is libavformat/libavcodec/
libswresample from a minimal **static, LGPL-only** FFmpeg (pinned commit, `scripts/build-ffmpeg.sh`; `cargo xtask ffmpeg`
runs it, and `cargo xtask installer` does too), linked through the small C shim `crates/audio/csrc/pxdec.c`. It has audio
decoders and demuxers only, for the formats below, no network protocols (HTTP stays in Rust: `HttpRangeSource` feeds
libavformat through read/seek callbacks), no filters, devices or programs.
- Decoders: Opus, AAC (LC), MP3, FLAC, Vorbis, ALAC, PCM. Containers: WebM/Matroska, MP4/M4A, Ogg, MP3, FLAC, WAV, ADTS, AIFF.
- `FORMAT_SELECTOR` in `ytdlp.rs` asks for `bestaudio[ext=webm][acodec=opus][protocol=https][audio_channels<=?2]`, then
  `bestaudio[ext=m4a][acodec^=mp4a.40.2][protocol=https][audio_channels<=?2]`. `mp4a.40.2` = AAC-LC (HE-AAC, itag 139, is not
  built in); `audio_channels<=?2` keeps stereo (the `?` must follow the operator); `protocol=https` = one plain file for
  range requests, not an HLS/DASH manifest. There is no third alternative on purpose: if YouTube offers neither, the song
  is skipped with "YouTube offered no audio stream this player can decode".
- Gapless: libav applies the container's encoder delay and padding, so Opus (Ogg and WebM) joins to the exact sample count.
  AAC in M4A is not fully trimmed by libav (plain files keep tail padding; YouTube-style fragmented files declare none), so
  quiet edges (max 2112 frames) are still trimmed for AAC only, on top of whatever libav did.
- A live YouTube join can show seconds of digital silence (`silence-at-join` in `check-windows`, INFO). That is the recordings' own
  fade-out/fade-in or padding, not the player: the join itself is proven by `gapless-join-contiguous` (track 1 ends on the very frame
  track 2 starts), `gapless-total-length` (rendered length matches the two durations) and the fixture test
  `two_tracks_play_in_order_gaplessly_and_playback_ends` (no silence run of 8+ frames).
- Seeking is in place for every format. Opus needs its 80 ms pre-roll after a seek: the decoder starts that much early and the
  lead-in is discarded, so Ogg Opus lands bit-exact (`crates/audio/tests/opus.rs`). Matroska timestamps are whole
  milliseconds, so a WebM seek lands within 1 ms (48 frames). A stream that cannot seek in place is re-opened and decoded
  through the offset (`LibavSource::skip_ms`). A server that ignores range requests fails the song.
- Output is always stereo `f32` at the stream's own rate (libswresample downmixes surround); `rubato` still resamples to the device.
- Updating FFmpeg: change the tag and commit in `scripts/build-ffmpeg.sh`, then rerun `cargo xtask ffmpeg`; the C shim is the
  only code that touches libav structs. FFmpeg is LGPL: it is linked statically here, so release builds must keep the
  object files or the build inputs available for relinking, and ship the LGPL notice (see "FFmpeg licence" in `docs/windows-checklist.md`; `scripts/tests/test_licenses.py` enforces what can be).
The `*_frag.m4a` test fixtures were made once, with ffmpeg as a dev-time tool, by stream-copying the plain m4a fixtures
into a fragmented layout (500 ms fragments, empty moov, global sidx); nothing at runtime uses it.

