#![allow(non_snake_case)]

mod config;
mod debug_text;
mod hud;
mod input;
mod logger;
mod offsets;
mod pane_utils;
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
use crate::logger::{log_startup_banner, reset_trace_file, trace, StartupBanner};
use crate::offsets::{
    display_version, draw_hook_offset_for_install, training_modpack_plugin_present, OFFSET_DRAW,
};
use crate::skin::{built_in_asset_metadata_count, built_in_skin_count, ACTIVE_SKIN};
use crate::ui::{draw_overlay, layout_name_is};
use crate::visual::install_draw_path_visual_reset_hooks;

static DRAW_HOOK_LOGGED: AtomicBool = AtomicBool::new(false);
static MATCH_HUD_LAYOUT_LOGGED: AtomicBool = AtomicBool::new(false);
const SUPPORTED_DRAW_PATH_DISPLAY_VERSION: &str = "13.0.4";

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
    let smash_display_version = display_version();
    log_startup_banner(StartupBanner {
        build_id: build_info::BUILD_ID,
        git_change_count: build_info::GIT_CHANGE_COUNT,
        local_build_number: build_info::LOCAL_BUILD_NUMBER,
        display_version: &smash_display_version,
        logical_control_count: logical_control_count(),
        active_skin: ACTIVE_SKIN.name,
        built_in_skin_count: built_in_skin_count(),
        asset_metadata_count: built_in_asset_metadata_count(),
    });

    if training_modpack_plugin_present() {
        trace("Training Modpack compatibility mode enabled");
        trace(&format!(
            "compatibility triggers: standard Training Modpack path {}, matching *training*modpack*.nro in the plugin folder, or force flag {}",
            crate::config::TRAINING_MODPACK_PLUGIN_PATH,
            crate::config::FORCE_TRAINING_MODPACK_COMPAT_FLAG_PATH
        ));
        trace("using non-draw HUD path and skipping shared layout/draw hooks");
        trace(
            "Training Modpack compatibility requires the patched info_melee layout to be installed as a normal layout replacement",
        );
        install_non_draw_hud_hooks();
        return;
    }

    if smash_display_version != SUPPORTED_DRAW_PATH_DISPLAY_VERSION {
        trace(&format!(
            "draw path not installed for Smash display version {smash_display_version}; ui2d helper offsets are currently supported only for {SUPPORTED_DRAW_PATH_DISPLAY_VERSION}"
        ));
        return;
    }

    trace("install patched info_melee layout as a normal data replacement");

    install_draw_path_visual_reset_hooks();

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
