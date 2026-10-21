//! Songs search straight from YouTube Music: the same request its website makes for the "Songs" tab.
//!
//! Why not yt-dlp for this: yt-dlp's YouTube Music search page lists only a bare title per row (no artist, no
//! length), so Pyxee used to throw that list away and show the ordinary YouTube *video* search instead: lyric
//! videos, live clips and covers under "Songs". This asks YouTube Music itself for its Songs section, whose rows
//! carry the artist, album and length.
//!
//! Nothing here is required: any failure but a rate limit makes the caller fall back to the yt-dlp flow.

use crate::{SearchItem, SourceError, TrackInfo, UNKNOWN_ARTIST};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const ENDPOINT: &str = "https://music.youtube.com/youtubei/v1/search?prettyPrint=false";

/// The website's own filter value for its "Songs" tab.
const SONGS_PARAMS: &str = "EgWKAQIIAWoMEA4QChADEAQQCRAF";
const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:128.0) Gecko/20100101 Firefox/128.0";
const TIMEOUT: Duration = Duration::from_secs(10);

/// What the YouTube Music web client reports as its version: `1.<yesterday as YYYYMMDD>.01.00`. A recent date is
/// what it accepts; a stale one is what gets a client turned away.
fn client_version(now: SystemTime) -> String {
    let days = now
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 86_400);
    let (y, m, d) = civil_from_days(days as i64 - 1);
    format!("1.{y:04}{m:02}{d:02}.01.00")
}

/// Calendar date (UTC) of a day count since 1970-01-01.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

pub fn request_body(query: &str, now: SystemTime) -> String {
    json!({
        "context": {
            "client": {
                "clientName": "WEB_REMIX",
                "clientVersion": client_version(now),
                "hl": "en",
            }
        },
        "query": query,
        "params": SONGS_PARAMS,
    })
    .to_string()
}

/// Asks YouTube Music for the songs matching `query`: at most `limit`, best match first.
pub fn fetch_songs(
    endpoint: &str,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchItem>, SourceError> {
    let agent = ureq::AgentBuilder::new()
        .timeout(TIMEOUT)
        .user_agent(USER_AGENT)
        .build();
    let answer = agent
        .post(endpoint)
        .set("Content-Type", "application/json")
        .set("Origin", "https://music.youtube.com")
        // The consent cookie the website sets once its banner is answered; without it some regions get a page.
        .set("Cookie", "SOCS=CAI")
        .send_string(&request_body(query, SystemTime::now()));
    let body = match answer {
        Ok(r) => r
            .into_string()
            .map_err(|e| SourceError::Failed(e.to_string()))?,
        // Only a real "slow down" is allowed to stop the other ways of searching; a 403 here may be this
        // endpoint alone.
        Err(ureq::Error::Status(429, _)) => return Err(SourceError::RateLimited),
        Err(ureq::Error::Status(c, _)) => {
            return Err(SourceError::Failed(format!(
                "YouTube Music search failed (HTTP {c})"
            )))
        }
        Err(_) => return Err(SourceError::Offline),
    };
    let mut items = parse_songs(&body)?;
    items.truncate(limit);
    Ok(items)
}

/// The song rows of a YouTube Music search answer, in the order it lists them.
pub fn parse_songs(body: &str) -> Result<Vec<SearchItem>, SourceError> {
    let v: Value = serde_json::from_str(body).map_err(|e| SourceError::Parse(e.to_string()))?;
    let mut shelves = Vec::new();
    collect_shelves(&v, &mut shelves);
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for shelf in shelves {
        let Some(rows) = shelf.get("contents").and_then(Value::as_array) else {
            continue;
        };
        for row in rows {
            let Some(song) = row
                .get("musicResponsiveListItemRenderer")
                .and_then(song_from_row)
            else {
                continue;
            };
            if seen.insert(song.key.clone()) {
                out.push(SearchItem::Track(song));
            }
        }
    }
    Ok(out)
}

/// Every `musicShelfRenderer` anywhere in the answer (where it sits among the tabs is not relied on).
fn collect_shelves<'a>(v: &'a Value, out: &mut Vec<&'a Value>) {
    match v {
        Value::Object(map) => {
            for (key, child) in map {
                if key == "musicShelfRenderer" {
                    out.push(child);
                } else {
                    collect_shelves(child, out);
                }
            }
        }
        Value::Array(list) => list.iter().for_each(|c| collect_shelves(c, out)),
        _ => {}
    }
}

fn runs_of(columns: &[Value], index: usize) -> &[Value] {
    columns
        .get(index)
        .and_then(|c| c.pointer("/musicResponsiveListItemFlexColumnRenderer/text/runs"))
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// The text of a row's detail line cut at its " • " separators, each piece with whether any part of it links
/// somewhere (artists and albums do; a type label or a length does not).
fn segments(runs: &[Value]) -> Vec<(String, bool)> {
    let mut out: Vec<(String, bool)> = vec![(String::new(), false)];
    for run in runs {
        let text = run.get("text").and_then(Value::as_str).unwrap_or_default();
        if text.trim() == "•" {
            out.push((String::new(), false));
            continue;
        }
        if let Some(last) = out.last_mut() {
            last.0.push_str(text);
            last.1 |= run.get("navigationEndpoint").is_some();
        }
    }
    out.retain(|(t, _)| !t.trim().is_empty());
    out
}

/// `3:45` or `1:02:03` as milliseconds.
fn clock_ms(text: &str) -> Option<u64> {
    let parts: Vec<u64> = text
        .trim()
        .split(':')
        .map(|p| {
            (p.len() <= 2 && !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
                .then(|| p.parse().ok())
                .flatten()
        })
        .collect::<Option<_>>()?;
    let secs = match parts[..] {
        [m, s] if s < 60 => m * 60 + s,
        [h, m, s] if m < 60 && s < 60 => (h * 60 + m) * 60 + s,
        _ => return None,
    };
    (secs > 0).then_some(secs * 1000)
}

fn song_from_row(row: &Value) -> Option<TrackInfo> {
    let id = row.pointer("/playlistItemData/videoId")?.as_str()?;
    // The id goes into a URL later, so nothing but a plain video id is let through.
    if id.is_empty()
        || id.len() > 64
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return None;
    }
    let columns = row.get("flexColumns")?.as_array()?;
    let title = runs_of(columns, 0)
        .first()?
        .get("text")?
        .as_str()
        .map(str::trim)
        .filter(|t| !t.is_empty())?;
    let mut details = segments(runs_of(columns, 1));
    // Rows listed under "Top result" lead with their type; the Songs tab does not, but this is cheap to allow.
    if details.len() > 1 && details[0].0.trim() == "Song" && !details[0].1 {
        details.remove(0);
    }
    let duration_ms = details.last().and_then(|(t, _)| clock_ms(t)).or_else(|| {
        row.pointer("/fixedColumns/0/musicResponsiveListItemFixedColumnRenderer/text/runs/0/text")
            .and_then(Value::as_str)
            .and_then(clock_ms)
    });
    let artist = details
        .first()
        .map(|(t, _)| t.trim())
        .filter(|t| !t.is_empty() && clock_ms(t).is_none())
        .map_or_else(|| UNKNOWN_ARTIST.to_string(), str::to_string);
    Some(TrackInfo {
        key: format!("yt:{id}"),
        title: title.to_string(),
        artist,
        duration_ms,
        thumbnail: thumbnail_of(row),
    })
}

/// The largest cover offered, asked for at a size that does not look soft in a list.
fn thumbnail_of(row: &Value) -> Option<String> {
    row.pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails")?
        .as_array()?
        .iter()
        .filter_map(|t| {
            let url = t.get("url")?.as_str().filter(|u| !u.is_empty())?;
            Some((url, t.get("width").and_then(Value::as_u64).unwrap_or(0)))
        })
        .max_by_key(|(_, w)| *w)
        .map(|(u, _)| enlarge(&u.replacen("http://", "https://", 1)))
}

/// Google's image hosts take the size from the URL (`...=w60-h60-l90-rj`): ask for a larger square. Anything
/// else is returned unchanged.
fn enlarge(url: &str) -> String {
    const SIZE: u32 = 544;
    let hosted = url.contains("googleusercontent.com") || url.contains("ggpht.com");
    let Some(at) = url.rfind("=w").filter(|_| hosted) else {
        return url.to_string();
    };
    let rest = &url[at + 2..];
    let width_len = rest.bytes().take_while(u8::is_ascii_digit).count();
    let Some(after_width) = rest[width_len..]
        .strip_prefix("-h")
        .filter(|_| width_len > 0)
    else {
        return url.to_string();
    };
    let height_len = after_width.bytes().take_while(u8::is_ascii_digit).count();
    if height_len == 0 {
        return url.to_string();
    }
    format!(
        "{}=w{SIZE}-h{SIZE}{}",
        &url[..at],
        &after_width[height_len..]
    )
}

#[cfg(test)]
pub(crate) mod testserver {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc::{channel, Receiver};

    /// A one-request local HTTP server: answers with `status` and `body`, and reports the request it got
    /// (head and body as one string). Returns the URL to call.
    pub fn once(status: u16, body: &str) -> (String, Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "http://{}/youtubei/v1/search",
            listener.local_addr().unwrap()
        );
        let (tx, rx) = channel();
        let body = body.to_string();
        std::thread::spawn(move || {
            let Ok((mut conn, _)) = listener.accept() else {
                return;
            };
            let mut got = Vec::new();
            let mut buf = [0u8; 4096];
            let (head_end, length) = loop {
                let n = conn.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    break (got.len(), 0);
                }
                got.extend_from_slice(&buf[..n]);
                if let Some(pos) = got.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&got[..pos]).to_lowercase();
                    let length = head
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:"))
                        .and_then(|v| v.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    break (pos + 4, length);
                }
            };
            while got.len() < head_end + length {
                let n = conn.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    break;
                }
                got.extend_from_slice(&buf[..n]);
            }
            let _ = tx.send(String::from_utf8_lossy(&got).to_string());
            let reply = format!(
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = conn.write_all(reply.as_bytes());
        });
        (url, rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_run(t: &str) -> Value {
        json!({ "text": t })
    }

    fn link_run(t: &str) -> Value {
        json!({ "text": t, "navigationEndpoint": { "browseEndpoint": { "browseId": "UC1" } } })
    }

    fn row(id: Option<&str>, title: &str, details: Vec<Value>) -> Value {
        let mut item = json!({
            "flexColumns": [
                { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [ text_run(title) ] } } },
                { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": details } } },
            ],
            "thumbnail": { "musicThumbnailRenderer": { "thumbnail": { "thumbnails": [
                { "url": "https://lh3.googleusercontent.com/abc=w60-h60-l90-rj", "width": 60 },
                { "url": "https://lh3.googleusercontent.com/abc=w120-h120-l90-rj", "width": 120 },
            ] } } },
        });
        if let Some(id) = id {
            item["playlistItemData"] = json!({ "videoId": id });
        }
        json!({ "musicResponsiveListItemRenderer": item })
    }

    fn answer(rows: Vec<Value>) -> String {
        json!({ "contents": { "tabbedSearchResultsRenderer": { "tabs": [ { "tabRenderer": { "content": {
            "sectionListRenderer": { "contents": [ { "musicShelfRenderer": { "contents": rows } } ] }
        } } } ] } } })
        .to_string()
    }

    fn tracks(body: &str) -> Vec<TrackInfo> {
        parse_songs(body)
            .unwrap()
            .into_iter()
            .map(|i| match i {
                SearchItem::Track(t) => t,
                other => panic!("not a track: {other:?}"),
            })
            .collect()
    }

    fn song(id: &str, title: &str, artist: &str, album: &str, length: &str) -> Value {
        row(
            Some(id),
            title,
            vec![
                link_run(artist),
                text_run(" • "),
                link_run(album),
                text_run(" • "),
                text_run(length),
            ],
        )
    }

    #[test]
    fn a_song_row_gives_title_artist_length_and_a_large_cover() {
        let t = tracks(&answer(vec![song(
            "abc12345678",
            "Neon Harbor",
            "Lowtide",
            "Night Shift",
            "3:45",
        )]));
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].key, "yt:abc12345678");
        assert_eq!(t[0].title, "Neon Harbor");
        assert_eq!(t[0].artist, "Lowtide");
        assert_eq!(t[0].duration_ms, Some(225_000));
        assert_eq!(
            t[0].thumbnail.as_deref(),
            Some("https://lh3.googleusercontent.com/abc=w544-h544-l90-rj")
        );
    }

    #[test]
    fn several_artists_are_kept_as_the_website_writes_them() {
        let details = vec![
            link_run("Ana"),
            text_run(", "),
            link_run("Ben"),
            text_run(" & "),
            link_run("Cy"),
            text_run(" • "),
            link_run("Album"),
            text_run(" • "),
            text_run("1:02:03"),
        ];
        let t = tracks(&answer(vec![row(Some("duet0000001"), "Duet", details)]));
        assert_eq!(t[0].artist, "Ana, Ben & Cy");
        assert_eq!(t[0].duration_ms, Some(3_723_000));
    }

    #[test]
    fn a_leading_type_label_is_not_taken_for_the_artist() {
        let details = vec![
            text_run("Song"),
            text_run(" • "),
            link_run("Lowtide"),
            text_run(" • "),
            link_run("Album"),
            text_run(" • "),
            text_run("2:10"),
        ];
        let t = tracks(&answer(vec![row(Some("lab00000001"), "Label", details)]));
        assert_eq!(t[0].artist, "Lowtide");
        assert_eq!(t[0].duration_ms, Some(130_000));
    }

    #[test]
    fn rows_without_a_playable_id_or_title_are_skipped_and_repeats_are_dropped() {
        let t = tracks(&answer(vec![
            song("one00000001", "One", "A", "X", "1:00"),
            row(None, "No id", vec![link_run("A")]),
            song("bad id/..", "Bad id", "A", "X", "1:00"),
            song("one00000001", "One again", "A", "X", "1:00"),
            row(Some("notitle0001"), "  ", vec![link_run("A")]),
            song("two00000002", "Two", "B", "Y", "2:00"),
        ]));
        let keys: Vec<&str> = t.iter().map(|t| t.key.as_str()).collect();
        assert_eq!(keys, ["yt:one00000001", "yt:two00000002"]);
    }

    #[test]
    fn a_row_with_no_artist_or_length_still_lists_with_what_it_has() {
        let t = tracks(&answer(vec![row(Some("bare0000001"), "Bare", vec![])]));
        assert_eq!(t[0].artist, UNKNOWN_ARTIST);
        assert_eq!(t[0].duration_ms, None);
        let t = tracks(&answer(vec![row(
            Some("len00000001"),
            "Only a length",
            vec![text_run("4:00")],
        )]));
        assert_eq!(t[0].artist, UNKNOWN_ARTIST);
        assert_eq!(t[0].duration_ms, Some(240_000));
    }

    #[test]
    fn other_shelves_and_unexpected_answers_do_not_break_parsing() {
        assert!(tracks("{}").is_empty());
        assert!(tracks(r#"{"contents":{"x":[{"musicShelfRenderer":{}}]}}"#).is_empty());
        assert!(matches!(parse_songs("<html>"), Err(SourceError::Parse(_))));
    }

    #[test]
    fn clock_text_is_read_strictly() {
        assert_eq!(clock_ms("0:59"), Some(59_000));
        assert_eq!(clock_ms(" 12:05 "), Some(725_000));
        for bad in ["", "3", "3:7x", "3:60", "1:2:3:4", "0:00", "Album", "1:002"] {
            assert_eq!(clock_ms(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn covers_are_enlarged_only_on_googles_image_hosts_in_the_known_shape() {
        assert_eq!(
            enlarge("https://lh3.googleusercontent.com/x=w226-h226-l90-rj"),
            "https://lh3.googleusercontent.com/x=w544-h544-l90-rj"
        );
        assert_eq!(
            enlarge("https://yt3.ggpht.com/y=w60-h60"),
            "https://yt3.ggpht.com/y=w544-h544"
        );
        for same in [
            "https://i.ytimg.com/vi/abc/hqdefault.jpg?sqp=-oaymw=w60-h60",
            "https://lh3.googleusercontent.com/x=s120",
            "https://lh3.googleusercontent.com/x=w60",
            "https://lh3.googleusercontent.com/x=w-h60",
            "https://lh3.googleusercontent.com/x=w60-h",
        ] {
            assert_eq!(enlarge(same), same);
        }
    }

    #[test]
    fn the_client_version_is_yesterdays_date() {
        // 2026-10-21 15:30 UTC
        let now = UNIX_EPOCH + Duration::from_secs(1_792_596_600);
        assert_eq!(client_version(now), "1.20261020.01.00");
        // The first day of a year and a leap day.
        let jan1 = UNIX_EPOCH + Duration::from_secs(1_767_225_600 + 3600); // 2026-01-01 01:00
        assert_eq!(client_version(jan1), "1.20251231.01.00");
        let mar1 = UNIX_EPOCH + Duration::from_secs(1_709_251_200 + 3600); // 2024-03-01 01:00
        assert_eq!(client_version(mar1), "1.20240229.01.00");
        assert_eq!(client_version(UNIX_EPOCH), "1.19691231.01.00");
    }

    #[test]
    fn the_request_asks_for_the_songs_tab_with_the_query() {
        let body: Value = serde_json::from_str(&request_body("low tide", UNIX_EPOCH)).unwrap();
        assert_eq!(body["query"], "low tide");
        assert_eq!(body["params"], SONGS_PARAMS);
        assert_eq!(body["context"]["client"]["clientName"], "WEB_REMIX");
    }

    #[test]
    fn fetching_posts_the_query_and_returns_the_songs_up_to_the_limit() {
        let rows = (0..5)
            .map(|i| {
                song(
                    &format!("id{i:09}"),
                    &format!("Song {i}"),
                    "Lowtide",
                    "LP",
                    "3:00",
                )
            })
            .collect();
        let (url, request) = testserver::once(200, &answer(rows));
        let items = fetch_songs(&url, "low tide", 3).unwrap();
        assert_eq!(items.len(), 3);
        let sent = request.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(sent.starts_with("POST /youtubei/v1/search"), "{sent}");
        assert!(
            sent.contains(SONGS_PARAMS) && sent.contains("low tide"),
            "{sent}"
        );
        assert!(sent.to_lowercase().contains("socs=cai"), "{sent}");
    }

    #[test]
    fn http_failures_are_told_apart() {
        let (url, _) = testserver::once(429, "{}");
        assert_eq!(fetch_songs(&url, "x", 5), Err(SourceError::RateLimited));
        let (url, _) = testserver::once(403, "{}");
        assert!(matches!(
            fetch_songs(&url, "x", 5),
            Err(SourceError::Failed(_))
        ));
        let (url, _) = testserver::once(200, "<html>");
        assert!(matches!(
            fetch_songs(&url, "x", 5),
            Err(SourceError::Parse(_))
        ));
        // Nothing is listening on a port that was just released.
        let gone = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            format!("http://{}/", l.local_addr().unwrap())
        };
        assert_eq!(fetch_songs(&gone, "x", 5), Err(SourceError::Offline));
    }
}
