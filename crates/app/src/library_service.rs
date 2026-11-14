//! The library as the UI sees it: DTOs in and out, one revision counter, and no Tauri types, so it can
//! be tested without a window. Commands wrap this, then tell the UI the library changed.

use crate::dto::{
    AddResultDto, HistoryItemDto, HomeDto, LibrarySnapshotDto, PlaylistDto, PlaylistEntryDto,
    PlaylistSnapshotDto, PlaylistSummaryDto, RemovedEntryDto, TrackDto,
};
use pyxee_core::Track;
use pyxee_library::{HistoryItem, Library, PlaylistSnapshot, RemovedEntry};

const HOME_ITEMS: usize = 12;

pub struct LibraryService {
    lib: Library,
    revision: u64,
    /// Why the on-disk library could not be used (the service then runs in memory).
    pub error: Option<String>,
}

type Res<T> = Result<T, String>;

fn err(e: pyxee_library::LibraryError) -> String {
    e.to_string()
}

fn track(t: &Track) -> TrackDto {
    t.into()
}

fn tracks(list: &[TrackDto]) -> Vec<Track> {
    list.iter().cloned().map(Track::from).collect()
}

impl LibraryService {
    pub fn new(lib: Library, error: Option<String>) -> Self {
        Self {
            lib,
            revision: 1,
            error,
        }
    }

    /// Opens the library file; if that fails, runs in memory and remembers why.
    pub fn open_or_memory(path: Option<&std::path::Path>) -> Self {
        match path.map(Library::open) {
            Some(Ok(lib)) => Self::new(lib, None),
            Some(Err(e)) => Self::in_memory(Some(e.to_string())),
            None => Self::in_memory(Some("No place to store the library was found.".into())),
        }
    }

    fn in_memory(error: Option<String>) -> Self {
        Self::new(
            Library::open_in_memory().expect("in-memory SQLite is always available"),
            error,
        )
    }

    fn changed<T>(&mut self, r: Res<T>) -> Res<T> {
        if r.is_ok() {
            self.revision += 1;
        }
        r
    }

    pub fn snapshot(&self) -> Res<LibrarySnapshotDto> {
        Ok(LibrarySnapshotDto {
            revision: self.revision,
            liked_keys: self.lib.liked_keys().map_err(err)?,
            playlists: self
                .lib
                .playlists()
                .map_err(err)?
                .into_iter()
                .map(|p| PlaylistSummaryDto {
                    id: p.id,
                    name: p.name,
                    track_count: p.track_count,
                    updated_at: p.updated_at,
                    thumbnail: p.thumbnail,
                })
                .collect(),
            history_count: self.lib.history_count().map_err(err)?,
            error: self.error.clone(),
        })
    }

    pub fn set_liked(&mut self, t: &TrackDto, liked: bool, now: i64) -> Res<()> {
        let r = if liked {
            self.lib.like(&Track::from(t.clone()), now).map(drop)
        } else {
            self.lib.unlike(&t.key).map(drop)
        };
        let r = r.map_err(err);
        self.changed(r)
    }

    pub fn liked(&self) -> Res<Vec<TrackDto>> {
        Ok(self.lib.liked().map_err(err)?.iter().map(track).collect())
    }

    pub fn create_playlist(&mut self, name: &str, list: &[TrackDto], now: i64) -> Res<i64> {
        let r = self
            .lib
            .create_playlist(name, &tracks(list), now)
            .map_err(err);
        self.changed(r)
    }

    pub fn rename_playlist(&mut self, id: i64, name: &str, now: i64) -> Res<()> {
        let r = self.lib.rename_playlist(id, name, now).map_err(err);
        self.changed(r)
    }

    pub fn add_to_playlist(&mut self, id: i64, list: &[TrackDto], now: i64) -> Res<AddResultDto> {
        let r = self
            .lib
            .add_to_playlist(id, &tracks(list), now)
            .map(|a| AddResultDto {
                added: a.added,
                skipped: a.skipped,
            })
            .map_err(err);
        self.changed(r)
    }

    pub fn delete_playlist(&mut self, id: i64) -> Res<PlaylistSnapshotDto> {
        let r = self
            .lib
            .delete_playlist(id)
            .map(|s| PlaylistSnapshotDto {
                id: s.id,
                name: s.name,
                created_at: s.created_at,
                updated_at: s.updated_at,
                tracks: s.tracks.iter().map(track).collect(),
            })
            .map_err(err);
        self.changed(r)
    }

    pub fn restore_playlist(&mut self, s: &PlaylistSnapshotDto) -> Res<()> {
        let snap = PlaylistSnapshot {
            id: s.id,
            name: s.name.clone(),
            created_at: s.created_at,
            updated_at: s.updated_at,
            tracks: tracks(&s.tracks),
        };
        let r = self.lib.restore_playlist(&snap).map_err(err);
        self.changed(r)
    }

    pub fn playlist(&self, id: i64) -> Res<PlaylistDto> {
        let p = self.lib.playlist(id).map_err(err)?;
        Ok(PlaylistDto {
            id: p.id,
            name: p.name,
            created_at: p.created_at,
            updated_at: p.updated_at,
            entries: p
                .entries
                .iter()
                .map(|e| PlaylistEntryDto {
                    entry_id: e.entry_id,
                    track: track(&e.track),
                })
                .collect(),
        })
    }

    pub fn remove_entry(&mut self, entry_id: i64, now: i64) -> Res<RemovedEntryDto> {
        let r = self
            .lib
            .remove_entry(entry_id, now)
            .map(|r| RemovedEntryDto {
                playlist_id: r.playlist_id,
                position: r.position,
                track: track(&r.track),
            })
            .map_err(err);
        self.changed(r)
    }

    pub fn restore_entry(&mut self, r: &RemovedEntryDto, now: i64) -> Res<i64> {
        let removed = RemovedEntry {
            playlist_id: r.playlist_id,
            position: r.position,
            track: r.track.clone().into(),
        };
        let res = self.lib.restore_entry(&removed, now).map_err(err);
        self.changed(res)
    }

    pub fn move_entry(&mut self, entry_id: i64, to: usize, now: i64) -> Res<()> {
        let r = self.lib.move_entry(entry_id, to, now).map_err(err);
        self.changed(r)
    }

    /// A saved song learned its real artist, length or cover.
    pub fn refresh_track(&mut self, t: &Track) -> Res<bool> {
        let r = self.lib.refresh_track(t).map_err(err);
        if matches!(r, Ok(true)) {
            self.revision += 1;
        }
        r
    }

    pub fn record_play(&mut self, t: &Track, now: i64) -> Res<()> {
        let r = self.lib.record_play(t, now).map_err(err);
        self.changed(r)
    }

    pub fn history(&self, limit: usize) -> Res<Vec<HistoryItemDto>> {
        Ok(self
            .lib
            .history(limit)
            .map_err(err)?
            .iter()
            .map(|h| HistoryItemDto {
                track: track(&h.track),
                played_at: h.played_at,
            })
            .collect())
    }

    pub fn clear_history(&mut self) -> Res<Vec<HistoryItemDto>> {
        let r = self
            .lib
            .clear_history()
            .map(|all| {
                all.iter()
                    .map(|h| HistoryItemDto {
                        track: track(&h.track),
                        played_at: h.played_at,
                    })
                    .collect()
            })
            .map_err(err);
        self.changed(r)
    }

    pub fn restore_history(&mut self, items: &[HistoryItemDto]) -> Res<()> {
        let items: Vec<HistoryItem> = items
            .iter()
            .map(|h| HistoryItem {
                track: h.track.clone().into(),
                played_at: h.played_at,
            })
            .collect();
        let r = self.lib.restore_history(&items).map_err(err);
        self.changed(r)
    }

    pub fn home(&self, now: i64) -> Res<HomeDto> {
        let h = self.lib.home(now, HOME_ITEMS).map_err(err)?;
        Ok(HomeDto {
            quick_picks: h.quick_picks.iter().map(track).collect(),
            recent: h.recent.iter().map(track).collect(),
            liked_count: h.liked_count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(n: &str) -> TrackDto {
        TrackDto {
            key: format!("yt:{n}"),
            title: format!("Title {n}"),
            artist: "A".into(),
            duration_ms: Some(1000),
            thumbnail: None,
        }
    }
    fn svc() -> LibraryService {
        LibraryService::new(Library::open_in_memory().unwrap(), None)
    }

    #[test]
    fn every_successful_change_bumps_the_revision_and_failures_do_not() {
        let mut s = svc();
        let r0 = s.snapshot().unwrap().revision;
        s.set_liked(&t("a"), true, 1).unwrap();
        let r1 = s.snapshot().unwrap().revision;
        assert!(r1 > r0);
        assert!(s.create_playlist("  ", &[], 2).is_err());
        assert_eq!(
            s.snapshot().unwrap().revision,
            r1,
            "a rejected change is not a change"
        );
        assert_eq!(s.snapshot().unwrap().liked_keys, ["yt:a"]);
    }

    #[test]
    fn errors_are_plain_sentences_for_the_ui() {
        let mut s = svc();
        assert_eq!(
            s.create_playlist("", &[], 1).unwrap_err(),
            "Playlist names must be 1 to 100 characters."
        );
        assert_eq!(
            s.playlist(404).unwrap_err(),
            "That playlist no longer exists."
        );
    }

    #[test]
    fn a_deleted_playlist_comes_back_through_the_dto_round_trip() {
        let mut s = svc();
        let id = s.create_playlist("Mix", &[t("a"), t("b")], 5).unwrap();
        let snap = s.delete_playlist(id).unwrap();
        let json = serde_json::to_string(&snap).unwrap(); // what the UI holds for Undo
        s.restore_playlist(&serde_json::from_str(&json).unwrap())
            .unwrap();
        let p = s.playlist(id).unwrap();
        assert_eq!(
            p.entries
                .iter()
                .map(|e| e.track.key.as_str())
                .collect::<Vec<_>>(),
            ["yt:a", "yt:b"]
        );
        assert_eq!(p.created_at, 5);
    }

    #[test]
    fn removing_and_restoring_an_entry_goes_through_the_dto_too() {
        let mut s = svc();
        let id = s
            .create_playlist("Mix", &[t("a"), t("b"), t("c")], 1)
            .unwrap();
        let mid = s.playlist(id).unwrap().entries[1].entry_id;
        let removed = s.remove_entry(mid, 2).unwrap();
        assert_eq!(removed.position, 1);
        s.restore_entry(&removed, 3).unwrap();
        let keys: Vec<_> = s
            .playlist(id)
            .unwrap()
            .entries
            .into_iter()
            .map(|e| e.track.key)
            .collect();
        assert_eq!(keys, ["yt:a", "yt:b", "yt:c"]);
    }

    #[test]
    fn adding_reports_how_many_were_new() {
        let mut s = svc();
        let id = s.create_playlist("Mix", &[t("a")], 1).unwrap();
        assert_eq!(
            s.add_to_playlist(id, &[t("a"), t("b")], 2).unwrap(),
            AddResultDto {
                added: 1,
                skipped: 1
            }
        );
    }

    #[test]
    fn snapshot_lists_playlists_with_counts_and_history_size() {
        let mut s = svc();
        s.create_playlist("One", &[t("a"), t("b")], 1).unwrap();
        s.record_play(&t("a").into(), 2).unwrap();
        let snap = s.snapshot().unwrap();
        assert_eq!(
            (
                snap.playlists[0].name.as_str(),
                snap.playlists[0].track_count,
                snap.history_count
            ),
            ("One", 2, 1)
        );
    }

    #[test]
    fn clearing_history_hands_back_what_it_removed_for_undo() {
        let mut s = svc();
        s.record_play(&t("a").into(), 10).unwrap();
        s.record_play(&t("b").into(), 20).unwrap();
        let cleared = s.clear_history().unwrap();
        assert_eq!(s.snapshot().unwrap().history_count, 0);
        s.restore_history(&cleared).unwrap();
        assert_eq!(
            s.history(10)
                .unwrap()
                .iter()
                .map(|h| h.played_at)
                .collect::<Vec<_>>(),
            [20, 10]
        );
    }

    #[test]
    fn an_unopenable_library_falls_back_to_memory_and_says_so() {
        let dir = std::env::temp_dir().join(format!("pyxee-svc-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("library.db");
        std::fs::write(
            &path,
            b"not a database, just text that is long enough to not look like an empty file",
        )
        .unwrap();
        let mut s = LibraryService::open_or_memory(Some(&path));
        assert!(s.error.as_deref().unwrap().contains("could not be read"));
        s.set_liked(&t("a"), true, 1).unwrap(); // still usable this session
        assert_eq!(s.snapshot().unwrap().error, s.error);
        assert!(LibraryService::open_or_memory(None).error.is_some());
    }

    #[test]
    fn home_surfaces_liked_and_played_songs() {
        let mut s = svc();
        s.set_liked(&t("liked"), true, 100).unwrap();
        s.record_play(&t("played").into(), 200).unwrap();
        let h = s.home(300).unwrap();
        assert_eq!(h.liked_count, 1);
        assert!(h.quick_picks.iter().any(|t| t.key == "yt:liked"));
        assert_eq!(h.recent[0].key, "yt:played");
    }
}
