#![allow(non_snake_case)]

mod config;
mod debug_text;
mod hud;
mod input;
#[cfg(sgpo_embed_layout)]
mod layout_inject;
mod logger;
mod offsets;
mod skin;
mod ui;
mod visual;

mod build_info {
    include!(concat!(env!("OUT_DIR"), "/sgpo_build_info.rs"));
}

use skyline::nn::ui2d::Layout;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::MATCH_HUD_LAYOUT;
use crate::hud::install_non_draw_hud_hooks;
use crate::input::{logical_control_count, poll_p1_controller};
#[cfg(sgpo_embed_layout)]
use crate::layout_inject::install_layout_injection_hook;
use crate::logger::{reset_trace_file, trace};
use crate::offsets::{
    display_version, draw_hook_offset_for_install, training_modpack_plugin_present, OFFSET_DRAW,
};
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

            draw_overlay(layout, (*layout).root_pane, poll_p1_controller());
        }
    }

    original!()(layout, draw_info, cmd_buffer);
}

#[skyline::main(name = "smash-gamepad-overlay")]
pub fn main() {
    reset_trace_file();
    trace("starting P1 input overlay");
    trace(&format!("build {}", build_info::BUILD_ID));
    trace(&format!(
        "git change count {} local build {}",
        build_info::GIT_CHANGE_COUNT,
        build_info::LOCAL_BUILD_NUMBER
    ));
    trace(&format!(
        "embedded layout injection {}",
        if build_info::EMBEDDED_LAYOUT_ENABLED {
            "enabled"
        } else {
            "disabled"
        }
    ));
    trace(&format!("Smash display version {}", display_version()));
    trace(&format!(
        "registered {} logical controls",
        logical_control_count()
    ));

    let training_modpack_present = training_modpack_plugin_present();
    if training_modpack_present {
        trace(&format!(
            "detected Training Modpack plugin at {}",
            crate::config::TRAINING_MODPACK_PLUGIN_PATH
        ));
        trace("using non-draw HUD path and skipping shared layout/draw hooks");
        trace(
            "Training Modpack compatibility requires the patched info_melee layout to be installed as a normal layout replacement",
        );
    }

    if training_modpack_present {
        install_non_draw_hud_hooks();
        return;
    }

    #[cfg(sgpo_embed_layout)]
    install_layout_injection_hook();

    #[cfg(not(sgpo_embed_layout))]
    trace("embedded layout injection disabled; install patched info_melee layout as a normal data replacement");

    match *OFFSET_DRAW {
        Some(offset) => {
            trace(&format!("installing draw hook at .text+0x{offset:x}"));
            skyline::install_hooks!(handle_layout_draw);
        }
        None => {
            trace("draw hook not installed because Layout::Draw was not found");
        }
    }
}
