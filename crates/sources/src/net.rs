//! One HTTP agent for the requests made straight to `YouTube` Music and the suggestion service. Its connection pool
//! keeps the TLS connection to a host open between requests, so a keystroke's suggestions (or the next search) do not
//! pay for a new handshake each time. Timeouts and the user agent are set per request, because they differ.

use std::sync::LazyLock;

pub(crate) fn agent() -> ureq::Agent {
    static AGENT: LazyLock<ureq::Agent> =
        LazyLock::new(|| pyxee_net::agent_config().build().into());
    AGENT.clone()
}

/// The most a `YouTube` Music answer may be. The biggest pages (the explore shelves) are a few megabytes.
pub(crate) const MAX_ANSWER_BYTES: u64 = 16 << 20;
