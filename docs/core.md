# Milestone 2: core (queue, state machine, heard tracker)

`pyxee-core` has no I/O, no audio, no clock. It is driven like this:

```
UI  --Command-->  Player  --Effect-->  app glue (audio engine, sources, storage)
                    ^                          |
                    +-------EngineEvent--------+
```

## The rule that makes the UI honest
`Transport` says a track is `Playing` only after the engine reports `Audible{id}`, which comes from
`HeardTracker`: the audio producer registers where each track starts/ends in the cumulative output
stream (`Event::Started/Finished { at_frame }` from `GaplessSource`), the device callback reports
frames consumed, and the tracker subtracts output latency. Decoding ahead or filling the ring
buffer never changes "now playing".
The latency estimate is re-measured on every device callback and can step up or down, so the heard
count only ever moves forward (`advance` keeps the larger value); `reset` is the one place it restarts, with the
consumed counter, when the pipeline is flushed.

## Effect contract for the engine glue
- `StartTrack{id, track, start_ms}`: stop everything now, resolve + open + play. May reuse a prepared track. Drops prefetched tracks.
- `Prefetch{id, track}`: resolve and prepare to follow the current track gaplessly. If the engine already ran dry, start it at once. Answer `Prefetched` or `PrefetchFailed`.
- `CancelPrefetch`, `Pause`, `Resume`, `Stop`, `Seek{ms}`.
- `NeedRadio{seed, count}`: answer with `Command::AppendRadio(tracks)` (an empty list means "nothing more").
- `Persist`: save `Player::persisted()` (queue, position, autoplay). Positions arrive often; the app should save them on a timer, not per event.

## Events the engine must send
`Audible`, `Ended`, `Position` (about every 250 ms), `Prefetched`, `PrefetchFailed{failure}`,
`LoadFailed{failure}`, `UrlExpired` (re-resolve at the current position, max 2 retries),
`Stalled`/`Recovered`. After `StartTrack`, `Stop` or a seek that flushes the buffer, call
`HeardTracker::reset(consumed_total)`.

## Failure policy
- `Offline` / `RateLimited`: stay on the track in `Failed` (no skipping through the queue); `Retry` restarts it.
- `Unavailable` / `Decode`: mark the entry unplayable and skip to the next, at most 3 in a row, then `Failed`.
- Expired stream URL: re-resolve in place, twice, then `Failed(Expired)`. Reaching "audible" again does not refill the
  two retries (a URL rejected right after every resume would otherwise loop forever); they refill when another song is
  audible, or after the re-resolved URL has played for a minute (a long stream outliving its URL again).

## Queue behaviour
Display order vs play order (shuffle keeps the current track first and is seeded, so it is testable);
repeat off/all/one (repeat-one replays only on natural end, user "Next" always moves on); consecutive
"Play next" keep their order; removing the current track lets the next one slide in; remove and clear
are undoable; the whole session serialises with serde and restores paused at the saved position.
