//! Remote text (LRCLIB answers, YouTube artist names) must never panic the lookup or pick the wrong lyrics.

use pyxee_lyrics::{clean_artist, from_lrclib, Lyrics};

/// `to_lowercase` can change a string's byte length (`İ` is 2 bytes, its lower case `i̇` is 3), so offsets found in
/// the lower-cased copy are not offsets into the original.
#[test]
fn artist_cleanup_cuts_at_the_right_place_when_lower_casing_changes_the_length() {
    assert_eq!(clean_artist("İstanbul & Foo"), "İstanbul");
    assert_eq!(clean_artist("İİİ, Foo"), "İİİ");
    // Never a panic, whatever the name is made of.
    for name in ["İ & ß", "ẞ x İ", "İ feat. İ", "K & İ", "ǅ and ǆ"] {
        let _ = clean_artist(name);
    }
}

#[test]
fn of_two_equally_good_candidates_the_closer_length_wins() {
    let list = format!(
        "[{},{}]",
        r#"{"duration":200,"syncedLyrics":"[00:01.00]exact"}"#,
        r#"{"duration":203,"syncedLyrics":"[00:01.00]slightly off"}"#
    );
    match from_lrclib(&list, Some(200.0)).unwrap() {
        Lyrics::Synced(l) => assert_eq!(l[0].text, "exact"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn one_malformed_candidate_does_not_hide_the_good_ones() {
    let list = format!(
        "[{},{}]",
        r#"{"duration":"3:20","instrumental":null,"syncedLyrics":12}"#,
        r#"{"duration":200,"syncedLyrics":"[00:01.00]fine"}"#
    );
    match from_lrclib(&list, Some(200.0)).unwrap() {
        Lyrics::Synced(l) => assert_eq!(l[0].text, "fine"),
        other => panic!("{other:?}"),
    }
}
