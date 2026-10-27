use pyxee_core::Track;
use pyxee_library::{HistoryItem, Library, LibraryError, MAX_NAME_LEN};

const DAY: i64 = 24 * 60 * 60 * 1000;

fn t(n: &str) -> Track {
    let mut t = Track::new(
        &format!("yt:{n}"),
        &format!("Title {n}"),
        &format!("Artist {n}"),
    );
    t.duration_ms = Some(200_000);
    t.thumbnail = Some(format!("https://img/{n}.jpg"));
    t
}
fn lib() -> Library {
    Library::open_in_memory().unwrap()
}
fn titles(l: &Library, id: i64) -> Vec<String> {
    l.playlist(id)
        .unwrap()
        .entries
        .iter()
        .map(|e| e.track.title.clone())
        .collect()
}
fn names(tracks: &[Track]) -> Vec<String> {
    tracks.iter().map(|t| t.title.clone()).collect()
}

// ---------------------------------------------------------------- likes
#[test]
fn liking_is_idempotent_and_listed_newest_first() {
    let mut l = lib();
    assert!(l.like(&t("a"), 100).unwrap());
    assert!(l.like(&t("b"), 200).unwrap());
    assert!(!l.like(&t("a"), 300).unwrap(), "already liked");
    assert_eq!(
        names(&l.liked().unwrap()),
        ["Title b", "Title a"],
        "order is by when it was first liked"
    );
    assert!(l.is_liked("yt:a").unwrap());
    assert_eq!(l.liked_keys().unwrap(), ["yt:b", "yt:a"]);
}

#[test]
fn unliking_removes_only_that_song() {
    let mut l = lib();
    l.like(&t("a"), 1).unwrap();
    l.like(&t("b"), 2).unwrap();
    assert!(l.unlike("yt:a").unwrap());
    assert!(!l.unlike("yt:a").unwrap(), "second unlike is a no-op");
    assert!(!l.is_liked("yt:a").unwrap());
    assert_eq!(names(&l.liked().unwrap()), ["Title b"]);
}

#[test]
fn liking_remembers_the_song_details() {
    let mut l = lib();
    l.like(&t("a"), 1).unwrap();
    assert_eq!(l.liked().unwrap()[0], t("a"));
}

#[test]
fn a_better_later_description_of_a_song_updates_it_without_losing_known_fields() {
    let mut l = lib();
    l.like(&t("a"), 1).unwrap();
    let mut sparse = Track::new("yt:a", "Title a (remaster)", "Artist a");
    sparse.duration_ms = None;
    sparse.thumbnail = None;
    l.record_play(&sparse, 2).unwrap();
    let got = &l.liked().unwrap()[0];
    assert_eq!(got.title, "Title a (remaster)");
    assert_eq!(got.duration_ms, Some(200_000), "kept the known duration");
    assert!(got.thumbnail.is_some(), "kept the known artwork");
}

// ------------------------------------------------------------ playlists
#[test]
fn creating_a_playlist_keeps_the_given_order_and_repeats() {
    let mut l = lib();
    let id = l
        .create_playlist("  Road trip  ", &[t("a"), t("b"), t("a")], 10)
        .unwrap();
    let p = l.playlist(id).unwrap();
    assert_eq!(p.name, "Road trip", "name is trimmed");
    assert_eq!(titles(&l, id), ["Title a", "Title b", "Title a"]);
    assert_eq!((p.created_at, p.updated_at), (10, 10));
}

#[test]
fn names_must_be_one_to_one_hundred_characters() {
    let mut l = lib();
    assert!(matches!(
        l.create_playlist("   ", &[], 1),
        Err(LibraryError::InvalidName)
    ));
    assert!(matches!(
        l.create_playlist(&"x".repeat(MAX_NAME_LEN + 1), &[], 1),
        Err(LibraryError::InvalidName)
    ));
    assert!(
        l.create_playlist(&"é".repeat(MAX_NAME_LEN), &[], 1).is_ok(),
        "limit counts characters, not bytes"
    );
    let id = l.create_playlist("ok", &[], 1).unwrap();
    assert!(matches!(
        l.rename_playlist(id, "", 2),
        Err(LibraryError::InvalidName)
    ));
    assert_eq!(l.playlist(id).unwrap().name, "ok");
}

#[test]
fn renaming_changes_the_name_and_bumps_the_update_time() {
    let mut l = lib();
    let id = l.create_playlist("Old", &[], 10).unwrap();
    l.rename_playlist(id, "日本の音楽", 99).unwrap();
    let p = l.playlist(id).unwrap();
    assert_eq!((p.name.as_str(), p.updated_at), ("日本の音楽", 99));
    assert!(matches!(
        l.rename_playlist(777, "x", 1),
        Err(LibraryError::NotFound("playlist"))
    ));
}

#[test]
fn adding_skips_songs_already_there_and_repeats_in_the_request() {
    let mut l = lib();
    let id = l.create_playlist("P", &[t("a")], 1).unwrap();
    let r = l
        .add_to_playlist(id, &[t("a"), t("b"), t("b"), t("c")], 2)
        .unwrap();
    assert_eq!((r.added, r.skipped), (2, 2));
    assert_eq!(titles(&l, id), ["Title a", "Title b", "Title c"]);
    assert!(matches!(
        l.add_to_playlist(42, &[t("a")], 3),
        Err(LibraryError::NotFound("playlist"))
    ));
}

#[test]
fn the_playlist_list_shows_counts_covers_and_most_recent_first() {
    let mut l = lib();
    let old = l.create_playlist("Old", &[t("a"), t("b")], 10).unwrap();
    let new = l.create_playlist("New", &[], 20).unwrap();
    let list = l.playlists().unwrap();
    assert_eq!(list.iter().map(|p| p.id).collect::<Vec<_>>(), [new, old]);
    assert_eq!(list[1].track_count, 2);
    assert_eq!(
        list[1].thumbnail.as_deref(),
        Some("https://img/a.jpg"),
        "cover = first track"
    );
    assert_eq!(list[0].thumbnail, None);
    l.add_to_playlist(old, &[t("z")], 30).unwrap();
    assert_eq!(
        l.playlists().unwrap()[0].id,
        old,
        "editing moves it to the top"
    );
}

#[test]
fn deleting_then_restoring_brings_back_everything() {
    let mut l = lib();
    let id = l
        .create_playlist("Keep me", &[t("a"), t("b"), t("c")], 5)
        .unwrap();
    l.add_to_playlist(id, &[t("d")], 6).unwrap();
    let snap = l.delete_playlist(id).unwrap();
    assert!(matches!(
        l.playlist(id),
        Err(LibraryError::NotFound("playlist"))
    ));
    assert!(l.playlists().unwrap().is_empty());
    l.restore_playlist(&snap).unwrap();
    let p = l.playlist(id).unwrap();
    assert_eq!(
        (p.name.as_str(), p.created_at, p.updated_at),
        ("Keep me", 5, 6)
    );
    assert_eq!(titles(&l, id), ["Title a", "Title b", "Title c", "Title d"]);
    assert!(matches!(
        l.delete_playlist(999),
        Err(LibraryError::NotFound(_))
    ));
}

#[test]
fn removing_an_entry_closes_the_gap_and_undo_puts_it_back_in_place() {
    let mut l = lib();
    let id = l
        .create_playlist("P", &[t("a"), t("b"), t("c"), t("d")], 1)
        .unwrap();
    let b = l.playlist(id).unwrap().entries[1].entry_id;
    let removed = l.remove_entry(b, 2).unwrap();
    assert_eq!(
        (removed.position, removed.track.title.as_str()),
        (1, "Title b")
    );
    assert_eq!(titles(&l, id), ["Title a", "Title c", "Title d"]);
    l.restore_entry(&removed, 3).unwrap();
    assert_eq!(titles(&l, id), ["Title a", "Title b", "Title c", "Title d"]);
    assert!(matches!(
        l.remove_entry(123_456, 4),
        Err(LibraryError::NotFound(_))
    ));
}

#[test]
fn restoring_an_entry_clamps_if_the_playlist_shrank_meanwhile() {
    let mut l = lib();
    let id = l
        .create_playlist("P", &[t("a"), t("b"), t("c")], 1)
        .unwrap();
    let c = l.playlist(id).unwrap().entries[2].entry_id;
    let removed = l.remove_entry(c, 2).unwrap();
    let a = l.playlist(id).unwrap().entries[0].entry_id;
    l.remove_entry(a, 3).unwrap();
    l.restore_entry(&removed, 4).unwrap();
    assert_eq!(titles(&l, id), ["Title b", "Title c"]);
}

#[test]
fn the_same_song_twice_is_two_independent_entries() {
    let mut l = lib();
    let id = l
        .create_playlist("P", &[t("a"), t("b"), t("a")], 1)
        .unwrap();
    let last = l.playlist(id).unwrap().entries[2].entry_id;
    l.remove_entry(last, 2).unwrap();
    assert_eq!(titles(&l, id), ["Title a", "Title b"]);
}

#[test]
fn moving_entries_matches_dragging_in_a_list() {
    let mut l = lib();
    let id = l
        .create_playlist("P", &[t("a"), t("b"), t("c"), t("d")], 1)
        .unwrap();
    let e = |l: &Library, i: usize| l.playlist(id).unwrap().entries[i].entry_id;
    l.move_entry(e(&l, 0), 2, 2).unwrap();
    assert_eq!(titles(&l, id), ["Title b", "Title c", "Title a", "Title d"]);
    l.move_entry(e(&l, 3), 0, 3).unwrap();
    assert_eq!(titles(&l, id), ["Title d", "Title b", "Title c", "Title a"]);
    l.move_entry(e(&l, 1), 1, 4).unwrap(); // no-op
    assert_eq!(titles(&l, id), ["Title d", "Title b", "Title c", "Title a"]);
    l.move_entry(e(&l, 0), 99, 5).unwrap(); // clamped to the end
    assert_eq!(titles(&l, id), ["Title b", "Title c", "Title a", "Title d"]);
}

#[test]
fn positions_stay_contiguous_after_many_edits() {
    let mut l = lib();
    let id = l
        .create_playlist(
            "P",
            &(0..8).map(|i| t(&i.to_string())).collect::<Vec<_>>(),
            1,
        )
        .unwrap();
    for step in 0..30usize {
        let entries = l.playlist(id).unwrap().entries;
        let pick = entries[step * 7 % entries.len()].entry_id;
        if step % 3 == 0 {
            let r = l.remove_entry(pick, step as i64).unwrap();
            l.restore_entry(&r, step as i64).unwrap();
        } else {
            l.move_entry(pick, step * 5 % entries.len(), step as i64)
                .unwrap();
        }
    }
    let mut got = titles(&l, id);
    got.sort();
    let mut want: Vec<String> = (0..8).map(|i| format!("Title {i}")).collect();
    want.sort();
    assert_eq!(got, want, "nothing lost or duplicated");
}

// -------------------------------------------------------------- history
#[test]
fn history_lists_every_play_newest_first_and_recent_tracks_are_distinct() {
    let mut l = lib();
    for (n, at) in [("a", 1), ("b", 2), ("a", 3), ("c", 4)] {
        l.record_play(&t(n), at).unwrap();
    }
    let h = l.history(10).unwrap();
    assert_eq!(
        h.iter().map(|i| i.track.title.as_str()).collect::<Vec<_>>(),
        ["Title c", "Title a", "Title b", "Title a"]
    );
    assert_eq!(
        names(&l.recent_tracks(10).unwrap()),
        ["Title c", "Title a", "Title b"]
    );
    assert_eq!(l.history(2).unwrap().len(), 2);
    assert_eq!(l.history_count().unwrap(), 4);
}

#[test]
fn clearing_history_can_be_undone_with_original_times() {
    let mut l = lib();
    l.record_play(&t("a"), 100).unwrap();
    l.record_play(&t("b"), 200).unwrap();
    let before = l.history(10).unwrap();
    let cleared = l.clear_history().unwrap();
    assert_eq!(l.history_count().unwrap(), 0);
    assert_eq!(cleared, before);
    l.restore_history(&cleared).unwrap();
    assert_eq!(l.history(10).unwrap(), before);
    assert_eq!(
        l.history(10)
            .unwrap()
            .iter()
            .map(|h: &HistoryItem| h.played_at)
            .collect::<Vec<_>>(),
        [200, 100]
    );
}

// ----------------------------------------------------------------- home
#[test]
fn quick_picks_favour_often_played_and_liked_songs_and_ignore_the_rest() {
    let mut l = lib();
    let now = 100 * DAY;
    for _ in 0..5 {
        l.record_play(&t("often"), now - DAY).unwrap();
    }
    l.record_play(&t("once"), now - 2 * DAY).unwrap();
    l.like(&t("liked-only"), now - 50 * DAY).unwrap();
    l.record_play(&t("ancient"), now - 90 * DAY).unwrap(); // outside the window, not liked
    let h = l.home(now, 10).unwrap();
    let keys: Vec<_> = h.quick_picks.iter().map(|t| t.key.as_str()).collect();
    assert_eq!(keys, ["yt:often", "yt:liked-only", "yt:once"]);
    assert_eq!(h.liked_count, 1);
    assert!(
        h.recent.iter().any(|t| t.key == "yt:ancient"),
        "recently played is just the latest plays"
    );
}

#[test]
fn quick_picks_respect_the_limit_and_are_empty_for_a_new_library() {
    let mut l = lib();
    assert!(l.home(10 * DAY, 5).unwrap().quick_picks.is_empty());
    for i in 0..10 {
        l.record_play(&t(&i.to_string()), 9 * DAY + i).unwrap();
    }
    assert_eq!(l.home(10 * DAY, 4).unwrap().quick_picks.len(), 4);
}

#[test]
fn a_recent_play_outranks_an_equally_played_old_one() {
    let mut l = lib();
    let now = 60 * DAY;
    l.record_play(&t("old"), now - 29 * DAY).unwrap();
    l.record_play(&t("new"), now - DAY).unwrap();
    let keys: Vec<_> = l
        .home(now, 5)
        .unwrap()
        .quick_picks
        .into_iter()
        .map(|t| t.key)
        .collect();
    assert_eq!(keys, ["yt:new", "yt:old"]);
}

// ---------------------------------------------------------- persistence
#[test]
fn everything_survives_closing_and_reopening_the_file() {
    let dir = std::env::temp_dir().join(format!("pyxee-lib-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("nested").join("library.db");
    {
        let mut l = Library::open(&path).unwrap();
        l.like(&t("a"), 1).unwrap();
        let id = l.create_playlist("Mine", &[t("a"), t("b")], 2).unwrap();
        l.record_play(&t("b"), 3).unwrap();
        assert_eq!(id, 1);
    }
    let l = Library::open(&path).unwrap();
    assert_eq!(names(&l.liked().unwrap()), ["Title a"]);
    assert_eq!(titles(&l, 1), ["Title a", "Title b"]);
    assert_eq!(l.history_count().unwrap(), 1);
    let again = Library::open(&path).unwrap(); // opening again must not re-run migrations or fail
    assert_eq!(again.playlists().unwrap().len(), 1);
}

#[test]
fn a_library_from_a_newer_version_is_refused_and_left_untouched() {
    let dir = std::env::temp_dir().join(format!("pyxee-lib-newer-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("library.db");
    {
        let mut l = Library::open(&path).unwrap();
        l.like(&t("a"), 1).unwrap();
    }
    {
        let raw = rusqlite::Connection::open(&path).unwrap();
        raw.pragma_update(None, "user_version", 99).unwrap();
    }
    let err = Library::open(&path).err().expect("must not open");
    assert!(matches!(err, LibraryError::NewerVersion { found: 99, .. }), "{err}");
    assert!(err.to_string().contains("newer Pyxee"), "{err}");
    let raw = rusqlite::Connection::open(&path).unwrap();
    let v: i64 = raw.pragma_query_value(None, "user_version", |r| r.get(0)).unwrap();
    assert_eq!(v, 99, "the file must not be changed");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_that_is_not_a_database_is_a_clear_error_not_a_panic() {
    let dir = std::env::temp_dir().join(format!("pyxee-lib-bad-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("library.db");
    std::fs::write(
        &path,
        b"this is definitely not sqlite, it is just some text padding it out to be long enough",
    )
    .unwrap();
    let err = Library::open(&path).err().expect("must not open");
    assert!(
        err.to_string().contains("library could not be read"),
        "{err}"
    );
}

#[test]
fn a_liked_song_saved_as_unknown_artist_learns_its_artist_length_and_cover() {
    let mut l = lib();
    let mut song = Track::new("yt:u", "Neon Harbor", pyxee_core::UNKNOWN_ARTIST);
    song.duration_ms = None;
    l.like(&song, 1).unwrap();
    let mut known = song.clone();
    known.artist = "Lowtide".into();
    known.duration_ms = Some(213_500);
    known.thumbnail = Some("https://big/cover.jpg".into());
    assert!(l.refresh_track(&known).unwrap());
    assert!(
        !l.refresh_track(&known).unwrap(),
        "second time nothing changes"
    );
    let back = &l.liked().unwrap()[0];
    assert_eq!(back, &known);
}

#[test]
fn refreshing_never_adds_a_song_and_a_later_unknown_artist_never_erases_a_known_one() {
    let mut l = lib();
    let ghost = t("ghost");
    assert!(!l.refresh_track(&ghost).unwrap());
    assert!(l.liked().unwrap().is_empty());

    l.like(&t("a"), 1).unwrap();
    let mut again = t("a");
    again.artist = pyxee_core::UNKNOWN_ARTIST.into(); // e.g. liked again from a search row
    l.record_play(&again, 2).unwrap();
    assert_eq!(l.liked().unwrap()[0].artist, "Artist a");
}
