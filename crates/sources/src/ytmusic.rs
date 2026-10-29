//! Songs search straight from YouTube Music: the same request its website makes for the "Songs" tab.
//!
//! Why not yt-dlp for this: yt-dlp's YouTube Music search page lists only a bare title per row (no artist, no
//! length), so Pyxee used to throw that list away and show the ordinary YouTube *video* search instead: lyric
//! videos, live clips and covers under "Songs". This asks YouTube Music itself for its Songs section, whose rows
//! carry the artist, album and length.
//!
//! Nothing here is required: any failure but a rate limit makes the caller fall back to the yt-dlp flow.
//!
//! The same website request, pointed at its `browse` endpoint, also gives the Explore page: trending songs, new
//! albums and singles, new music videos. The shapes read here follow what the website sends (and what the
//! open-source `ytmusicapi` reads from it). Every piece is optional, so a shelf the website reshapes is left out
//! rather than failing the page.

use crate::{
    AlbumInfo, ExploreShelf, GenreInfo, PlaylistInfo, SearchItem, SourceError, TrackInfo,
    UNKNOWN_ARTIST,
};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const ENDPOINT: &str = "https://music.youtube.com/youtubei/v1/search?prettyPrint=false";
/// The setting (environment variable or `.env` line) that turns the direct requests of this module off.
pub const ENV_VAR: &str = "PYXEE_YTMUSIC_DIRECT";

/// Reads the on/off switch: `1`, `on`, `true` or `yes` is on, `0`, `off`, `false` or `no` is off (any case).
pub fn parse_switch(value: &str) -> Result<bool, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "on" | "true" | "yes" => Ok(true),
        "0" | "off" | "false" | "no" => Ok(false),
        other => Err(format!("use 0 or 1 (or off or on), not \"{other}\"")),
    }
}

/// Whether Pyxee may make the direct requests of this module (songs search and Explore). On unless the setting
/// says off; an empty or unreadable value keeps the default, which is on.
pub fn direct_requests_allowed(setting: Option<&str>) -> bool {
    match setting.map(str::trim).filter(|v| !v.is_empty()) {
        None => true,
        Some(v) => parse_switch(v).unwrap_or_else(|e| {
            log::warn!("{ENV_VAR}: {e}; leaving the direct YouTube Music requests on");
            true
        }),
    }
}

/// [`direct_requests_allowed`] for the environment.
pub fn direct_requests_allowed_from_env() -> bool {
    direct_requests_allowed(std::env::var(ENV_VAR).ok().as_deref())
}

/// The website's request for a page (Explore here).
pub const BROWSE_ENDPOINT: &str = "https://music.youtube.com/youtubei/v1/browse?prettyPrint=false";
/// The page behind the website's Explore tab.
const EXPLORE_PAGE: &str = "FEmusic_explore";
/// The page behind the website's "Moods & genres" button, and the page of one of its genres (asked for with the
/// `params` token from the first).
const GENRES_PAGE: &str = "FEmusic_moods_and_genres";
const GENRE_PAGE: &str = "FEmusic_moods_and_genres_category";
/// Shelves longer than this are cut: a row on a page, not a catalogue.
const SHELF_MAX: usize = 20;
/// A playlist's first page holds about this many songs; that is what is played.
const PLAYLIST_MAX: usize = 100;

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

fn context(now: SystemTime) -> Value {
    json!({
        "client": {
            "clientName": "WEB_REMIX",
            "clientVersion": client_version(now),
            "hl": "en",
        }
    })
}

pub fn request_body(query: &str, now: SystemTime) -> String {
    json!({
        "context": context(now),
        "query": query,
        "params": SONGS_PARAMS,
    })
    .to_string()
}

pub fn explore_body(now: SystemTime) -> String {
    json!({
        "context": context(now),
        "browseId": EXPLORE_PAGE,
    })
    .to_string()
}

pub fn genres_body(now: SystemTime) -> String {
    json!({
        "context": context(now),
        "browseId": GENRES_PAGE,
    })
    .to_string()
}

pub fn genre_body(params: &str, now: SystemTime) -> String {
    json!({
        "context": context(now),
        "browseId": GENRE_PAGE,
        "params": params,
    })
    .to_string()
}

pub fn playlist_body(id: &str, now: SystemTime) -> String {
    json!({
        "context": context(now),
        "browseId": format!("VL{id}"),
    })
    .to_string()
}

/// One POST to the website's own API, answered with the body text. `what` names the request in an error.
fn post(endpoint: &str, body: &str, what: &str) -> Result<String, SourceError> {
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
        .send_string(body);
    match answer {
        Ok(r) => r
            .into_string()
            .map_err(|e| SourceError::Failed(e.to_string())),
        // Only a real "slow down" is allowed to stop the other ways of searching; a 403 here may be this
        // endpoint alone.
        Err(ureq::Error::Status(429, _)) => Err(SourceError::RateLimited),
        Err(ureq::Error::Status(c, _)) => Err(SourceError::Failed(format!(
            "YouTube Music {what} failed (HTTP {c})"
        ))),
        Err(_) => Err(SourceError::Offline),
    }
}

/// Asks YouTube Music for the songs matching `query`: at most `limit`, best match first.
pub fn fetch_songs(
    endpoint: &str,
    query: &str,
    limit: usize,
) -> Result<Vec<SearchItem>, SourceError> {
    let body = post(endpoint, &request_body(query, SystemTime::now()), "search")?;
    let mut items = parse_songs(&body)?;
    items.truncate(limit);
    Ok(items)
}

/// Asks YouTube Music for its Explore page.
pub fn fetch_explore(endpoint: &str) -> Result<Vec<ExploreShelf>, SourceError> {
    let body = post(endpoint, &explore_body(SystemTime::now()), "explore")?;
    parse_explore(&body)
}

/// Asks YouTube Music for its list of genres and moods.
pub fn fetch_genres(endpoint: &str) -> Result<Vec<GenreInfo>, SourceError> {
    let body = post(endpoint, &genres_body(SystemTime::now()), "genres")?;
    parse_genres(&body)
}

/// Asks YouTube Music for the page of one genre (`params` as `parse_genres` found it).
pub fn fetch_genre(endpoint: &str, params: &str) -> Result<Vec<ExploreShelf>, SourceError> {
    if !is_token(params) {
        return Err(SourceError::Failed("that is not a genre token".into()));
    }
    let body = post(endpoint, &genre_body(params, SystemTime::now()), "genre")?;
    parse_genre(&body)
}

/// Asks YouTube Music for the songs of a playlist.
pub fn fetch_playlist(endpoint: &str, id: &str) -> Result<Vec<TrackInfo>, SourceError> {
    let id = plain_video_id(id)
        .ok_or_else(|| SourceError::Failed("that is not a playlist id".into()))?;
    let body = post(endpoint, &playlist_body(id, SystemTime::now()), "playlist")?;
    parse_playlist(&body)
}

/// The song rows of a YouTube Music search answer, in the order it lists them.
pub fn parse_songs(body: &str) -> Result<Vec<SearchItem>, SourceError> {
    let v: Value = serde_json::from_str(body).map_err(|e| SourceError::Parse(e.to_string()))?;
    let mut shelves = Vec::new();
    collect_keys(&v, &["musicShelfRenderer"], &mut shelves);
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

/// Every value stored under one of `keys` anywhere in the answer (where it sits among the tabs is not relied on),
/// in the order of the answer's lists. What is found is not searched further.
fn collect_keys<'a>(v: &'a Value, keys: &[&str], out: &mut Vec<&'a Value>) {
    match v {
        Value::Object(map) => {
            for (key, child) in map {
                if keys.contains(&key.as_str()) {
                    out.push(child);
                } else {
                    collect_keys(child, keys, out);
                }
            }
        }
        Value::Array(list) => list.iter().for_each(|c| collect_keys(c, keys, out)),
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

// ------------------------------------------------------------------ Explore

const EXPLORE_SECTIONS: &str = "/contents/singleColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents";

/// The shelves of the Explore page, top to bottom, under the website's own titles.
///
/// A shelf is recognised by what it holds, not by where it sits or what it is called: rows that play a song (Trending,
/// Top songs) and cards that play a music video make a shelf of songs; cards that open an album or single make a shelf
/// of albums. Anything else (the Moods and genres buttons, podcast episodes) is not shown, and neither is a shelf with
/// nothing usable in it.
pub fn parse_explore(body: &str) -> Result<Vec<ExploreShelf>, SourceError> {
    let v: Value = serde_json::from_str(body).map_err(|e| SourceError::Parse(e.to_string()))?;
    let sections = v
        .pointer(EXPLORE_SECTIONS)
        .and_then(Value::as_array)
        .ok_or_else(|| SourceError::Parse("the Explore page has an unexpected layout".into()))?;
    let mut out = Vec::new();
    for (i, section) in sections.iter().enumerate() {
        let Some(shelf) = section.get("musicCarouselShelfRenderer") else {
            continue;
        };
        let Some(title) = shelf
            .pointer("/header/musicCarouselShelfBasicHeaderRenderer/title/runs/0/text")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|t| !t.is_empty())
        else {
            continue;
        };
        let Some(items) = shelf.get("contents").and_then(Value::as_array) else {
            continue;
        };
        let mut tracks: Vec<TrackInfo> = Vec::new();
        let mut albums: Vec<AlbumInfo> = Vec::new();
        for item in items {
            let track = if let Some(row) = item.get("musicResponsiveListItemRenderer") {
                song_from_row(row)
            } else if let Some(card) = item.get("musicTwoRowItemRenderer") {
                if let Some(video) = video_from_card(card) {
                    Some(video)
                } else {
                    albums.extend(
                        album_from_card(card).filter(|a| !albums.iter().any(|x| x.id == a.id)),
                    );
                    None
                }
            } else {
                None
            };
            if let Some(t) = track {
                if !tracks.iter().any(|x| x.key == t.key) {
                    tracks.push(t);
                }
            }
        }
        let id = format!("explore-{i}");
        let title = title.to_string();
        if !tracks.is_empty() {
            tracks.truncate(SHELF_MAX);
            out.push(ExploreShelf::Songs { id, title, tracks });
        } else if !albums.is_empty() {
            albums.truncate(SHELF_MAX);
            out.push(ExploreShelf::Albums { id, title, albums });
        }
    }
    Ok(out)
}

// ------------------------------------------------------------------ genres

/// The genres and moods on the "Moods & genres" page, in the order the page lists them, each once.
///
/// Every button on the page that opens a genre page counts, wherever it sits: the page's first rows are a
/// personalised pick of the same buttons, and a repeat is dropped.
pub fn parse_genres(body: &str) -> Result<Vec<GenreInfo>, SourceError> {
    let v: Value = serde_json::from_str(body).map_err(|e| SourceError::Parse(e.to_string()))?;
    let mut buttons = Vec::new();
    collect_keys(&v, &["musicNavigationButtonRenderer"], &mut buttons);
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for button in buttons {
        let endpoint = button.pointer("/clickCommand/browseEndpoint");
        let is_genre = endpoint
            .and_then(|e| e.get("browseId"))
            .and_then(Value::as_str)
            == Some(GENRE_PAGE);
        let params = endpoint
            .and_then(|e| e.get("params"))
            .and_then(Value::as_str)
            .filter(|p| is_token(p));
        let title = button
            .pointer("/buttonText/runs/0/text")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|t| !t.is_empty());
        let (true, Some(params), Some(title)) = (is_genre, params, title) else {
            continue;
        };
        if seen.insert(params.to_string()) {
            out.push(GenreInfo {
                title: title.to_string(),
                params: params.to_string(),
            });
        }
    }
    if out.is_empty() {
        return Err(SourceError::Parse(
            "the genres page has an unexpected layout".into(),
        ));
    }
    Ok(out)
}

/// A `params` token as the website hands it out (base64 text). It only goes back into a request body, but nothing
/// odd is let through.
fn is_token(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 512
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'=' | b'%'))
}

/// The shelves of one genre's page, top to bottom: shelves of playlists (YouTube Music's own picks) and of songs.
///
/// Album shelves ("New albums", "New releases") are left out on purpose: they are a feed of whatever distributors
/// uploaded this week, which is where bulk-made releases pile up.
pub fn parse_genre(body: &str) -> Result<Vec<ExploreShelf>, SourceError> {
    let v: Value = serde_json::from_str(body).map_err(|e| SourceError::Parse(e.to_string()))?;
    let sections = v
        .pointer(EXPLORE_SECTIONS)
        .and_then(Value::as_array)
        .ok_or_else(|| SourceError::Parse("the genre page has an unexpected layout".into()))?;
    let mut out = Vec::new();
    for (i, section) in sections.iter().enumerate() {
        let (title, items) = if let Some(shelf) = section.get("musicCarouselShelfRenderer") {
            (
                shelf.pointer("/header/musicCarouselShelfBasicHeaderRenderer/title/runs/0/text"),
                shelf.get("contents"),
            )
        } else if let Some(grid) = section.get("gridRenderer") {
            (
                grid.pointer("/header/gridHeaderRenderer/title/runs/0/text"),
                grid.get("items"),
            )
        } else {
            continue;
        };
        let (Some(title), Some(items)) = (
            title
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|t| !t.is_empty()),
            items.and_then(Value::as_array),
        ) else {
            continue;
        };
        let mut playlists: Vec<PlaylistInfo> = Vec::new();
        let mut tracks: Vec<TrackInfo> = Vec::new();
        for item in items {
            if let Some(card) = item.get("musicTwoRowItemRenderer") {
                if let Some(p) = playlist_from_card(card) {
                    if !playlists.iter().any(|x| x.id == p.id) {
                        playlists.push(p);
                    }
                } else if let Some(t) = video_from_card(card) {
                    if !tracks.iter().any(|x| x.key == t.key) {
                        tracks.push(t);
                    }
                }
            } else if let Some(t) = item
                .get("musicResponsiveListItemRenderer")
                .and_then(song_from_row)
            {
                if !tracks.iter().any(|x| x.key == t.key) {
                    tracks.push(t);
                }
            }
        }
        let id = format!("genre-{i}");
        let title = title.to_string();
        if !playlists.is_empty() {
            playlists.truncate(SHELF_MAX);
            out.push(ExploreShelf::Playlists {
                id,
                title,
                playlists,
            });
        } else if !tracks.is_empty() {
            tracks.truncate(SHELF_MAX);
            out.push(ExploreShelf::Songs { id, title, tracks });
        }
    }
    Ok(out)
}

/// A playlist card: its title links to `VL<playlist id>`, its subtitle reads `Playlist • YouTube Music` or similar.
fn playlist_from_card(card: &Value) -> Option<PlaylistInfo> {
    let id = card
        .pointer("/title/runs/0/navigationEndpoint/browseEndpoint/browseId")
        .and_then(Value::as_str)
        .and_then(|b| b.strip_prefix("VL"))
        .and_then(plain_video_id)?;
    let title = card_title(card)?;
    // The type label ("Playlist") says nothing the cover does not; what is left is who made it or how long it is.
    let subtitle = segments(runs_in(card, "/subtitle/runs"))
        .into_iter()
        .map(|(t, _)| t.trim().to_string())
        .filter(|t| t != "Playlist")
        .collect::<Vec<_>>()
        .join(" • ");
    Some(PlaylistInfo {
        id: id.to_string(),
        title: title.to_string(),
        subtitle,
        thumbnail: best_cover(
            card.pointer("/thumbnailRenderer/musicThumbnailRenderer/thumbnail/thumbnails"),
        ),
    })
}

/// The songs of a playlist page, in order, each once: the song list sits in a `musicShelfRenderer` (or an older
/// `musicPlaylistShelfRenderer`) whichever way the page is laid out. Rows that cannot be played (removed or
/// unavailable songs carry no video id) are skipped.
pub fn parse_playlist(body: &str) -> Result<Vec<TrackInfo>, SourceError> {
    let v: Value = serde_json::from_str(body).map_err(|e| SourceError::Parse(e.to_string()))?;
    let mut shelves = Vec::new();
    collect_keys(
        &v,
        &["musicPlaylistShelfRenderer", "musicShelfRenderer"],
        &mut shelves,
    );
    if shelves.is_empty() {
        return Err(SourceError::Parse(
            "the playlist page has an unexpected layout".into(),
        ));
    }
    let mut out: Vec<TrackInfo> = Vec::new();
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
            if !out.iter().any(|x| x.key == song.key) {
                out.push(song);
            }
        }
    }
    out.truncate(PLAYLIST_MAX);
    Ok(out)
}

fn runs_in<'a>(card: &'a Value, pointer: &str) -> &'a [Value] {
    card.pointer(pointer)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn card_title(card: &Value) -> Option<&str> {
    card.pointer("/title/runs/0/text")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

/// A music video card: its title links to the video, its subtitle reads `Artist • 46M views`.
fn video_from_card(card: &Value) -> Option<TrackInfo> {
    let id = card
        .pointer("/title/runs/0/navigationEndpoint/watchEndpoint/videoId")
        .and_then(Value::as_str)
        .and_then(plain_video_id)?;
    let title = card_title(card)?;
    let details = segments(runs_in(card, "/subtitle/runs"));
    let artist = details
        .iter()
        .find(|(_, linked)| *linked)
        .or_else(|| {
            details
                .iter()
                .find(|(t, _)| !t.trim_end().ends_with("views") && !t.trim_end().ends_with("view"))
        })
        .map_or_else(|| UNKNOWN_ARTIST.to_string(), |(t, _)| t.trim().to_string());
    Some(TrackInfo {
        key: format!("yt:{id}"),
        title: title.to_string(),
        artist,
        duration_ms: None,
        thumbnail: best_cover(
            card.pointer("/thumbnailRenderer/musicThumbnailRenderer/thumbnail/thumbnails"),
        ),
    })
}

/// An album or single card: its title links to the release, its subtitle reads `Single • Artist` or `Album • Artist`.
fn album_from_card(card: &Value) -> Option<AlbumInfo> {
    let id = card
        .pointer("/title/runs/0/navigationEndpoint/browseEndpoint/browseId")
        .and_then(Value::as_str)
        .filter(|id| id.starts_with("MPRE") && id.len() <= 64)?;
    let title = card_title(card)?;
    let details = segments(runs_in(card, "/subtitle/runs"));
    let artist = details
        .iter()
        .find(|(_, linked)| *linked)
        .or_else(|| details.get(1))
        .map_or_else(|| UNKNOWN_ARTIST.to_string(), |(t, _)| t.trim().to_string());
    Some(AlbumInfo {
        id: id.to_string(),
        title: title.to_string(),
        artist,
        thumbnail: best_cover(
            card.pointer("/thumbnailRenderer/musicThumbnailRenderer/thumbnail/thumbnails"),
        ),
    })
}

/// A plain video id: it goes into a URL later, so nothing else is let through.
fn plain_video_id(id: &str) -> Option<&str> {
    (!id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'))
    .then_some(id)
}

/// Where a list row keeps the song it plays. Search rows say it in `playlistItemData`; rows on a page (Explore) link
/// their title to the song or carry it on the cover's play button.
fn row_video_id(row: &Value) -> Option<&str> {
    [
        "/playlistItemData/videoId",
        "/flexColumns/0/musicResponsiveListItemFlexColumnRenderer/text/runs/0/navigationEndpoint/watchEndpoint/videoId",
        "/overlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/playNavigationEndpoint/watchEndpoint/videoId",
    ]
    .iter()
    .find_map(|p| row.pointer(p).and_then(Value::as_str))
    .and_then(plain_video_id)
}

const PODCAST_EPISODE: &str = "MUSIC_VIDEO_TYPE_PODCAST_EPISODE";

/// Podcast episodes sit among the songs on some pages; they are not music.
fn is_podcast_episode(row: &Value) -> bool {
    row.pointer("/overlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/playNavigationEndpoint/watchEndpoint/watchEndpointMusicSupportedConfigs/watchEndpointMusicConfig/musicVideoType")
        .and_then(Value::as_str)
        == Some(PODCAST_EPISODE)
}

fn song_from_row(row: &Value) -> Option<TrackInfo> {
    let id = row_video_id(row)?;
    if is_podcast_episode(row) {
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
    best_cover(row.pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails"))
}

fn best_cover(thumbnails: Option<&Value>) -> Option<String> {
    thumbnails?
        .as_array()?
        .iter()
        .filter_map(|t| {
            let url = t.get("url")?.as_str().filter(|u| !u.is_empty())?;
            Some((url, t.get("width").and_then(Value::as_u64).unwrap_or(0)))
        })
        .max_by_key(|(_, w)| *w)
        .map(|(u, _)| enlarge(&u.replacen("http://", "https://", 1)))
}

/// The host of an `http(s)` address, lower-cased, without user info or port.
fn host_of(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit('@').next()?;
    let host = host.split(':').next()?;
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// Google's image hosts take the size from the URL (`...=w60-h60-l90-rj`): ask for a larger square. Anything
/// else is returned unchanged.
fn enlarge(url: &str) -> String {
    const SIZE: u32 = 544;
    let hosted = host_of(url).is_some_and(|h| {
        ["googleusercontent.com", "ggpht.com"]
            .iter()
            .any(|d| h == *d || h.strip_suffix(d).is_some_and(|p| p.ends_with('.')))
    });
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

    // ------------------------------------------------------------- Explore

    fn carousel(title: &str, items: Vec<Value>) -> Value {
        json!({ "musicCarouselShelfRenderer": {
            "header": { "musicCarouselShelfBasicHeaderRenderer": { "title": { "runs": [
                { "text": title, "navigationEndpoint": { "browseEndpoint": { "browseId": "VLOLAK5uy_x" } } }
            ] } } },
            "contents": items
        } })
    }

    fn explore_answer(sections: Vec<Value>) -> String {
        json!({ "contents": { "singleColumnBrowseResultsRenderer": { "tabs": [ { "tabRenderer": { "content": {
            "sectionListRenderer": { "contents": sections }
        } } } ] } } })
        .to_string()
    }

    /// A row as the Explore page lists it: no `playlistItemData`, the title links to the song.
    fn page_row(id: &str, title: &str, artist: &str, views: &str) -> Value {
        json!({ "musicResponsiveListItemRenderer": {
            "flexColumns": [
                { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [
                    { "text": title, "navigationEndpoint": { "watchEndpoint": { "videoId": id } } }
                ] } } },
                { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [
                    link_run(artist), text_run(" • "), text_run(views)
                ] } } },
            ],
            "thumbnail": { "musicThumbnailRenderer": { "thumbnail": { "thumbnails": [
                { "url": "https://lh3.googleusercontent.com/t=w120-h120-l90-rj", "width": 120 },
            ] } } },
        } })
    }

    fn album_card(browse_id: &str, title: &str, subtitle: Vec<Value>) -> Value {
        json!({ "musicTwoRowItemRenderer": {
            "title": { "runs": [
                { "text": title, "navigationEndpoint": { "browseEndpoint": { "browseId": browse_id } } }
            ] },
            "subtitle": { "runs": subtitle },
            "thumbnailRenderer": { "musicThumbnailRenderer": { "thumbnail": { "thumbnails": [
                { "url": "https://lh3.googleusercontent.com/a=w226-h226-l90-rj", "width": 226 },
            ] } } },
        } })
    }

    fn video_card(id: &str, title: &str, subtitle: Vec<Value>) -> Value {
        json!({ "musicTwoRowItemRenderer": {
            "title": { "runs": [
                { "text": title, "navigationEndpoint": { "watchEndpoint": { "videoId": id } } }
            ] },
            "subtitle": { "runs": subtitle },
            "thumbnailRenderer": { "musicThumbnailRenderer": { "thumbnail": { "thumbnails": [
                { "url": "https://i.ytimg.com/vi/abc/hq720.jpg", "width": 480 },
            ] } } },
        } })
    }

    #[test]
    fn explore_gives_songs_albums_and_videos_shelves_under_the_websites_titles() {
        let body = explore_answer(vec![
            carousel(
                "New albums & singles",
                vec![
                    album_card(
                        "MPREb_one",
                        "Night Shift",
                        vec![text_run("Single"), text_run(" • "), link_run("Lowtide")],
                    ),
                    album_card(
                        "MPREb_one",
                        "Night Shift (again)",
                        vec![text_run("Single"), text_run(" • "), link_run("Lowtide")],
                    ),
                    album_card(
                        "MPREb_two",
                        "Tidal",
                        vec![
                            text_run("Album"),
                            text_run(" • "),
                            link_run("Ana"),
                            text_run(" & "),
                            link_run("Ben"),
                        ],
                    ),
                ],
            ),
            carousel(
                "Trending",
                vec![
                    page_row("trend000001", "Neon Harbor", "Lowtide", "108M views"),
                    page_row("trend000002", "Slow Orbit", "Kite", "2.1M views"),
                ],
            ),
            carousel(
                "New music videos",
                vec![video_card(
                    "video000001",
                    "Paper Planes (Official Video)",
                    vec![link_run("Kite"), text_run(" • "), text_run("46M views")],
                )],
            ),
        ]);
        let shelves = parse_explore(&body).unwrap();
        assert_eq!(shelves.len(), 3);
        match &shelves[0] {
            ExploreShelf::Albums { title, albums, .. } => {
                assert_eq!(title, "New albums & singles");
                assert_eq!(albums.len(), 2);
                assert_eq!(albums[0].title, "Night Shift");
                assert_eq!(albums[0].artist, "Lowtide");
                assert_eq!(albums[0].id, "MPREb_one");
                assert_eq!(
                    albums[0].thumbnail.as_deref(),
                    Some("https://lh3.googleusercontent.com/a=w544-h544-l90-rj")
                );
                assert_eq!(albums[1].artist, "Ana & Ben");
            }
            other => panic!("not an albums shelf: {other:?}"),
        }
        match &shelves[1] {
            ExploreShelf::Songs { title, tracks, .. } => {
                assert_eq!(title, "Trending");
                assert_eq!(tracks[0].key, "yt:trend000001");
                assert_eq!(tracks[0].title, "Neon Harbor");
                assert_eq!(tracks[0].artist, "Lowtide");
                assert_eq!(tracks[0].duration_ms, None);
                assert!(tracks[0]
                    .thumbnail
                    .as_deref()
                    .unwrap()
                    .contains("w544-h544"));
            }
            other => panic!("not a songs shelf: {other:?}"),
        }
        match &shelves[2] {
            ExploreShelf::Songs { title, tracks, .. } => {
                assert_eq!(title, "New music videos");
                assert_eq!(tracks[0].key, "yt:video000001");
                assert_eq!(tracks[0].artist, "Kite", "the views are not the artist");
            }
            other => panic!("not a songs shelf: {other:?}"),
        }
        let ids: HashSet<String> = shelves
            .iter()
            .map(|s| match s {
                ExploreShelf::Songs { id, .. }
                | ExploreShelf::Albums { id, .. }
                | ExploreShelf::Playlists { id, .. } => id.clone(),
            })
            .collect();
        assert_eq!(ids.len(), 3, "every shelf has its own id");
    }

    #[test]
    fn explore_leaves_out_moods_podcasts_and_shelves_with_nothing_usable() {
        let podcast = json!({ "musicResponsiveListItemRenderer": {
            "flexColumns": [
                { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [
                    { "text": "Episode 1", "navigationEndpoint": { "watchEndpoint": { "videoId": "pod00000001" } } }
                ] } } },
            ],
            "overlay": { "musicItemThumbnailOverlayRenderer": { "content": { "musicPlayButtonRenderer": {
                "playNavigationEndpoint": { "watchEndpoint": { "videoId": "pod00000001", "watchEndpointMusicSupportedConfigs": {
                    "watchEndpointMusicConfig": { "musicVideoType": "MUSIC_VIDEO_TYPE_PODCAST_EPISODE" }
                } } }
            } } } },
        } });
        let body = explore_answer(vec![
            carousel(
                "Moods & genres",
                vec![
                    json!({ "musicNavigationButtonRenderer": { "buttonText": { "runs": [ { "text": "Chill" } ] } } }),
                ],
            ),
            carousel(
                "Top episodes",
                vec![
                    json!({ "musicMultiRowListItemRenderer": { "title": { "runs": [ { "text": "Ep" } ] } } }),
                ],
            ),
            carousel("Mixed", vec![podcast]),
            carousel("Empty", vec![]),
            json!({ "musicShelfRenderer": { "contents": [] } }),
            json!({ "musicCarouselShelfRenderer": { "contents": [ page_row("noheader0001", "No title", "A", "1 views") ] } }),
            carousel(
                "Bad ids",
                vec![
                    page_row("bad id/..", "Bad", "A", "1 views"),
                    album_card("UCnotanalbum", "Not an album", vec![link_run("A")]),
                ],
            ),
        ]);
        assert!(parse_explore(&body).unwrap().is_empty());
    }

    #[test]
    fn explore_repeats_are_dropped_and_long_shelves_are_cut() {
        let rows: Vec<Value> = (0..30)
            .map(|i| page_row(&format!("id{i:09}"), &format!("Song {i}"), "A", "1 views"))
            .chain([page_row("id000000000", "Song 0 again", "A", "1 views")])
            .collect();
        let shelves = parse_explore(&explore_answer(vec![carousel("Trending", rows)])).unwrap();
        let ExploreShelf::Songs { tracks, .. } = &shelves[0] else {
            panic!("not songs")
        };
        assert_eq!(tracks.len(), SHELF_MAX);
        assert_eq!(tracks[0].title, "Song 0");
    }

    #[test]
    fn an_explore_answer_in_an_unexpected_shape_is_a_parse_error_but_an_empty_page_is_not() {
        assert!(matches!(
            parse_explore("<html>"),
            Err(SourceError::Parse(_))
        ));
        assert!(matches!(parse_explore("{}"), Err(SourceError::Parse(_))));
        assert_eq!(parse_explore(&explore_answer(vec![])).unwrap(), vec![]);
    }

    #[test]
    fn the_explore_request_asks_for_the_explore_page() {
        let body: Value = serde_json::from_str(&explore_body(UNIX_EPOCH)).unwrap();
        assert_eq!(body["browseId"], "FEmusic_explore");
        assert_eq!(body["context"]["client"]["clientName"], "WEB_REMIX");
        assert!(body.get("query").is_none());
    }

    #[test]
    fn fetching_explore_posts_the_page_and_tells_http_failures_apart() {
        let answer = explore_answer(vec![carousel(
            "Trending",
            vec![page_row(
                "trend000001",
                "Neon Harbor",
                "Lowtide",
                "1M views",
            )],
        )]);
        let (url, request) = testserver::once(200, &answer);
        let shelves = fetch_explore(&url).unwrap();
        assert_eq!(shelves.len(), 1);
        let sent = request.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(sent.starts_with("POST "), "{sent}");
        assert!(sent.contains("FEmusic_explore"), "{sent}");
        assert!(sent.to_lowercase().contains("socs=cai"), "{sent}");

        let (url, _) = testserver::once(429, "{}");
        assert_eq!(fetch_explore(&url), Err(SourceError::RateLimited));
        let (url, _) = testserver::once(500, "{}");
        assert!(matches!(fetch_explore(&url), Err(SourceError::Failed(_))));
        let (url, _) = testserver::once(200, "<html>");
        assert!(matches!(fetch_explore(&url), Err(SourceError::Parse(_))));
    }

    #[test]
    fn a_song_on_a_page_is_found_by_its_title_link_or_its_play_button() {
        let by_button = json!({ "musicResponsiveListItemRenderer": {
            "flexColumns": [
                { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [ text_run("Button") ] } } },
                { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [ link_run("A") ] } } },
            ],
            "overlay": { "musicItemThumbnailOverlayRenderer": { "content": { "musicPlayButtonRenderer": {
                "playNavigationEndpoint": { "watchEndpoint": { "videoId": "button00001" } }
            } } } },
        } });
        let body = explore_answer(vec![carousel(
            "Trending",
            vec![by_button, page_row("link0000001", "Link", "B", "1 views")],
        )]);
        let shelves = parse_explore(&body).unwrap();
        let ExploreShelf::Songs { tracks, .. } = &shelves[0] else {
            panic!("not songs")
        };
        let keys: Vec<&str> = tracks.iter().map(|t| t.key.as_str()).collect();
        assert_eq!(keys, ["yt:button00001", "yt:link0000001"]);
    }

    #[test]
    fn the_direct_requests_switch_is_on_unless_it_says_off() {
        for on in [
            None,
            Some(""),
            Some("  "),
            Some("1"),
            Some("ON"),
            Some("true"),
            Some("yes"),
            Some("maybe"),
        ] {
            assert!(direct_requests_allowed(on), "{on:?}");
        }
        for off in ["0", "off", "OFF", "false", " No "] {
            assert!(!direct_requests_allowed(Some(off)), "{off}");
        }
        assert!(parse_switch("maybe").is_err());
    }

    // ------------------------------------------------------------- Genres and playlists

    fn genre_button(title: &str, params: &str) -> Value {
        json!({ "musicNavigationButtonRenderer": {
            "buttonText": { "runs": [ text_run(title) ] },
            "clickCommand": { "browseEndpoint": { "browseId": GENRE_PAGE, "params": params } },
        } })
    }

    fn grid(items: Vec<Value>) -> Value {
        json!({ "gridRenderer": { "items": items } })
    }

    #[test]
    fn genres_are_the_buttons_that_open_a_genre_page_each_once_in_the_pages_order() {
        let other = json!({ "musicNavigationButtonRenderer": {
            "buttonText": { "runs": [ text_run("Charts") ] },
            "clickCommand": { "browseEndpoint": { "browseId": "FEmusic_charts", "params": "xyz" } },
        } });
        let body = explore_answer(vec![
            // The personalised pick at the top repeats buttons that come again below.
            grid(vec![genre_button("K-Pop", "ggMPkpop")]),
            grid(vec![genre_button("Chill", "ggMPchill"), other]),
            grid(vec![
                genre_button("K-Pop", "ggMPkpop"),
                genre_button("J-Pop", "ggMPjpop"),
                genre_button("Anime", "ggMPanime"),
                json!({ "musicNavigationButtonRenderer": { "buttonText": { "runs": [ text_run("No link") ] } } }),
                genre_button("Bad token", "has space"),
                genre_button("  ", "ggMPblank"),
            ]),
        ]);
        let genres = parse_genres(&body).unwrap();
        let seen: Vec<(&str, &str)> = genres
            .iter()
            .map(|g| (g.title.as_str(), g.params.as_str()))
            .collect();
        assert_eq!(
            seen,
            [
                ("K-Pop", "ggMPkpop"),
                ("Chill", "ggMPchill"),
                ("J-Pop", "ggMPjpop"),
                ("Anime", "ggMPanime"),
            ]
        );
    }

    #[test]
    fn a_genres_page_without_genre_buttons_is_a_parse_error() {
        assert!(matches!(
            parse_genres(&explore_answer(vec![grid(vec![])])),
            Err(SourceError::Parse(_))
        ));
        assert!(matches!(parse_genres("<html>"), Err(SourceError::Parse(_))));
    }

    #[test]
    fn a_genre_page_gives_playlist_and_song_shelves_and_no_albums() {
        let body = explore_answer(vec![
            carousel(
                "Featured playlists",
                vec![
                    album_card(
                        "VLRDCLAK5uy_a",
                        "J-Pop Rising",
                        vec![
                            text_run("Playlist"),
                            text_run(" • "),
                            text_run("YouTube Music"),
                        ],
                    ),
                    album_card(
                        "VLRDCLAK5uy_a",
                        "J-Pop Rising (again)",
                        vec![text_run("Playlist")],
                    ),
                    album_card("VLPLabc123", "Anime Openings", vec![text_run("Playlist")]),
                    // Not a playlist: a release. Genre pages do not list those.
                    album_card("MPREb_x", "Some Album", vec![text_run("Album")]),
                ],
            ),
            carousel(
                "New albums",
                vec![album_card("MPREb_y", "Fresh", vec![text_run("Album")])],
            ),
            carousel(
                "Top songs",
                vec![page_row("song0000001", "Idol", "YOASOBI", "1M views")],
            ),
            json!({ "musicCarouselShelfRenderer": { "contents": [] } }),
            json!({ "gridRenderer": {
                "header": { "gridHeaderRenderer": { "title": { "runs": [ text_run("More picks") ] } } },
                "items": [ album_card("VLPLdef456", "City Pop Nights", vec![link_run("Someone")]) ],
            } }),
        ]);
        let shelves = parse_genre(&body).unwrap();
        assert_eq!(shelves.len(), 3, "{shelves:?}");
        let ExploreShelf::Playlists {
            title, playlists, ..
        } = &shelves[0]
        else {
            panic!("not playlists: {:?}", shelves[0])
        };
        assert_eq!(title, "Featured playlists");
        let ids: Vec<&str> = playlists.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            ids,
            ["RDCLAK5uy_a", "PLabc123"],
            "VL is cut off, repeats and releases dropped"
        );
        assert_eq!(
            playlists[0].subtitle, "YouTube Music",
            "the type label is not repeated"
        );
        assert_eq!(playlists[1].subtitle, "");
        assert!(playlists[0]
            .thumbnail
            .as_deref()
            .is_some_and(|t| t.contains("w544")));
        assert!(
            matches!(&shelves[1], ExploreShelf::Songs { title, tracks, .. } if title == "Top songs" && tracks.len() == 1)
        );
        let ExploreShelf::Playlists {
            title, playlists, ..
        } = &shelves[2]
        else {
            panic!("not playlists: {:?}", shelves[2])
        };
        assert_eq!(title, "More picks");
        assert_eq!(playlists[0].subtitle, "Someone");
    }

    #[test]
    fn a_genre_page_with_an_unexpected_layout_is_a_parse_error() {
        assert!(matches!(parse_genre("{}"), Err(SourceError::Parse(_))));
    }

    fn playlist_row(id: Option<&str>, title: &str, artist: &str, length: &str) -> Value {
        let mut r = row(id, title, vec![link_run(artist)]);
        r["musicResponsiveListItemRenderer"]["fixedColumns"] = json!([
            { "musicResponsiveListItemFixedColumnRenderer": { "text": { "runs": [ text_run(length) ] } } }
        ]);
        r
    }

    #[test]
    fn a_playlist_gives_its_playable_songs_in_order_in_either_page_layout() {
        let rows = vec![
            playlist_row(Some("song0000001"), "Idol", "YOASOBI", "3:33"),
            playlist_row(None, "Removed song", "Nobody", "1:00"),
            playlist_row(Some("song0000002"), "Gurenge", "LiSA", "3:57"),
            playlist_row(Some("song0000001"), "Idol (again)", "YOASOBI", "3:33"),
        ];
        // Two-column layout: the list is in the secondary column.
        let two = json!({ "contents": { "twoColumnBrowseResultsRenderer": {
            "tabs": [ { "tabRenderer": { "content": { "sectionListRenderer": { "contents": [ { "musicResponsiveHeaderRenderer": {} } ] } } } } ],
            "secondaryContents": { "sectionListRenderer": { "contents": [ { "musicShelfRenderer": { "contents": rows.clone() } } ] } }
        } } })
        .to_string();
        // Older layout: one column, a playlist shelf.
        let one = json!({ "contents": { "singleColumnBrowseResultsRenderer": { "tabs": [ { "tabRenderer": { "content": {
            "sectionListRenderer": { "contents": [ { "musicPlaylistShelfRenderer": { "contents": rows } } ] }
        } } } ] } } })
        .to_string();
        for body in [two, one] {
            let tracks = parse_playlist(&body).unwrap();
            let seen: Vec<(&str, &str, Option<u64>)> = tracks
                .iter()
                .map(|t| (t.key.as_str(), t.artist.as_str(), t.duration_ms))
                .collect();
            assert_eq!(
                seen,
                [
                    ("yt:song0000001", "YOASOBI", Some(213_000)),
                    ("yt:song0000002", "LiSA", Some(237_000)),
                ]
            );
        }
    }

    #[test]
    fn a_playlist_is_cut_at_its_first_page_and_a_page_without_a_list_is_an_error() {
        let rows: Vec<Value> = (0..150)
            .map(|i| playlist_row(Some(&format!("song{i:07}")), "T", "A", "1:00"))
            .collect();
        let body = answer(rows);
        assert_eq!(parse_playlist(&body).unwrap().len(), PLAYLIST_MAX);
        assert!(matches!(parse_playlist("{}"), Err(SourceError::Parse(_))));
    }

    // ------------------------------------------------------------- hostile answers

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
    }

    /// Every place in `v` (as a path of keys/indices), so one can be picked at random.
    fn paths(v: &Value, here: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
        out.push(here.clone());
        match v {
            Value::Object(m) => {
                for (k, c) in m {
                    here.push(k.clone());
                    paths(c, here, out);
                    here.pop();
                }
            }
            Value::Array(a) => {
                for (i, c) in a.iter().enumerate() {
                    here.push(i.to_string());
                    paths(c, here, out);
                    here.pop();
                }
            }
            _ => {}
        }
    }

    fn replace_at(v: &mut Value, path: &[String], with: Value) {
        let Some((first, rest)) = path.split_first() else {
            *v = with;
            return;
        };
        let child = match v {
            Value::Object(m) => m.get_mut(first),
            Value::Array(a) => first.parse::<usize>().ok().and_then(|i| a.get_mut(i)),
            _ => None,
        };
        if let Some(c) = child {
            replace_at(c, rest, with);
        }
    }

    #[test]
    fn no_parser_panics_on_answers_with_wrong_types_or_odd_values() {
        use std::panic::{catch_unwind, AssertUnwindSafe};
        let songs = answer(vec![
            song("song0000001", "Idol", "YOASOBI", "THE BOOK", "3:33"),
            row(Some("song0000002"), "No artist", vec![]),
        ]);
        let explore = explore_answer(vec![
            carousel(
                "Trending",
                vec![
                    page_row("song0000003", "Page song", "Artist", "1M views"),
                    album_card(
                        "MPREb_x",
                        "Album",
                        vec![text_run("Album"), text_run(" • "), link_run("A")],
                    ),
                    video_card(
                        "vid00000001",
                        "Video",
                        vec![link_run("A"), text_run(" • "), text_run("2M views")],
                    ),
                    album_card("VLPLabc123", "A playlist", vec![text_run("Playlist")]),
                ],
            ),
            json!({ "musicShelfRenderer": { "contents": [ song("song0000004", "Shelf", "B", "C", "1:02:03") ] } }),
        ]);
        let genres = explore_answer(vec![grid(vec![
            genre_button("Pop", "ggMPOg"),
            genre_button("Rock", "ggMPOh"),
        ])]);
        let rows = vec![playlist_row(Some("song0000005"), "Row", "Artist", "3:57")];
        let playlist = json!({ "contents": { "twoColumnBrowseResultsRenderer": {
            "tabs": [ { "tabRenderer": { "content": { "sectionListRenderer": { "contents": [ { "musicResponsiveHeaderRenderer": {} } ] } } } } ],
            "secondaryContents": { "sectionListRenderer": { "contents": [ { "musicShelfRenderer": { "contents": rows } } ] } }
        } } })
        .to_string();

        let odd = || -> Vec<Value> {
            vec![
                Value::Null,
                json!(0),
                json!(-1),
                json!(u64::MAX),
                json!(1.0e308),
                json!(""),
                json!("x"),
                json!(" • "),
                json!("9".repeat(5_000)),
                json!("é\u{0}\u{10FFFF}=w1-h1"),
                json!("https://lh3.googleusercontent.com/=w=h-h=w9999999999999999999-h1"),
                json!([]),
                json!({}),
                json!([null, [], {}]),
                json!({ "text": null, "runs": [null], "url": 5, "width": "wide" }),
            ]
        };

        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let mut worst = Vec::new();
        for (name, body) in [
            ("songs", &songs),
            ("explore", &explore),
            ("genres", &genres),
            ("playlist", &playlist),
        ] {
            let original: Value = serde_json::from_str(body).unwrap();
            let mut all = Vec::new();
            paths(&original, &mut Vec::new(), &mut all);
            for round in 0..1_500 {
                let mut v = original.clone();
                for _ in 0..=rng.below(3) {
                    let at = &all[rng.below(all.len())];
                    let options = odd();
                    let pick = options[rng.below(options.len())].clone();
                    replace_at(&mut v, at, pick);
                }
                let text = v.to_string();
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    let _ = parse_songs(&text);
                    let _ = parse_explore(&text);
                    let _ = parse_genres(&text);
                    let _ = parse_genre(&text);
                    let _ = parse_playlist(&text);
                }));
                if outcome.is_err() {
                    worst.push(format!("{name} round {round}: {text}"));
                    if worst.len() >= 3 {
                        break;
                    }
                }
            }
        }
        assert!(worst.is_empty(), "a parser panicked on: {worst:#?}");
    }

    #[test]
    fn genre_and_playlist_requests_name_the_page_and_refuse_an_odd_id() {
        let (url, request) = testserver::once(200, &explore_answer(vec![]));
        assert_eq!(fetch_genre(&url, "ggMPkpop"), Ok(vec![]));
        let sent = request.recv().unwrap();
        assert!(sent.contains("FEmusic_moods_and_genres_category"), "{sent}");
        assert!(sent.contains("ggMPkpop"), "{sent}");

        let (url, request) = testserver::once(200, &answer(vec![]));
        assert_eq!(fetch_playlist(&url, "RDCLAK5uy_a"), Ok(vec![]));
        assert!(request.recv().unwrap().contains("VLRDCLAK5uy_a"));

        let (url, request) = testserver::once(200, "{}");
        assert!(matches!(
            fetch_playlist(&url, "a\"b"),
            Err(SourceError::Failed(_))
        ));
        assert!(request.try_recv().is_err(), "nothing was sent");
    }

    #[test]
    fn a_genre_token_that_is_not_a_plain_token_is_refused_before_any_request() {
        // The playlist id is checked the same way (see the test above); the token comes from the page, but it also
        // arrives from the webview (`genre_get`), so it is held to the same shape `parse_genres` accepts.
        let (url, request) = testserver::once(200, &explore_answer(vec![]));
        for bad in ["", "a b", "x\"y", "{}", &"a".repeat(513)] {
            assert!(
                matches!(fetch_genre(&url, bad), Err(SourceError::Failed(_))),
                "{bad:?}"
            );
        }
        assert!(request.try_recv().is_err(), "nothing was sent");
    }

    #[test]
    fn covers_are_enlarged_by_host_not_by_a_piece_of_the_address() {
        for same in [
            "https://example.com/p?u=googleusercontent.com=w60-h60",
            "https://example.com/ggpht.com=w60-h60",
            "https://evil-googleusercontent.com/x=w60-h60",
            "https://googleusercontent.com.evil.example/x=w60-h60",
        ] {
            assert_eq!(enlarge(same), same);
        }
        assert_eq!(
            enlarge("https://lh3.googleusercontent.com:443/x=w60-h60"),
            "https://lh3.googleusercontent.com:443/x=w544-h544"
        );
    }
}
