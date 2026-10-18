//! Helpers shared by the integration tests.
#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::TcpListener;

/// Serves `body` with range support, like googlevideo does for these streams.
pub fn range_server(body: Vec<u8>) -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let body = body.clone();
            std::thread::spawn(move || {
                let (mut req, mut b) = (Vec::new(), [0u8; 1]);
                while !req.ends_with(b"\r\n\r\n") {
                    if s.read(&mut b).unwrap_or(0) == 0 {
                        return;
                    }
                    req.push(b[0]);
                }
                let req = String::from_utf8_lossy(&req).to_string();
                let spec = req
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("range: bytes=")
                            .map(str::to_string)
                    })
                    .unwrap_or_else(|| "0-".into());
                let (a, z) = spec.split_once('-').unwrap();
                let from: usize = a.trim().parse().unwrap_or(0);
                let to = z
                    .trim()
                    .parse::<usize>()
                    .unwrap_or(body.len() - 1)
                    .min(body.len() - 1);
                let part = &body[from..=to];
                let head = format!(
                    "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {from}-{to}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len(),
                    part.len()
                );
                let _ = s.write_all(head.as_bytes());
                let _ = s.write_all(part);
            });
        }
    });
    format!("http://{addr}/audio")
}
