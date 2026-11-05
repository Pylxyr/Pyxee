//! How Pyxee talks HTTP. Every crate that makes a request starts from here, so that three decisions are made once:
//!
//! - **Which certificates are trusted:** the system's store, as the app always did. ureq 3 would otherwise use the
//!   Mozilla roots built into it, and a certificate that a company proxy or an antivirus adds to the system would stop
//!   working.
//! - **How big an answer may be:** a body is read through `Body::with_config().limit(..)`, which stops at the limit and
//!   says so (`Error::BodyExceedsLimit`), instead of a hand-made `take` and a length check.
//! - **When a request is tried again:** ureq 3 never retries by itself (ureq 2 did not either, but its callers were
//!   used to a pooled connection being replaced). A connection the server closed while it sat idle fails the first
//!   try with a reset or an early end of the stream; for a request that is safe to repeat, [`retry_stale`] makes
//!   exactly one more try, on a new connection. Anything else (a status, a timeout, a refused connection) is the
//!   caller's to judge.

use std::io::ErrorKind;
use ureq::config::ConfigBuilder;
use ureq::tls::{RootCerts, TlsConfig};
use ureq::typestate::AgentScope;

/// A configuration to start an agent from: `pyxee_net::agent_config().timeout_connect(..)....build().into()`.
/// Callers add their own timeouts, user agent and redirect rules.
pub fn agent_config() -> ConfigBuilder<AgentScope> {
    ureq::Agent::config_builder().tls_config(
        TlsConfig::builder()
            .root_certs(RootCerts::PlatformVerifier)
            .build(),
    )
}

/// Whether `e` looks like a reused connection that the server had already closed: the connection was reset, aborted
/// or broken, or the answer ended before it began. A new connection is likely to work; a status answer, a timeout
/// or a refused connection says something else, so those are not this.
pub fn is_stale_connection(e: &ureq::Error) -> bool {
    matches!(
        e,
        ureq::Error::Io(io) if matches!(
            io.kind(),
            ErrorKind::ConnectionReset
                | ErrorKind::ConnectionAborted
                | ErrorKind::BrokenPipe
                | ErrorKind::UnexpectedEof
        )
    )
}

/// Runs `attempt`, and once more when the first try failed on a stale connection ([`is_stale_connection`]). Only for a
/// request that may be sent twice (a GET). The second answer, good or bad, is the result.
pub fn retry_stale<T>(
    mut attempt: impl FnMut() -> Result<T, ureq::Error>,
) -> Result<T, ureq::Error> {
    match attempt() {
        Err(e) if is_stale_connection(&e) => {
            log::debug!("http: the connection was stale ({e}); trying once more");
            attempt()
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    /// Reads one request head from `s`.
    fn read_head(s: &mut TcpStream) {
        let mut seen = Vec::new();
        let mut b = [0u8; 256];
        while !seen.windows(4).any(|w| w == b"\r\n\r\n") {
            match s.read(&mut b) {
                Ok(0) | Err(_) => return,
                Ok(n) => seen.extend_from_slice(&b[..n]),
            }
        }
    }

    /// A server that answers connection number `drop_first` and earlier by closing without a word, then answers
    /// `status`. Returns its address and the number of connections it has seen.
    fn server(drop_first: usize, status: &'static str) -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let seen = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&seen);
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut s) = stream else { return };
                let n = count.fetch_add(1, Ordering::SeqCst) + 1;
                read_head(&mut s);
                if n > drop_first {
                    let _ = write!(
                        s,
                        "HTTP/1.1 {status}\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok"
                    );
                }
                // Dropping `s` closes the connection; for the first ones that is the whole answer.
            }
        });
        (url, seen)
    }

    fn agent() -> ureq::Agent {
        agent_config()
            .timeout_global(Some(Duration::from_secs(5)))
            .build()
            .into()
    }

    #[test]
    fn only_a_closed_connection_counts_as_stale() {
        let io = |k| ureq::Error::Io(std::io::Error::from(k));
        for k in [
            ErrorKind::ConnectionReset,
            ErrorKind::ConnectionAborted,
            ErrorKind::BrokenPipe,
            ErrorKind::UnexpectedEof,
        ] {
            assert!(is_stale_connection(&io(k)), "{k:?}");
        }
        for k in [
            ErrorKind::ConnectionRefused,
            ErrorKind::TimedOut,
            ErrorKind::PermissionDenied,
        ] {
            assert!(!is_stale_connection(&io(k)), "{k:?}");
        }
        assert!(!is_stale_connection(&ureq::Error::StatusCode(500)));
        assert!(!is_stale_connection(&ureq::Error::HostNotFound));
        assert!(!is_stale_connection(&ureq::Error::ConnectionFailed));
    }

    #[test]
    fn a_connection_dropped_without_an_answer_is_tried_once_more() {
        let (url, seen) = server(1, "200 OK");
        let agent = agent();
        let mut resp = retry_stale(|| agent.get(&url).call()).expect("the second try answers");
        assert_eq!(resp.body_mut().read_to_string().unwrap(), "ok");
        assert_eq!(seen.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_server_that_keeps_dropping_is_tried_only_twice() {
        let (url, seen) = server(usize::MAX, "200 OK");
        let agent = agent();
        let err = retry_stale(|| agent.get(&url).call()).unwrap_err();
        assert!(is_stale_connection(&err), "{err:?}");
        assert_eq!(seen.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_status_answer_is_not_tried_again() {
        let (url, seen) = server(0, "503 Service Unavailable");
        let agent = agent();
        let err = retry_stale(|| agent.get(&url).call()).unwrap_err();
        assert!(matches!(err, ureq::Error::StatusCode(503)), "{err:?}");
        assert_eq!(seen.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_body_over_the_limit_is_an_error_not_a_cut() {
        let (url, _) = server(0, "200 OK");
        let agent = agent();
        let mut resp = agent.get(&url).call().unwrap();
        let err = resp
            .body_mut()
            .with_config()
            .limit(1)
            .read_to_vec()
            .unwrap_err();
        assert!(matches!(err, ureq::Error::BodyExceedsLimit(1)), "{err:?}");
    }
}
