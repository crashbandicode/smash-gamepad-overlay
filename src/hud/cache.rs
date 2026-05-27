use skyline::nn::ui2d::Pane;
use std::cell::UnsafeCell;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::input::{poll_p1_controller, ControllerViewState};
use crate::logger::trace;
use crate::skin::{BuiltInSkin, ACTIVE_SKIN};
use crate::visual::{
    hide_visual_skin_root, update_visual_skin_pane, update_visual_skin_root_with_config,
    VisualRenderError, MAX_RESOLVED_SKIN_ELEMENTS,
};

use super::capture::{
    cstr_bytes_to_str, find_pane_in_layout_data, CapturedHudLayout, HudLayoutKind,
};

const HUD_CACHE_SLOTS: usize = 3;

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
enum HudCache {
    Empty,
    Resolved(ResolvedHudSkin),
    Missing {
        layout_data: u64,
        layout_kind: HudLayoutKind,
        skin_name: &'static str,
    },
}

struct HudVisualRuntime {
    caches: [HudCache; HUD_CACHE_SLOTS],
}

struct HudVisualRuntimeCell(UnsafeCell<HudVisualRuntime>);

// These hooks run on Smash's UI path; the cache is intentionally single-threaded.
unsafe impl Sync for HudVisualRuntimeCell {}

impl HudVisualRuntime {
    const fn new() -> Self {
        Self {
            caches: [HudCache::Empty; HUD_CACHE_SLOTS],
        }
    }

    fn reset(&mut self) {
        self.caches = [HudCache::Empty; HUD_CACHE_SLOTS];
    }

    unsafe fn capture_layout_data(
        &mut self,
        captured: CapturedHudLayout,
        skin: &'static BuiltInSkin,
        training_mode: bool,
    ) {
        if captured.layout_data == 0 || self.has_cache_for(captured, skin) {
            return;
        }

        let Some(slot_index) = self.available_slot_index(captured.kind) else {
            log_cache_full(captured.kind.name(), captured.layout_data);
            return;
        };

        match resolve_hud_skin(captured, skin) {
            Ok(resolved) => {
                self.caches[slot_index] = HudCache::Resolved(resolved);
                HUD_CAPTURED.store(true, Ordering::Relaxed);

                let view_state = poll_p1_controller()
                    .map(ControllerViewState::from_snapshot)
                    .unwrap_or_else(ControllerViewState::neutral);
                let _ = self.update(&view_state, training_mode);

                trace(&format!(
                    "non-draw HUD path captured skin '{}' from {} layout data 0x{:x}",
                    skin.name,
                    captured.kind.name(),
                    captured.layout_data
                ));
            }
            Err(error) => {
                self.caches[slot_index] = HudCache::Missing {
                    layout_data: captured.layout_data,
                    layout_kind: captured.kind,
                    skin_name: skin.name,
                };
                log_capture_miss(error);
                log_layout_patch_probe(captured.layout_data, captured.kind.name(), skin);
            }
        }
    }

    unsafe fn update(&mut self, state: &ControllerViewState, training_mode: bool) -> bool {
        let mut updated_any = false;
        let prefer_match_root = self.has_valid_match_root();

        for cache in &mut self.caches {
            let HudCache::Resolved(resolved) = *cache else {
                continue;
            };

            if !resolved.is_valid() {
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
            );
            for (pane, element) in resolved.panes[..resolved.pane_count]
                .iter()
                .zip(ACTIVE_SKIN.elements.iter())
            {
                update_visual_skin_pane(*pane, element, state);
            }

            *cache = HudCache::Resolved(resolved);
            updated_any = true;
        }

        HUD_CAPTURED.store(updated_any, Ordering::Relaxed);
        updated_any
    }

    fn has_cache_for(&self, captured: CapturedHudLayout, skin: &BuiltInSkin) -> bool {
        self.caches.iter().any(|cache| match *cache {
            HudCache::Empty => false,
            HudCache::Resolved(resolved) => {
                resolved.layout_data == captured.layout_data
                    && resolved.layout_kind == captured.kind
                    && resolved.skin_name == skin.name
            }
            HudCache::Missing {
                layout_data: cached_layout_data,
                layout_kind: cached_layout_kind,
                skin_name,
            } => {
                cached_layout_data == captured.layout_data
                    && cached_layout_kind == captured.kind
                    && skin_name == skin.name
            }
        })
    }

    fn has_valid_match_root(&self) -> bool {
        self.caches.iter().any(|cache| match *cache {
            HudCache::Resolved(resolved) => {
                resolved.layout_kind.is_match_root() && resolved.is_valid()
            }
            _ => false,
        })
    }

    fn available_slot_index(&self, layout_kind: HudLayoutKind) -> Option<usize> {
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

    fn is_valid(&self) -> bool {
        self.layout_data != 0
            && !self.skin_root.is_null()
            && self.skin_name == ACTIVE_SKIN.name
            && matches!(
                self.layout_kind,
                HudLayoutKind::MatchRoot | HudLayoutKind::P1Parts | HudLayoutKind::P1AltParts
            )
            && self.pane_count == ACTIVE_SKIN.elements.len()
            && self.panes[..self.pane_count]
                .iter()
                .all(|pane| !pane.is_null())
    }
}

static HUD_CAPTURE_MISS_LOGGED: AtomicBool = AtomicBool::new(false);
static HUD_LAYOUT_PATCH_PROBE_LOGGED: AtomicBool = AtomicBool::new(false);
static HUD_CACHE_FULL_LOGGED: AtomicBool = AtomicBool::new(false);
static HUD_CAPTURED: AtomicBool = AtomicBool::new(false);
static HUD_VISUAL_RUNTIME: HudVisualRuntimeCell =
    HudVisualRuntimeCell(UnsafeCell::new(HudVisualRuntime::new()));

pub(super) fn runtime_has_capture() -> bool {
    HUD_CAPTURED.load(Ordering::Relaxed)
}

pub(super) fn reset_runtime() {
    HUD_CAPTURED.store(false, Ordering::Relaxed);
    unsafe {
        (*HUD_VISUAL_RUNTIME.0.get()).reset();
    }
}

pub(super) unsafe fn capture_runtime(captured: CapturedHudLayout, training_mode: bool) {
    (*HUD_VISUAL_RUNTIME.0.get()).capture_layout_data(captured, &ACTIVE_SKIN, training_mode);
}

pub(super) unsafe fn update_runtime(state: &ControllerViewState, training_mode: bool) -> bool {
    (*HUD_VISUAL_RUNTIME.0.get()).update(state, training_mode)
}

unsafe fn resolve_hud_skin(
    captured: CapturedHudLayout,
    skin: &'static BuiltInSkin,
) -> Result<ResolvedHudSkin, VisualRenderError> {
    let skin_root = find_pane_in_layout_data(captured.layout_data, skin.root_pane_name).ok_or(
        VisualRenderError::MissingSkinPane {
            skin_name: skin.name,
            pane_name: skin.root_pane_name,
        },
    )?;
    let mut resolved = ResolvedHudSkin::new(captured.layout_data, captured.kind, skin, skin_root);

    for element in skin.elements.iter().take(MAX_RESOLVED_SKIN_ELEMENTS) {
        let pane = find_pane_in_layout_data(captured.layout_data, element.pane_name).ok_or(
            VisualRenderError::MissingSkinPane {
                skin_name: skin.name,
                pane_name: element.pane_name,
            },
        )?;
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
        } => trace(&format!(
            "non-draw HUD path could not find skin '{skin_name}' pane '{}'",
            cstr_bytes_to_str(pane_name)
        )),
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
