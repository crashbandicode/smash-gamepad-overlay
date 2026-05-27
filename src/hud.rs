use skyline::hooks::{getRegionAddress, InlineCtx, Region};
use skyline::libc::c_char;
use skyline::nn::ui2d::Pane;
use std::cell::UnsafeCell;
use std::path::Path;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::config::{
    OverlayConfig, HIDE_TRAINING_GAMEPAD_FLAG_PATH, HUD_MATCH_END_OFFSET, HUD_MATCH_START_OFFSET,
    HUD_SET_INFO_ALPHA_OFFSET, LAYOUT_GET_PANE_BY_NAME_OFFSET, MATCH_HUD_LAYOUT, OVERLAY_CONFIG,
    P1_PARTS_PANE_NAME, SCENE_UPDATE_OFFSET, TRAINING_COMPAT_P1_2_PARTS_OVERLAY_CONFIG,
    TRAINING_COMPAT_P1_PARTS_OVERLAY_CONFIG, TRAINING_MODE_P1_2_PARTS_OVERLAY_CONFIG,
    TRAINING_MODE_P1_PARTS_OVERLAY_CONFIG,
};
use crate::input::{poll_p1_controller, ControllerViewState};
use crate::logger::trace;
use crate::offsets::display_version;
use crate::skin::{BuiltInSkin, PRO_CONTROLLER_STATIC_SKIN};
use crate::visual::{
    hide_visual_skin_root, update_visual_skin_pane, update_visual_skin_root_with_config,
    VisualRenderError, MAX_RESOLVED_SKIN_ELEMENTS,
};

const SUPPORTED_NON_DRAW_DISPLAY_VERSION: &str = "13.0.4";
const HUD_CACHE_SLOTS: usize = 3;
const HUD_LAYOUT_PROBE_LIMIT: usize = 16;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum HudLayoutKind {
    MatchRoot,
    P1Parts,
    P1AltParts,
}

impl HudLayoutKind {
    fn name(self) -> &'static str {
        match self {
            Self::MatchRoot => MATCH_HUD_LAYOUT,
            Self::P1Parts => "p1",
            Self::P1AltParts => "p1_2",
        }
    }

    fn overlay_config(self) -> &'static OverlayConfig {
        match (self, unsafe { is_training_mode() }) {
            (Self::MatchRoot, _) => &OVERLAY_CONFIG,
            (Self::P1Parts, true) => &TRAINING_MODE_P1_PARTS_OVERLAY_CONFIG,
            (Self::P1AltParts, true) => &TRAINING_MODE_P1_2_PARTS_OVERLAY_CONFIG,
            (Self::P1Parts, false) => &TRAINING_COMPAT_P1_PARTS_OVERLAY_CONFIG,
            (Self::P1AltParts, false) => &TRAINING_COMPAT_P1_2_PARTS_OVERLAY_CONFIG,
        }
    }

    fn is_match_root(self) -> bool {
        self == Self::MatchRoot
    }
}

#[derive(Debug, Copy, Clone)]
struct CapturedHudLayout {
    layout_data: u64,
    kind: HudLayoutKind,
}

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
                let _ = self.update(&view_state);

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

    unsafe fn update(&mut self, state: &ControllerViewState) -> bool {
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
                resolved.layout_kind.overlay_config(),
            );
            for (pane, element) in resolved.panes[..resolved.pane_count]
                .iter()
                .zip(PRO_CONTROLLER_STATIC_SKIN.elements.iter())
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
            && self.skin_name == PRO_CONTROLLER_STATIC_SKIN.name
            && matches!(
                self.layout_kind,
                HudLayoutKind::MatchRoot | HudLayoutKind::P1Parts | HudLayoutKind::P1AltParts
            )
            && self.pane_count == PRO_CONTROLLER_STATIC_SKIN.elements.len()
            && self.panes[..self.pane_count]
                .iter()
                .all(|pane| !pane.is_null())
    }
}

static HUD_CAPTURE_MISS_LOGGED: AtomicBool = AtomicBool::new(false);
static HUD_LAYOUT_PATCH_PROBE_LOGGED: AtomicBool = AtomicBool::new(false);
static HUD_CACHE_FULL_LOGGED: AtomicBool = AtomicBool::new(false);
static HUD_HOOKS_LOGGED: AtomicBool = AtomicBool::new(false);
static HUD_CAPTURED: AtomicBool = AtomicBool::new(false);
static HUD_SCENE_UPDATE_LOGGED: AtomicBool = AtomicBool::new(false);
static HUD_VISUAL_UPDATE_LOGGED: AtomicBool = AtomicBool::new(false);
static TRAINING_MODE_SUPPRESSED_LOGGED: AtomicBool = AtomicBool::new(false);
static TRAINING_MODE_ALLOWED_LOGGED: AtomicBool = AtomicBool::new(false);
static HUD_MATCH_ROOT_CAPTURED_LOGGED: AtomicBool = AtomicBool::new(false);
static HUD_KNOWN_LAYOUT_PROBE_COUNT: AtomicUsize = AtomicUsize::new(0);
static HUD_OTHER_LAYOUT_PROBE_COUNT: AtomicUsize = AtomicUsize::new(0);
static HUD_VISUAL_RUNTIME: HudVisualRuntimeCell =
    HudVisualRuntimeCell(UnsafeCell::new(HudVisualRuntime::new()));

#[skyline::hook(offset = HUD_SET_INFO_ALPHA_OFFSET, inline)]
unsafe fn capture_match_hud_layout_data(ctx: &InlineCtx) {
    if training_mode_overlay_disabled() {
        return;
    }

    let Some(captured) = captured_layout(ctx) else {
        return;
    };
    if captured.kind.is_match_root()
        && !HUD_MATCH_ROOT_CAPTURED_LOGGED.swap(true, Ordering::Relaxed)
    {
        trace("non-draw HUD path found root info_melee layout; using bottom-right root overlay");
    }

    (*HUD_VISUAL_RUNTIME.0.get()).capture_layout_data(captured, &PRO_CONTROLLER_STATIC_SKIN);
}

#[skyline::hook(offset = SCENE_UPDATE_OFFSET, inline)]
unsafe fn update_match_hud_overlay_from_scene(_: &InlineCtx) {
    if !HUD_SCENE_UPDATE_LOGGED.swap(true, Ordering::Relaxed) {
        trace("non-draw HUD scene update hook fired");
    }

    if !HUD_CAPTURED.load(Ordering::Relaxed) {
        return;
    }
    if training_mode_overlay_disabled() {
        HUD_CAPTURED.store(false, Ordering::Relaxed);
        (*HUD_VISUAL_RUNTIME.0.get()).reset();
        return;
    }

    let view_state = poll_p1_controller()
        .map(ControllerViewState::from_snapshot)
        .unwrap_or_else(ControllerViewState::neutral);
    if (*HUD_VISUAL_RUNTIME.0.get()).update(&view_state)
        && !HUD_VISUAL_UPDATE_LOGGED.swap(true, Ordering::Relaxed)
    {
        trace("non-draw HUD path updated visual panes");
    }
}

#[skyline::hook(offset = HUD_MATCH_START_OFFSET, inline)]
unsafe fn reset_match_hud_capture_on_start(_: &InlineCtx) {
    HUD_CAPTURED.store(false, Ordering::Relaxed);
    (*HUD_VISUAL_RUNTIME.0.get()).reset();
}

#[skyline::hook(offset = HUD_MATCH_END_OFFSET, inline)]
unsafe fn reset_match_hud_capture_on_end(_: &InlineCtx) {
    HUD_CAPTURED.store(false, Ordering::Relaxed);
    (*HUD_VISUAL_RUNTIME.0.get()).reset();
}

pub(crate) fn install_non_draw_hud_hooks() {
    let version = display_version();
    if version != SUPPORTED_NON_DRAW_DISPLAY_VERSION {
        trace(&format!(
            "non-draw HUD hooks not installed for Smash display version {version}; supported version is {SUPPORTED_NON_DRAW_DISPLAY_VERSION}"
        ));
        return;
    }

    if !HUD_HOOKS_LOGGED.swap(true, Ordering::Relaxed) {
        trace(&format!(
            "installing non-draw HUD hooks at .text+0x{HUD_SET_INFO_ALPHA_OFFSET:x}/0x{SCENE_UPDATE_OFFSET:x}"
        ));
    }

    skyline::install_hooks!(
        capture_match_hud_layout_data,
        update_match_hud_overlay_from_scene,
        reset_match_hud_capture_on_start,
        reset_match_hud_capture_on_end
    );
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

unsafe fn find_pane_in_layout_data(
    layout_data: u64,
    pane_name: &'static [u8],
) -> Option<*mut Pane> {
    let pane_handle = find_pane_handle_in_layout_data(layout_data, pane_name);
    let pane = pane_from_layout_handle(pane_handle);
    (!pane.is_null()).then_some(pane)
}

unsafe fn find_pane_handle_in_layout_data(layout_data: u64, pane_name: &'static [u8]) -> u64 {
    type GetPaneByName = unsafe extern "C" fn(u64, *const c_char, ...) -> [u64; 4];
    let func_addr =
        (getRegionAddress(Region::Text) as *const u8).add(LAYOUT_GET_PANE_BY_NAME_OFFSET);
    let get_pane_by_name: GetPaneByName = std::mem::transmute(func_addr);
    let pane_udata = get_pane_by_name(layout_data, pane_name.as_ptr() as *const c_char);
    pane_udata[1]
}

unsafe fn pane_from_layout_handle(pane_handle: u64) -> *mut Pane {
    if pane_handle == 0 {
        return ptr::null_mut();
    }

    let pane = *(pane_handle as *const u64) as *mut Pane;
    if pane.is_null() {
        ptr::null_mut()
    } else {
        pane
    }
}

unsafe fn captured_layout(ctx: &InlineCtx) -> Option<CapturedHudLayout> {
    let layout_data = ctx.registers[0].x();
    if layout_data == 0 {
        return None;
    }

    let layout_view = *((layout_data as *const u64).add(1));
    if layout_view == 0 {
        return None;
    }

    let layout_pane = *((layout_view as *const u64).add(3));
    if layout_pane == 0 {
        return None;
    }

    let ui2d_pane = *(layout_pane as *const u64);
    if ui2d_pane == 0 {
        return None;
    }

    let name = std::ffi::CStr::from_ptr(ui2d_pane.wrapping_add(0xb0) as *const c_char).to_bytes();
    log_captured_layout_probe(layout_data, name);

    match name {
        b"p1" => Some(CapturedHudLayout {
            layout_data,
            kind: HudLayoutKind::P1Parts,
        }),
        b"p1_2" => Some(CapturedHudLayout {
            layout_data,
            kind: HudLayoutKind::P1AltParts,
        }),
        b"info_melee" => Some(CapturedHudLayout {
            layout_data,
            kind: HudLayoutKind::MatchRoot,
        }),
        b"RootPane" if layout_data_looks_like_match_root(layout_data) => Some(CapturedHudLayout {
            layout_data,
            kind: HudLayoutKind::MatchRoot,
        }),
        _ => None,
    }
}

unsafe fn layout_data_looks_like_match_root(layout_data: u64) -> bool {
    find_pane_in_layout_data(layout_data, P1_PARTS_PANE_NAME).is_some()
        && find_pane_in_layout_data(layout_data, PRO_CONTROLLER_STATIC_SKIN.root_pane_name)
            .is_some()
}

fn log_captured_layout_probe(layout_data: u64, name: &[u8]) {
    let is_known_parts_layout = matches!(name, b"p1" | b"p1_2");
    let probe_index = if is_known_parts_layout {
        HUD_KNOWN_LAYOUT_PROBE_COUNT.fetch_add(1, Ordering::Relaxed)
    } else {
        HUD_OTHER_LAYOUT_PROBE_COUNT.fetch_add(1, Ordering::Relaxed)
    };

    if probe_index >= HUD_LAYOUT_PROBE_LIMIT {
        return;
    }

    trace(&format!(
        "non-draw HUD observed layout probe {}: name='{}' layout data 0x{layout_data:x}",
        probe_index + 1,
        std::str::from_utf8(name).unwrap_or("<non-utf8>")
    ));
}

fn training_mode_overlay_disabled() -> bool {
    if unsafe { !is_training_mode() } {
        return false;
    }

    let disabled = Path::new(HIDE_TRAINING_GAMEPAD_FLAG_PATH).exists();
    if disabled && !TRAINING_MODE_SUPPRESSED_LOGGED.swap(true, Ordering::Relaxed) {
        trace(&format!(
            "Training Modpack is loaded and Smash is in training mode; SGPO overlay suppressed because {HIDE_TRAINING_GAMEPAD_FLAG_PATH} exists"
        ));
    } else if !disabled && !TRAINING_MODE_ALLOWED_LOGGED.swap(true, Ordering::Relaxed) {
        trace(&format!(
            "Training Modpack is loaded and Smash is in training mode; SGPO overlay enabled because {HIDE_TRAINING_GAMEPAD_FLAG_PATH} is absent"
        ));
    }

    disabled
}

extern "C" {
    #[link_name = "\u{1}_ZN3app9smashball16is_training_modeEv"]
    fn is_training_mode() -> bool;
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
            "loaded P1 HUD parts layout looks unpatched; install generated layout.arc through Arcropolis at ui/layout/info/info_melee/info_melee/layout.arc",
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

fn cstr_bytes_to_str(bytes: &[u8]) -> &str {
    std::ffi::CStr::from_bytes_with_nul(bytes)
        .ok()
        .and_then(|name| name.to_str().ok())
        .unwrap_or("<invalid>")
}
