//! Which addresses may be a cover. The window's content-security policy lets pictures come from these hosts only, so
//! a cover on any other host could never be shown; keeping it out where it is read also keeps it from being handed to
//! anything else (the cover fetch, Discord, which would load it from its servers).

/// The image hosts of YouTube and Google. A host must be a subdomain of one of these (`i.ytimg.com`).
const SUFFIXES: [&str; 3] = [".ytimg.com", ".googleusercontent.com", ".ggpht.com"];

/// True for an `https` address whose host is one of YouTube's or Google's image hosts: lower-case letters, digits,
/// dots and hyphens only (no user info, no port), so what is checked is what is contacted.
pub fn is_cover_host(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    let plain = !host.is_empty()
        && host
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-');
    plain && SUFFIXES.iter().any(|s| host.ends_with(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_https_on_a_known_image_host_is_a_cover() {
        for yes in [
            "https://i.ytimg.com/vi/abc/hqdefault.jpg",
            "https://lh3.googleusercontent.com/x=w544-h544",
            "https://yt3.ggpht.com/a?b=c",
            "https://i.ytimg.com?x=1",
        ] {
            assert!(is_cover_host(yes), "{yes}");
        }
        for no in [
            "",
            "http://i.ytimg.com/x",
            "https://ytimg.com/x",
            "https://example.com/i.ytimg.com",
            "https://evil.example/?u=https://i.ytimg.com/x",
            "https://i.ytimg.com.evil.example/x",
            "https://evil-ytimg.com/x",
            "https://i.ytimg.com@evil.example/x",
            "https://evil.example@i.ytimg.com/x",
            "https://i.ytimg.com:8443/x",
            "https://I.YTIMG.COM/x",
            "https://i.ytimg.com\\@evil.example/x",
            "data:image/png;base64,AAAA",
            r"C:\Music\cover.jpg",
        ] {
            assert!(!is_cover_host(no), "{no}");
        }
    }
}
