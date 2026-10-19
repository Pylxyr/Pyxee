//! A song that has been downloaded plays to the end with the network gone.

use pyxee_audio::http::HttpRangeSource;
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A range server that can be switched "offline": while the flag is set it drops every connection unanswered.
fn serve(body: Vec<u8>, offline: Arc<AtomicBool>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            if offline.load(Ordering::SeqCst) {
                continue; // dropped: the client sees a closed connection
            }
            let mut req = Vec::new();
            let mut b = [0u8; 1];
            while !req.ends_with(b"\r\n\r\n") {
                if s.read(&mut b).unwrap_or(0) == 0 {
                    break;
                }
                req.push(b[0]);
            }
            let text = String::from_utf8_lossy(&req).to_string();
            let range = text
                .lines()
                .find_map(|l| {
                    l.to_ascii_lowercase()
                        .strip_prefix("range: bytes=")
                        .map(str::to_string)
                })
                .unwrap_or_default();
            let (a, b) = range.split_once('-').unwrap_or(("0", "0"));
            let from: usize = a.trim().parse().unwrap_or(0);
            let to: usize = b.trim().parse::<usize>().unwrap_or(0).min(body.len() - 1);
            let part = &body[from..=to];
            let head = format!(
                "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {from}-{to}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len(),
                part.len()
            );
            let _ = s.write_all(head.as_bytes());
            let _ = s.write_all(part);
        }
    });
    format!("http://{addr}/track")
}

fn song() -> Vec<u8> {
    // Several chunks long, with a pattern that shows any misplaced byte.
    (0..3_000_000u32).map(|i| (i % 251) as u8).collect()
}

fn wait_until_downloaded(src: &HttpRangeSource, len: u64) {
    let start = Instant::now();
    while src.buffered_bytes() < len {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "the background download did not finish ({} of {len} bytes)",
            src.buffered_bytes()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_downloaded_song_plays_to_the_end_with_the_network_gone() {
    let body = song();
    let offline = Arc::new(AtomicBool::new(false));
    let url = serve(body.clone(), offline.clone());
    let mut src = HttpRangeSource::open(&url).unwrap();

    // Nothing is read yet: the download runs by itself, ahead of the decoder.
    wait_until_downloaded(&src, body.len() as u64);

    offline.store(true, Ordering::SeqCst);
    let mut got = Vec::new();
    src.read_to_end(&mut got).unwrap();
    assert_eq!(got, body);

    // Seeking around works offline too.
    src.seek(SeekFrom::Start(1_234_567)).unwrap();
    let mut few = [0u8; 100];
    src.read_exact(&mut few).unwrap();
    assert_eq!(&few[..], &body[1_234_567..1_234_667]);
}

#[test]
fn the_download_continues_while_the_reader_is_still_at_the_start() {
    let body = song();
    let offline = Arc::new(AtomicBool::new(false));
    let url = serve(body.clone(), offline);
    let mut src = HttpRangeSource::open(&url).unwrap();
    let mut first = [0u8; 10];
    src.read_exact(&mut first).unwrap();
    assert_eq!(&first[..], &body[..10]);
    wait_until_downloaded(&src, body.len() as u64);
}
