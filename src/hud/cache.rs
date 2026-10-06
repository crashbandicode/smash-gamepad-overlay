use sgpo_ui_cache_metadata::{
    apply_finalize, apply_skin_swap, classify_finalize, publish_after_epoch, reset_slots,
    EpochPublish, FinalizeClass, FinalizeTouch, SlotKind, WatchedPointers,
};
use skyline::nn::ui2d::Pane;
use std::cell::UnsafeCell;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::input::ControllerViewState;
use crate::logger::trace;
use crate::pane_lifetime::{self, RoutineKind};
use crate::pane_utils::{cstr_bytes_to_str, pane_is_user_allocated, pane_name_matches};
use crate::skin::{active_skin, BuiltInSkin};
use crate::ui::find_pane_by_name;
use crate::visual::{
    hide_visual_skin_pane, hide_visual_skin_root, update_visual_skin_pane,
    update_visual_skin_root_with_config, validate_skin_size, VisualRenderError,
    MAX_RESOLVED_SKIN_ELEMENTS,
};

use super::capture::{find_pane_in_layout_data, CapturedHudLayout, HudLayoutKind};

const HUD_CACHE_SLOTS: usize = 3;

const _: () = assert!(
    sgpo_ui_cache_metadata::MAX_WATCHED_CHILDREN == crate::visual::MAX_RESOLVED_SKIN_ELEMENTS
);

#[derive(Debug, Copy, Clone)]
struct ResolvedHudSkin {
    layout_data: u64,
    layout_kind: HudLayoutKind,
    skin_name: &'static str,
    skin_root: *mut Pane,
    panes: [*mut Pane; MAX_RESOLVED_SKIN_ELEMENTS],
    pane_count: usize,
}

#[derive(Debug, Copy, Clone)]
#[expect(
    clippy::large_enum_variant,
    reason = "bounded static inline pointer storage intentionally avoids allocation in hooks"
)]
enum HudCache {
    Empty,
    Resolved(ResolvedHudSkin),
    Missing {
        layout_data: u64,
        layout_kind: HudLayoutKind,
        skin_name: &'static str,
    },
}

impl HudCache {
    fn cache_key(&self) -> Option<(u64, HudLayoutKind, &'static str)> {
        match *self {
            HudCache::Empty => None,
            HudCache::Resolved(resolved) => Some((
                resolved.layout_data,
                resolved.layout_kind,
                resolved.skin_name,
            )),
            HudCache::Missing {
                layout_data,
                layout_kind,
                skin_name,
            } => Some((layout_data, layout_kind, skin_name)),
        }
    }

    fn layout_kind(&self) -> Option<HudLayoutKind> {
        match *self {
            HudCache::Empty => None,
            HudCache::Resolved(resolved) => Some(resolved.layout_kind),
            HudCache::Missing { layout_kind, .. } => Some(layout_kind),
        }
    }
}

struct HudVisualRuntime {
    caches: [HudCache; HUD_CACHE_SLOTS],
}

struct HudVisualRuntimeCell(UnsafeCell<HudVisualRuntime>);

unsafe impl Sync for HudVisualRuntimeCell {}

struct HudRuntimeGuard;

impl HudRuntimeGuard {
    fn acquire() -> Self {
        pane_lifetime::acquire_flag(&HUD_RUNTIME_LOCK);
        Self
    }
}

impl Drop for HudRuntimeGuard {
    fn drop(&mut self) {
        HUD_RUNTIME_LOCK.store(false, Ordering::Release);
    }
}

impl HudVisualRuntime {
    const fn new() -> Self {
        Self {
            caches: [HudCache::Empty; HUD_CACHE_SLOTS],
        }
    }

    fn reset(&mut self) {
        self.caches = [HudCache::Empty; HUD_CACHE_SLOTS];
    }

    fn publish_capture(
        &mut self,
        captured: CapturedHudLayout,
        skin: &'static BuiltInSkin,
        resolved: Result<ResolvedHudSkin, VisualRenderError>,
        epoch: u64,
        notes: &mut HudNotes,
    ) {
        if publish_after_epoch(epoch, pane_lifetime::retire_epoch()) == EpochPublish::Rejected {
            notes.push(HudNote::EpochRejected {
                layout_data: captured.layout_data,
            });
            return;
        }
        if captured.layout_data == 0 || self.has_cache_for(captured, skin) {
            return;
        }

        let Some(slot_index) = self.available_slot_index(captured.kind) else {
            notes.push(HudNote::CacheFull {
                layout_name: captured.kind.name(),
                layout_data: captured.layout_data,
            });
            return;
        };

        // A replaced slot's old panes are not the live owner of this callback.
        // Drop the addresses only. The previous instance may stay visible until
        // the game finalizes those panes.
        if matches!(self.caches[slot_index], HudCache::Resolved(_)) {
            self.caches[slot_index] = HudCache::Empty;
            notes.push(HudNote::ReplacedSlot);
        }

        match resolved {
            Ok(resolved) => {
                remember_resolved(&resolved);
                self.caches[slot_index] = HudCache::Resolved(resolved);
                notes.push(HudNote::Captured {
                    skin: skin.name,
                    layout_name: captured.kind.name(),
                    layout_data: captured.layout_data,
                });
            }
            Err(error) => {
                self.caches[slot_index] = HudCache::Missing {
                    layout_data: captured.layout_data,
                    layout_kind: captured.kind,
                    skin_name: skin.name,
                };
                notes.push(HudNote::CaptureMiss(error));
            }
        }
    }

    unsafe fn update(
        &mut self,
        state: &ControllerViewState,
        training_mode: bool,
        skin: &'static BuiltInSkin,
        notes: &mut HudNotes,
    ) -> bool {
        self.resolve_skin_swaps(skin, notes);

        let mut updated_any = false;
        let prefer_match_root = self.has_valid_match_root(skin);

        for cache in &mut self.caches {
            let HudCache::Resolved(resolved) = *cache else {
                continue;
            };

            if resolved.skin_name != skin.name {
                *cache = HudCache::Resolved(resolved);
                continue;
            }

            if !resolved.is_valid(skin) {
                notes.push(HudNote::InvalidDiscard);
                *cache = HudCache::Empty;
                continue;
            }

            if prefer_match_root && !resolved.layout_kind.is_match_root() {
                hide_visual_skin_root(resolved.skin_root);
                *cache = HudCache::Resolved(resolved);
                continue;
            }

            update_visual_skin_root_with_config(
                resolved.skin_root,
                resolved.layout_kind.overlay_config(training_mode),
                skin,
            );
            for (pane, element) in resolved.panes[..resolved.pane_count]
                .iter()
                .zip(skin.elements.iter())
            {
                update_visual_skin_pane(*pane, element, state);
            }

            *cache = HudCache::Resolved(resolved);
            updated_any = true;
        }

        updated_any
    }

    unsafe fn resolve_skin_swaps(&mut self, skin: &'static BuiltInSkin, notes: &mut HudNotes) {
        for cache in &mut self.caches {
            match *cache {
                HudCache::Resolved(resolved) if resolved.skin_name != skin.name => {
                    let swap = apply_skin_swap(SlotKind::Resolved, || unsafe {
                        resolve_hud_skin_from_root(
                        CapturedHudLayout {
                            layout_data: resolved.layout_data,
                            kind: resolved.layout_kind,
                        },
                        skin,
                        resolved.skin_root,
                    )
                    });
                    match swap {
                        Ok(Ok(new_resolved)) => {
                            hide_resolved_hud_panes(resolved);
                            remember_resolved(&new_resolved);
                            *cache = HudCache::Resolved(new_resolved);
                            notes.push(HudNote::SkinSwap {
                                layout_name: resolved.layout_kind.name(),
                                previous: resolved.skin_name,
                                next: skin.name,
                            });
                        }
                        Ok(Err(error)) => {
                            if matches!(error, VisualRenderError::UserAllocatedPane { .. }) {
                                *cache = HudCache::Empty;
                            }
                            notes.push(HudNote::SwapMiss {
                                error,
                                previous: resolved.skin_name,
                                next: skin.name,
                            });
                        }
                        Err(_) => {
                            *cache = HudCache::Empty;
                        }
                    }
                }
                HudCache::Missing {
                    layout_kind,
                    skin_name,
                    ..
                } if skin_name != skin.name => {
                    // The remembered layout handle is not a live owner on scene update.
                    let invalidated = apply_skin_swap(SlotKind::Missing, || ());
                    if invalidated.is_err() {
                        *cache = HudCache::Empty;
                        notes.push(HudNote::MissingSwap {
                            layout_name: layout_kind.name(),
                            previous: skin_name,
                            next: skin.name,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    fn has_cache_for(&self, captured: CapturedHudLayout, skin: &BuiltInSkin) -> bool {
        let request = (captured.layout_data, captured.kind, skin.name);
        self.caches.iter().any(|cache| {
            matches!(cache, HudCache::Resolved(_)) && cache.cache_key() == Some(request)
        })
    }

    fn has_valid_match_root(&self, skin: &BuiltInSkin) -> bool {
        self.caches.iter().any(|cache| match *cache {
            HudCache::Resolved(resolved) => {
                resolved.layout_kind.is_match_root() && resolved.is_valid(skin)
            }
            _ => false,
        })
    }

    fn available_slot_index(&self, layout_kind: HudLayoutKind) -> Option<usize> {
        if let Some(slot_index) = self
            .caches
            .iter()
            .position(|cache| cache.layout_kind() == Some(layout_kind))
        {
            return Some(slot_index);
        }

        if let Some(slot_index) = self
            .caches
            .iter()
            .position(|cache| matches!(cache, HudCache::Empty | HudCache::Missing { .. }))
        {
            return Some(slot_index);
        }

        layout_kind.is_match_root().then(|| {
            self.caches
                .iter()
                .position(|cache| match *cache {
                    HudCache::Resolved(resolved) => !resolved.layout_kind.is_match_root(),
                    _ => false,
                })
                .unwrap_or(0)
        })
    }
}

impl ResolvedHudSkin {
    fn new(
        layout_data: u64,
        layout_kind: HudLayoutKind,
        skin: &BuiltInSkin,
        skin_root: *mut Pane,
    ) -> Self {
        Self {
            layout_data,
            layout_kind,
            skin_name: skin.name,
            skin_root,
            panes: [ptr::null_mut(); MAX_RESOLVED_SKIN_ELEMENTS],
            pane_count: 0,
        }
    }

    fn push(&mut self, pane: *mut Pane) {
        self.panes[self.pane_count] = pane;
        self.pane_count += 1;
    }

    fn is_valid(&self, skin: &BuiltInSkin) -> bool {
        self.layout_data != 0
            && !self.skin_root.is_null()
            && self.skin_name == skin.name
            && matches!(
                self.layout_kind,
                HudLayoutKind::MatchRoot | HudLayoutKind::P1Parts | HudLayoutKind::P1AltParts
            )
            && self.pane_count == skin.elements.len()
            && self.panes[..self.pane_count]
                .iter()
                .all(|pane| !pane.is_null())
    }
}

static HUD_EPOCH_REJECT_LOGGED: AtomicBool = AtomicBool::new(false);
static HUD_REPLACED_SLOT_LOGGED: AtomicBool = AtomicBool::new(false);
static HUD_CAPTURE_MISS_LOGGED: AtomicBool = AtomicBool::new(false);
static HUD_INVALID_DISCARD_LOGGED: AtomicBool = AtomicBool::new(false);
static HUD_MISSING_SWAP_LOGGED: AtomicBool = AtomicBool::new(false);
static HUD_LAYOUT_PATCH_PROBE_LOGGED: AtomicBool = AtomicBool::new(false);
static HUD_CACHE_FULL_LOGGED: AtomicBool = AtomicBool::new(false);
static HUD_SKIN_SWAP_RESOLVED_LOGGED: AtomicBool = AtomicBool::new(false);
static HUD_SKIN_SWAP_MISS_LOGGED: AtomicBool = AtomicBool::new(false);
static HUD_RUNTIME_LOCK: AtomicBool = AtomicBool::new(false);
static HUD_VISUAL_RUNTIME: HudVisualRuntimeCell =
    HudVisualRuntimeCell(UnsafeCell::new(HudVisualRuntime::new()));

#[derive(Clone, Copy)]
enum HudNote {
    ReplacedSlot,
    Captured {
        skin: &'static str,
        layout_name: &'static str,
        layout_data: u64,
    },
    InvalidDiscard,
    SkinSwap {
        layout_name: &'static str,
        previous: &'static str,
        next: &'static str,
    },
    SwapMiss {
        error: VisualRenderError,
        previous: &'static str,
        next: &'static str,
    },
    MissingSwap {
        layout_name: &'static str,
        previous: &'static str,
        next: &'static str,
    },
    CacheFull {
        layout_name: &'static str,
        layout_data: u64,
    },
    EpochRejected {
        layout_data: u64,
    },
    CaptureMiss(VisualRenderError),
}

struct HudNotes {
    items: [Option<HudNote>; 8],
    len: usize,
}

impl HudNotes {
    fn new() -> Self {
        Self {
            items: [None; 8],
            len: 0,
        }
    }

    fn push(&mut self, note: HudNote) {
        if self.len < self.items.len() {
            self.items[self.len] = Some(note);
            self.len += 1;
        }
    }

    fn emit(self) {
        for note in self.items.into_iter().flatten() {
            match note {
                HudNote::ReplacedSlot => {
                    if !HUD_REPLACED_SLOT_LOGGED.swap(true, Ordering::Relaxed) {
                        trace(
                            "non-draw HUD path replaced a cached slot without hiding the previous panes; that instance may stay visible until pane finalize",
                        );
                    }
                }
                HudNote::Captured {
                    skin,
                    layout_name,
                    layout_data,
                } => {
                    trace(&format!(
                        "non-draw HUD path captured skin '{skin}' from {layout_name} layout data 0x{layout_data:x}"
                    ));
                }
                HudNote::InvalidDiscard => {
                    if !HUD_INVALID_DISCARD_LOGGED.swap(true, Ordering::Relaxed) {
                        trace(
                            "non-draw HUD path discarded a cached skin without hiding panes; that instance may stay visible until pane finalize",
                        );
                    }
                }
                HudNote::SkinSwap {
                    layout_name,
                    previous,
                    next,
                } => {
                    if !HUD_SKIN_SWAP_RESOLVED_LOGGED.swap(true, Ordering::Relaxed) {
                        trace(&format!(
                            "non-draw HUD path swapped cached {layout_name} layout from skin '{previous}' to '{next}'"
                        ));
                    }
                }
                HudNote::SwapMiss {
                    error,
                    previous,
                    next,
                } => log_skin_swap_miss(error, previous, next),
                HudNote::MissingSwap {
                    layout_name,
                    previous,
                    next,
                } => {
                    if !HUD_MISSING_SWAP_LOGGED.swap(true, Ordering::Relaxed) {
                        trace(&format!(
                            "non-draw HUD path dropped missing {layout_name} cache for skin '{previous}'; waiting for a fresh set-info-alpha capture before resolving '{next}'"
                        ));
                    }
                }
                HudNote::CacheFull {
                    layout_name,
                    layout_data,
                } => log_cache_full(layout_name, layout_data),
                HudNote::EpochRejected { layout_data } => {
                    if !HUD_EPOCH_REJECT_LOGGED.swap(true, Ordering::Relaxed) {
                        trace(&format!(
                            "non-draw HUD path discarded an unpublished capture for layout data 0x{layout_data:x} because pane finalize started during resolve"
                        ));
                    }
                }
                HudNote::CaptureMiss(error) => log_capture_miss(error),
            }
        }
    }
}

pub(super) fn discard_runtime() {
    let _guard = HudRuntimeGuard::acquire();
    unsafe {
        clear_locked_hud(&mut *HUD_VISUAL_RUNTIME.0.get());
    }
}

pub(super) fn reset_runtime(reason: &'static str) {
    {
        let _guard = HudRuntimeGuard::acquire();
        unsafe {
            clear_locked_hud(&mut *HUD_VISUAL_RUNTIME.0.get());
        }
    }
    pane_lifetime::note_match_reset(reason);
}

pub(super) unsafe fn capture_runtime(captured: CapturedHudLayout, skin: &'static BuiltInSkin) {
    if !pane_lifetime::begin_routine(RoutineKind::Capture) {
        pane_lifetime::flush_diagnostics(None);
        return;
    }
    let already_cached = {
        let _guard = HudRuntimeGuard::acquire();
        !pane_lifetime::cache_access_enabled()
            || (*HUD_VISUAL_RUNTIME.0.get()).has_cache_for(captured, skin)
    };
    if already_cached {
        pane_lifetime::flush_diagnostics(None);
        return;
    }

    let epoch = pane_lifetime::retire_epoch();
    let resolved = resolve_hud_skin(captured, skin);
    if resolved.is_err() {
        log_layout_patch_probe(captured.layout_data, captured.kind.name(), skin);
    }

    let mut notes = HudNotes::new();
    {
        let _guard = HudRuntimeGuard::acquire();
        if pane_lifetime::cache_access_enabled() {
            (*HUD_VISUAL_RUNTIME.0.get()).publish_capture(
                captured, skin, resolved, epoch, &mut notes,
            );
        }
    }
    notes.emit();
    pane_lifetime::flush_diagnostics(None);
}

pub(super) unsafe fn update_runtime(
    state: &ControllerViewState,
    training_mode: bool,
    skin: &'static BuiltInSkin,
) -> bool {
    if !pane_lifetime::begin_routine(RoutineKind::Update) {
        pane_lifetime::flush_diagnostics(None);
        return false;
    }
    let mut notes = HudNotes::new();
    let updated = {
        let _guard = HudRuntimeGuard::acquire();
        if !pane_lifetime::cache_access_enabled() {
            false
        } else {
            (*HUD_VISUAL_RUNTIME.0.get()).update(state, training_mode, skin, &mut notes)
        }
    };
    notes.emit();
    pane_lifetime::flush_diagnostics(None);
    updated
}

pub(crate) fn finalize_retired_pane(retiring: u64) -> FinalizeTouch {
    let _guard = HudRuntimeGuard::acquire();
    let runtime = unsafe { &mut *HUD_VISUAL_RUNTIME.0.get() };
    let mut projected = [None; HUD_CACHE_SLOTS];
    for (slot, cache) in projected.iter_mut().zip(runtime.caches.iter()) {
        *slot = watched_cache(cache);
    }
    let class = classify_finalize(&projected, retiring, |watched| *watched);
    let cleared = apply_finalize(&mut projected, retiring, class, |watched| *watched);
    if class == FinalizeClass::EvictMatching {
        for (cache, slot) in runtime.caches.iter_mut().zip(projected.iter()) {
            if slot.is_none() && matches!(cache, HudCache::Resolved(_)) {
                *cache = HudCache::Empty;
            }
        }
    }
    FinalizeTouch { class, cleared }
}

fn clear_locked_hud(runtime: &mut HudVisualRuntime) -> usize {
    let mut projected = [None; HUD_CACHE_SLOTS];
    for (slot, cache) in projected.iter_mut().zip(runtime.caches.iter()) {
        *slot = watched_cache(cache);
    }
    let cleared = reset_slots(&mut projected);
    let missing = runtime
        .caches
        .iter()
        .filter(|cache| matches!(cache, HudCache::Missing { .. }))
        .count();
    runtime.reset();
    cleared + missing
}

fn watched_cache(cache: &HudCache) -> Option<WatchedPointers> {
    match cache {
        HudCache::Resolved(resolved) => Some(watched_resolved(resolved)),
        HudCache::Missing { .. } | HudCache::Empty => None,
    }
}

fn remember_resolved(resolved: &ResolvedHudSkin) {
    let mut children = [0u64; MAX_RESOLVED_SKIN_ELEMENTS];
    for (slot, pane) in children
        .iter_mut()
        .zip(resolved.panes[..resolved.pane_count].iter())
    {
        *slot = *pane as u64;
    }
    pane_lifetime::remember_captured(
        resolved.skin_root as u64,
        0,
        &children[..resolved.pane_count],
        resolved.layout_data,
    );
}

unsafe fn reject_user_allocated(
    pane: *mut Pane,
    skin: &BuiltInSkin,
    pane_name: &'static [u8],
) -> Result<(), VisualRenderError> {
    if pane_is_user_allocated(pane) {
        Err(VisualRenderError::UserAllocatedPane {
            skin_name: skin.name,
            pane_name,
        })
    } else {
        Ok(())
    }
}

fn watched_resolved(resolved: &ResolvedHudSkin) -> WatchedPointers {
    let mut watched = WatchedPointers::new();
    watched.set_root(resolved.skin_root as u64);
    for pane in &resolved.panes[..resolved.pane_count] {
        let _ = watched.push_child(*pane as u64);
    }
    watched
}

unsafe fn hide_resolved_hud_panes(resolved: ResolvedHudSkin) {
    for pane in &resolved.panes[..resolved.pane_count] {
        hide_visual_skin_pane(*pane);
    }
}

unsafe fn resolve_hud_skin(
    captured: CapturedHudLayout,
    skin: &'static BuiltInSkin,
) -> Result<ResolvedHudSkin, VisualRenderError> {
    validate_skin_size(skin)?;

    let skin_root = find_pane_in_layout_data(captured.layout_data, skin.root_pane_name).ok_or(
        VisualRenderError::MissingSkinPane {
            skin_name: skin.name,
            pane_name: skin.root_pane_name,
        },
    )?;
    reject_user_allocated(skin_root, skin, skin.root_pane_name)?;
    let mut resolved = ResolvedHudSkin::new(captured.layout_data, captured.kind, skin, skin_root);

    for element in skin.elements.iter().take(MAX_RESOLVED_SKIN_ELEMENTS) {
        let pane = find_pane_in_layout_data(captured.layout_data, element.pane_name).ok_or(
            VisualRenderError::MissingSkinPane {
                skin_name: skin.name,
                pane_name: element.pane_name,
            },
        )?;
        reject_user_allocated(pane, skin, element.pane_name)?;
        resolved.push(pane);
    }

    Ok(resolved)
}

unsafe fn resolve_hud_skin_from_root(
    captured: CapturedHudLayout,
    skin: &'static BuiltInSkin,
    skin_root: *mut Pane,
) -> Result<ResolvedHudSkin, VisualRenderError> {
    validate_skin_size(skin)?;

    if !pane_name_matches(skin_root, skin.root_pane_name) {
        return Err(VisualRenderError::MissingSkinPane {
            skin_name: skin.name,
            pane_name: skin.root_pane_name,
        });
    }
    reject_user_allocated(skin_root, skin, skin.root_pane_name)?;

    let mut resolved = ResolvedHudSkin::new(captured.layout_data, captured.kind, skin, skin_root);
    for element in skin.elements.iter().take(MAX_RESOLVED_SKIN_ELEMENTS) {
        let pane = find_pane_by_name(skin_root, element.pane_name);
        if pane.is_null() {
            return Err(VisualRenderError::MissingSkinPane {
                skin_name: skin.name,
                pane_name: element.pane_name,
            });
        }
        reject_user_allocated(pane, skin, element.pane_name)?;

        resolved.push(pane);
    }

    Ok(resolved)
}

fn log_capture_miss(error: VisualRenderError) {
    if HUD_CAPTURE_MISS_LOGGED.swap(true, Ordering::Relaxed) {
        return;
    }

    match error {
        VisualRenderError::MissingSkinPane {
            skin_name,
            pane_name,
        } => {
            let pane = cstr_bytes_to_str(pane_name);
            trace(&format!(
                "non-draw HUD path could not find skin '{skin_name}' pane '{pane}'"
            ));
            trace(&format!(
                "skin/layout mismatch: active skin '{}' missing first pane '{pane}'; expected layout flavor: {}",
                active_skin().name, active_skin().expected_layout_flavor
            ));
            trace(
                "regenerate layout with `python tools/patch_info_melee_layout.py`, then stage with `python tools/stage_arcropolis_layout.py`",
            );
        }
        VisualRenderError::LifetimeGuardInactive => {
            trace(
                "non-draw HUD path did not cache panes because the pane finalize observer is inactive",
            );
        }
        VisualRenderError::UserAllocatedPane {
            skin_name,
            pane_name,
        } => {
            trace(&format!(
                "non-draw HUD path refused user-allocated skin '{skin_name}' pane '{}'; engine finalize skips that flag",
                cstr_bytes_to_str(pane_name)
            ));
        }
        VisualRenderError::SkinTooLarge {
            skin_name,
            element_count,
            max_elements,
        } => {
            trace(&format!(
                "non-draw HUD path cannot use skin '{skin_name}' because it has {element_count} elements; max supported is {max_elements}"
            ));
            trace("split the skin or raise MAX_RESOLVED_SKIN_ELEMENTS before activating it");
        }
    }
}

fn log_skin_swap_miss(
    error: VisualRenderError,
    previous_skin_name: &'static str,
    requested_skin_name: &'static str,
) {
    if HUD_SKIN_SWAP_MISS_LOGGED.swap(true, Ordering::Relaxed) {
        return;
    }

    match error {
        VisualRenderError::MissingSkinPane { pane_name, .. } => {
            trace(&format!(
                "non-draw HUD path could not swap cached skin '{previous_skin_name}' to '{requested_skin_name}' because pane '{}' was missing; keeping previous skin cache",
                cstr_bytes_to_str(pane_name)
            ));
        }
        VisualRenderError::LifetimeGuardInactive => {
            trace(
                "non-draw HUD path did not swap cached skins because the pane finalize observer is inactive",
            );
        }
        VisualRenderError::UserAllocatedPane { pane_name, .. } => {
            trace(&format!(
                "non-draw HUD path dropped cached skin '{previous_skin_name}' without hiding because pane '{}' is user-allocated",
                cstr_bytes_to_str(pane_name)
            ));
        }
        VisualRenderError::SkinTooLarge {
            element_count,
            max_elements,
            ..
        } => {
            trace(&format!(
                "non-draw HUD path could not swap cached skin '{previous_skin_name}' to '{requested_skin_name}' because it has {element_count} elements; max supported is {max_elements}"
            ));
        }
    }
}

fn log_layout_patch_probe(layout_data: u64, layout_name: &str, skin: &BuiltInSkin) {
    if HUD_LAYOUT_PATCH_PROBE_LOGGED.swap(true, Ordering::Relaxed) {
        return;
    }

    let has_skin_root =
        unsafe { find_pane_in_layout_data(layout_data, skin.root_pane_name).is_some() };
    let has_a_marker =
        unsafe { find_pane_in_layout_data(layout_data, b"sgpo_pro_a_marker\0").is_some() };
    let has_original_marker =
        unsafe { find_pane_in_layout_data(layout_data, b"set_rep_01\0").is_some() };

    trace(&format!(
        "non-draw HUD layout probe for {layout_name}: sgpo_root={has_skin_root} sgpo_pro_a_marker={has_a_marker} original_set_rep_01={has_original_marker}"
    ));

    if has_original_marker && !has_skin_root {
        trace(
            "loaded P1 HUD parts layout looks unpatched; install generated layout.arc through ARCropolis at ui/layout/info/info_melee/info_melee/layout.arc",
        );
    }
}

fn log_cache_full(layout_name: &str, layout_data: u64) {
    if HUD_CACHE_FULL_LOGGED.swap(true, Ordering::Relaxed) {
        return;
    }

    trace(&format!(
        "non-draw HUD path has no free cache slot for {layout_name} layout data 0x{layout_data:x}"
    ));
}
