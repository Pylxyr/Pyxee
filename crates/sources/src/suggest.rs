//! Live search suggestions from Google's public suggest endpoint (the one YouTube's search box uses).

use crate::SourceError;
use std::time::Duration;

pub const ENDPOINT: &str = "https://suggestqueries.google.com/complete/search";

/// Percent-encodes everything except unreserved characters.
pub fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Response shape: `["query", ["suggestion 1", "suggestion 2", ...]]`.
pub fn parse(body: &str) -> Result<Vec<String>, SourceError> {
    let v: serde_json::Value =
        serde_json::from_str(body).map_err(|e| SourceError::Parse(e.to_string()))?;
    let list = v
        .get(1)
        .and_then(|x| x.as_array())
        .ok_or_else(|| SourceError::Parse("no suggestion list".into()))?;
    Ok(list
        .iter()
        .filter_map(|s| s.as_str())
        .map(str::to_string)
        .take(8)
        .collect())
}

pub fn fetch(prefix: &str) -> Result<Vec<String>, SourceError> {
    if prefix.trim().is_empty() {
        return Ok(Vec::new());
    }
    let url = format!(
        "{ENDPOINT}?client=firefox&ds=yt&q={}",
        encode(prefix.trim())
    );
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(4))
        .build();
    match agent.get(&url).call() {
        Ok(r) => parse(
            &r.into_string()
                .map_err(|e| SourceError::Failed(e.to_string()))?,
        ),
        Err(ureq::Error::Status(429 | 403, _)) => Err(SourceError::RateLimited),
        Err(ureq::Error::Status(c, _)) => Err(SourceError::Failed(format!(
            "suggestions failed (HTTP {c})"
        ))),
        Err(_) => Err(SourceError::Offline),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_spaces_unicode_and_reserved_characters() {
        assert_eq!(encode("lowtide neon"), "lowtide%20neon");
        assert_eq!(encode("a&b=c"), "a%26b%3Dc");
        assert_eq!(encode("é"), "%C3%A9");
        assert_eq!(encode("safe-_.~"), "safe-_.~");
    }

    #[test]
    fn parses_the_suggest_response() {
        let body = r#"["lowt",["lowtide","lowtide neon harbor","lowtide live"]]"#;
        assert_eq!(
            parse(body).unwrap(),
            ["lowtide", "lowtide neon harbor", "lowtide live"]
        );
    }

    #[test]
    fn caps_the_number_of_suggestions_and_rejects_garbage() {
        let many = format!(
            r#"["q",[{}]]"#,
            (0..20)
                .map(|i| format!("\"s{i}\""))
                .collect::<Vec<_>>()
                .join(",")
        );
        assert_eq!(parse(&many).unwrap().len(), 8);
        assert!(matches!(parse("<html>"), Err(SourceError::Parse(_))));
        assert!(matches!(parse(r#"["q"]"#), Err(SourceError::Parse(_))));
    }

    #[test]
    fn blank_input_makes_no_request() {
        assert_eq!(fetch("   ").unwrap(), Vec::<String>::new());
    }
}
