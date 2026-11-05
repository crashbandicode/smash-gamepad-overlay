#![allow(non_snake_case)]

mod config;
mod final_input;
mod hud;
mod input;
mod logger;
mod offsets;
mod pane_lifetime;
mod pane_utils;
mod scene;
mod skin;
mod ui;
#[cfg(feature = "diagnostic-pane-userdata")]
mod userdata_probe;
mod visual;

mod build_info {
    include!(concat!(env!("OUT_DIR"), "/sgpo_build_info.rs"));
}

use crate::hud::install_non_draw_hud_hooks;
use crate::input::logical_control_count;
use crate::logger::{log_startup_banner, reset_trace_file, trace, StartupBanner};
use crate::offsets::display_version;
use crate::pane_lifetime::install_pane_finalize_observer;
use crate::skin::{
    active_skin, built_in_asset_metadata_count, built_in_skin_count, reload_active_skin_config,
};

#[skyline::main(name = "smash-gamepad-overlay")]
pub fn main() {
    #[cfg(feature = "diagnostic-pane-userdata")]
    if userdata_probe::run_probe_only_if_marked(build_info::BUILD_ID) {
        return;
    }

    reset_trace_file();
    reload_active_skin_config("startup");
    let smash_display_version = display_version();
    crate::final_input::install(&smash_display_version);
    log_startup_banner(StartupBanner {
        build_id: build_info::BUILD_ID,
        git_change_count: build_info::GIT_CHANGE_COUNT,
        local_build_number: build_info::LOCAL_BUILD_NUMBER,
        display_version: &smash_display_version,
        logical_control_count: logical_control_count(),
        active_skin: active_skin().name,
        built_in_skin_count: built_in_skin_count(),
        asset_metadata_count: built_in_asset_metadata_count(),
    });

    #[cfg(feature = "diagnostic-pane-userdata")]
    userdata_probe::arm_with_overlay(build_info::BUILD_ID, &smash_display_version);

    let pane_finalize_ready = install_pane_finalize_observer(&smash_display_version);
    scene::init(&smash_display_version);

    // One render path, whatever else is installed: the overlay lives in the match HUD's P1 parts layout, found
    // from the HUD's own set-info-alpha call and updated from the scene update. Up to 0.3.0 the plugin used this
    // path only when it found Training Modpack's NRO by file name, and otherwise hooked Layout::Draw and drew into
    // the root info_melee layout (bottom right, full size). Replacing Training Modpack with a plugin of another
    // name (TrainingRooms, 2026-11-05) silently switched it to that second path.
    trace("rendering in the match HUD's player parts (HUD path); other plugins are not looked up");
    trace("the patched info_melee layout must be installed as a normal layout replacement");
    if pane_finalize_ready {
        install_non_draw_hud_hooks();
    } else {
        trace("overlay not installed because the pane finalize observer guard failed");
    }
}
