//! Local files: `file:<path>` keys. Library scanning comes with milestone 5; this resolves a file for playback.

use crate::{MusicSource, SearchFilter, SearchItem, SourceError, StreamInfo, TrackInfo};
use std::path::Path;

#[derive(Default)]
pub struct LocalSource;

pub fn key_for(path: &Path) -> String {
    crate::key::file(path)
}

/// A track entry for a local file, titled from its file name until tag reading arrives.
pub fn track_for(path: &Path) -> TrackInfo {
    TrackInfo {
        key: key_for(path),
        title: path
            .file_stem()
            .map_or_else(|| "Unknown".into(), |s| s.to_string_lossy().into_owned()),
        artist: "Local file".into(),
        duration_ms: None,
        thumbnail: None,
    }
}

impl MusicSource for LocalSource {
    fn search(&self, _q: &str, _f: SearchFilter) -> Result<Vec<SearchItem>, SourceError> {
        Ok(Vec::new())
    }

    fn resolve_stream(&self, key: &str) -> Result<StreamInfo, SourceError> {
        let Some(crate::key::TrackKey::File(path)) = crate::key::parse(key) else {
            return Err(SourceError::Failed(format!("not a local key: {key}")));
        };
        if !Path::new(path).is_absolute() {
            return Err(SourceError::Failed("not a local file path".into()));
        }
        if !Path::new(path).is_file() {
            return Err(SourceError::Unavailable(format!(
                "the file {path} is missing"
            )));
        }
        let ext = Path::new(path)
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        Ok(StreamInfo {
            url: path.to_string(),
            ext,
            ..Default::default()
        })
    }

    fn related(&self, _key: &str, _count: usize) -> Result<Vec<TrackInfo>, SourceError> {
        Ok(Vec::new())
    }
    fn suggest(&self, _prefix: &str) -> Result<Vec<String>, SourceError> {
        Ok(Vec::new())
    }
    fn supported_filters(&self) -> Vec<SearchFilter> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_an_existing_file_and_reports_a_missing_one_clearly() {
        let dir = std::env::temp_dir().join(format!("pyxee-local-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("Song One.FLAC");
        std::fs::write(&f, b"x").unwrap();
        let s = LocalSource.resolve_stream(&key_for(&f)).unwrap();
        assert_eq!(s.ext, "flac");
        assert_eq!(s.url, f.display().to_string());
        let err = LocalSource
            .resolve_stream(&key_for(&dir.join("gone.mp3")))
            .unwrap_err();
        assert!(matches!(err, SourceError::Unavailable(m) if m.contains("missing")));
        assert_eq!(track_for(&f).title, "Song One");
    }

    #[test]
    fn a_relative_or_network_path_is_refused_before_it_is_touched() {
        for key in [
            "file:relative/a.mp3",
            "file:a.mp3",
            r"file:\\attacker\share\a.mp3",
            "file://attacker/share/a.mp3",
            "file:",
        ] {
            assert!(
                matches!(LocalSource.resolve_stream(key), Err(SourceError::Failed(_))),
                "{key:?}"
            );
        }
    }
}
