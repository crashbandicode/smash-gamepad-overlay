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
const MIN_REASONABLE_POINTER: u64 = 0x10000;
const PANE_NAME_OFFSET: u64 = 0xb0;
const PANE_NAME_CAPACITY: usize = 25;
const ORIGINAL_PLAYER_MARKER_PANE_NAME: &[u8] = b"set_rep_01\0";

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
    if !is_plausible_pointer(layout_data) || !layout_data_has_relevant_panes(layout_data) {
        return None;
    }

    let layout_view = read_pointer_field(layout_data, 1)?;
    let layout_pane = read_pointer_field(layout_view, 3)?;
    let ui2d_pane = read_pointer_field(layout_pane, 0)?;
    let name_buffer = read_pane_name(ui2d_pane)?;
    let name = pane_name_bytes(&name_buffer);
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
    if !is_plausible_pointer(layout_data) {
        return 0;
    }

    type GetPaneByName = unsafe extern "C" fn(u64, *const c_char, ...) -> [u64; 4];
    let func_addr =
        (getRegionAddress(Region::Text) as *const u8).add(LAYOUT_GET_PANE_BY_NAME_OFFSET);
    let get_pane_by_name: GetPaneByName = std::mem::transmute(func_addr);
    let pane_udata = get_pane_by_name(layout_data, pane_name.as_ptr() as *const c_char);
    pane_udata[1]
}

unsafe fn pane_from_layout_handle(pane_handle: u64) -> *mut Pane {
    if !is_plausible_pointer(pane_handle) {
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

unsafe fn layout_data_has_relevant_panes(layout_data: u64) -> bool {
    find_pane_in_layout_data(layout_data, ACTIVE_SKIN.root_pane_name).is_some()
        || find_pane_in_layout_data(layout_data, P1_PARTS_PANE_NAME).is_some()
        || find_pane_in_layout_data(layout_data, ORIGINAL_PLAYER_MARKER_PANE_NAME).is_some()
}

unsafe fn read_pointer_field(base: u64, index: usize) -> Option<u64> {
    if !is_plausible_pointer(base) {
        return None;
    }

    let value = *((base as *const u64).add(index));
    is_plausible_pointer(value).then_some(value)
}

unsafe fn read_pane_name(ui2d_pane: u64) -> Option<[u8; PANE_NAME_CAPACITY]> {
    if !is_plausible_pointer(ui2d_pane) {
        return None;
    }

    let name_ptr = ui2d_pane.checked_add(PANE_NAME_OFFSET)? as *const u8;
    let mut name = [0; PANE_NAME_CAPACITY];
    ptr::copy_nonoverlapping(name_ptr, name.as_mut_ptr(), name.len());
    Some(name)
}

fn pane_name_bytes(name: &[u8; PANE_NAME_CAPACITY]) -> &[u8] {
    let len = name
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(name.len());
    &name[..len]
}

fn is_plausible_pointer(value: u64) -> bool {
    value >= MIN_REASONABLE_POINTER && value & 0x7 == 0
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
