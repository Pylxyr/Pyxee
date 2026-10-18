use pyxee_lyrics::{
    clean_artist, clean_title, current_index, fetch, from_lrclib, parse_lrc, pick_best, Candidate,
    Line, Lyrics, Query,
};
use std::io::{Read, Write};
use std::net::TcpListener;

fn lines(ts: &[(u64, &str)]) -> Vec<Line> {
    ts.iter()
        .map(|(t, s)| Line {
            time_ms: *t,
            text: s.to_string(),
        })
        .collect()
}

#[test]
fn timestamps_in_every_common_form_are_understood() {
    let l = parse_lrc("[00:12.34]a\n[1:05.5]b\n[02:00]c\n[00:03.123]d\n[00:07:25]e\n[120:00.00]f");
    let t: Vec<u64> = l.iter().map(|x| x.time_ms).collect();
    assert_eq!(t, vec![3_123, 7_250, 12_340, 65_500, 120_000, 7_200_000]);
}

#[test]
fn several_timestamps_before_one_line_repeat_it_and_the_result_is_sorted() {
    let l = parse_lrc("[00:30.00][01:10.00]chorus\n[00:10.00]verse");
    assert_eq!(
        l,
        lines(&[(10_000, "verse"), (30_000, "chorus"), (70_000, "chorus")])
    );
}

#[test]
fn metadata_tags_and_junk_are_ignored_but_bracketed_words_stay() {
    let l = parse_lrc(
        "[ar:Someone]\n[ti:Song]\n[by:me]\nno stamp here\n[00:01.00][Chorus] la la\n\n[00:02.00]",
    );
    assert_eq!(l, lines(&[(1_000, "[Chorus] la la"), (2_000, "")]));
}

#[test]
fn the_offset_tag_shifts_every_line_and_never_goes_below_zero() {
    // Positive offset: lyrics appear earlier.
    let l = parse_lrc("[offset:+500]\n[00:01.00]a\n[00:00.20]b");
    assert_eq!(l, lines(&[(0, "b"), (500, "a")]));
    let l = parse_lrc("[offset:-250]\n[00:01.00]a");
    assert_eq!(l, lines(&[(1_250, "a")]));
}

#[test]
fn enhanced_word_marks_are_stripped() {
    let l = parse_lrc("[00:01.00]<00:01.00>Hello <00:01.50>world <b>");
    assert_eq!(l[0].text, "Hello world <b>");
}

#[test]
fn byte_order_marks_windows_line_endings_and_garbage_do_not_panic() {
    let l = parse_lrc(
        "\u{feff}[00:01.00]a\r\n[00:02.00]b\r\n[zz:yy]c\r\n[99:99.99]d\r\n[00:61.00]e\r\n[:]f",
    );
    assert_eq!(l, lines(&[(1_000, "a"), (2_000, "b")]));
    assert!(parse_lrc("").is_empty());
    assert!(parse_lrc("[").is_empty());
}

#[test]
fn the_current_line_is_the_last_one_that_has_started() {
    let l = lines(&[(1_000, "a"), (5_000, "b"), (9_000, "c")]);
    assert_eq!(current_index(&l, 0), None);
    assert_eq!(current_index(&l, 999), None);
    assert_eq!(current_index(&l, 1_000), Some(0));
    assert_eq!(current_index(&l, 4_999), Some(0));
    assert_eq!(current_index(&l, 5_000), Some(1));
    assert_eq!(current_index(&l, 1_000_000), Some(2));
    assert_eq!(current_index(&[], 5), None);
}

#[test]
fn search_terms_lose_video_noise_and_extra_artists() {
    assert_eq!(clean_title("Neon Harbor (Official Video)"), "Neon Harbor");
    assert_eq!(
        clean_title("Neon Harbor [Lyrics] (feat. Someone)"),
        "Neon Harbor"
    );
    assert_eq!(clean_title("Song (Remastered 2011)"), "Song");
    assert_eq!(clean_title("Song (Interlude)"), "Song (Interlude)");
    assert_eq!(clean_title("Song (Live)"), "Song (Live)");
    assert_eq!(clean_title("  Plain  "), "Plain");
    assert_eq!(clean_artist("Lowtide - Topic"), "Lowtide");
    assert_eq!(clean_artist("Ana, Ben"), "Ana");
    assert_eq!(clean_artist("Ana & Ben"), "Ana");
    assert_eq!(clean_artist("Ana feat. Ben"), "Ana");
    assert_eq!(clean_artist("ArtistVEVO"), "Artist");
}

const SYNCED: &str = r#"{"id":1,"duration":200.0,"instrumental":false,"plainLyrics":"la\nla","syncedLyrics":"[00:01.00]la\n[00:02.00]la"}"#;

#[test]
fn a_service_answer_gives_synced_lyrics_when_it_has_them_else_plain_else_instrumental() {
    match from_lrclib(SYNCED, Some(200.0)).unwrap() {
        Lyrics::Synced(l) => assert_eq!(l.len(), 2),
        other => panic!("{other:?}"),
    }
    let plain = r#"{"duration":200,"plainLyrics":"just words","syncedLyrics":null}"#;
    assert_eq!(
        from_lrclib(plain, None).unwrap(),
        Lyrics::Plain("just words".into())
    );
    let inst = r#"{"duration":200,"instrumental":true,"plainLyrics":null,"syncedLyrics":null}"#;
    assert_eq!(from_lrclib(inst, None).unwrap(), Lyrics::Instrumental);
    let none = r#"{"duration":200,"plainLyrics":"","syncedLyrics":""}"#;
    assert_eq!(from_lrclib(none, None).unwrap(), Lyrics::None);
    assert!(from_lrclib("not json", None).is_err());
    assert!(from_lrclib("42", None).is_err());
}

#[test]
fn a_search_list_prefers_the_right_length_and_synced_over_plain() {
    let list = format!(
        "[{},{},{}]",
        r#"{"duration":330,"syncedLyrics":"[00:01.00]wrong recording"}"#,
        r#"{"duration":199,"plainLyrics":"plain"}"#,
        r#"{"duration":201,"syncedLyrics":"[00:01.00]right"}"#
    );
    match from_lrclib(&list, Some(200.0)).unwrap() {
        Lyrics::Synced(l) => assert_eq!(l[0].text, "right"),
        other => panic!("{other:?}"),
    }
    // Only a wrong-length candidate: nothing is better than another song's words.
    let only_wrong = r#"[{"duration":330,"syncedLyrics":"[00:01.00]x"}]"#;
    assert_eq!(from_lrclib(only_wrong, Some(200.0)).unwrap(), Lyrics::None);
    let c: Vec<Candidate> = Vec::new();
    assert!(pick_best(c, None).is_none());
}

/// Serves `routes` (path prefix -> (status, body)) and records the request lines.
fn server(routes: Vec<(&'static str, u16, String)>) -> (String, std::sync::mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let mut buf = vec![0u8; 4096];
            let n = s.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            let line = req.lines().next().unwrap_or("").to_string();
            let ua = req
                .lines()
                .any(|l| l.to_lowercase().starts_with("user-agent: pyxee/"));
            let _ = tx.send(format!("{line} ua={ua}"));
            let path = line.split(' ').nth(1).unwrap_or("");
            let (status, body) = routes
                .iter()
                .find(|(p, _, _)| path.starts_with(p))
                .map_or((404, String::new()), |(_, st, b)| (*st, b.clone()));
            let head = format!(
                "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = s.write_all(head.as_bytes());
            let _ = s.write_all(body.as_bytes());
        }
    });
    (format!("http://{addr}"), rx)
}

#[test]
fn fetch_asks_with_the_cleaned_terms_a_user_agent_and_the_rounded_length() {
    let (base, rx) = server(vec![("/api/get", 200, SYNCED.to_string())]);
    let q = Query::new(
        "Neon Harbor (Official Video)",
        "Lowtide - Topic",
        Some(199_600),
    );
    let got = fetch(&base, &q).unwrap();
    assert!(matches!(got, Lyrics::Synced(_)));
    let line = rx.recv().unwrap();
    assert!(
        line.starts_with("GET /api/get?track_name=Neon%20Harbor&artist_name=Lowtide&duration=200 "),
        "{line}"
    );
    assert!(line.ends_with("ua=true"), "{line}");
}

#[test]
fn fetch_falls_back_to_search_when_there_is_no_exact_match() {
    let (base, rx) = server(vec![
        ("/api/get", 404, String::new()),
        ("/api/search", 200, format!("[{SYNCED}]")),
    ]);
    let got = fetch(&base, &Query::new("Song", "Artist", Some(200_000))).unwrap();
    assert!(matches!(got, Lyrics::Synced(_)));
    assert!(rx.recv().unwrap().contains("/api/get"));
    assert!(rx.recv().unwrap().contains("/api/search"));
}

#[test]
fn nothing_found_is_none_not_an_error_and_a_dead_server_is_a_network_error() {
    let (base, _rx) = server(vec![]);
    assert_eq!(
        fetch(&base, &Query::new("x", "y", None)).unwrap(),
        Lyrics::None
    );
    let dead = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://{}", l.local_addr().unwrap())
    };
    assert!(matches!(
        fetch(&dead, &Query::new("x", "y", None)),
        Err(pyxee_lyrics::LyricsError::Network)
    ));
}

#[test]
fn service_refusals_are_told_apart_by_what_the_player_should_do() {
    use pyxee_lyrics::LyricsError;
    let ask = |status: u16| {
        let (base, _rx) = server(vec![("/api", status, "no".into())]);
        fetch(&base, &Query::new("x", "y", None)).unwrap_err()
    };
    // 429: transient, with the default wait when the service names none.
    assert!(matches!(
        ask(429),
        LyricsError::RateLimited {
            retry_after_secs: 60
        }
    ));
    // 403/401: the service refuses this client; 5xx: the service is failing.
    assert!(matches!(ask(403), LyricsError::Blocked(403)));
    assert!(matches!(ask(401), LyricsError::Blocked(401)));
    assert!(matches!(ask(500), LyricsError::Unavailable(500)));
    assert!(matches!(ask(503), LyricsError::Unavailable(503)));
    // Anything else is an unexpected answer, shown as such and asked again right away.
    let other = ask(418);
    assert!(
        matches!(&other, LyricsError::Parse(m) if m.contains("418")),
        "{other:?}"
    );
    assert_eq!(other.pause_secs(), None);
    // Every message says what happened in words the lyrics panel can show.
    assert!(ask(429).to_string().contains("limiting requests"));
    assert!(ask(403).to_string().contains("refused"));
    assert!(ask(503).to_string().contains("trouble"));
}

#[test]
fn a_retry_after_header_sets_the_wait_within_sane_bounds() {
    use pyxee_lyrics::retry_after_secs;
    assert_eq!(retry_after_secs(Some("45")), 45);
    assert_eq!(retry_after_secs(Some(" 7 ")), 7);
    assert_eq!(
        retry_after_secs(Some("0")),
        1,
        "never zero: that would be a retry storm"
    );
    assert_eq!(
        retry_after_secs(Some("86400")),
        600,
        "capped at ten minutes"
    );
    assert_eq!(retry_after_secs(None), 60);
    assert_eq!(retry_after_secs(Some("Wed, 21 Oct 2026 07:28:00 GMT")), 60);

    // And it is read from the real response.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        if let Ok((mut s, _)) = listener.accept() {
            let mut buf = [0u8; 2048];
            let _ = s.read(&mut buf);
            let _ = s.write_all(
                b"HTTP/1.1 429 Too Many Requests\r\nRetry-After: 17\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
        }
    });
    assert!(matches!(
        fetch(&base, &Query::new("x", "y", None)),
        Err(pyxee_lyrics::LyricsError::RateLimited {
            retry_after_secs: 17
        })
    ));
}

#[test]
fn the_cooldown_stops_asking_a_service_that_refused_and_ignores_harmless_errors() {
    use pyxee_lyrics::{Cooldown, LyricsError};
    let mut c = Cooldown::default();
    assert_eq!(c.remaining_secs(0), None, "a fresh cooldown asks freely");

    // Offline and bad data are not the service's refusal: ask again straight away.
    c.record(&LyricsError::Network, 1_000);
    c.record(&LyricsError::Parse("x".into()), 1_000);
    assert_eq!(c.remaining_secs(1_000), None);

    // A 429 with Retry-After: 30 pauses for 30 s, counting down, rounded up to whole seconds.
    c.record(
        &LyricsError::RateLimited {
            retry_after_secs: 30,
        },
        10_000,
    );
    assert_eq!(c.remaining_secs(10_000), Some(30));
    assert_eq!(c.remaining_secs(39_001), Some(1));
    assert_eq!(
        c.remaining_secs(40_000),
        None,
        "over exactly when the time is up"
    );

    // A shorter pause never shortens a longer one that is still running.
    c.record(
        &LyricsError::RateLimited {
            retry_after_secs: 100,
        },
        50_000,
    );
    c.record(&LyricsError::Unavailable(503), 50_000);
    assert_eq!(c.remaining_secs(50_000), Some(100));
    // A blocked client waits longer than a failing server.
    let mut b = Cooldown::default();
    b.record(&LyricsError::Blocked(403), 0);
    let mut u = Cooldown::default();
    u.record(&LyricsError::Unavailable(500), 0);
    assert!(b.remaining_secs(0) > u.remaining_secs(0));
}
