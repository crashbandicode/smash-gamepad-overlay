use skyline::nn::ui2d::Pane;
use std::cell::UnsafeCell;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::input::ControllerViewState;
use crate::logger::trace;
use crate::pane_utils::{cstr_bytes_to_str, pane_name_matches};
use crate::skin::{active_skin, BuiltInSkin};
use crate::visual::{
    hide_visual_skin_root, update_visual_skin_pane, update_visual_skin_root_with_config,
    validate_skin_size, VisualRenderError, MAX_RESOLVED_SKIN_ELEMENTS,
};

use super::capture::{find_pane_in_layout_data, CapturedHudLayout, HudLayoutKind};

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
        while HUD_RUNTIME_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            std::hint::spin_loop();
        }

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

    unsafe fn capture_layout_data(
        &mut self,
        captured: CapturedHudLayout,
        skin: &'static BuiltInSkin,
    ) {
        if captured.layout_data == 0 || self.has_cache_for(captured, skin) {
            return;
        }

        let Some(slot_index) = self.available_slot_index(captured.kind) else {
            log_cache_full(captured.kind.name(), captured.layout_data);
            return;
        };

        if let HudCache::Resolved(resolved) = self.caches[slot_index] {
            hide_visual_skin_root(resolved.skin_root);
        }

        match resolve_hud_skin(captured, skin) {
            Ok(resolved) => {
                self.caches[slot_index] = HudCache::Resolved(resolved);

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
        let skin = active_skin();
        let prefer_match_root = self.has_valid_match_root();

        for cache in &mut self.caches {
            let HudCache::Resolved(resolved) = *cache else {
                continue;
            };

            if !resolved.is_valid(skin) {
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

    fn has_cache_for(&self, captured: CapturedHudLayout, skin: &BuiltInSkin) -> bool {
        let request = (captured.layout_data, captured.kind, skin.name);
        self.caches
            .iter()
            .any(|cache| cache.cache_key() == Some(request))
    }

    fn has_valid_match_root(&self) -> bool {
        self.caches.iter().any(|cache| match *cache {
            HudCache::Resolved(resolved) => {
                resolved.layout_kind.is_match_root() && resolved.is_valid(active_skin())
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
            && unsafe { pane_name_matches(self.skin_root, skin.root_pane_name) }
            && self.pane_count == skin.elements.len()
            && self.panes[..self.pane_count]
                .iter()
                .zip(skin.elements.iter())
                .all(|(pane, element)| unsafe { pane_name_matches(*pane, element.pane_name) })
    }
}

static HUD_CAPTURE_MISS_LOGGED: AtomicBool = AtomicBool::new(false);
static HUD_LAYOUT_PATCH_PROBE_LOGGED: AtomicBool = AtomicBool::new(false);
static HUD_CACHE_FULL_LOGGED: AtomicBool = AtomicBool::new(false);
static HUD_RUNTIME_LOCK: AtomicBool = AtomicBool::new(false);
static HUD_VISUAL_RUNTIME: HudVisualRuntimeCell =
    HudVisualRuntimeCell(UnsafeCell::new(HudVisualRuntime::new()));

pub(super) fn reset_runtime() {
    let _guard = HudRuntimeGuard::acquire();
    unsafe {
        (*HUD_VISUAL_RUNTIME.0.get()).reset();
    }
}

pub(super) unsafe fn capture_runtime(captured: CapturedHudLayout) {
    let _guard = HudRuntimeGuard::acquire();
    (*HUD_VISUAL_RUNTIME.0.get()).capture_layout_data(captured, active_skin());
}

pub(super) unsafe fn update_runtime(state: &ControllerViewState, training_mode: bool) -> bool {
    let _guard = HudRuntimeGuard::acquire();
    (*HUD_VISUAL_RUNTIME.0.get()).update(state, training_mode)
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
