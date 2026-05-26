#![allow(non_snake_case)]

mod config;
mod debug_text;
mod input;
mod layout_inject;
mod logger;
mod offsets;
mod skin;
mod ui;
mod visual;

use skyline::nn::ui2d::Layout;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::MATCH_HUD_LAYOUT;
use crate::input::{logical_control_count, poll_p1_controller};
use crate::layout_inject::install_layout_injection_hook;
use crate::logger::{reset_trace_file, trace};
use crate::offsets::{draw_hook_offset_for_install, OFFSET_DRAW};
use crate::ui::{draw_overlay, layout_name_is};

static DRAW_HOOK_LOGGED: AtomicBool = AtomicBool::new(false);
static MATCH_HUD_LAYOUT_LOGGED: AtomicBool = AtomicBool::new(false);

#[skyline::hook(offset = draw_hook_offset_for_install())]
unsafe fn handle_layout_draw(layout: *mut Layout, draw_info: u64, cmd_buffer: u64) {
    if !layout.is_null() {
        if !DRAW_HOOK_LOGGED.swap(true, Ordering::Relaxed) {
            trace("Layout::Draw hook fired");
        }

        if layout_name_is(layout, MATCH_HUD_LAYOUT) {
            if !MATCH_HUD_LAYOUT_LOGGED.swap(true, Ordering::Relaxed) {
                trace(&format!(
                    "saw {MATCH_HUD_LAYOUT} root={:p}",
                    (*layout).root_pane
                ));
            }

            draw_overlay((*layout).root_pane, poll_p1_controller());
        }
    }

    original!()(layout, draw_info, cmd_buffer);
}

#[skyline::main(name = "smash-gamepad-overlay")]
pub fn main() {
    reset_trace_file();
    trace("starting P1 input overlay");
    trace(&format!(
        "registered {} logical controls",
        logical_control_count()
    ));

    match *OFFSET_DRAW {
        Some(offset) => {
            trace(&format!("installing draw hook at .text+0x{offset:x}"));
            install_layout_injection_hook();
            skyline::install_hooks!(handle_layout_draw);
        }
        None => {
            trace("draw hook not installed because Layout::Draw was not found");
        }
    }
}
