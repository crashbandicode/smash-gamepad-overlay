use skyline::libc::c_char;
use skyline::nn::ui2d::{Layout, Pane, TextBox};
use std::ffi::{CStr, CString};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::{configured_display_mode, DisplayMode};
use crate::debug_text::draw_debug_text_overlay;
use crate::input::ControllerSnapshot;
use crate::logger::trace;
use crate::skin::ACTIVE_SKIN;
use crate::visual::{render_visual_overlay, VisualRenderError};

#[skyline::from_offset(0x59970)]
unsafe fn find_pane_by_name_recursive(pane: *const Pane, name: *const c_char) -> *mut Pane;

#[skyline::from_offset(0x37a22f0)]
unsafe fn pane_set_text_string(pane: *mut TextBox, text: *const c_char);

static DISPLAY_MODE_LOGGED: AtomicBool = AtomicBool::new(false);
static VISUAL_FALLBACK_LOGGED: AtomicBool = AtomicBool::new(false);

pub(crate) unsafe fn layout_name_is(layout: *const Layout, expected: &str) -> bool {
    if (*layout).layout_name.is_null() {
        return false;
    }

    CStr::from_ptr((*layout).layout_name)
        .to_str()
        .map(|name| name == expected)
        .unwrap_or(false)
}

pub(crate) unsafe fn draw_overlay(
    layout: *mut Layout,
    root_pane: *mut Pane,
    snapshot: Option<ControllerSnapshot>,
) {
    if root_pane.is_null() {
        return;
    }

    let display_mode = configured_display_mode();
    if !DISPLAY_MODE_LOGGED.swap(true, Ordering::Relaxed) {
        trace(&format!("display mode {display_mode:?}"));
    }

    match display_mode {
        DisplayMode::DebugText => draw_debug_text_overlay(root_pane, snapshot),
        DisplayMode::Visual => {
            if let Err(error) = render_visual_overlay(layout, root_pane, snapshot) {
                log_visual_fallback(error);
                draw_debug_text_overlay(root_pane, snapshot);
            }
        }
    }
}

pub(crate) unsafe fn find_pane_by_name(root_pane: *mut Pane, name: &[u8]) -> *mut Pane {
    find_pane_by_name_recursive(root_pane, name.as_ptr() as *const c_char)
}

pub(crate) unsafe fn set_textbox_text(textbox: &mut TextBox, text: &str) {
    if let Ok(c_text) = CString::new(text) {
        pane_set_text_string(textbox, c_text.as_ptr());
    }
}

fn log_visual_fallback(error: VisualRenderError) {
    if VISUAL_FALLBACK_LOGGED.swap(true, Ordering::Relaxed) {
        return;
    }

    match error {
        VisualRenderError::MissingSkinPane {
            skin_name,
            pane_name,
        } => {
            trace(&format!(
                "visual mode fallback: skin '{skin_name}' pane '{}' is missing",
                cstr_bytes_to_str(pane_name)
            ));
            trace(&format!(
                "skin/layout mismatch: active skin '{}' missing first pane '{}'; expected layout flavor: {}",
                ACTIVE_SKIN.name,
                cstr_bytes_to_str(pane_name),
                ACTIVE_SKIN.expected_layout_flavor
            ));
            trace(
                "regenerate layout with `python tools/patch_info_melee_layout.py`, then stage with `python tools/stage_arcropolis_layout.py`",
            );
        }
    }
}

fn cstr_bytes_to_str(bytes: &[u8]) -> &str {
    CStr::from_bytes_with_nul(bytes)
        .ok()
        .and_then(|name| name.to_str().ok())
        .unwrap_or("<invalid>")
}
