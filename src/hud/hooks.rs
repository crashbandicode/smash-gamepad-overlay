use skyline::hooks::InlineCtx;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::{
    HIDE_TRAINING_GAMEPAD_FLAG_PATH, HUD_MATCH_END_OFFSET, HUD_MATCH_START_OFFSET,
    HUD_SET_INFO_ALPHA_OFFSET, SCENE_UPDATE_OFFSET,
};
use crate::input::poll_view_state_now;
use crate::logger::trace;
use crate::offsets::display_version;
use crate::skin::reload_active_skin_config;

use super::cache;
use super::capture;

const SUPPORTED_NON_DRAW_DISPLAY_VERSION: &str = "13.0.4";

static HUD_HOOKS_LOGGED: AtomicBool = AtomicBool::new(false);
static HUD_SCENE_UPDATE_LOGGED: AtomicBool = AtomicBool::new(false);
static HUD_VISUAL_UPDATE_LOGGED: AtomicBool = AtomicBool::new(false);
static TRAINING_MODE_SUPPRESSED_LOGGED: AtomicBool = AtomicBool::new(false);
static TRAINING_MODE_ALLOWED_LOGGED: AtomicBool = AtomicBool::new(false);
static HUD_MATCH_ROOT_CAPTURED_LOGGED: AtomicBool = AtomicBool::new(false);
static HIDE_TRAINING_GAMEPAD_PRESENT: AtomicBool = AtomicBool::new(false);

#[skyline::hook(offset = HUD_SET_INFO_ALPHA_OFFSET, inline)]
unsafe fn capture_match_hud_layout_data(ctx: &InlineCtx) {
    if training_mode_overlay_disabled() {
        return;
    }

    let Some(captured) = capture::captured_layout(ctx) else {
        return;
    };
    if captured.kind.is_match_root()
        && !HUD_MATCH_ROOT_CAPTURED_LOGGED.swap(true, Ordering::Relaxed)
    {
        trace("non-draw HUD path found root info_melee layout; using bottom-right root overlay");
    }

    let training_mode = training_mode_active();
    cache::capture_runtime(captured);

    let view_state = poll_view_state_now();
    if cache::update_runtime(&view_state, training_mode)
        && !HUD_VISUAL_UPDATE_LOGGED.swap(true, Ordering::Relaxed)
    {
        trace("non-draw HUD path updated visual panes");
    }
}

#[skyline::hook(offset = SCENE_UPDATE_OFFSET, inline)]
unsafe fn update_match_hud_overlay_from_scene(_: &InlineCtx) {
    if !HUD_SCENE_UPDATE_LOGGED.swap(true, Ordering::Relaxed) {
        trace("non-draw HUD scene update hook fired");
    }

    if training_mode_overlay_disabled() {
        cache::reset_runtime();
        return;
    }

    let view_state = poll_view_state_now();
    if cache::update_runtime(&view_state, training_mode_active())
        && !HUD_VISUAL_UPDATE_LOGGED.swap(true, Ordering::Relaxed)
    {
        trace("non-draw HUD path updated visual panes");
    }
}

#[skyline::hook(offset = HUD_MATCH_START_OFFSET, inline)]
unsafe fn reset_match_hud_capture_on_start(_: &InlineCtx) {
    reload_active_skin_config("match start");
    refresh_hide_training_gamepad_flag();
    cache::reset_runtime();
}

#[skyline::hook(offset = HUD_MATCH_END_OFFSET, inline)]
unsafe fn reset_match_hud_capture_on_end(_: &InlineCtx) {
    cache::reset_runtime();
}

pub(crate) fn install_non_draw_hud_hooks() {
    let version = display_version();
    trace(&format!(
        "non-draw HUD hooks enabled only for display version {SUPPORTED_NON_DRAW_DISPLAY_VERSION}"
    ));

    if version != SUPPORTED_NON_DRAW_DISPLAY_VERSION {
        trace(&format!(
            "non-draw HUD hooks not installed for Smash display version {version}; supported version is {SUPPORTED_NON_DRAW_DISPLAY_VERSION}"
        ));
        return;
    }

    if !HUD_HOOKS_LOGGED.swap(true, Ordering::Relaxed) {
        trace(&format!(
            "installing non-draw HUD hooks at .text+0x{HUD_SET_INFO_ALPHA_OFFSET:x}/0x{SCENE_UPDATE_OFFSET:x}/0x{HUD_MATCH_START_OFFSET:x}/0x{HUD_MATCH_END_OFFSET:x}"
        ));
    }

    skyline::install_hooks!(
        capture_match_hud_layout_data,
        update_match_hud_overlay_from_scene,
        reset_match_hud_capture_on_start,
        reset_match_hud_capture_on_end
    );
}

fn refresh_hide_training_gamepad_flag() {
    let hidden = Path::new(HIDE_TRAINING_GAMEPAD_FLAG_PATH).exists();
    HIDE_TRAINING_GAMEPAD_PRESENT.store(hidden, Ordering::Relaxed);
}

fn training_mode_overlay_disabled() -> bool {
    if !training_mode_active() {
        return false;
    }

    let disabled = HIDE_TRAINING_GAMEPAD_PRESENT.load(Ordering::Relaxed);
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

fn training_mode_active() -> bool {
    unsafe { is_training_mode() }
}

extern "C" {
    #[link_name = "\u{1}_ZN3app9smashball16is_training_modeEv"]
    fn is_training_mode() -> bool;
}
