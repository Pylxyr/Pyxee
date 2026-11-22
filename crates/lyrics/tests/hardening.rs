//! Remote text (LRCLIB answers, `YouTube` artist names) must never panic the lookup or pick the wrong lyrics.

// Pedantic lints this project does not follow, allowed once per crate root. (A workspace-level allow loses to the
// `-D clippy::pedantic` that is passed on the command line, so it has to be an attribute.) Each is a trade-off for
// application code: `# Errors` / `# Panics` sections and `#[must_use]` on internal items, long match-heavy functions,
// flag structs, short names in numeric code, and the casts between sample counts, milliseconds and pixels (their
// ranges are bounded by what the device and the files can hold) are not worth the noise.
#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::return_self_not_must_use,
    clippy::too_many_lines,
    clippy::struct_excessive_bools,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss
)]

use pyxee_lyrics::{Lyrics, clean_artist, from_lrclib};

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
