//! Metadata-only decisions for cached UI pane addresses.
//!
//! Addresses are plain integers. Nothing in this crate reads or writes the
//! memory they name, so tests can use values that are not mapped.
//!
//! The plugin retirement path is `classify_finalize` then `apply_finalize`.
//! `apply_finalize` is what calls `evict_matching_slots` or `reset_slots`.
//! Cache-match reset must not clear `CoverageWatch`; that list stays until
//! `note_finalize` reports the same integers.
//!
//! `userdata_probe` formats the diagnostic getter record. It does not read
//! pane memory.

pub mod userdata_probe;

/// Matches `MAX_RESOLVED_SKIN_ELEMENTS` in the plugin.
pub const MAX_WATCHED_CHILDREN: usize = 32;

/// Captured pane identities kept past a cache reset. Overflow is explicit.
pub const WATCHLIST_CAP: usize = 128;

/// Pane addresses remembered for one cached skin entry.
///
/// `layout_root == 0` means this cache has no layout-root pane.
/// A retiring address of `0` never matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WatchedPointers {
    root: u64,
    layout_root: u64,
    children: [u64; MAX_WATCHED_CHILDREN],
    child_count: usize,
}

impl Default for WatchedPointers {
    fn default() -> Self {
        Self::new()
    }
}

impl WatchedPointers {
    pub const fn new() -> Self {
        Self {
            root: 0,
            layout_root: 0,
            children: [0; MAX_WATCHED_CHILDREN],
            child_count: 0,
        }
    }

    pub fn set_root(&mut self, address: u64) {
        self.root = address;
    }

    pub fn set_layout_root(&mut self, address: u64) {
        self.layout_root = address;
    }

    pub fn push_child(&mut self, address: u64) -> bool {
        if self.child_count >= MAX_WATCHED_CHILDREN {
            return false;
        }
        self.children[self.child_count] = address;
        self.child_count += 1;
        true
    }

    pub fn root(&self) -> u64 {
        self.root
    }

    pub fn child(&self, index: usize) -> Option<u64> {
        self.children.get(index).copied()
    }

    pub fn child_count(&self) -> usize {
        self.child_count
    }

    /// Compare the retiring address to the skin root, each child, and the
    /// layout root when one was stored. This does not follow pointers.
    pub fn contains(&self, retiring: u64) -> bool {
        if retiring == 0 {
            return false;
        }
        if self.root == retiring || self.layout_root == retiring {
            return true;
        }
        self.children[..self.child_count].contains(&retiring)
    }
}

/// Forget every slot whose watched root, child, or layout root is `retiring`.
/// Matching slots are dropped whole. Unrelated slots stay. No callback is
/// invoked, so callers cannot hide or read a pane from here.
pub fn evict_matching_slots<T>(
    slots: &mut [Option<T>],
    retiring: u64,
    watch: impl Fn(&T) -> WatchedPointers,
) -> usize {
    let mut cleared = 0;
    for slot in slots.iter_mut() {
        let matches = slot
            .as_ref()
            .is_some_and(|value| watch(value).contains(retiring));
        if matches {
            *slot = None;
            cleared += 1;
        }
    }
    cleared
}

/// Drop every slot. The signature has no pane-write callback.
pub fn reset_slots<T>(slots: &mut [Option<T>]) -> usize {
    let cleared = slots.iter().filter(|slot| slot.is_some()).count();
    for slot in slots.iter_mut() {
        *slot = None;
    }
    cleared
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotKind {
    Resolved,
    Missing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissingHandleInvalidated;

/// Resolved entries may resolve the new skin from the already-live root.
/// Missing entries are invalidated and must not call a remembered-handle resolver.
pub fn apply_skin_swap<T>(
    kind: SlotKind,
    resolve_from_live_root: impl FnOnce() -> T,
) -> Result<T, MissingHandleInvalidated> {
    match kind {
        SlotKind::Missing => Err(MissingHandleInvalidated),
        SlotKind::Resolved => Ok(resolve_from_live_root()),
    }
}

/// What a pane-finalize observer should do with one cache.
///
/// `classify_finalize` does not mutate slots and does not look at thread
/// identity. A match drops that whole entry. An unrelated address stays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinalizeClass {
    Unrelated,
    EvictMatching,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FinalizeTouch {
    pub class: FinalizeClass,
    pub cleared: usize,
}

pub fn classify_finalize<T>(
    slots: &[Option<T>],
    retiring: u64,
    watch: impl Fn(&T) -> WatchedPointers,
) -> FinalizeClass {
    let matched = slots
        .iter()
        .flatten()
        .any(|value| watch(value).contains(retiring));
    if matched {
        FinalizeClass::EvictMatching
    } else {
        FinalizeClass::Unrelated
    }
}

/// Apply a class already returned by `classify_finalize`.
///
/// `EvictMatching` drops only whole entries that contain `retiring`.
/// `Unrelated` writes nothing.
pub fn apply_finalize<T>(
    slots: &mut [Option<T>],
    retiring: u64,
    class: FinalizeClass,
    watch: impl Fn(&T) -> WatchedPointers,
) -> usize {
    match class {
        FinalizeClass::Unrelated => 0,
        FinalizeClass::EvictMatching => evict_matching_slots(slots, retiring, watch),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EpochPublish {
    Publish,
    Rejected,
}

/// Capture publishes a fresh resolution only when no Pane::Finalize began
/// after the snapshot. Production calls this helper; tests call the same one.
pub fn publish_after_epoch(snapshot: u64, current: u64) -> EpochPublish {
    if snapshot == current {
        EpochPublish::Publish
    } else {
        EpochPublish::Rejected
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureNote {
    Stored,
    AlreadyLive,
    IgnoredNull,
    Overflow,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WatchRecord {
    address: u64,
    owner_token: u64,
    retired: bool,
}

/// Address-only coverage list. It survives cache reset because nothing in this
/// type clears it except a later `note_finalize` marking one address retired.
#[derive(Clone, Copy, Debug)]
pub struct CoverageWatch {
    records: [WatchRecord; WATCHLIST_CAP],
    stored: u64,
    retired_matches: u64,
    alias_observations: u64,
    overflow: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WatchSummary {
    pub stored: u64,
    pub retired_matches: u64,
    pub alias_observations: u64,
    pub overflow: bool,
    pub live: usize,
    pub live_sample: [u64; 8],
    pub live_sample_len: usize,
}

impl CoverageWatch {
    pub const fn new() -> Self {
        Self {
            records: [WatchRecord {
                address: 0,
                owner_token: 0,
                retired: false,
            }; WATCHLIST_CAP],
            stored: 0,
            retired_matches: 0,
            alias_observations: 0,
            overflow: false,
        }
    }

    pub fn note_capture(&mut self, address: u64, owner_token: u64) -> CaptureNote {
        if address == 0 {
            return CaptureNote::IgnoredNull;
        }
        if let Some(existing) = self
            .records
            .iter_mut()
            .find(|record| !record.retired && record.address == address)
        {
            if existing.owner_token != owner_token {
                self.alias_observations += 1;
            }
            return CaptureNote::AlreadyLive;
        }
        let Some(slot) = self
            .records
            .iter_mut()
            .find(|record| record.address == 0 || record.retired)
        else {
            self.overflow = true;
            return CaptureNote::Overflow;
        };
        *slot = WatchRecord {
            address,
            owner_token,
            retired: false,
        };
        self.stored += 1;
        CaptureNote::Stored
    }

    /// Mark matching live records retired. Does not follow `address`.
    pub fn note_finalize(&mut self, address: u64) -> usize {
        if address == 0 {
            return 0;
        }
        let mut matched = 0;
        for record in &mut self.records {
            if !record.retired && record.address == address {
                record.retired = true;
                matched += 1;
                self.retired_matches += 1;
            }
        }
        matched
    }

    pub fn summary(&self) -> WatchSummary {
        let mut live_sample = [0u64; 8];
        let mut live_sample_len = 0;
        let mut live = 0;
        for record in &self.records {
            if record.retired || record.address == 0 {
                continue;
            }
            live += 1;
            if live_sample_len < live_sample.len() {
                live_sample[live_sample_len] = record.address;
                live_sample_len += 1;
            }
        }
        WatchSummary {
            stored: self.stored,
            retired_matches: self.retired_matches,
            alias_observations: self.alias_observations,
            overflow: self.overflow,
            live,
            live_sample,
            live_sample_len,
        }
    }
}

impl Default for CoverageWatch {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn child_entry(root: u64, child: u64) -> WatchedPointers {
        let mut watched = WatchedPointers::new();
        watched.set_root(root);
        assert!(watched.push_child(child));
        watched
    }

    fn retire(slots: &mut [Option<WatchedPointers>], retiring: u64) -> FinalizeTouch {
        let class = classify_finalize(slots, retiring, |watched| *watched);
        let cleared = apply_finalize(slots, retiring, class, |watched| *watched);
        FinalizeTouch { class, cleared }
    }

    #[test]
    fn unreadable_addresses_are_compared_without_loads() {
        let retiring = 0x200_0000;
        let child = 0x400_0000_0000;
        let mut slots = [Some(child_entry(retiring, child))];
        let touch = retire(&mut slots, retiring);
        assert_eq!(touch.class, FinalizeClass::EvictMatching);
        assert_eq!(touch.cleared, 1);
        assert!(slots[0].is_none());

        let mut watch = CoverageWatch::new();
        assert_eq!(watch.note_capture(retiring, 1), CaptureNote::Stored);
        assert_eq!(watch.note_capture(child, 1), CaptureNote::Stored);
        assert_eq!(watch.note_finalize(retiring), 1);
        assert_eq!(watch.summary().live, 1);
    }

    #[test]
    fn retiring_child_or_root_or_layout_root_drops_the_whole_entry() {
        let mut slots = [Some(child_entry(0x1000, 0x2000)), None, None];
        assert_eq!(retire(&mut slots, 0x2000).cleared, 1);
        assert!(slots[0].is_none());

        let mut with_layout_root = WatchedPointers::new();
        with_layout_root.set_root(0x10);
        with_layout_root.set_layout_root(0x30);
        assert!(with_layout_root.push_child(0x20));
        let mut slots = [Some(with_layout_root)];
        assert_eq!(retire(&mut slots, 0x30).cleared, 1);
        assert!(slots[0].is_none());

        let mut slots = [Some(child_entry(0x1000, 0x2000))];
        assert_eq!(retire(&mut slots, 0x1000).cleared, 1);
        assert!(slots[0].is_none());
    }

    #[test]
    fn unrelated_pane_preserves_the_entry() {
        let original = child_entry(0x1000, 0x2000);
        let mut slots = [Some(original), Some(child_entry(0x3000, 0x4000))];
        let touch = retire(&mut slots, 0x9999);
        assert_eq!(touch.class, FinalizeClass::Unrelated);
        assert_eq!(touch.cleared, 0);
        assert_eq!(slots[0], Some(original));
        assert_eq!(slots[1].unwrap().root(), 0x3000);
        assert_eq!(slots[1].unwrap().child(0), Some(0x4000));
    }

    #[test]
    fn null_retiring_address_clears_nothing() {
        let mut slots = [Some(child_entry(0, 0x20))];
        assert_eq!(retire(&mut slots, 0).cleared, 0);
        assert!(slots[0].is_some());
    }

    #[test]
    fn match_reset_clears_without_pane_writes() {
        let mut slots = [
            Some(child_entry(0x11, 0x22)),
            None,
            Some(child_entry(0x33, 0x44)),
        ];
        let cleared = reset_slots(&mut slots);
        assert_eq!(cleared, 2);
        assert!(slots.iter().all(Option::is_none));
    }

    #[test]
    fn reused_address_does_not_keep_old_children() {
        let mut slots = [Some(child_entry(0x1000, 0x2000))];
        assert_eq!(retire(&mut slots, 0x1000).cleared, 1);
        slots[0] = Some(child_entry(0x1000, 0x3000));
        let current = slots[0].unwrap();
        assert_eq!(current.child_count(), 1);
        assert_eq!(current.child(0), Some(0x3000));
        assert!(!current.contains(0x2000));
    }

    #[test]
    fn missing_handle_skin_swap_never_calls_resolver() {
        let mut calls = 0;
        let missing = apply_skin_swap(SlotKind::Missing, || {
            calls += 1;
            1
        });
        assert_eq!(missing, Err(MissingHandleInvalidated));
        assert_eq!(calls, 0);

        let resolved = apply_skin_swap(SlotKind::Resolved, || {
            calls += 1;
            7
        });
        assert_eq!(resolved, Ok(7));
        assert_eq!(calls, 1);
    }

    #[test]
    fn classify_does_not_mutate_before_apply() {
        let mut slots = [Some(child_entry(0x1000, 0x2000)), Some(child_entry(0x3000, 0x4000))];
        let class = classify_finalize(&slots, 0x2000, |watched| *watched);
        assert_eq!(class, FinalizeClass::EvictMatching);
        assert!(slots[0].is_some());
        assert!(slots[1].is_some());
        let cleared = apply_finalize(&mut slots, 0x2000, class, |watched| *watched);
        assert_eq!(cleared, 1);
        assert!(slots[0].is_none());
        assert_eq!(slots[1].unwrap().root(), 0x3000);
    }

    #[test]
    fn publish_after_epoch_rejects_a_finalize_that_started_during_resolve() {
        assert_eq!(publish_after_epoch(4, 4), EpochPublish::Publish);
        assert_eq!(publish_after_epoch(4, 5), EpochPublish::Rejected);
    }

    #[test]
    fn same_thread_evicts_only_the_matching_entry() {
        let mut slots = [Some(child_entry(0x1000, 0x2000)), Some(child_entry(0x3000, 0x4000))];
        let touch = retire(&mut slots, 0x1000);
        assert_eq!(touch.class, FinalizeClass::EvictMatching);
        assert!(slots[0].is_none());
        assert_eq!(slots[1].unwrap().root(), 0x3000);
    }

    #[test]
    fn watchlist_survives_until_finalize_and_reports_overflow_and_alias() {
        let mut watch = CoverageWatch::new();
        assert_eq!(watch.note_capture(0x2000_0000, 7), CaptureNote::Stored);
        let summary = watch.summary();
        assert_eq!(summary.live, 1);
        assert_eq!(summary.live_sample[0], 0x2000_0000);
        assert_eq!(summary.retired_matches, 0);

        assert_eq!(watch.note_capture(0x2000_0000, 8), CaptureNote::AlreadyLive);
        assert_eq!(watch.summary().alias_observations, 1);
        assert_eq!(watch.summary().retired_matches, 0);
        assert_eq!(watch.note_finalize(0x9999), 0);
        assert_eq!(watch.summary().live, 1);
        assert_eq!(watch.note_finalize(0x2000_0000), 1);
        assert_eq!(watch.summary().live, 0);
        assert_eq!(watch.summary().retired_matches, 1);

        let mut full = CoverageWatch::new();
        for address in 1..=WATCHLIST_CAP as u64 {
            assert_eq!(full.note_capture(address, 1), CaptureNote::Stored);
        }
        assert_eq!(full.note_capture(0xFFFF_FFFF, 1), CaptureNote::Overflow);
        assert!(full.summary().overflow);
        assert_eq!(full.summary().live, WATCHLIST_CAP);
    }
}
