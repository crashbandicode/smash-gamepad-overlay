use skyline::libc::c_char;
use skyline::nn::ui2d::{Layout, Pane, TextBox};
use std::ffi::CStr;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::{configured_display_mode, DisplayMode};
use crate::debug_text::draw_debug_text_overlay;
use crate::input::ControllerSnapshot;
use crate::logger::trace;
use crate::pane_utils::cstr_bytes_to_str;
use crate::skin::active_skin;
use crate::visual::{render_visual_overlay, VisualRenderError};

// 13.0.5: early nn::ui2d helper, unmoved from 13.0.4.
#[skyline::from_offset(0x59970)]
unsafe fn find_pane_by_name_recursive(pane: *const Pane, name: *const c_char) -> *mut Pane;

// 13.0.5: 13.0.4 `0x37a22f0` plus the `+0x5B0` ui2d shift.
#[skyline::from_offset(0x37a28a0)]
unsafe fn pane_set_text_string(pane: *mut TextBox, text: *const c_char);

static DISPLAY_MODE_LOGGED: AtomicBool = AtomicBool::new(false);
static VISUAL_FALLBACK_LOGGED: AtomicBool = AtomicBool::new(false);
static VISUAL_GUARD_LOGGED: AtomicBool = AtomicBool::new(false);

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
                if matches!(error, VisualRenderError::LifetimeGuardInactive) {
                    log_lifetime_guard_inactive();
                } else {
                    log_visual_fallback(error);
                    draw_debug_text_overlay(root_pane, snapshot);
                }
            }
        }
    }
}

pub(crate) unsafe fn find_pane_by_name(root_pane: *mut Pane, name: &[u8]) -> *mut Pane {
    find_pane_by_name_recursive(root_pane, name.as_ptr() as *const c_char)
}

/// Longest text accepted by `set_textbox_text`, in bytes, before the NUL terminator.
/// `debug_text` sizes its line buffers to this so a formatted line is never dropped here.
pub(crate) const TEXTBOX_TEXT_CAP: usize = 256;

pub(crate) unsafe fn set_textbox_text(textbox: &mut TextBox, text: &str) {
    // Interior NUL matches the old CString::new failure, which skipped the write.
    // The buffer is one byte longer than the cap so the terminator always stays zero.
    if text.len() > TEXTBOX_TEXT_CAP || text.as_bytes().contains(&0) {
        return;
    }

    let mut stack = [0u8; TEXTBOX_TEXT_CAP + 1];
    stack[..text.len()].copy_from_slice(text.as_bytes());
    pane_set_text_string(textbox, stack.as_ptr() as *const c_char);
}

fn log_lifetime_guard_inactive() {
    if VISUAL_GUARD_LOGGED.swap(true, Ordering::Relaxed) {
        return;
    }

    trace(
        "visual pane cache inactive because the pane finalize observer is not guarding lifetimes; overlay not drawn",
    );
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
                active_skin().name,
                cstr_bytes_to_str(pane_name),
                active_skin().expected_layout_flavor
            ));
            trace(
                "regenerate and reinstall the layout with tools/sgpo_installer (README, Install)",
            );
        }
        VisualRenderError::LifetimeGuardInactive => {
            trace("visual mode did not fall back to debug text because cached panes are disabled");
        }
        VisualRenderError::UserAllocatedPane {
            skin_name,
            pane_name,
        } => {
            trace(&format!(
                "visual mode refused user-allocated skin '{skin_name}' pane '{}'",
                cstr_bytes_to_str(pane_name)
            ));
        }
        VisualRenderError::SkinTooLarge {
            skin_name,
            element_count,
            max_elements,
        } => {
            trace(&format!(
                "visual mode fallback: skin '{skin_name}' has {element_count} elements but runtime supports at most {max_elements}"
            ));
            trace("split the skin or raise MAX_RESOLVED_SKIN_ELEMENTS before activating it");
        }
    }
}
