//! Stream addresses straight from `YouTube`'s own `player` endpoint, with no yt-dlp run.
//!
//! Why: every song that plays needs its audio address, and until now only yt-dlp could find it. That made yt-dlp the
//! one thing between the listener and a song: a rate limit, a bot check, an extractor that broke overnight or a missing
//! JavaScript runtime each stopped playback. Asking the endpoint directly is one HTTPS request made inside this
//! process, with the same pacing gate as every other request.
//!
//! Which client to ask as is the part `YouTube` keeps changing. The web client needs a proof-of-origin token and the
//! player's JavaScript to read the address; the Android VR client was refused by the media host (August 2026);
//! `VISIONOS` (the Vision Pro app) answers anonymously with plain, un-ciphered addresses, so it needs neither. That is
//! also what yt-dlp and `NewPipeExtractor` moved to, and the constants below follow yt-dlp's `INNERTUBE_CLIENTS` table
//! (`yt_dlp/extractor/youtube/_base.py`). When `YouTube` retires them the fix is this one table, and until then nothing
//! stops working: any failure but a rate limit makes the caller use yt-dlp, which keeps its own client list up to date.
//!
//! The same idea (call the app's own API, keep the extractor as the fallback) is what Spotube and the other
//! multi-source players do; here the second source is the tool the app already ships, so there is no instance list to
//! keep alive and no third party that learns what is being played.

use crate::ytmusic::parse_switch;
use crate::{SourceError, StreamInfo, StreamMeta};
use serde_json::{Value, json};
use std::time::Duration;
use ureq::http::header::{CONTENT_TYPE, COOKIE, RANGE, USER_AGENT};

pub const ENDPOINT: &str = "https://www.youtube.com/youtubei/v1/player?prettyPrint=false";
/// The setting (environment variable) that turns the direct stream requests of this module off: every song is then
/// resolved by yt-dlp alone, as before.
pub const ENV_VAR: &str = "PYXEE_DIRECT_STREAMS";

const TIMEOUT: Duration = Duration::from_secs(10);

/// Whether Pyxee may ask `YouTube` for stream addresses itself. On unless the setting says off; an empty or unreadable
/// value keeps the default, which is on.
pub fn direct_streams_allowed(setting: Option<&str>) -> bool {
    match setting.map(str::trim).filter(|v| !v.is_empty()) {
        None => true,
        Some(v) => parse_switch(v).unwrap_or_else(|e| {
            log::warn!("{ENV_VAR}: {e}; leaving the direct stream requests on");
            true
        }),
    }
}

/// [`direct_streams_allowed`] for the environment.
#[must_use]
pub fn direct_streams_allowed_from_env() -> bool {
    direct_streams_allowed(std::env::var(ENV_VAR).ok().as_deref())
}

/// Who the request says is asking.
struct Client {
    name: &'static str,
    version: &'static str,
    /// `X-YouTube-Client-Name`.
    id: &'static str,
    device_make: &'static str,
    device_model: &'static str,
    os_name: &'static str,
    os_version: &'static str,
    /// Also sent when the stream is fetched: the media host checks it.
    user_agent: &'static str,
}

/// yt-dlp's `visionos` client.
const VISIONOS: Client = Client {
    name: "VISIONOS",
    version: "1.02",
    id: "101",
    device_make: "Apple",
    device_model: "RealityDevice17,1",
    os_name: "visionOS",
    os_version: "26.5.23O471",
    user_agent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15",
};

#[must_use]
pub fn request_body(id: &str) -> String {
    let c = &VISIONOS;
    json!({
        "context": {
            "client": {
                "clientName": c.name,
                "clientVersion": c.version,
                "deviceMake": c.device_make,
                "deviceModel": c.device_model,
                "osName": c.os_name,
                "osVersion": c.os_version,
                "userAgent": c.user_agent,
                "hl": "en",
            }
        },
        "videoId": id,
        "contentCheckOk": true,
        "racyCheckOk": true,
    })
    .to_string()
}

/// One POST to the `player` endpoint, answered with the body text.
fn post(endpoint: &str, id: &str) -> Result<String, SourceError> {
    let body = request_body(id);
    // A question, not an action: a connection the server closed while it sat idle is simply tried again.
    let agent = crate::net::agent();
    let answer = pyxee_net::retry_stale(|| {
        agent
            .post(endpoint)
            .config()
            .timeout_global(Some(TIMEOUT))
            .build()
            .header(USER_AGENT, VISIONOS.user_agent)
            .header(CONTENT_TYPE, "application/json")
            .header("X-YouTube-Client-Name", VISIONOS.id)
            .header("X-YouTube-Client-Version", VISIONOS.version)
            // The consent cookie the website sets once its banner is answered.
            .header(COOKIE, "SOCS=CAI")
            .send(&body)
    });
    match answer {
        Ok(mut r) => r
            .body_mut()
            .with_config()
            .limit(crate::net::MAX_ANSWER_BYTES)
            .read_to_string()
            .map_err(|e| SourceError::Failed(e.to_string())),
        // Only a real "slow down" stops the caller from using yt-dlp; any other refusal may be this client alone.
        Err(ureq::Error::StatusCode(429)) => Err(SourceError::RateLimited),
        Err(ureq::Error::StatusCode(c)) => Err(SourceError::Failed(format!(
            "YouTube's player endpoint failed (HTTP {c})"
        ))),
        Err(_) => Err(SourceError::Offline),
    }
}

/// The host of an `https` URL (no port), lower-cased; `None` for anything else.
fn https_host(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit('@').next()?.split(':').next()?;
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// A stream address that may be handed to the player: `https`, on `YouTube`'s media hosts. The answer is trusted, but
/// not so far that whatever it names gets fetched.
fn is_media_url(url: &str) -> bool {
    https_host(url).is_some_and(|h| h.ends_with(".googlevideo.com"))
}

/// `(container, codec)` of a format's `mimeType` (`audio/webm; codecs="opus"`).
fn mime_parts(mime: &str) -> Option<(&str, &str)> {
    let (container, rest) = mime.split_once(';')?;
    let codec = rest.split_once("codecs=")?.1.trim().trim_matches('"');
    Some((container.trim(), codec))
}

/// One audio format the built-in decoder plays, ranked like `ytdlp::FORMAT_SELECTOR` ranks them.
struct Pick<'a> {
    url: &'a str,
    /// 0: Opus in `WebM` (the best quality per bit), 1: AAC-LC in M4A.
    tier: u8,
    ext: &'static str,
    acodec: &'static str,
    kbps: f64,
}

/// What `ytdlp::FORMAT_SELECTOR` asks for, over the answer's `adaptiveFormats`: Opus in `WebM`, else AAC-LC in M4A,
/// stereo or mono, the best bitrate of the best kind. Formats without a plain `url` (a `signatureCipher` needs the
/// player's JavaScript), the loudness-boosted copies and the dubbed audio tracks are left out. Anything else the build
/// could decode is not offered by `YouTube` in practice, and yt-dlp is there if it ever is.
fn choose(formats: &[Value]) -> Option<Pick<'_>> {
    let mut picks: Vec<Pick<'_>> = formats
        .iter()
        .filter_map(|f| {
            let url = f.get("url").and_then(Value::as_str)?;
            let (container, codec) = mime_parts(f.get("mimeType").and_then(Value::as_str)?)?;
            let (tier, ext, acodec) = match (container, codec) {
                ("audio/webm", "opus") => (0, "webm", "opus"),
                ("audio/mp4", "mp4a.40.2") => (1, "m4a", "mp4a.40.2"),
                _ => return None,
            };
            if f.get("isDrc").and_then(Value::as_bool) == Some(true)
                || f.pointer("/audioTrack/audioIsDefault")
                    .and_then(Value::as_bool)
                    == Some(false)
                || f.get("audioChannels").and_then(Value::as_u64) > Some(2)
            {
                return None;
            }
            let bits = f
                .get("averageBitrate")
                .or_else(|| f.get("bitrate"))
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            Some(Pick {
                url,
                tier,
                ext,
                acodec,
                kbps: bits / 1000.0,
            })
        })
        .collect();
    picks.sort_by(|a, b| a.tier.cmp(&b.tier).then(b.kbps.total_cmp(&a.kbps)));
    picks.into_iter().next()
}

/// The text of a `playabilityStatus` that is not `OK`: its reason, else its status.
fn refusal(status: &Value) -> String {
    let reason = status
        .get("reason")
        .and_then(Value::as_str)
        .filter(|r| !r.is_empty())
        .or_else(|| status.get("status").and_then(Value::as_str))
        .unwrap_or("no reason given");
    crate::scrub(reason)
}

/// Reads a `player` answer for the video `id`: the best stream the decoder plays, and what the answer says about the
/// song. A video that is not playable, or has no plain address to offer, is an error the caller answers with yt-dlp.
pub fn parse_player(body: &str, id: &str) -> Result<StreamInfo, SourceError> {
    let v: Value = serde_json::from_str(body).map_err(|e| SourceError::Parse(e.to_string()))?;
    let status = v.get("playabilityStatus").unwrap_or(&Value::Null);
    if status.get("status").and_then(Value::as_str) != Some("OK") {
        return Err(SourceError::Unavailable(refusal(status)));
    }
    let details = v.get("videoDetails").unwrap_or(&Value::Null);
    // An answer about another video is never played under this one's name.
    if details.get("videoId").and_then(Value::as_str) != Some(id) {
        return Err(SourceError::Failed(
            "the player endpoint answered for another video".into(),
        ));
    }
    let formats = v
        .pointer("/streamingData/adaptiveFormats")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    let pick = choose(formats)
        .ok_or_else(|| SourceError::Failed("no plain audio stream in the answer".into()))?;
    if !is_media_url(pick.url) {
        return Err(SourceError::Failed(
            "the stream address is not on YouTube's media hosts".into(),
        ));
    }
    let artist = details
        .get("author")
        .and_then(Value::as_str)
        .map(|a| a.strip_suffix(" - Topic").unwrap_or(a).trim())
        .filter(|a| !a.is_empty())
        .map(str::to_string);
    let duration_secs = details
        .get("lengthSeconds")
        .and_then(Value::as_str)
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|d| *d > 0.0);
    Ok(StreamInfo {
        url: pick.url.to_string(),
        ext: pick.ext.to_string(),
        acodec: Some(pick.acodec.to_string()),
        abr_kbps: Some(pick.kbps).filter(|k| *k > 0.0),
        headers: vec![("User-Agent".to_string(), VISIONOS.user_agent.to_string())],
        meta: StreamMeta {
            title: None,
            artist,
            duration_secs,
            thumbnail: crate::ytdlp::thumbnail(
                details.get("thumbnail").unwrap_or(&Value::Null),
                Some(id),
            ),
        },
    })
}

/// Asks the media host for the first byte of `stream`, which is what opening it does. An address the host refuses
/// (this client's URLs need something this network does not have, or the answer was stale) must not be handed to the
/// player: it would fail there, where yt-dlp can no longer be asked. Refused here, the caller asks yt-dlp instead.
fn probe(stream: &StreamInfo) -> Result<(), SourceError> {
    let agent = crate::net::agent();
    let answer = pyxee_net::retry_stale(|| {
        let mut req = agent
            .get(&stream.url)
            .config()
            .timeout_global(Some(TIMEOUT))
            .build();
        for (k, v) in &stream.headers {
            req = req.header(k.as_str(), v.as_str());
        }
        req.header(RANGE, "bytes=0-0").call()
    });
    match answer {
        Ok(_) => Ok(()),
        Err(ureq::Error::StatusCode(429)) => Err(SourceError::RateLimited),
        Err(ureq::Error::StatusCode(c)) => Err(SourceError::Failed(format!(
            "the media host refused the stream address (HTTP {c})"
        ))),
        Err(_) => Err(SourceError::Offline),
    }
}

/// The audio stream of the video `id`, asked of `endpoint` (see [`ENDPOINT`]) and checked to be fetchable.
pub fn fetch_stream(endpoint: &str, id: &str) -> Result<StreamInfo, SourceError> {
    let stream = parse_player(&post(endpoint, id)?, id)?;
    probe(&stream)?;
    Ok(stream)
}

#[cfg(test)]
// The JSON fixtures take their formats by value so that the call sites read like the literals they build.
#[allow(clippy::needless_pass_by_value)]
mod tests {
    use super::*;
    use crate::ytmusic::testserver;
    use serde_json::json;

    const ID: &str = "abc12345678";

    fn audio(itag: u32, mime: &str, kbps: u32, extra: &Value) -> Value {
        let mut f = json!({
            "itag": itag,
            "url": format!("https://rr1---sn-x.googlevideo.com/videoplayback?itag={itag}"),
            "mimeType": mime,
            "bitrate": kbps * 1000 + 5000,
            "averageBitrate": kbps * 1000,
            "audioChannels": 2,
        });
        f.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        f
    }

    fn opus(itag: u32, kbps: u32) -> Value {
        audio(itag, "audio/webm; codecs=\"opus\"", kbps, &json!({}))
    }

    fn aac(itag: u32, kbps: u32) -> Value {
        audio(itag, "audio/mp4; codecs=\"mp4a.40.2\"", kbps, &json!({}))
    }

    fn answer(formats: Vec<Value>) -> String {
        json!({
            "playabilityStatus": { "status": "OK" },
            "videoDetails": {
                "videoId": ID,
                "title": "Neon Harbor",
                "author": "Lowtide - Topic",
                "lengthSeconds": "213",
                "thumbnail": { "thumbnails": [
                    { "url": "https://i.ytimg.com/vi/abc12345678/default.jpg", "width": 120, "height": 90 },
                    { "url": "https://i.ytimg.com/vi/abc12345678/hq720.jpg", "width": 1280, "height": 720 },
                ] },
            },
            "streamingData": { "adaptiveFormats": formats },
        })
        .to_string()
    }

    #[test]
    fn opus_in_webm_wins_and_the_song_details_are_reported() {
        let s = parse_player(
            &answer(vec![
                aac(140, 129),
                opus(250, 70),
                opus(251, 135),
                opus(249, 50),
            ]),
            ID,
        )
        .unwrap();
        assert!(s.url.contains("itag=251"), "{}", s.url);
        assert_eq!(
            (s.ext.as_str(), s.acodec.as_deref(), s.abr_kbps),
            ("webm", Some("opus"), Some(135.0))
        );
        assert_eq!(s.meta.artist.as_deref(), Some("Lowtide"));
        assert_eq!(s.meta.duration_secs, Some(213.0));
        assert_eq!(
            s.meta.thumbnail.as_deref(),
            Some("https://i.ytimg.com/vi/abc12345678/hq720.jpg")
        );
        assert_eq!(s.headers.len(), 1);
        assert_eq!(s.headers[0].0, "User-Agent");
        assert_eq!(s.headers[0].1, VISIONOS.user_agent);
    }

    #[test]
    fn aac_lc_is_the_fallback_and_the_best_bitrate_of_a_kind_wins() {
        let s = parse_player(&answer(vec![aac(139, 48), aac(140, 129)]), ID).unwrap();
        assert!(s.url.contains("itag=140"), "{}", s.url);
        assert_eq!(
            (s.ext.as_str(), s.acodec.as_deref()),
            ("m4a", Some("mp4a.40.2"))
        );
    }

    #[test]
    fn formats_the_player_cannot_use_are_left_out() {
        let heaac = audio(139, "audio/mp4; codecs=\"mp4a.40.5\"", 48, &json!({}));
        let surround = audio(
            258,
            "audio/mp4; codecs=\"mp4a.40.2\"",
            384,
            &json!({ "audioChannels": 6 }),
        );
        let boosted = audio(
            251,
            "audio/webm; codecs=\"opus\"",
            140,
            &json!({ "isDrc": true }),
        );
        let dubbed = audio(
            251,
            "audio/webm; codecs=\"opus\"",
            140,
            &json!({ "audioTrack": { "audioIsDefault": false } }),
        );
        let ciphered = json!({
            "itag": 251, "mimeType": "audio/webm; codecs=\"opus\"", "bitrate": 140_000,
            "signatureCipher": "s=abc&url=https%3A%2F%2Fx",
        });
        let video = audio(137, "video/mp4; codecs=\"avc1.640028\"", 4000, &json!({}));
        let only_junk = vec![heaac, surround, boosted, dubbed, ciphered, video];
        assert!(matches!(
            parse_player(&answer(only_junk.clone()), ID),
            Err(SourceError::Failed(m)) if m.contains("no plain audio stream")
        ));
        // With one good format among them, that one is chosen.
        let mut with_good = only_junk;
        with_good.push(aac(140, 129));
        let s = parse_player(&answer(with_good), ID).unwrap();
        assert!(s.url.contains("itag=140"), "{}", s.url);
        // The original audio track is kept when a language is marked as the default one.
        let original = audio(
            251,
            "audio/webm; codecs=\"opus\"",
            140,
            &json!({ "audioTrack": { "audioIsDefault": true } }),
        );
        assert!(parse_player(&answer(vec![original]), ID).is_ok());
    }

    #[test]
    fn an_unplayable_video_is_unavailable_with_its_reason() {
        let body =
            json!({ "playabilityStatus": { "status": "ERROR", "reason": "Video unavailable" } })
                .to_string();
        assert_eq!(
            parse_player(&body, ID),
            Err(SourceError::Unavailable("Video unavailable".into()))
        );
        let body = json!({ "playabilityStatus": { "status": "LOGIN_REQUIRED" } }).to_string();
        assert_eq!(
            parse_player(&body, ID),
            Err(SourceError::Unavailable("LOGIN_REQUIRED".into()))
        );
    }

    #[test]
    fn an_answer_about_another_video_or_off_the_media_hosts_is_refused() {
        assert!(matches!(
            parse_player(&answer(vec![opus(251, 135)]), "zzz12345678"),
            Err(SourceError::Failed(m)) if m.contains("another video")
        ));
        for bad in [
            "http://rr1---sn-x.googlevideo.com/videoplayback",
            "https://evil.example/videoplayback",
            "https://googlevideo.com.evil.example/videoplayback",
            "https://user@evil.example/?x=.googlevideo.com",
            "ftp://rr1---sn-x.googlevideo.com/x",
        ] {
            let mut f = opus(251, 135);
            f["url"] = json!(bad);
            assert!(
                matches!(
                    parse_player(&answer(vec![f]), ID),
                    Err(SourceError::Failed(_))
                ),
                "{bad}"
            );
        }
        assert!(is_media_url(
            "https://rr1---sn-x.googlevideo.com:443/videoplayback?a=b"
        ));
    }

    #[test]
    fn answers_in_an_unexpected_shape_are_errors_and_never_panics() {
        for body in [
            "<html>",
            "{}",
            "[]",
            "null",
            r#"{"playabilityStatus":{"status":"OK"}}"#,
            r#"{"playabilityStatus":{"status":"OK"},"videoDetails":{"videoId":"abc12345678"}}"#,
            r#"{"playabilityStatus":{"status":"OK"},"videoDetails":{"videoId":"abc12345678"},"streamingData":{"adaptiveFormats":[1,"x",null,{"url":5,"mimeType":7}]}}"#,
        ] {
            assert!(parse_player(body, ID).is_err(), "{body}");
        }
    }

    #[test]
    fn the_request_is_the_visionos_clients() {
        let body: Value = serde_json::from_str(&request_body(ID)).unwrap();
        let c = &body["context"]["client"];
        assert_eq!(c["clientName"], "VISIONOS");
        assert_eq!(c["clientVersion"], VISIONOS.version);
        assert_eq!(c["osName"], "visionOS");
        assert_eq!(body["videoId"], ID);
    }

    #[test]
    fn asking_sends_the_client_headers_and_tells_http_failures_apart() {
        let (url, request) = testserver::once(200, "<html>");
        assert!(matches!(post(&url, ID), Ok(b) if b == "<html>"));
        let sent = request.recv_timeout(Duration::from_secs(5)).unwrap();
        let low = sent.to_lowercase();
        assert!(sent.starts_with("POST /youtubei/v1/search"), "{sent}");
        assert!(low.contains("x-youtube-client-name: 101"), "{sent}");
        assert!(low.contains("x-youtube-client-version: 1.02"), "{sent}");
        assert!(low.contains("user-agent: mozilla/5.0 (macintosh"), "{sent}");
        assert!(low.contains("socs=cai") && sent.contains(ID), "{sent}");

        let (url, _) = testserver::once(429, "{}");
        assert_eq!(post(&url, ID), Err(SourceError::RateLimited));
        let (url, _) = testserver::once(403, "{}");
        assert!(matches!(post(&url, ID), Err(SourceError::Failed(_))));
        let gone = {
            let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            format!("http://{}/", l.local_addr().unwrap())
        };
        assert_eq!(post(&gone, ID), Err(SourceError::Offline));
    }

    #[test]
    fn a_stream_address_is_checked_with_a_one_byte_request_that_carries_the_headers() {
        let stream = |url: String| StreamInfo {
            url,
            headers: vec![("User-Agent".into(), "UA/1".into())],
            ..StreamInfo::default()
        };
        let (url, request) = testserver::once(206, "x");
        assert_eq!(probe(&stream(url)), Ok(()));
        let sent = request
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .to_lowercase();
        assert!(sent.starts_with("get "), "{sent}");
        assert!(sent.contains("range: bytes=0-0"), "{sent}");
        assert!(sent.contains("user-agent: ua/1"), "{sent}");

        let (url, _) = testserver::once(403, "");
        assert!(
            matches!(probe(&stream(url)), Err(SourceError::Failed(m)) if m.contains("HTTP 403"))
        );
        let (url, _) = testserver::once(429, "");
        assert_eq!(probe(&stream(url)), Err(SourceError::RateLimited));
    }

    #[test]
    fn the_direct_streams_switch_is_on_unless_it_says_off() {
        assert!(direct_streams_allowed(None));
        assert!(direct_streams_allowed(Some("")));
        assert!(direct_streams_allowed(Some("1")));
        assert!(direct_streams_allowed(Some("maybe")));
        for off in ["0", "off", "FALSE", " no "] {
            assert!(!direct_streams_allowed(Some(off)), "{off}");
        }
    }
}
