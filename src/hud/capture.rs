use skyline::hooks::{getRegionAddress, InlineCtx, Region};
use skyline::libc::c_char;
use skyline::nn::ui2d::Pane;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::config::{
    OverlayConfig, LAYOUT_GET_PANE_BY_NAME_OFFSET, MATCH_HUD_LAYOUT, OVERLAY_CONFIG,
    P1_PARTS_PANE_NAME, TRAINING_COMPAT_P1_2_PARTS_OVERLAY_CONFIG,
    TRAINING_COMPAT_P1_PARTS_OVERLAY_CONFIG, TRAINING_MODE_P1_2_PARTS_OVERLAY_CONFIG,
    TRAINING_MODE_P1_PARTS_OVERLAY_CONFIG,
};
use crate::logger::trace;
use crate::skin::ACTIVE_SKIN;

const HUD_LAYOUT_PROBE_LIMIT: usize = 16;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(super) enum HudLayoutKind {
    MatchRoot,
    P1Parts,
    P1AltParts,
}

impl HudLayoutKind {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::MatchRoot => MATCH_HUD_LAYOUT,
            Self::P1Parts => "p1",
            Self::P1AltParts => "p1_2",
        }
    }

    pub(super) fn overlay_config(self, training_mode: bool) -> &'static OverlayConfig {
        match (self, training_mode) {
            (Self::MatchRoot, _) => &OVERLAY_CONFIG,
            (Self::P1Parts, true) => &TRAINING_MODE_P1_PARTS_OVERLAY_CONFIG,
            (Self::P1AltParts, true) => &TRAINING_MODE_P1_2_PARTS_OVERLAY_CONFIG,
            (Self::P1Parts, false) => &TRAINING_COMPAT_P1_PARTS_OVERLAY_CONFIG,
            (Self::P1AltParts, false) => &TRAINING_COMPAT_P1_2_PARTS_OVERLAY_CONFIG,
        }
    }

    pub(super) fn is_match_root(self) -> bool {
        self == Self::MatchRoot
    }
}

#[derive(Debug, Copy, Clone)]
pub(super) struct CapturedHudLayout {
    pub(super) layout_data: u64,
    pub(super) kind: HudLayoutKind,
}

static HUD_KNOWN_LAYOUT_PROBE_COUNT: AtomicUsize = AtomicUsize::new(0);
static HUD_OTHER_LAYOUT_PROBE_COUNT: AtomicUsize = AtomicUsize::new(0);

pub(super) unsafe fn captured_layout(ctx: &InlineCtx) -> Option<CapturedHudLayout> {
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

pub(super) unsafe fn find_pane_in_layout_data(
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

unsafe fn layout_data_looks_like_match_root(layout_data: u64) -> bool {
    find_pane_in_layout_data(layout_data, P1_PARTS_PANE_NAME).is_some()
        && find_pane_in_layout_data(layout_data, ACTIVE_SKIN.root_pane_name).is_some()
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

pub(super) fn cstr_bytes_to_str(bytes: &[u8]) -> &str {
    std::ffi::CStr::from_bytes_with_nul(bytes)
        .ok()
        .and_then(|name| name.to_str().ok())
        .unwrap_or("<invalid>")
}
