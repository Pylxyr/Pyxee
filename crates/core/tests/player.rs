//! State machine scenarios. The engine is simulated by feeding `EngineEvent`s by hand.

use pyxee_core::{
    Command, Effect, EngineEvent as E, EntryId, Failure, Persisted, Player, Repeat, Track,
    TrackDetails, Transport, UNKNOWN_ARTIST,
};

fn tr(n: &str) -> Track {
    Track::new(n, n, "artist")
}

/// Player with tracks a..(n) queued, first one loading. Returns the ids in order.
fn started(n: usize) -> (Player, Vec<EntryId>, Vec<Effect>) {
    let mut p = Player::new(7);
    let tracks: Vec<Track> = (0..n)
        .map(|i| tr(&((b'a' + i as u8) as char).to_string()))
        .collect();
    let fx = p.command(Command::PlayTracks { tracks, start: 0 });
    let ids = p.queue().entries().iter().map(|e| e.id).collect();
    (p, ids, fx)
}

fn start_id(fx: &[Effect]) -> Option<EntryId> {
    fx.iter().find_map(|e| match e {
        Effect::StartTrack { id, .. } => Some(*id),
        _ => None,
    })
}
fn prefetch_id(fx: &[Effect]) -> Option<EntryId> {
    fx.iter().find_map(|e| match e {
        Effect::Prefetch { id, .. } => Some(*id),
        _ => None,
    })
}
fn has(fx: &[Effect], f: impl Fn(&Effect) -> bool) -> bool {
    fx.iter().any(f)
}

#[test]
fn play_tracks_requests_the_first_track_and_waits_until_it_is_audible() {
    let (p, ids, fx) = started(3);
    assert_eq!(start_id(&fx), Some(ids[0]));
    assert_eq!(p.snapshot().transport, Transport::Loading { id: ids[0] });
    assert_eq!(p.snapshot().audible, None, "requested is not audible");
    assert_eq!(p.snapshot().loading, Some(ids[0]));
}

#[test]
fn audible_starts_playing_and_prefetches_the_next_track() {
    let (mut p, ids, _) = started(3);
    let fx = p.engine(E::Audible { id: ids[0] });
    assert_eq!(p.snapshot().transport, Transport::Playing { id: ids[0] });
    assert_eq!(
        prefetch_id(&fx),
        Some(ids[1]),
        "resolve the next track while this one plays"
    );
    assert!(has(&fx, |e| *e == Effect::Persist));
}

#[test]
fn gapless_advance_changes_now_playing_only_when_the_next_track_is_heard() {
    let (mut p, ids, _) = started(3);
    p.engine(E::Audible { id: ids[0] });
    p.engine(E::Prefetched { id: ids[1] });

    // The decoder has already crossed into track b, but the user still hears the end of a.
    assert_eq!(p.snapshot().audible, Some(ids[0]));

    let fx = p.engine(E::Ended { id: ids[0] });
    assert!(
        fx.is_empty(),
        "gapless: nothing to do, the engine continues by itself"
    );
    assert_eq!(p.snapshot().audible, Some(ids[0]));

    let fx = p.engine(E::Audible { id: ids[1] });
    assert_eq!(p.snapshot().audible, Some(ids[1]));
    assert_eq!(p.queue().current_id(), Some(ids[1]));
    assert_eq!(prefetch_id(&fx), Some(ids[2]));
}

#[test]
fn ending_while_the_next_track_is_still_preparing_shows_loading() {
    let (mut p, ids, _) = started(2);
    p.engine(E::Audible { id: ids[0] });
    let fx = p.engine(E::Ended { id: ids[0] });
    assert!(
        start_id(&fx).is_none(),
        "the engine starts the prefetched track itself"
    );
    assert_eq!(p.snapshot().transport, Transport::Loading { id: ids[1] });
    p.engine(E::Audible { id: ids[1] });
    assert_eq!(p.snapshot().audible, Some(ids[1]));
}

#[test]
fn ending_with_no_usable_prefetch_cold_starts_the_next_track() {
    let (mut p, ids, _) = started(2);
    p.engine(E::Audible { id: ids[0] });
    p.engine(E::PrefetchFailed {
        id: ids[1],
        failure: Failure::Offline,
    });
    let fx = p.engine(E::Ended { id: ids[0] });
    assert_eq!(start_id(&fx), Some(ids[1]));
}

#[test]
fn unavailable_track_is_skipped_automatically() {
    let (mut p, ids, _) = started(3);
    let fx = p.engine(E::LoadFailed {
        id: ids[0],
        failure: Failure::Unavailable,
    });
    assert_eq!(start_id(&fx), Some(ids[1]));
    assert_eq!(p.snapshot().transport, Transport::Loading { id: ids[1] });
}

#[test]
fn prefetch_failure_of_an_unavailable_track_skips_it_for_the_following_advance() {
    let (mut p, ids, _) = started(3);
    p.engine(E::Audible { id: ids[0] });
    let fx = p.engine(E::PrefetchFailed {
        id: ids[1],
        failure: Failure::Unavailable,
    });
    assert_eq!(
        prefetch_id(&fx),
        Some(ids[2]),
        "re-plans around the dead track"
    );
}

#[test]
fn skipping_gives_up_after_three_consecutive_failures() {
    let (mut p, ids, _) = started(6);
    let mut last = ids[0];
    for i in 1..=3 {
        let fx = p.engine(E::LoadFailed {
            id: last,
            failure: Failure::Unavailable,
        });
        last = start_id(&fx).unwrap_or_else(|| panic!("skip {i} should start the next track"));
    }
    let fx = p.engine(E::LoadFailed {
        id: last,
        failure: Failure::Unavailable,
    });
    assert!(start_id(&fx).is_none());
    assert!(matches!(p.snapshot().transport, Transport::Failed { .. }));
}

#[test]
fn offline_does_not_burn_through_the_queue_and_retry_works() {
    let (mut p, ids, _) = started(3);
    let fx = p.engine(E::LoadFailed {
        id: ids[0],
        failure: Failure::Offline,
    });
    assert!(start_id(&fx).is_none());
    assert_eq!(
        p.snapshot().transport,
        Transport::Failed {
            id: ids[0],
            failure: Failure::Offline
        }
    );
    let fx = p.command(Command::Retry);
    assert_eq!(start_id(&fx), Some(ids[0]));
}

#[test]
fn expired_url_re_resolves_at_the_same_position_up_to_twice() {
    let (mut p, ids, _) = started(2);
    p.engine(E::Audible { id: ids[0] });
    p.engine(E::Position {
        id: ids[0],
        ms: 61_000,
    });
    for _ in 0..2 {
        let fx = p.engine(E::UrlExpired { id: ids[0] });
        assert!(has(&fx, |e| matches!(
            e,
            Effect::StartTrack {
                start_ms: 61_000,
                ..
            }
        )));
    }
    let fx = p.engine(E::UrlExpired { id: ids[0] });
    assert!(start_id(&fx).is_none());
    assert_eq!(
        p.snapshot().transport,
        Transport::Failed {
            id: ids[0],
            failure: Failure::Expired(None)
        }
    );
}

#[test]
fn pause_and_resume_a_loaded_track() {
    let (mut p, ids, _) = started(2);
    p.engine(E::Audible { id: ids[0] });
    assert!(has(&p.command(Command::Pause), |e| *e == Effect::Pause));
    assert_eq!(
        p.snapshot().transport,
        Transport::Paused {
            id: ids[0],
            loaded: true
        }
    );
    assert!(has(&p.command(Command::Toggle), |e| *e == Effect::Resume));
    assert_eq!(p.snapshot().transport, Transport::Playing { id: ids[0] });
}

#[test]
fn pausing_while_loading_cancels_the_load() {
    let (mut p, ids, _) = started(2);
    assert!(has(&p.command(Command::Pause), |e| *e == Effect::Stop));
    assert_eq!(
        p.snapshot().transport,
        Transport::Paused {
            id: ids[0],
            loaded: false
        }
    );
    let fx = p.command(Command::Resume);
    assert_eq!(start_id(&fx), Some(ids[0]));
}

#[test]
fn previous_restarts_after_three_seconds_otherwise_goes_back() {
    let (mut p, ids, _) = started(3);
    p.engine(E::Audible { id: ids[0] });
    p.command(Command::Next);
    p.engine(E::Audible { id: ids[1] });

    p.engine(E::Position {
        id: ids[1],
        ms: 5_000,
    });
    let fx = p.command(Command::Previous);
    assert!(has(&fx, |e| *e == Effect::Seek { ms: 0 }), "{fx:?}");
    assert_eq!(start_id(&fx), None);

    p.engine(E::Position {
        id: ids[1],
        ms: 1_000,
    });
    let fx = p.command(Command::Previous);
    assert_eq!(start_id(&fx), Some(ids[0]));
}

#[test]
fn previous_at_the_very_first_track_restarts_it() {
    let (mut p, ids, _) = started(2);
    p.engine(E::Audible { id: ids[0] });
    let fx = p.command(Command::Previous);
    assert!(has(&fx, |e| *e == Effect::Seek { ms: 0 }));
}

#[test]
fn seek_is_clamped_to_the_track_duration() {
    let mut p = Player::new(1);
    let mut t = tr("a");
    t.duration_ms = Some(200_000);
    p.command(Command::PlayTracks {
        tracks: vec![t],
        start: 0,
    });
    let id = p.queue().entries()[0].id;
    p.engine(E::Audible { id });
    let fx = p.command(Command::Seek(900_000));
    assert!(has(&fx, |e| *e == Effect::Seek { ms: 200_000 }));
}

#[test]
fn next_at_the_end_without_autoplay_stops() {
    let (mut p, ids, _) = started(1);
    p.command(Command::SetAutoplay(false));
    p.engine(E::Audible { id: ids[0] });
    let fx = p.command(Command::Next);
    assert!(has(&fx, |e| *e == Effect::Stop));
    assert_eq!(p.snapshot().transport, Transport::Idle);
}

#[test]
fn autoplay_asks_for_radio_once_the_queue_is_nearly_empty() {
    let (mut p, ids, _) = started(2);
    let fx = p.engine(E::Audible { id: ids[0] });
    assert!(
        has(&fx, |e| matches!(e, Effect::NeedRadio { .. })),
        "one track left: ask for more"
    );
    let fx = p.engine(E::Position {
        id: ids[0],
        ms: 1000,
    });
    assert!(
        !has(&fx, |e| matches!(e, Effect::NeedRadio { .. })),
        "only asks once"
    );
}

#[test]
fn radio_tracks_are_appended_and_dont_repeat_queue_tracks() {
    let (mut p, ids, _) = started(2);
    p.engine(E::Audible { id: ids[0] });
    p.command(Command::AppendRadio(vec![tr("a"), tr("r1"), tr("r2")]));
    let keys: Vec<_> = p
        .queue()
        .entries()
        .iter()
        .map(|e| e.track.key.clone())
        .collect();
    assert_eq!(keys, ["a", "b", "r1", "r2"], "'a' is already queued");
}

#[test]
fn next_at_the_end_with_autoplay_waits_for_radio_then_continues() {
    let (mut p, ids, _) = started(1);
    p.engine(E::Audible { id: ids[0] });
    let fx = p.command(Command::Next);
    assert!(
        has(&fx, |e| matches!(e, Effect::NeedRadio { .. }))
            || p.snapshot().transport == Transport::Playing { id: ids[0] }
    );
    assert_eq!(
        p.snapshot().transport,
        Transport::Playing { id: ids[0] },
        "keeps playing meanwhile"
    );
    let fx = p.command(Command::AppendRadio(vec![tr("r1")]));
    let r1 = p.queue().entries()[1].id;
    assert_eq!(start_id(&fx), Some(r1));
}

#[test]
fn empty_radio_result_stops_instead_of_looping() {
    let (mut p, ids, _) = started(1);
    p.engine(E::Audible { id: ids[0] });
    p.command(Command::Next);
    let fx = p.command(Command::AppendRadio(vec![]));
    assert!(has(&fx, |e| *e == Effect::Stop));
    let fx = p.engine(E::Position { id: ids[0], ms: 5 });
    assert!(!has(&fx, |e| matches!(e, Effect::NeedRadio { .. })));
}

#[test]
fn removing_the_playing_track_moves_on_immediately() {
    let (mut p, ids, _) = started(3);
    p.engine(E::Audible { id: ids[0] });
    let fx = p.command(Command::Remove(ids[0]));
    assert_eq!(start_id(&fx), Some(ids[1]));
}

#[test]
fn removing_the_prefetched_track_replans_and_undo_brings_it_back() {
    let (mut p, ids, _) = started(3);
    p.engine(E::Audible { id: ids[0] });
    p.engine(E::Prefetched { id: ids[1] });
    let fx = p.command(Command::Remove(ids[1]));
    assert!(has(&fx, |e| *e == Effect::CancelPrefetch));
    assert_eq!(prefetch_id(&fx), Some(ids[2]));
    assert!(p.snapshot().can_undo);
    let fx = p.command(Command::Undo);
    assert_eq!(
        prefetch_id(&fx),
        Some(ids[1]),
        "restored track is next again"
    );
}

#[test]
fn play_next_replaces_the_prefetch_target() {
    let (mut p, ids, _) = started(3);
    p.engine(E::Audible { id: ids[0] });
    p.engine(E::Prefetched { id: ids[1] });
    let fx = p.command(Command::PlayNext(tr("x")));
    assert!(has(&fx, |e| *e == Effect::CancelPrefetch));
    let x = p.queue().entries()[1].id;
    assert_eq!(prefetch_id(&fx), Some(x));
}

#[test]
fn shuffle_toggle_replans_the_prefetch() {
    let (mut p, ids, _) = started(8);
    p.engine(E::Audible { id: ids[0] });
    p.engine(E::Prefetched { id: ids[1] });
    let fx = p.command(Command::SetShuffle(true));
    let next = p.queue().next_candidate(true, &|_| false).unwrap();
    if next != ids[1] {
        assert_eq!(prefetch_id(&fx), Some(next));
    }
    assert!(p.queue().shuffle());
}

#[test]
fn repeat_one_prefetches_the_same_track_and_resets_position() {
    let (mut p, ids, _) = started(2);
    p.command(Command::SetRepeat(Repeat::One));
    let fx = p.engine(E::Audible { id: ids[0] });
    assert_eq!(prefetch_id(&fx), Some(ids[0]));
    p.engine(E::Prefetched { id: ids[0] });
    p.engine(E::Position {
        id: ids[0],
        ms: 180_000,
    });
    assert!(p.engine(E::Ended { id: ids[0] }).is_empty());
    p.engine(E::Audible { id: ids[0] });
    assert_eq!(p.snapshot().audible, Some(ids[0]));
    assert_eq!(p.snapshot().position_ms, 0);
}

#[test]
fn clear_stops_playback_and_undo_restores_the_selection() {
    let (mut p, ids, _) = started(2);
    p.engine(E::Audible { id: ids[0] });
    let fx = p.command(Command::Clear);
    assert!(has(&fx, |e| *e == Effect::Stop));
    assert_eq!(p.snapshot().transport, Transport::Idle);
    p.command(Command::Undo);
    assert_eq!(
        p.snapshot().transport,
        Transport::Paused {
            id: ids[0],
            loaded: false
        }
    );
}

#[test]
fn stalled_and_recovered_toggle_the_transport() {
    let (mut p, ids, _) = started(2);
    p.engine(E::Audible { id: ids[0] });
    p.engine(E::Stalled { id: ids[0] });
    assert_eq!(p.snapshot().transport, Transport::Stalled { id: ids[0] });
    assert_eq!(
        p.snapshot().audible,
        Some(ids[0]),
        "the old audio is still what the user last heard"
    );
    p.engine(E::Recovered { id: ids[0] });
    assert_eq!(p.snapshot().transport, Transport::Playing { id: ids[0] });
}

#[test]
fn stale_engine_events_are_ignored() {
    let (mut p, ids, _) = started(3);
    p.engine(E::Audible { id: ids[0] });
    p.command(Command::Next);
    p.engine(E::Audible { id: ids[1] });
    // Late event from the track we skipped:
    assert!(p.engine(E::Ended { id: ids[0] }).is_empty());
    assert_eq!(p.snapshot().audible, Some(ids[1]));
}

#[test]
fn session_survives_a_restart_and_resumes_where_it_left_off() {
    let (mut p, ids, _) = started(3);
    p.engine(E::Audible { id: ids[0] });
    p.command(Command::Next);
    p.engine(E::Audible { id: ids[1] });
    p.engine(E::Position {
        id: ids[1],
        ms: 83_000,
    });
    p.command(Command::SetRepeat(Repeat::All));

    let json = serde_json::to_string(&p.persisted()).unwrap();
    let saved: Persisted = serde_json::from_str(&json).unwrap();
    let mut q = Player::restore(saved);

    assert_eq!(
        q.snapshot().transport,
        Transport::Paused {
            id: ids[1],
            loaded: false
        }
    );
    assert_eq!(q.snapshot().position_ms, 83_000);
    assert_eq!(q.snapshot().repeat, Repeat::All);
    assert_eq!(q.queue().entries().len(), 3);
    let fx = q.command(Command::Resume);
    assert!(has(
        &fx,
        |e| matches!(e, Effect::StartTrack { id, start_ms: 83_000, .. } if *id == ids[1])
    ));
}

#[test]
fn every_queue_edit_asks_to_persist() {
    let (mut p, ids, _) = started(2);
    p.engine(E::Audible { id: ids[0] });
    for c in [
        Command::AddToQueue(tr("x")),
        Command::Move { id: ids[1], to: 0 },
        Command::SetRepeat(Repeat::All),
    ] {
        assert!(has(&p.command(c), |e| *e == Effect::Persist));
    }
}

fn details(artist: Option<&str>, ms: Option<u64>, thumb: Option<&str>) -> TrackDetails {
    TrackDetails {
        artist: artist.map(str::to_string),
        duration_ms: ms,
        thumbnail: thumb.map(str::to_string),
    }
}

#[test]
fn resolver_details_fill_an_unknown_artist_and_a_missing_length_in_the_queue_entry() {
    let mut p = Player::new(7);
    let mut t = Track::new("yt:x", "Neon Harbor", UNKNOWN_ARTIST);
    t.thumbnail = Some("https://i.ytimg.com/vi/x/hqdefault.jpg".into());
    p.command(Command::PlayTracks {
        tracks: vec![t],
        start: 0,
    });
    let id = p.queue().entries()[0].id;
    p.engine(E::TrackDetails {
        id,
        details: details(
            Some("Lowtide"),
            Some(213_500),
            Some("https://big/cover.jpg"),
        ),
    });
    let t = &p.queue().get(id).unwrap().track;
    assert_eq!(t.artist, "Lowtide");
    assert_eq!(t.duration_ms, Some(213_500));
    assert_eq!(t.thumbnail.as_deref(), Some("https://big/cover.jpg"));
    assert_eq!(t.title, "Neon Harbor", "the title is never touched");
}

#[test]
fn resolver_details_never_overwrite_a_known_artist_or_length() {
    let mut t = Track::new("yt:x", "S", "Curated Artist");
    t.duration_ms = Some(200_000);
    let mut p = Player::new(7);
    p.command(Command::PlayTracks {
        tracks: vec![t],
        start: 0,
    });
    let id = p.queue().entries()[0].id;
    p.engine(E::TrackDetails {
        id,
        details: details(Some("Channel Name"), Some(199_000), None),
    });
    let t = &p.queue().get(id).unwrap().track;
    assert_eq!(
        (t.artist.as_str(), t.duration_ms),
        ("Curated Artist", Some(200_000))
    );
}

#[test]
fn details_for_an_entry_that_left_the_queue_are_ignored_and_empty_details_change_nothing() {
    let (mut p, ids, _) = started(1);
    p.engine(E::TrackDetails {
        id: EntryId(9_999),
        details: details(Some("A"), Some(1), None),
    });
    p.engine(E::TrackDetails {
        id: ids[0],
        details: details(Some("  "), Some(0), Some("")),
    });
    let t = &p.queue().get(ids[0]).unwrap().track;
    assert_eq!(
        (t.artist.as_str(), t.duration_ms, t.thumbnail.as_deref()),
        ("artist", None, None)
    );
}

fn upcoming(p: &Player) -> Vec<EntryId> {
    p.queue().upcoming()
}

#[test]
fn an_upcoming_song_can_be_moved_to_play_next_and_the_visible_list_follows() {
    let (mut p, ids, _) = started(4); // a (current), b, c, d
    p.command(Command::MoveUpcoming { id: ids[3], to: 0 });
    assert_eq!(upcoming(&p), vec![ids[3], ids[1], ids[2]]);
    let shown: Vec<EntryId> = p.queue().entries().iter().map(|e| e.id).collect();
    assert_eq!(shown, vec![ids[0], ids[3], ids[1], ids[2]]);
    // And down again, to the end (an index past the end clamps).
    p.command(Command::MoveUpcoming { id: ids[3], to: 99 });
    assert_eq!(upcoming(&p), vec![ids[1], ids[2], ids[3]]);
}

#[test]
fn the_current_song_and_played_songs_cannot_be_moved_in_up_next() {
    let (mut p, ids, _) = started(3);
    p.command(Command::PlayEntry(ids[1])); // b is current; a is behind us
    let before = upcoming(&p);
    p.command(Command::MoveUpcoming { id: ids[1], to: 0 });
    p.command(Command::MoveUpcoming { id: ids[0], to: 0 });
    p.command(Command::MoveUpcoming {
        id: EntryId(999),
        to: 0,
    });
    assert_eq!(upcoming(&p), before);
}

#[test]
fn under_shuffle_up_next_reorders_play_order_but_not_the_list_the_user_arranged() {
    let (mut p, ids, _) = started(6);
    p.command(Command::SetShuffle(true));
    let shown_before: Vec<EntryId> = p.queue().entries().iter().map(|e| e.id).collect();
    let last = *upcoming(&p).last().unwrap();
    p.command(Command::MoveUpcoming { id: last, to: 0 });
    assert_eq!(upcoming(&p)[0], last, "it plays next now");
    let shown_after: Vec<EntryId> = p.queue().entries().iter().map(|e| e.id).collect();
    assert_eq!(shown_before, shown_after);
    assert_eq!(ids.len(), 6);
}

#[test]
fn moving_a_song_to_play_next_changes_what_is_prefetched() {
    let (mut p, ids, _) = started(4);
    p.engine(E::Audible { id: ids[0] });
    p.engine(E::Prefetched { id: ids[1] });
    let fx = p.command(Command::MoveUpcoming { id: ids[3], to: 0 });
    assert!(has(&fx, |e| matches!(e, Effect::CancelPrefetch)), "{fx:?}");
    assert_eq!(prefetch_id(&fx), Some(ids[3]), "{fx:?}");
}
