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
   Opus goes through ffmpeg (symphonia has no Opus decoder); other formats use symphonia, with ffmpeg as a fallback.
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
  Does `music.youtube.com/search?q=` (used for Songs) work in your yt-dlp? Does the `list=RD<id>` mix return tracks?
  If yt-dlp needs a JavaScript runtime for YouTube it will say so on stderr.
- The suggestions endpoint (`suggestqueries.google.com`).
- Real WASAPI output: gapless on a real device, device latency compensation, underruns.
- Albums search is not supported yet (needs the YouTube Music API); the filter is shown disabled.
- Instant skip to a prefetched track is not implemented in the real engine (the sim has it); a skip re-opens the track, using
  the cached URL, so it is fast but not instant.
