//! Track keys: the one place that says what `yt:<id>` and `file:<path>` mean. A key travels through the player, the
//! queue, the library and the UI as text, so everything that builds or reads one goes through here, and a key from
//! outside (the webview can send any text) is parsed once, with the same rules everywhere.

use std::path::Path;

const YOUTUBE: &str = "yt:";
const FILE: &str = "file:";

/// What a key refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackKey<'a> {
    /// A YouTube video (a plain id: it goes into a URL later).
    Youtube(&'a str),
    /// A file on this computer.
    File(&'a str),
}

/// A plain video id: at most 64 of letters, digits, `_` and `-`. It goes into a URL, so nothing else is let through.
pub fn is_video_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// A path that is on this computer: not a network path (`\\host\share`, `//host/x`), which Windows would contact with
/// the person's credentials just to look for the file, not a device path (`\\?\`, `\\.\`), and without a NUL.
fn is_local_path(path: &str) -> bool {
    !path.is_empty() && !path.starts_with("\\\\") && !path.starts_with("//") && !path.contains('\0')
}

/// Reads a key. `None` for anything that is neither a valid `yt:` key nor a `file:` key with a local path.
pub fn parse(key: &str) -> Option<TrackKey<'_>> {
    if let Some(id) = key.strip_prefix(YOUTUBE) {
        return is_video_id(id).then_some(TrackKey::Youtube(id));
    }
    key.strip_prefix(FILE)
        .filter(|p| is_local_path(p))
        .map(TrackKey::File)
}

/// Whether the key names a local file (which source plays it), whatever follows the prefix.
pub fn is_file(key: &str) -> bool {
    key.starts_with(FILE)
}

/// The key of a YouTube video.
pub fn youtube(id: &str) -> String {
    format!("{YOUTUBE}{id}")
}

/// The key of a local file.
pub fn file(path: &Path) -> String {
    format!("{FILE}{}", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_round_trip() {
        assert_eq!(
            parse(&youtube("dQw4w9WgXcQ")),
            Some(TrackKey::Youtube("dQw4w9WgXcQ"))
        );
        let k = file(Path::new("/music/a b.flac"));
        assert_eq!(parse(&k), Some(TrackKey::File("/music/a b.flac")));
        assert!(is_file(&k));
        assert!(!is_file("yt:abc"));
    }

    #[test]
    fn only_plain_video_ids_are_let_through() {
        for bad in [
            "", "yt:", "yt:a b", "yt:a/b", "yt:a?x=1", "yt:ä", "file:", "http://x", "abc",
        ] {
            assert_eq!(parse(bad), None, "{bad:?}");
        }
        assert!(parse(&youtube(&"a".repeat(64))).is_some());
        assert!(parse(&youtube(&"a".repeat(65))).is_none());
        assert!(is_video_id("a-b_C9"));
        assert!(!is_video_id("a.b"));
    }

    #[test]
    fn a_file_key_keeps_every_character_of_the_path() {
        let p = "C:\\Users\\me\\Music\\a:b #1.mp3";
        assert_eq!(parse(&format!("file:{p}")), Some(TrackKey::File(p)));
    }

    #[test]
    fn a_network_or_device_path_is_not_a_file_key() {
        // The webview can send any key; naming a network share must not get as far as touching it.
        for bad in [
            r"file:\\attacker\share\a.mp3",
            "file://attacker/share/a.mp3",
            r"file:\\?\C:\a.mp3",
            r"file:\\.\pipe\x",
            "file:/tmp/a\0.mp3",
        ] {
            assert_eq!(parse(bad), None, "{bad:?}");
        }
        assert!(parse("file:/tmp/a.mp3").is_some());
        assert!(parse(r"file:C:\Music\a.mp3").is_some());
    }
}
