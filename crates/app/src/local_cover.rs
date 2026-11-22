//! Answers the window's requests for the cover of a song on this PC (`pyxeecover` addresses, see
//! `pyxee_sources::local::cover_url`). The window shows covers from addresses, so the picture inside a file's tags (or
//! beside it) is served the way a website's image would be. Only a song's own picture is ever read: the address is
//! checked in `pyxee_sources::local::cover_url_path`, and nothing but a recognised picture leaves here.

use pyxee_sources::local::{cover_url_path, read_cover, COVER_SCHEME};
use tauri::http::{header, Response, StatusCode};

/// The answer to a request for `uri`: the cover, or 404 when the song has none (the window then shows its placeholder).
pub fn answer(uri: &str) -> Response<Vec<u8>> {
    let cover = cover_url_path(uri).and_then(|path| read_cover(&path));
    let response = match cover {
        Some(cover) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, cover.mime)
            // A changed picture comes with a new address (the file's modified time is in it).
            .header(header::CACHE_CONTROL, "private, max-age=31536000")
            // So the page can also read the pixels (the colours of the cover) without asking the app.
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(cover.bytes),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .body(Vec::new()),
    };
    response.unwrap_or_else(|_| Response::new(Vec::new()))
}

/// Registers the scheme on the app: each request is answered on a worker thread, so reading a big file never holds up
/// the window.
pub fn register<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.register_asynchronous_uri_scheme_protocol(COVER_SCHEME, |_ctx, request, responder| {
        let uri = request.uri().to_string();
        std::thread::spawn(move || responder.respond(answer(&uri)));
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pyxee_sources::local::cover_url;

    #[test]
    fn a_song_with_a_cover_picture_beside_it_is_answered_and_anything_else_is_not_found() {
        let dir = std::env::temp_dir().join(format!("pyxee-cover-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let song = dir.join("song.flac");
        std::fs::write(&song, b"not really audio").unwrap();
        let url = cover_url(&song, 1);
        assert_eq!(answer(&url).status(), StatusCode::NOT_FOUND, "no picture yet");

        let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 5, 6];
        std::fs::write(dir.join("cover.png"), png).unwrap();
        let found = answer(&url);
        assert_eq!(found.status(), StatusCode::OK);
        assert_eq!(found.headers()[header::CONTENT_TYPE], "image/png");
        assert_eq!(found.body(), &png);

        // An address that is not a song's cover is never opened.
        assert_eq!(answer("https://i.ytimg.com/x.jpg").status(), StatusCode::NOT_FOUND);
        assert_eq!(
            answer(&format!("{COVER_SCHEME}://localhost/not-hex")).status(),
            StatusCode::NOT_FOUND
        );
    }
}
