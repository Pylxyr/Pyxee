//! The user's library, stored in SQLite: liked songs, playlists, listening history, and the data
//! behind Home's quick picks. Every method takes the current time as an argument, so behaviour is
//! deterministic and testable; nothing here reads a clock.

use pyxee_core::Track;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use std::collections::HashSet;
use std::path::Path;
use thiserror::Error;

pub const MAX_NAME_LEN: usize = 100;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
/// Plays older than this no longer influence quick picks.
const QUICK_PICK_WINDOW_DAYS: i64 = 30;

#[derive(Debug, Error)]
pub enum LibraryError {
    #[error("The library could not be read or written: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("This library was made by a newer Pyxee (database version {found}; this one understands up to {supported}). It was left exactly as it is; update Pyxee to use it.")]
    NewerVersion { found: i64, supported: i64 },
    #[error("Playlist names must be 1 to {MAX_NAME_LEN} characters.")]
    InvalidName,
    #[error("That {0} no longer exists.")]
    NotFound(&'static str),
}

type Result<T> = std::result::Result<T, LibraryError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistSummary {
    pub id: i64,
    pub name: String,
    pub track_count: usize,
    pub updated_at: i64,
    /// Artwork of the first track, used as the cover.
    pub thumbnail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistEntry {
    /// Identifies this occurrence of a track in this playlist (a track can appear twice).
    pub entry_id: i64,
    pub track: Track,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub entries: Vec<PlaylistEntry>,
}

/// Everything needed to bring a deleted playlist back exactly as it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistSnapshot {
    pub id: i64,
    pub name: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovedEntry {
    pub playlist_id: i64,
    pub position: usize,
    pub track: Track,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddResult {
    pub added: usize,
    /// Already in the playlist (or repeated in the request).
    pub skipped: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryItem {
    pub track: Track,
    pub played_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeData {
    pub quick_picks: Vec<Track>,
    pub recent: Vec<Track>,
    pub liked_count: usize,
}

pub struct Library {
    conn: Connection,
}

/// The schema version this build understands (`PRAGMA user_version`).
const SCHEMA_VERSION: i64 = 1;

const SCHEMA_V1: &str = "
CREATE TABLE tracks (
    key TEXT PRIMARY KEY, title TEXT NOT NULL, artist TEXT NOT NULL,
    duration_ms INTEGER, thumbnail TEXT
);
CREATE TABLE likes (
    track_key TEXT PRIMARY KEY REFERENCES tracks(key), liked_at INTEGER NOT NULL
);
CREATE TABLE playlists (
    id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL,
    created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
);
CREATE TABLE playlist_entries (
    entry_id INTEGER PRIMARY KEY AUTOINCREMENT,
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    track_key TEXT NOT NULL REFERENCES tracks(key),
    position INTEGER NOT NULL
);
CREATE INDEX idx_playlist_entries ON playlist_entries(playlist_id, position);
CREATE TABLE history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    track_key TEXT NOT NULL REFERENCES tracks(key), played_at INTEGER NOT NULL
);
CREATE INDEX idx_history_time ON history(played_at);
";

fn track_from_row(r: &rusqlite::Row<'_>, at: usize) -> rusqlite::Result<Track> {
    Ok(Track {
        key: r.get(at)?,
        title: r.get(at + 1)?,
        artist: r.get(at + 2)?,
        duration_ms: r.get::<_, Option<i64>>(at + 3)?.map(|v| v as u64),
        thumbnail: r.get(at + 4)?,
    })
}

const TRACK_COLS: &str = "t.key, t.title, t.artist, t.duration_ms, t.thumbnail";

fn upsert_track(tx: &Transaction<'_>, t: &Track) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO tracks (key, title, artist, duration_ms, thumbnail) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(key) DO UPDATE SET title = excluded.title,
            artist = CASE WHEN trim(excluded.artist) = '' OR lower(excluded.artist) = lower(?6)
                          THEN tracks.artist ELSE excluded.artist END,
            duration_ms = COALESCE(excluded.duration_ms, tracks.duration_ms),
            thumbnail = COALESCE(excluded.thumbnail, tracks.thumbnail)",
        params![
            t.key,
            t.title,
            t.artist,
            t.duration_ms.map(|v| v as i64),
            t.thumbnail,
            pyxee_core::UNKNOWN_ARTIST
        ],
    )?;
    Ok(())
}

fn clean_name(name: &str) -> Result<String> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > MAX_NAME_LEN {
        return Err(LibraryError::InvalidName);
    }
    Ok(n.to_string())
}

fn touch(tx: &Transaction<'_>, playlist_id: i64, now: i64) -> Result<()> {
    let n = tx.execute(
        "UPDATE playlists SET updated_at = ?2 WHERE id = ?1",
        params![playlist_id, now],
    )?;
    if n == 0 {
        return Err(LibraryError::NotFound("playlist"));
    }
    Ok(())
}

fn playlist_len(tx: &Transaction<'_>, id: i64) -> rusqlite::Result<usize> {
    tx.query_row(
        "SELECT COUNT(*) FROM playlist_entries WHERE playlist_id = ?1",
        [id],
        |r| r.get::<_, i64>(0),
    )
    .map(|n| n as usize)
}

impl Library {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| {
                LibraryError::Db(rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
            })?;
        }
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", true)?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        // A database from a newer Pyxee may have tables and columns this one does not know: writing to it could
        // damage it, so it is not opened at all (the app then runs on an empty library and says why).
        if version > SCHEMA_VERSION {
            return Err(LibraryError::NewerVersion {
                found: version,
                supported: SCHEMA_VERSION,
            });
        }
        let _ = conn.pragma_update(None, "journal_mode", "WAL"); // not supported for in-memory databases
        if version < 1 {
            if let Err(e) = conn.execute_batch(&format!(
                "BEGIN; {SCHEMA_V1} PRAGMA user_version = 1; COMMIT;"
            )) {
                // A batch that stops half way leaves its transaction open: close it, so nothing is half made.
                let _ = conn.execute_batch("ROLLBACK");
                return Err(e.into());
            }
        }
        Ok(Self { conn })
    }

    /// A song already in the library learned its real artist, length or cover (from its stream's
    /// metadata): store that. Never adds a song; returns whether a row changed.
    pub fn refresh_track(&mut self, t: &Track) -> Result<bool> {
        let n = self.conn.execute(
            "UPDATE tracks SET artist = ?2, duration_ms = ?3, thumbnail = ?4
             WHERE key = ?1 AND (artist IS NOT ?2 OR duration_ms IS NOT ?3 OR thumbnail IS NOT ?4)",
            params![
                t.key,
                t.artist,
                t.duration_ms.map(|v| v as i64),
                t.thumbnail
            ],
        )?;
        Ok(n > 0)
    }

    // ---------------------------------------------------------------- likes
    /// Returns true if the song was newly liked.
    pub fn like(&mut self, t: &Track, now: i64) -> Result<bool> {
        let tx = self.conn.transaction()?;
        upsert_track(&tx, t)?;
        let n = tx.execute(
            "INSERT OR IGNORE INTO likes (track_key, liked_at) VALUES (?1, ?2)",
            params![t.key, now],
        )?;
        tx.commit()?;
        Ok(n > 0)
    }

    /// Returns true if the song was liked before.
    pub fn unlike(&mut self, key: &str) -> Result<bool> {
        Ok(self
            .conn
            .execute("DELETE FROM likes WHERE track_key = ?1", [key])?
            > 0)
    }

    pub fn is_liked(&self, key: &str) -> Result<bool> {
        Ok(self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM likes WHERE track_key = ?1)",
            [key],
            |r| r.get(0),
        )?)
    }

    /// Most recently liked first.
    pub fn liked(&self) -> Result<Vec<Track>> {
        let sql = format!("SELECT {TRACK_COLS} FROM likes l JOIN tracks t ON t.key = l.track_key ORDER BY l.liked_at DESC, t.key");
        let mut st = self.conn.prepare(&sql)?;
        let rows = st.query_map([], |r| track_from_row(r, 0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn liked_keys(&self) -> Result<Vec<String>> {
        let mut st = self
            .conn
            .prepare("SELECT track_key FROM likes ORDER BY liked_at DESC, track_key")?;
        let rows = st.query_map([], |r| r.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    // ------------------------------------------------------------ playlists
    /// Creates a playlist, optionally filled with `tracks` (kept as given, repeats included).
    pub fn create_playlist(&mut self, name: &str, tracks: &[Track], now: i64) -> Result<i64> {
        let name = clean_name(name)?;
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO playlists (name, created_at, updated_at) VALUES (?1, ?2, ?2)",
            params![name, now],
        )?;
        let id = tx.last_insert_rowid();
        for (i, t) in tracks.iter().enumerate() {
            upsert_track(&tx, t)?;
            tx.execute(
                "INSERT INTO playlist_entries (playlist_id, track_key, position) VALUES (?1, ?2, ?3)",
                params![id, t.key, i as i64],
            )?;
        }
        tx.commit()?;
        Ok(id)
    }

    pub fn rename_playlist(&mut self, id: i64, name: &str, now: i64) -> Result<()> {
        let name = clean_name(name)?;
        let tx = self.conn.transaction()?;
        let n = tx.execute(
            "UPDATE playlists SET name = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, name, now],
        )?;
        if n == 0 {
            return Err(LibraryError::NotFound("playlist"));
        }
        tx.commit()?;
        Ok(())
    }

    /// Appends tracks that are not already in the playlist.
    pub fn add_to_playlist(&mut self, id: i64, tracks: &[Track], now: i64) -> Result<AddResult> {
        let tx = self.conn.transaction()?;
        touch(&tx, id, now)?;
        let mut present: HashSet<String> = {
            let mut st =
                tx.prepare("SELECT track_key FROM playlist_entries WHERE playlist_id = ?1")?;
            let rows = st.query_map([id], |r| r.get::<_, String>(0))?;
            rows.collect::<rusqlite::Result<_>>()?
        };
        let mut pos = playlist_len(&tx, id)?;
        let (mut added, mut skipped) = (0, 0);
        for t in tracks {
            if !present.insert(t.key.clone()) {
                skipped += 1;
                continue;
            }
            upsert_track(&tx, t)?;
            tx.execute(
                "INSERT INTO playlist_entries (playlist_id, track_key, position) VALUES (?1, ?2, ?3)",
                params![id, t.key, pos as i64],
            )?;
            pos += 1;
            added += 1;
        }
        tx.commit()?;
        Ok(AddResult { added, skipped })
    }

    /// Deletes a playlist and returns what is needed to restore it (Undo).
    pub fn delete_playlist(&mut self, id: i64) -> Result<PlaylistSnapshot> {
        let p = self.playlist(id)?;
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM playlists WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(PlaylistSnapshot {
            id: p.id,
            name: p.name,
            created_at: p.created_at,
            updated_at: p.updated_at,
            tracks: p.entries.into_iter().map(|e| e.track).collect(),
        })
    }

    /// Brings a deleted playlist back with its original id, order and timestamps.
    pub fn restore_playlist(&mut self, s: &PlaylistSnapshot) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO playlists (id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params![s.id, s.name, s.created_at, s.updated_at],
        )?;
        for (i, t) in s.tracks.iter().enumerate() {
            upsert_track(&tx, t)?;
            tx.execute(
                "INSERT INTO playlist_entries (playlist_id, track_key, position) VALUES (?1, ?2, ?3)",
                params![s.id, t.key, i as i64],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Most recently changed first.
    pub fn playlists(&self) -> Result<Vec<PlaylistSummary>> {
        let mut st = self.conn.prepare(
            "SELECT p.id, p.name, p.updated_at,
                    (SELECT COUNT(*) FROM playlist_entries e WHERE e.playlist_id = p.id),
                    (SELECT t.thumbnail FROM playlist_entries e JOIN tracks t ON t.key = e.track_key
                      WHERE e.playlist_id = p.id ORDER BY e.position LIMIT 1)
             FROM playlists p ORDER BY p.updated_at DESC, p.id DESC",
        )?;
        let rows = st.query_map([], |r| {
            Ok(PlaylistSummary {
                id: r.get(0)?,
                name: r.get(1)?,
                updated_at: r.get(2)?,
                track_count: r.get::<_, i64>(3)? as usize,
                thumbnail: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn playlist(&self, id: i64) -> Result<Playlist> {
        let head = self
            .conn
            .query_row(
                "SELECT name, created_at, updated_at FROM playlists WHERE id = ?1",
                [id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?
            .ok_or(LibraryError::NotFound("playlist"))?;
        let sql = format!(
            "SELECT e.entry_id, {TRACK_COLS} FROM playlist_entries e JOIN tracks t ON t.key = e.track_key
             WHERE e.playlist_id = ?1 ORDER BY e.position"
        );
        let mut st = self.conn.prepare(&sql)?;
        let rows = st.query_map([id], |r| {
            Ok(PlaylistEntry {
                entry_id: r.get(0)?,
                track: track_from_row(r, 1)?,
            })
        })?;
        Ok(Playlist {
            id,
            name: head.0,
            created_at: head.1,
            updated_at: head.2,
            entries: rows.collect::<rusqlite::Result<_>>()?,
        })
    }

    /// Removes one entry (undoable with `restore_entry`).
    pub fn remove_entry(&mut self, entry_id: i64, now: i64) -> Result<RemovedEntry> {
        let tx = self.conn.transaction()?;
        let (pid, position, track) = tx
            .query_row(
                &format!(
                    "SELECT e.playlist_id, e.position, {TRACK_COLS} FROM playlist_entries e
                     JOIN tracks t ON t.key = e.track_key WHERE e.entry_id = ?1"
                ),
                [entry_id],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, i64>(1)? as usize,
                        track_from_row(r, 2)?,
                    ))
                },
            )
            .optional()?
            .ok_or(LibraryError::NotFound("playlist entry"))?;
        tx.execute(
            "DELETE FROM playlist_entries WHERE entry_id = ?1",
            [entry_id],
        )?;
        tx.execute(
            "UPDATE playlist_entries SET position = position - 1 WHERE playlist_id = ?1 AND position > ?2",
            params![pid, position as i64],
        )?;
        touch(&tx, pid, now)?;
        tx.commit()?;
        Ok(RemovedEntry {
            playlist_id: pid,
            position,
            track,
        })
    }

    /// Puts a removed entry back at its old position (clamped if the playlist shrank meanwhile).
    pub fn restore_entry(&mut self, r: &RemovedEntry, now: i64) -> Result<i64> {
        let tx = self.conn.transaction()?;
        touch(&tx, r.playlist_id, now)?;
        let pos = r.position.min(playlist_len(&tx, r.playlist_id)?);
        upsert_track(&tx, &r.track)?;
        tx.execute(
            "UPDATE playlist_entries SET position = position + 1 WHERE playlist_id = ?1 AND position >= ?2",
            params![r.playlist_id, pos as i64],
        )?;
        tx.execute(
            "INSERT INTO playlist_entries (playlist_id, track_key, position) VALUES (?1, ?2, ?3)",
            params![r.playlist_id, r.track.key, pos as i64],
        )?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(id)
    }

    /// Moves an entry so it ends up at index `to` (clamped), like dragging in a list.
    pub fn move_entry(&mut self, entry_id: i64, to: usize, now: i64) -> Result<()> {
        let tx = self.conn.transaction()?;
        let (pid, from) = tx
            .query_row(
                "SELECT playlist_id, position FROM playlist_entries WHERE entry_id = ?1",
                [entry_id],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)? as usize)),
            )
            .optional()?
            .ok_or(LibraryError::NotFound("playlist entry"))?;
        let to = to.min(playlist_len(&tx, pid)?.saturating_sub(1));
        if to != from {
            // Park the entry, shift the ones in between, then drop it into place.
            tx.execute(
                "UPDATE playlist_entries SET position = -1 WHERE entry_id = ?1",
                [entry_id],
            )?;
            if to > from {
                tx.execute(
                    "UPDATE playlist_entries SET position = position - 1 WHERE playlist_id = ?1 AND position > ?2 AND position <= ?3",
                    params![pid, from as i64, to as i64],
                )?;
            } else {
                tx.execute(
                    "UPDATE playlist_entries SET position = position + 1 WHERE playlist_id = ?1 AND position >= ?2 AND position < ?3",
                    params![pid, to as i64, from as i64],
                )?;
            }
            tx.execute(
                "UPDATE playlist_entries SET position = ?2 WHERE entry_id = ?1",
                params![entry_id, to as i64],
            )?;
        }
        touch(&tx, pid, now)?;
        tx.commit()?;
        Ok(())
    }

    // -------------------------------------------------------------- history
    pub fn record_play(&mut self, t: &Track, now: i64) -> Result<()> {
        let tx = self.conn.transaction()?;
        upsert_track(&tx, t)?;
        tx.execute(
            "INSERT INTO history (track_key, played_at) VALUES (?1, ?2)",
            params![t.key, now],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Every play, newest first.
    pub fn history(&self, limit: usize) -> Result<Vec<HistoryItem>> {
        let sql = format!(
            "SELECT {TRACK_COLS}, h.played_at FROM history h JOIN tracks t ON t.key = h.track_key
             ORDER BY h.played_at DESC, h.id DESC LIMIT ?1"
        );
        let mut st = self.conn.prepare(&sql)?;
        let rows = st.query_map([limit as i64], |r| {
            Ok(HistoryItem {
                track: track_from_row(r, 0)?,
                played_at: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// Each track once, by its most recent play.
    pub fn recent_tracks(&self, limit: usize) -> Result<Vec<Track>> {
        let sql = format!(
            "SELECT {TRACK_COLS} FROM tracks t JOIN
                (SELECT track_key, MAX(played_at) AS last, MAX(id) AS lastid FROM history GROUP BY track_key) h
                ON h.track_key = t.key
             ORDER BY h.last DESC, h.lastid DESC LIMIT ?1"
        );
        let mut st = self.conn.prepare(&sql)?;
        let rows = st.query_map([limit as i64], |r| track_from_row(r, 0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn history_count(&self) -> Result<usize> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM history", [], |r| r.get::<_, i64>(0))?
            as usize)
    }

    /// Empties the history and returns it so it can be restored (Undo).
    pub fn clear_history(&mut self) -> Result<Vec<HistoryItem>> {
        let all = self.history(usize::MAX >> 1)?;
        self.conn.execute("DELETE FROM history", [])?;
        Ok(all)
    }

    pub fn restore_history(&mut self, items: &[HistoryItem]) -> Result<()> {
        let tx = self.conn.transaction()?;
        for it in items.iter().rev() {
            upsert_track(&tx, &it.track)?;
            tx.execute(
                "INSERT INTO history (track_key, played_at) VALUES (?1, ?2)",
                params![it.track.key, it.played_at],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    // ----------------------------------------------------------------- home
    /// Quick picks: songs you play a lot lately and songs you like, with a nudge for recency.
    pub fn home(&self, now: i64, limit: usize) -> Result<HomeData> {
        let since = now - QUICK_PICK_WINDOW_DAYS * DAY_MS;
        let sql = format!(
            "SELECT {TRACK_COLS},
                    COALESCE((SELECT COUNT(*) FROM history h WHERE h.track_key = t.key AND h.played_at >= ?1), 0),
                    COALESCE((SELECT MAX(played_at) FROM history h WHERE h.track_key = t.key), 0),
                    EXISTS(SELECT 1 FROM likes l WHERE l.track_key = t.key)
             FROM tracks t"
        );
        let mut st = self.conn.prepare(&sql)?;
        let mut scored: Vec<(f64, i64, Track)> = st
            .query_map([since], |r| {
                let track = track_from_row(r, 0)?;
                let plays: i64 = r.get(5)?;
                let last: i64 = r.get(6)?;
                let liked: bool = r.get(7)?;
                let recency = if last >= since {
                    3.0 * (last - since) as f64 / (now - since).max(1) as f64
                } else {
                    0.0
                };
                let score = plays as f64 * 2.0
                    + if liked { 6.0 } else { 0.0 }
                    + if plays > 0 { recency } else { 0.0 };
                Ok((score, last, track))
            })?
            .collect::<rusqlite::Result<_>>()?;
        scored.retain(|(score, ..)| *score > 0.0);
        scored.sort_by(|a, b| {
            b.0.total_cmp(&a.0)
                .then(b.1.cmp(&a.1))
                .then(a.2.key.cmp(&b.2.key))
        });
        let liked_count = self
            .conn
            .query_row("SELECT COUNT(*) FROM likes", [], |r| r.get::<_, i64>(0))?
            as usize;
        Ok(HomeData {
            quick_picks: scored.into_iter().take(limit).map(|(_, _, t)| t).collect(),
            recent: self.recent_tracks(limit)?,
            liked_count,
        })
    }
}
