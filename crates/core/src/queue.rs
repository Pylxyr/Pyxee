//! The play queue: display order vs play order (shuffle), repeat, history, play-next,
//! and undoable removal. Pure data structure; the state machine drives it.

use crate::track::Track;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntryId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Origin {
    User,
    /// Added by autoplay radio.
    Radio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Repeat {
    #[default]
    Off,
    All,
    One,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub id: EntryId,
    pub track: Track,
    pub origin: Origin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UndoKind {
    Remove,
    Clear,
}

#[derive(Debug, Clone)]
enum Undo {
    Remove {
        entry: Entry,
        display_idx: usize,
        order_idx: usize,
    },
    Clear {
        entries: Vec<Entry>,
        order: Vec<EntryId>,
        current: Option<EntryId>,
    },
}

const HISTORY_CAP: usize = 200;
const UNDO_CAP: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Queue {
    /// What the user sees and can drag around.
    entries: Vec<Entry>,
    /// The order tracks are played in (same as `entries` unless shuffled).
    order: Vec<EntryId>,
    current: Option<EntryId>,
    /// Where the current entry was in `order` when it was removed; the next track slides into it.
    slot: Option<usize>,
    repeat: Repeat,
    shuffle: bool,
    history: Vec<EntryId>,
    next_id: u64,
    rng: u64,
    play_next_tail: Option<EntryId>,
    #[serde(skip)]
    undo: Vec<Undo>,
}

fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Queue {
    /// `seed` makes shuffle deterministic (tests) or random (the app passes the clock).
    pub fn new(seed: u64) -> Self {
        Self {
            entries: Vec::new(),
            order: Vec::new(),
            current: None,
            slot: None,
            repeat: Repeat::Off,
            shuffle: false,
            history: Vec::new(),
            next_id: 1,
            rng: seed,
            play_next_tail: None,
            undo: Vec::new(),
        }
    }

    // ---- reading ----
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn get(&self, id: EntryId) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }
    /// Changes a track in place (its id and position stay). Returns whether the entry exists.
    pub fn update_track(&mut self, id: EntryId, f: impl FnOnce(&mut Track)) -> bool {
        match self.entries.iter_mut().find(|e| e.id == id) {
            Some(e) => {
                f(&mut e.track);
                true
            }
            None => false,
        }
    }
    pub fn contains(&self, id: EntryId) -> bool {
        self.get(id).is_some()
    }
    pub fn current_id(&self) -> Option<EntryId> {
        self.current
    }
    pub fn current(&self) -> Option<&Entry> {
        self.current.and_then(|id| self.get(id))
    }
    pub fn repeat(&self) -> Repeat {
        self.repeat
    }
    pub fn shuffle(&self) -> bool {
        self.shuffle
    }
    pub fn play_order(&self) -> &[EntryId] {
        &self.order
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    fn current_pos(&self) -> Option<usize> {
        let c = self.current?;
        self.order.iter().position(|&i| i == c)
    }

    /// Entries after the current one in play order (not counting wrap-around).
    pub fn upcoming(&self) -> Vec<EntryId> {
        let start = match (self.current_pos(), self.slot) {
            (Some(p), _) => p + 1,
            (None, Some(s)) => s,
            (None, None) => 0,
        };
        self.order.iter().skip(start).copied().collect()
    }

    // ---- adding ----
    fn alloc(&mut self) -> EntryId {
        let id = EntryId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Undoing a clear puts the old list back over whatever was added since: once something is added,
    /// that clear can no longer be undone.
    fn forget_clear_undo(&mut self) {
        if matches!(self.undo.last(), Some(Undo::Clear { .. })) {
            self.undo.clear();
        }
    }

    pub fn add(&mut self, track: Track, origin: Origin) -> EntryId {
        self.forget_clear_undo();
        let id = self.alloc();
        self.entries.push(Entry { id, track, origin });
        self.order.push(id);
        id
    }

    /// Inserts right after the current track; consecutive calls keep their order
    /// (A, B, C added in turn play A, B, C).
    pub fn play_next(&mut self, track: Track) -> EntryId {
        self.forget_clear_undo();
        let cur_pos = self.current_pos();
        let tail = self
            .play_next_tail
            .and_then(|t| self.order.iter().position(|&i| i == t))
            .filter(|&tp| cur_pos.is_none_or(|cp| tp >= cp));
        let order_idx = match (tail, cur_pos, self.slot) {
            (Some(tp), _, _) => tp + 1,
            (None, Some(cp), _) => cp + 1,
            (None, None, Some(s)) => s.min(self.order.len()),
            (None, None, None) => 0,
        };
        let display_idx = if self.shuffle {
            let anchor = order_idx.checked_sub(1).map(|i| self.order[i]);
            anchor
                .and_then(|a| self.entries.iter().position(|e| e.id == a))
                .map_or(0, |p| p + 1)
        } else {
            order_idx
        };
        let id = self.alloc();
        self.entries.insert(
            display_idx.min(self.entries.len()),
            Entry {
                id,
                track,
                origin: Origin::User,
            },
        );
        self.order.insert(order_idx.min(self.order.len()), id);
        self.play_next_tail = Some(id);
        id
    }

    // ---- removing / reordering ----
    /// Removes an entry (undoable). Returns whether it was the current one.
    pub fn remove(&mut self, id: EntryId) -> Option<bool> {
        let display_idx = self.entries.iter().position(|e| e.id == id)?;
        let order_idx = self.order.iter().position(|&i| i == id)?;
        let entry = self.entries.remove(display_idx);
        self.order.remove(order_idx);
        let was_current = self.current == Some(id);
        if was_current {
            self.current = None;
            self.slot = Some(order_idx);
        } else if let Some(s) = self.slot {
            if order_idx < s {
                self.slot = Some(s - 1);
            }
        }
        self.history.retain(|&h| h != id);
        if self.play_next_tail == Some(id) {
            self.play_next_tail = None;
        }
        self.push_undo(Undo::Remove {
            entry,
            display_idx,
            order_idx,
        });
        Some(was_current)
    }

    pub fn clear(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        let undo = Undo::Clear {
            entries: std::mem::take(&mut self.entries),
            order: std::mem::take(&mut self.order),
            current: self.current.take(),
        };
        self.slot = None;
        self.play_next_tail = None;
        self.history.clear();
        self.push_undo(undo);
    }

    fn push_undo(&mut self, u: Undo) {
        self.undo.push(u);
        if self.undo.len() > UNDO_CAP {
            self.undo.remove(0);
        }
    }

    /// Reverts the last removal or clear. The restored entry is a normal queue item, not "current".
    pub fn undo(&mut self) -> Option<UndoKind> {
        match self.undo.pop()? {
            Undo::Remove {
                entry,
                display_idx,
                order_idx,
            } => {
                self.entries
                    .insert(display_idx.min(self.entries.len()), entry.clone());
                self.order.insert(order_idx.min(self.order.len()), entry.id);
                Some(UndoKind::Remove)
            }
            Undo::Clear {
                entries,
                order,
                current,
            } => {
                self.entries = entries;
                self.order = order;
                self.current = current;
                Some(UndoKind::Clear)
            }
        }
    }

    /// Moves an entry to a new display position. Under shuffle only the display order changes.
    pub fn move_item(&mut self, id: EntryId, to: usize) -> bool {
        let Some(from) = self.entries.iter().position(|e| e.id == id) else {
            return false;
        };
        let e = self.entries.remove(from);
        self.entries.insert(to.min(self.entries.len()), e);
        if !self.shuffle {
            self.order = self.entries.iter().map(|e| e.id).collect();
            self.slot = self.slot.map(|s| s.min(self.order.len()));
        }
        true
    }

    /// Moves an upcoming entry within the play order: `to` is its index in `upcoming()` (0 = plays
    /// right after the current song). Under shuffle only the play order changes; the list the user
    /// arranged (`entries`) stays as it is. The current and already-played entries cannot be moved
    /// this way. Returns whether anything moved.
    pub fn move_upcoming(&mut self, id: EntryId, to: usize) -> bool {
        let base = match (self.current_pos(), self.slot) {
            (Some(p), _) => p + 1,
            (None, Some(s)) => s,
            (None, None) => 0,
        };
        let Some(from) = self.order.iter().position(|&i| i == id) else {
            return false;
        };
        if from < base || Some(id) == self.current {
            return false;
        }
        self.order.remove(from);
        let at = (base + to).min(self.order.len());
        self.order.insert(at, id);
        if !self.shuffle {
            // Not shuffled: the visible list is the play order.
            if let Some(ef) = self.entries.iter().position(|e| e.id == id) {
                let e = self.entries.remove(ef);
                self.entries.insert(at.min(self.entries.len()), e);
            }
        }
        from != at
    }

    // ---- modes ----
    pub fn set_repeat(&mut self, r: Repeat) {
        self.repeat = r;
    }

    pub fn set_shuffle(&mut self, on: bool) {
        if on == self.shuffle {
            return;
        }
        self.shuffle = on;
        if on {
            let cur = self.current;
            let mut rest: Vec<EntryId> = self
                .entries
                .iter()
                .map(|e| e.id)
                .filter(|&i| Some(i) != cur)
                .collect();
            for i in (1..rest.len()).rev() {
                let j = (splitmix(&mut self.rng) % (i as u64 + 1)) as usize;
                rest.swap(i, j);
            }
            self.order = cur.into_iter().chain(rest).collect();
            self.slot = self.slot.map(|_| 0);
        } else {
            self.order = self.entries.iter().map(|e| e.id).collect();
            self.slot = self.slot.map(|_| {
                self.current_pos()
                    .map_or(0, |p| p + 1)
                    .min(self.order.len())
            });
        }
    }

    // ---- navigation ----
    /// The track that follows the current one. `natural` = the track ended by itself
    /// (repeat-one replays it); a user "Next" always moves on. `skip` hides unplayable entries.
    pub fn next_candidate(&self, natural: bool, skip: &dyn Fn(EntryId) -> bool) -> Option<EntryId> {
        let n = self.order.len();
        if n == 0 {
            return None;
        }
        if natural && self.repeat == Repeat::One {
            if let Some(c) = self.current {
                return Some(c);
            }
        }
        let start = match (self.current_pos(), self.slot) {
            (Some(p), _) => p + 1,
            (None, Some(s)) => s,
            (None, None) => 0,
        };
        for k in 0..n {
            let idx = if self.repeat == Repeat::All {
                (start + k) % n
            } else {
                start + k
            };
            if idx >= n {
                break;
            }
            let id = self.order[idx];
            if !skip(id) {
                return Some(id);
            }
        }
        None
    }

    fn set_current(&mut self, id: EntryId) {
        if let Some(c) = self.current {
            if c != id {
                self.history.push(c);
                if self.history.len() > HISTORY_CAP {
                    self.history.remove(0);
                }
            }
        }
        self.current = Some(id);
        self.slot = None;
    }

    /// Moves to the next track; returns it, or `None` at the end (position unchanged).
    pub fn advance(&mut self, natural: bool, skip: &dyn Fn(EntryId) -> bool) -> Option<EntryId> {
        let id = self.next_candidate(natural, skip)?;
        self.set_current(id);
        Some(id)
    }

    /// Goes back to the previously played track, if any.
    pub fn previous(&mut self, skip: &dyn Fn(EntryId) -> bool) -> Option<EntryId> {
        while let Some(id) = self.history.pop() {
            if self.contains(id) && !skip(id) {
                self.current = Some(id);
                self.slot = None;
                return Some(id);
            }
        }
        None
    }

    pub fn jump_to(&mut self, id: EntryId) -> bool {
        if !self.contains(id) {
            return false;
        }
        self.set_current(id);
        true
    }

    /// Replaces the queue with `tracks` and makes `start` current (the usual "play this list").
    pub fn replace(&mut self, tracks: Vec<Track>, start: usize) -> Option<EntryId> {
        self.entries.clear();
        self.order.clear();
        self.history.clear();
        self.undo.clear();
        self.current = None;
        self.slot = None;
        self.play_next_tail = None;
        let ids: Vec<EntryId> = tracks
            .into_iter()
            .map(|t| self.add(t, Origin::User))
            .collect();
        let start_id = *ids.get(start.min(ids.len().saturating_sub(1)))?;
        self.current = Some(start_id);
        if self.shuffle {
            self.shuffle = false;
            self.set_shuffle(true); // re-shuffle with the new current first
        }
        Some(start_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(n: &str) -> Track {
        Track::new(n, n, "artist")
    }

    fn q(names: &[&str]) -> (Queue, Vec<EntryId>) {
        let mut q = Queue::new(42);
        let ids = names.iter().map(|n| q.add(t(n), Origin::User)).collect();
        (q, ids)
    }

    fn names(q: &Queue) -> Vec<String> {
        q.entries().iter().map(|e| e.track.title.clone()).collect()
    }

    fn order_names(q: &Queue) -> Vec<String> {
        q.play_order()
            .iter()
            .map(|&i| q.get(i).unwrap().track.title.clone())
            .collect()
    }

    const NO_SKIP: &dyn Fn(EntryId) -> bool = &|_| false;

    #[test]
    fn advance_walks_the_queue_and_stops_at_the_end() {
        let (mut q, ids) = q(&["a", "b", "c"]);
        assert!(q.jump_to(ids[0]));
        assert_eq!(q.advance(true, NO_SKIP), Some(ids[1]));
        assert_eq!(q.advance(true, NO_SKIP), Some(ids[2]));
        assert_eq!(q.advance(true, NO_SKIP), None);
        assert_eq!(q.current_id(), Some(ids[2]), "unchanged at the end");
    }

    #[test]
    fn repeat_all_wraps_and_repeat_one_replays_only_on_natural_end() {
        let (mut q, ids) = q(&["a", "b"]);
        q.jump_to(ids[1]);
        q.set_repeat(Repeat::All);
        assert_eq!(q.next_candidate(true, NO_SKIP), Some(ids[0]));
        q.set_repeat(Repeat::One);
        assert_eq!(
            q.next_candidate(true, NO_SKIP),
            Some(ids[1]),
            "natural end replays"
        );
        assert_eq!(
            q.next_candidate(false, NO_SKIP),
            None,
            "user Next moves on (end of queue)"
        );
        q.jump_to(ids[0]);
        assert_eq!(q.next_candidate(false, NO_SKIP), Some(ids[1]));
    }

    #[test]
    fn single_track_repeat_all_replays_it() {
        let (mut q, ids) = q(&["a"]);
        q.jump_to(ids[0]);
        q.set_repeat(Repeat::All);
        assert_eq!(q.next_candidate(true, NO_SKIP), Some(ids[0]));
    }

    #[test]
    fn skip_predicate_hides_unplayable_entries() {
        let (mut q, ids) = q(&["a", "b", "c"]);
        q.jump_to(ids[0]);
        let b = ids[1];
        let skip_b = move |i: EntryId| i == b;
        assert_eq!(q.advance(true, &skip_b), Some(ids[2]));
    }

    #[test]
    fn previous_follows_history() {
        let (mut q, ids) = q(&["a", "b", "c"]);
        q.jump_to(ids[0]);
        q.advance(true, NO_SKIP);
        q.advance(true, NO_SKIP);
        assert_eq!(q.previous(NO_SKIP), Some(ids[1]));
        assert_eq!(q.previous(NO_SKIP), Some(ids[0]));
        assert_eq!(q.previous(NO_SKIP), None);
    }

    #[test]
    fn play_next_inserts_after_current_and_keeps_call_order() {
        let (mut q, ids) = q(&["a", "b", "c"]);
        q.jump_to(ids[0]);
        q.play_next(t("x"));
        q.play_next(t("y"));
        assert_eq!(names(&q), ["a", "x", "y", "b", "c"]);
        assert_eq!(order_names(&q), ["a", "x", "y", "b", "c"]);
    }

    #[test]
    fn play_next_after_advancing_goes_after_the_new_current() {
        let (mut q, ids) = q(&["a", "b", "c"]);
        q.jump_to(ids[0]);
        q.play_next(t("x"));
        q.advance(true, NO_SKIP); // now on x
        q.play_next(t("y"));
        assert_eq!(names(&q), ["a", "x", "y", "b", "c"]);
    }

    #[test]
    fn play_next_with_nothing_playing_goes_first() {
        let (mut q, _) = q(&["a", "b"]);
        q.play_next(t("x"));
        assert_eq!(names(&q), ["x", "a", "b"]);
    }

    #[test]
    fn removing_before_current_keeps_current() {
        let (mut q, ids) = q(&["a", "b", "c"]);
        q.jump_to(ids[2]);
        assert_eq!(q.remove(ids[0]), Some(false));
        assert_eq!(q.current_id(), Some(ids[2]));
    }

    #[test]
    fn removing_current_lets_the_next_track_slide_in() {
        let (mut q, ids) = q(&["a", "b", "c"]);
        q.jump_to(ids[1]);
        assert_eq!(q.remove(ids[1]), Some(true));
        assert_eq!(q.current_id(), None);
        assert_eq!(q.next_candidate(false, NO_SKIP), Some(ids[2]));
    }

    #[test]
    fn removing_the_last_current_leaves_nothing_next() {
        let (mut q, ids) = q(&["a", "b"]);
        q.jump_to(ids[1]);
        q.remove(ids[1]);
        assert_eq!(q.next_candidate(false, NO_SKIP), None);
    }

    #[test]
    fn undo_restores_position() {
        let (mut q, ids) = q(&["a", "b", "c"]);
        q.remove(ids[1]);
        assert_eq!(names(&q), ["a", "c"]);
        assert_eq!(q.undo(), Some(UndoKind::Remove));
        assert_eq!(names(&q), ["a", "b", "c"]);
        assert_eq!(order_names(&q), ["a", "b", "c"]);
        assert_eq!(q.undo(), None);
    }

    #[test]
    fn clear_is_undoable() {
        let (mut q, ids) = q(&["a", "b"]);
        q.jump_to(ids[1]);
        q.clear();
        assert!(q.is_empty());
        assert_eq!(q.undo(), Some(UndoKind::Clear));
        assert_eq!(names(&q), ["a", "b"]);
        assert_eq!(q.current_id(), Some(ids[1]));
    }

    #[test]
    fn move_item_reorders() {
        let (mut q, ids) = q(&["a", "b", "c"]);
        q.move_item(ids[0], 2);
        assert_eq!(names(&q), ["b", "c", "a"]);
        assert_eq!(order_names(&q), ["b", "c", "a"]);
    }

    #[test]
    fn shuffle_keeps_current_first_and_is_a_permutation() {
        let (mut q, ids) = q(&["a", "b", "c", "d", "e", "f"]);
        q.jump_to(ids[2]);
        q.set_shuffle(true);
        assert_eq!(q.play_order()[0], ids[2]);
        let mut sorted = q.play_order().to_vec();
        sorted.sort();
        let mut all = ids.clone();
        all.sort();
        assert_eq!(sorted, all);
        assert_eq!(
            names(&q),
            ["a", "b", "c", "d", "e", "f"],
            "display order unchanged"
        );
    }

    #[test]
    fn shuffle_is_deterministic_for_a_seed() {
        let (mut a, ia) = q(&["a", "b", "c", "d", "e"]);
        let (mut b, _) = q(&["a", "b", "c", "d", "e"]);
        a.jump_to(ia[0]);
        b.jump_to(ia[0]);
        a.set_shuffle(true);
        b.set_shuffle(true);
        assert_eq!(a.play_order(), b.play_order());
    }

    #[test]
    fn shuffle_off_restores_order_around_the_current_track() {
        let (mut q, ids) = q(&["a", "b", "c", "d"]);
        q.jump_to(ids[1]);
        q.set_shuffle(true);
        q.set_shuffle(false);
        assert_eq!(order_names(&q), ["a", "b", "c", "d"]);
        assert_eq!(q.next_candidate(false, NO_SKIP), Some(ids[2]));
    }

    #[test]
    fn play_next_under_shuffle_plays_next_in_play_order() {
        let (mut q, ids) = q(&["a", "b", "c", "d"]);
        q.jump_to(ids[0]);
        q.set_shuffle(true);
        let x = q.play_next(t("x"));
        assert_eq!(q.next_candidate(false, NO_SKIP), Some(x));
    }

    #[test]
    fn replace_sets_current_and_clears_history() {
        let (mut q, ids) = q(&["old1", "old2"]);
        q.jump_to(ids[0]);
        let start = q.replace(vec![t("a"), t("b"), t("c")], 1).unwrap();
        assert_eq!(q.current().unwrap().track.title, "b");
        assert_eq!(q.current_id(), Some(start));
        assert_eq!(q.previous(NO_SKIP), None);
        assert_eq!(names(&q), ["a", "b", "c"]);
    }

    #[test]
    fn upcoming_counts_tracks_after_current() {
        let (mut q, ids) = q(&["a", "b", "c"]);
        q.jump_to(ids[0]);
        assert_eq!(q.upcoming(), vec![ids[1], ids[2]]);
        q.jump_to(ids[2]);
        assert!(q.upcoming().is_empty());
    }

    #[test]
    fn survives_serialization() {
        let (mut q, ids) = q(&["a", "b", "c"]);
        q.jump_to(ids[1]);
        q.set_repeat(Repeat::All);
        q.set_shuffle(true);
        let json = serde_json::to_string(&q).unwrap();
        let back: Queue = serde_json::from_str(&json).unwrap();
        assert_eq!(back.current_id(), q.current_id());
        assert_eq!(back.play_order(), q.play_order());
        assert_eq!(back.repeat(), Repeat::All);
        assert!(back.shuffle());
    }

    #[test]
    fn adding_after_a_clear_forgets_the_clear_so_undo_cannot_overwrite_the_new_songs() {
        let (mut queue, _) = q(&["a", "b"]);
        queue.clear();
        assert!(queue.can_undo());
        queue.add(t("c"), Origin::User);
        assert!(!queue.can_undo(), "add");

        let (mut queue, _) = q(&["a", "b"]);
        queue.clear();
        queue.play_next(t("c"));
        assert!(!queue.can_undo(), "play_next");

        // A removal made after a clear is still undoable, and a clear followed by nothing is too.
        let (mut queue, ids) = q(&["a", "b"]);
        queue.remove(ids[0]);
        queue.add(t("c"), Origin::User);
        assert!(queue.can_undo(), "only a clear is forgotten");
    }
}
