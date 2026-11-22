//! A song that has been downloaded completely is handed over, in order, to whoever asked (the offline cache).

#![allow(clippy::cast_possible_truncation)]

mod common;

use pyxee_audio::http::{CompletionHook, HttpRangeSource};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn body(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

#[test]
fn the_hook_gets_every_byte_in_order_once_the_download_is_complete() {
    // 1.3 MiB: three chunks of 512 KiB, the last one partial.
    let bytes = body(1_300_000);
    let url = common::range_server(bytes.clone());
    let (tx, rx) = channel::<Vec<u8>>();
    let tx = Mutex::new(tx);
    let hook: CompletionHook = Arc::new(move |chunks: &[Arc<Vec<u8>>]| {
        let whole: Vec<u8> = chunks.iter().flat_map(|c| c.iter().copied()).collect();
        let _ = tx.lock().unwrap().send(whole);
    });
    let _src =
        HttpRangeSource::open_with_hook(&url, &[], Arc::new(AtomicBool::new(false)), Some(hook))
            .unwrap();
    let got = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the hook was called");
    assert_eq!(got.len(), bytes.len());
    assert!(got == bytes, "the chunks put together are the file");
    assert!(
        rx.recv_timeout(Duration::from_millis(300)).is_err(),
        "the hook is called once"
    );
}
