//! The typed contract between Rust and the UI. TypeScript types are generated from these
//! (`cargo test -p pyxee-app` writes them to `ui/src/lib/bindings`), so the two sides cannot drift.

use pyxee_core::{Command, EntryId, Failure, Origin, Player, Repeat, Track, Transport};
use pyxee_sources::{SearchFilter, SearchItem, SourceError};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TrackDto {
    pub key: String,
    pub title: String,
    pub artist: String,
    #[ts(type = "number | null")]
    pub duration_ms: Option<u64>,
    pub thumbnail: Option<String>,
}

impl From<&Track> for TrackDto {
    fn from(t: &Track) -> Self {
        Self {
            key: t.key.clone(),
            title: t.title.clone(),
            artist: t.artist.clone(),
            duration_ms: t.duration_ms,
            thumbnail: t.thumbnail.clone(),
        }
    }
}

impl From<&pyxee_sources::TrackInfo> for TrackDto {
    fn from(t: &pyxee_sources::TrackInfo) -> Self {
        Self {
            key: t.key.clone(),
            title: t.title.clone(),
            artist: t.artist.clone(),
            duration_ms: t.duration_ms,
            thumbnail: t.thumbnail.clone(),
        }
    }
}

impl From<TrackDto> for Track {
    fn from(t: TrackDto) -> Self {
        Self {
            key: t.key,
            title: t.title,
            artist: t.artist,
            duration_ms: t.duration_ms,
            thumbnail: t.thumbnail,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum OriginDto {
    User,
    Radio,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EntryDto {
    #[ts(type = "number")]
    pub id: u64,
    pub track: TrackDto,
    pub origin: OriginDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum FailureDto {
    Offline,
    RateLimited,
    Unavailable,
    Decode,
    Expired { status: Option<u16> },
    Other { message: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum TransportDto {
    Idle,
    Loading {
        #[ts(type = "number")]
        id: u64,
    },
    Playing {
        #[ts(type = "number")]
        id: u64,
    },
    Paused {
        #[ts(type = "number")]
        id: u64,
        loaded: bool,
    },
    Stalled {
        #[ts(type = "number")]
        id: u64,
    },
    Failed {
        #[ts(type = "number")]
        id: u64,
        failure: FailureDto,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RepeatDto {
    Off,
    All,
    One,
}

/// Everything the UI needs to draw the player and the queue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlayerStateDto {
    pub transport: TransportDto,
    /// What the user can hear right now.
    #[ts(type = "number | null")]
    pub audible: Option<u64>,
    /// Requested but not yet audible.
    #[ts(type = "number | null")]
    pub loading: Option<u64>,
    #[ts(type = "number")]
    pub position_ms: u64,
    pub repeat: RepeatDto,
    pub shuffle: bool,
    pub autoplay: bool,
    pub can_undo: bool,
    pub queue: Vec<EntryDto>,
    /// The selected entry (audible, loading or restored-paused).
    #[ts(type = "number | null")]
    pub current: Option<u64>,
    #[ts(type = "Array<number>")]
    pub upcoming: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum CommandDto {
    PlayEntry {
        #[ts(type = "number")]
        id: u64,
    },
    PlayTracks {
        tracks: Vec<TrackDto>,
        #[ts(type = "number")]
        start: usize,
    },
    Toggle,
    Pause,
    Resume,
    Next,
    Previous,
    Seek {
        #[ts(type = "number")]
        ms: u64,
    },
    Retry,
    SetRepeat {
        repeat: RepeatDto,
    },
    SetShuffle {
        on: bool,
    },
    SetAutoplay {
        on: bool,
    },
    AddToQueue {
        track: TrackDto,
    },
    PlayNext {
        track: TrackDto,
    },
    Remove {
        #[ts(type = "number")]
        id: u64,
    },
    Move {
        #[ts(type = "number")]
        id: u64,
        #[ts(type = "number")]
        to: usize,
    },
    /// Reorders Up next: `to` counts among the upcoming songs, 0 = plays next.
    MoveUpcoming {
        #[ts(type = "number")]
        id: u64,
        #[ts(type = "number")]
        to: usize,
    },
    /// The sleep timer's "stop after this song".
    StopAfterCurrent {
        on: bool,
    },
    Clear,
    Undo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SearchFilterDto {
    All,
    Songs,
    Videos,
    Albums,
    Artists,
    Playlists,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FilterInfoDto {
    pub filter: SearchFilterDto,
    /// False when the current source cannot serve this filter yet (the UI shows it disabled).
    pub supported: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum SearchItemDto {
    Track {
        track: TrackDto,
    },
    Playlist {
        key: String,
        title: String,
        uploader: String,
        thumbnail: Option<String>,
    },
    Artist {
        key: String,
        name: String,
        thumbnail: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SearchResponseDto {
    pub items: Vec<SearchItemDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SearchErrorKind {
    Offline,
    RateLimited,
    ToolMissing,
    Unsupported,
    Other,
}

/// Why a search failed, in a form the UI can explain and offer a retry for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SearchErrorDto {
    pub kind: SearchErrorKind,
    pub message: String,
}

impl From<&SourceError> for SearchErrorDto {
    fn from(e: &SourceError) -> Self {
        let kind = match e {
            SourceError::Offline => SearchErrorKind::Offline,
            SourceError::RateLimited => SearchErrorKind::RateLimited,
            SourceError::ToolMissing(_) => SearchErrorKind::ToolMissing,
            SourceError::Unsupported(_) => SearchErrorKind::Unsupported,
            _ => SearchErrorKind::Other,
        };
        Self {
            kind,
            message: e.to_string(),
        }
    }
}

impl From<SearchFilterDto> for SearchFilter {
    fn from(f: SearchFilterDto) -> Self {
        match f {
            SearchFilterDto::All => SearchFilter::All,
            SearchFilterDto::Songs => SearchFilter::Songs,
            SearchFilterDto::Videos => SearchFilter::Videos,
            SearchFilterDto::Albums => SearchFilter::Albums,
            SearchFilterDto::Artists => SearchFilter::Artists,
            SearchFilterDto::Playlists => SearchFilter::Playlists,
        }
    }
}

impl From<SearchFilter> for SearchFilterDto {
    fn from(f: SearchFilter) -> Self {
        match f {
            SearchFilter::All => SearchFilterDto::All,
            SearchFilter::Songs => SearchFilterDto::Songs,
            SearchFilter::Videos => SearchFilterDto::Videos,
            SearchFilter::Albums => SearchFilterDto::Albums,
            SearchFilter::Artists => SearchFilterDto::Artists,
            SearchFilter::Playlists => SearchFilterDto::Playlists,
        }
    }
}

impl From<&SearchItem> for SearchItemDto {
    fn from(i: &SearchItem) -> Self {
        match i {
            SearchItem::Track(t) => SearchItemDto::Track {
                track: TrackDto {
                    key: t.key.clone(),
                    title: t.title.clone(),
                    artist: t.artist.clone(),
                    duration_ms: t.duration_ms,
                    thumbnail: t.thumbnail.clone(),
                },
            },
            SearchItem::Playlist {
                key,
                title,
                uploader,
                thumbnail,
            } => SearchItemDto::Playlist {
                key: key.clone(),
                title: title.clone(),
                uploader: uploader.clone(),
                thumbnail: thumbnail.clone(),
            },
            SearchItem::Artist {
                key,
                name,
                thumbnail,
            } => SearchItemDto::Artist {
                key: key.clone(),
                name: name.clone(),
                thumbnail: thumbnail.clone(),
            },
        }
    }
}

// ------------------------------------------------------------------ library

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlaylistSummaryDto {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    #[ts(type = "number")]
    pub track_count: usize,
    #[ts(type = "number")]
    pub updated_at: i64,
    pub thumbnail: Option<String>,
}

/// What the UI keeps in sync with the library: which songs are liked and the playlist list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LibrarySnapshotDto {
    /// Increases with every change, so the UI can tell when a detail view is out of date.
    #[ts(type = "number")]
    pub revision: u64,
    pub liked_keys: Vec<String>,
    pub playlists: Vec<PlaylistSummaryDto>,
    #[ts(type = "number")]
    pub history_count: usize,
    /// Set when the library file could not be opened: the app then runs without saving.
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlaylistEntryDto {
    #[ts(type = "number")]
    pub entry_id: i64,
    pub track: TrackDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlaylistDto {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
    pub entries: Vec<PlaylistEntryDto>,
}

/// Handed back by delete so the UI can offer Undo by sending it to `playlist_restore`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlaylistSnapshotDto {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number")]
    pub updated_at: i64,
    pub tracks: Vec<TrackDto>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RemovedEntryDto {
    #[ts(type = "number")]
    pub playlist_id: i64,
    #[ts(type = "number")]
    pub position: usize,
    pub track: TrackDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AddResultDto {
    #[ts(type = "number")]
    pub added: usize,
    #[ts(type = "number")]
    pub skipped: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HistoryItemDto {
    pub track: TrackDto,
    #[ts(type = "number")]
    pub played_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HomeDto {
    pub quick_picks: Vec<TrackDto>,
    pub recent: Vec<TrackDto>,
    #[ts(type = "number")]
    pub liked_count: usize,
}

/// Where the log lives, so the UI can show it and open the folder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LogInfoDto {
    pub dir: String,
    pub file: String,
    /// Set if the log file could not be created (logging then goes to the console only).
    pub error: Option<String>,
}

/// A message for the listener that is not player state (a song was skipped, a stream was cut off).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NoticeDto {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum BackdropDto {
    /// Windows 11 Mica is active: the page background must be transparent.
    Mica,
    /// Solid fallback (Windows 10 or Mica unavailable).
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WindowInfoDto {
    pub backdrop: BackdropDto,
}

impl From<RepeatDto> for Repeat {
    fn from(r: RepeatDto) -> Self {
        match r {
            RepeatDto::Off => Repeat::Off,
            RepeatDto::All => Repeat::All,
            RepeatDto::One => Repeat::One,
        }
    }
}

impl From<Repeat> for RepeatDto {
    fn from(r: Repeat) -> Self {
        match r {
            Repeat::Off => RepeatDto::Off,
            Repeat::All => RepeatDto::All,
            Repeat::One => RepeatDto::One,
        }
    }
}

impl From<CommandDto> for Command {
    fn from(c: CommandDto) -> Self {
        match c {
            CommandDto::PlayEntry { id } => Command::PlayEntry(EntryId(id)),
            CommandDto::PlayTracks { tracks, start } => Command::PlayTracks {
                tracks: tracks.into_iter().map(Track::from).collect(),
                start,
            },
            CommandDto::Toggle => Command::Toggle,
            CommandDto::Pause => Command::Pause,
            CommandDto::Resume => Command::Resume,
            CommandDto::Next => Command::Next,
            CommandDto::Previous => Command::Previous,
            CommandDto::Seek { ms } => Command::Seek(ms),
            CommandDto::Retry => Command::Retry,
            CommandDto::SetRepeat { repeat } => Command::SetRepeat(repeat.into()),
            CommandDto::SetShuffle { on } => Command::SetShuffle(on),
            CommandDto::SetAutoplay { on } => Command::SetAutoplay(on),
            CommandDto::AddToQueue { track } => Command::AddToQueue(track.into()),
            CommandDto::PlayNext { track } => Command::PlayNext(track.into()),
            CommandDto::Remove { id } => Command::Remove(EntryId(id)),
            CommandDto::Move { id, to } => Command::Move {
                id: EntryId(id),
                to,
            },
            CommandDto::MoveUpcoming { id, to } => Command::MoveUpcoming {
                id: EntryId(id),
                to,
            },
            CommandDto::StopAfterCurrent { on } => Command::StopAfterCurrent(on),
            CommandDto::Clear => Command::Clear,
            CommandDto::Undo => Command::Undo,
        }
    }
}

fn failure_dto(f: &Failure) -> FailureDto {
    match f {
        Failure::Offline => FailureDto::Offline,
        Failure::RateLimited => FailureDto::RateLimited,
        Failure::Unavailable => FailureDto::Unavailable,
        Failure::Decode => FailureDto::Decode,
        Failure::Expired(status) => FailureDto::Expired { status: *status },
        Failure::Other(m) => FailureDto::Other { message: m.clone() },
    }
}

fn transport_dto(t: &Transport) -> TransportDto {
    match t {
        Transport::Idle => TransportDto::Idle,
        Transport::Loading { id } => TransportDto::Loading { id: id.0 },
        Transport::Playing { id } => TransportDto::Playing { id: id.0 },
        Transport::Paused { id, loaded } => TransportDto::Paused {
            id: id.0,
            loaded: *loaded,
        },
        Transport::Stalled { id } => TransportDto::Stalled { id: id.0 },
        Transport::Failed { id, failure } => TransportDto::Failed {
            id: id.0,
            failure: failure_dto(failure),
        },
    }
}

pub fn state_dto(p: &Player) -> PlayerStateDto {
    let s = p.snapshot();
    let q = p.queue();
    PlayerStateDto {
        transport: transport_dto(&s.transport),
        audible: s.audible.map(|i| i.0),
        loading: s.loading.map(|i| i.0),
        position_ms: s.position_ms,
        repeat: s.repeat.into(),
        shuffle: s.shuffle,
        autoplay: s.autoplay,
        can_undo: s.can_undo,
        queue: q
            .entries()
            .iter()
            .map(|e| EntryDto {
                id: e.id.0,
                track: (&e.track).into(),
                origin: match e.origin {
                    Origin::User => OriginDto::User,
                    Origin::Radio => OriginDto::Radio,
                },
            })
            .collect(),
        current: q.current_id().map(|i| i.0),
        upcoming: q.upcoming().into_iter().map(|i| i.0).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_parse_from_the_json_the_ui_sends() {
        let c: CommandDto = serde_json::from_str(r#"{"type":"seek","ms":1500}"#).unwrap();
        assert_eq!(c, CommandDto::Seek { ms: 1500 });
        let c: CommandDto = serde_json::from_str(r#"{"type":"setRepeat","repeat":"one"}"#).unwrap();
        assert_eq!(Command::from(c), Command::SetRepeat(Repeat::One));
        let c: CommandDto = serde_json::from_str(
            r#"{"type":"playTracks","start":1,"tracks":[
              {"key":"a","title":"A","artist":"x","durationMs":1000,"thumbnail":null},
              {"key":"b","title":"B","artist":"y","durationMs":null,"thumbnail":null}]}"#,
        )
        .unwrap();
        match Command::from(c) {
            Command::PlayTracks { tracks, start } => {
                assert_eq!((tracks.len(), start), (2, 1));
                assert_eq!(tracks[0].duration_ms, Some(1000));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn state_serializes_with_the_shapes_the_ui_expects() {
        let mut p = Player::new(1);
        p.command(Command::PlayTracks {
            tracks: vec![Track::new("a", "A", "x"), Track::new("b", "B", "y")],
            start: 0,
        });
        let v = serde_json::to_value(state_dto(&p)).unwrap();
        assert_eq!(v["transport"]["state"], "loading");
        assert_eq!(v["positionMs"], 0);
        assert_eq!(v["repeat"], "off");
        assert_eq!(v["queue"].as_array().unwrap().len(), 2);
        assert_eq!(v["queue"][0]["origin"], "user");
        assert_eq!(v["audible"], serde_json::Value::Null);
        assert_eq!(v["current"], v["queue"][0]["id"]);
    }

    #[test]
    fn search_errors_carry_a_kind_the_ui_can_branch_on() {
        let e = SearchErrorDto::from(&SourceError::RateLimited);
        assert_eq!(serde_json::to_value(&e).unwrap()["kind"], "rateLimited");
        assert!(e.message.contains("limiting"));
        assert_eq!(
            SearchErrorDto::from(&SourceError::ToolMissing("yt-dlp".into())).kind,
            SearchErrorKind::ToolMissing
        );
        assert_eq!(
            SearchErrorDto::from(&SourceError::Failed("x".into())).kind,
            SearchErrorKind::Other
        );
    }

    #[test]
    fn source_tracks_convert_to_track_dtos_without_losing_fields() {
        let info = pyxee_sources::TrackInfo {
            key: "yt:abc".into(),
            title: "T".into(),
            artist: "A".into(),
            duration_ms: Some(2000),
            thumbnail: Some("u".into()),
        };
        let dto = TrackDto::from(&info);
        assert_eq!(
            (dto.key.as_str(), dto.duration_ms, dto.thumbnail.as_deref()),
            ("yt:abc", Some(2000), Some("u"))
        );
    }

    #[test]
    fn search_items_serialize_with_a_type_tag() {
        let t = SearchItem::Track(pyxee_sources::TrackInfo {
            key: "yt:abc".into(),
            title: "T".into(),
            artist: "A".into(),
            duration_ms: Some(1000),
            thumbnail: None,
        });
        let v = serde_json::to_value(SearchItemDto::from(&t)).unwrap();
        assert_eq!(v["type"], "track");
        assert_eq!(v["track"]["durationMs"], 1000);
        let a = SearchItem::Artist {
            key: "yt-channel:1".into(),
            name: "N".into(),
            thumbnail: None,
        };
        assert_eq!(
            serde_json::to_value(SearchItemDto::from(&a)).unwrap()["type"],
            "artist"
        );
    }

    #[test]
    fn filters_round_trip_through_the_dto() {
        for f in [
            SearchFilter::All,
            SearchFilter::Songs,
            SearchFilter::Videos,
            SearchFilter::Albums,
            SearchFilter::Artists,
            SearchFilter::Playlists,
        ] {
            assert_eq!(SearchFilter::from(SearchFilterDto::from(f)), f);
        }
    }

    #[test]
    fn failure_shapes_are_tagged() {
        let v = serde_json::to_value(TransportDto::Failed {
            id: 3,
            failure: FailureDto::Other {
                message: "boom".into(),
            },
        })
        .unwrap();
        assert_eq!(v["state"], "failed");
        assert_eq!(v["failure"]["kind"], "other");
        assert_eq!(v["failure"]["message"], "boom");
    }
}
