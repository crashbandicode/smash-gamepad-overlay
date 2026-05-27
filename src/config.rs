#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum DisplayMode {
    DebugText,
    Visual,
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct OverlayConfig {
    pub mode: DisplayMode,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub opacity: u8,
}

pub(crate) const PLUGIN_NAME: &str = "smash-gamepad-overlay";
pub(crate) const LOG_PATH: &str = "sd:/smash-gamepad-overlay.log";

pub(crate) const MATCH_HUD_LAYOUT: &str = "info_melee";
pub(crate) const TRAINING_MODPACK_PLUGIN_PATH: &str =
    "sd:/atmosphere/contents/01006A800016E000/romfs/skyline/plugins/libtraining_modpack.nro";
pub(crate) const HIDE_TRAINING_GAMEPAD_FLAG_PATH: &str =
    "sd:/ultimate/mods/smash-gamepad-overlay/HIDE_TRAINING_GAMEPAD";

pub(crate) const NPAD_ID_NO1: u32 = 0;
pub(crate) const NPAD_ID_HANDHELD: u32 = 0x20;

pub(crate) const NPAD_STYLE_FULL_KEY: u32 = 1 << 0;
pub(crate) const NPAD_STYLE_HANDHELD: u32 = 1 << 1;
pub(crate) const NPAD_STYLE_JOY_DUAL: u32 = 1 << 2;
pub(crate) const NPAD_STYLE_JOY_LEFT: u32 = 1 << 3;
pub(crate) const NPAD_STYLE_JOY_RIGHT: u32 = 1 << 4;
pub(crate) const NPAD_STYLE_GAMECUBE: u32 = 1 << 5;

pub(crate) const LEGACY_DRAW_OFFSET: usize = 0x4b620;
pub(crate) const HUD_SET_INFO_ALPHA_OFFSET: usize = 0x1b6cc08;
pub(crate) const SCENE_UPDATE_OFFSET: usize = 0x3747b7c;
pub(crate) const HUD_MATCH_START_OFFSET: usize = 0x1345558;
pub(crate) const HUD_MATCH_END_OFFSET: usize = 0x1d68b94;
pub(crate) const LAYOUT_GET_PANE_BY_NAME_OFFSET: usize = 0x3776360;
pub(crate) const BEST_MATCHES_TO_LOG: usize = 3;
pub(crate) const TEXT_SCAN_ALIGNMENT: usize = 4;

pub(crate) const DEBUG_TEXT_LINE_COUNT: usize = 4;
pub(crate) const MAX_OVERLAY_TEXT_PANES: usize = 24;
pub(crate) const STICK_AXIS_MAX: f32 = 32767.0;
pub(crate) const VISUAL_TRIGGER_ACTIVE_THRESHOLD: u32 = 25;

pub(crate) const P1_PARTS_PANE_NAME: &[u8] = b"p1\0";

// Adjust these first when moving or resizing the overlay.
pub(crate) const OVERLAY_CONFIG: OverlayConfig = OverlayConfig {
    mode: DisplayMode::Visual,
    x: 760.0,
    y: -330.0,
    scale: 1.0,
    opacity: 255,
};

pub(crate) const TRAINING_COMPAT_P1_PARTS_OVERLAY_CONFIG: OverlayConfig = OverlayConfig {
    mode: DisplayMode::Visual,
    x: 315.0,
    y: -30.0,
    scale: 0.55,
    opacity: 255,
};

pub(crate) const TRAINING_COMPAT_P1_2_PARTS_OVERLAY_CONFIG: OverlayConfig = OverlayConfig {
    mode: DisplayMode::Visual,
    x: 160.0,
    y: -80.0,
    scale: 0.5,
    opacity: 255,
};

pub(crate) const TRAINING_MODE_P1_PARTS_OVERLAY_CONFIG: OverlayConfig = OverlayConfig {
    mode: DisplayMode::Visual,
    x: -240.0,
    y: -30.0,
    scale: 0.55,
    opacity: 255,
};

pub(crate) const TRAINING_MODE_P1_2_PARTS_OVERLAY_CONFIG: OverlayConfig = OverlayConfig {
    mode: DisplayMode::Visual,
    x: -85.0,
    y: -80.0,
    scale: 0.5,
    opacity: 255,
};

pub(crate) fn configured_display_mode() -> DisplayMode {
    match option_env!("SMASH_GAMEPAD_OVERLAY_MODE") {
        Some("debug_text") | Some("DebugText") => DisplayMode::DebugText,
        _ => OVERLAY_CONFIG.mode,
    }
}

pub(crate) const DEBUG_TEXT_WIDTH: f32 = 760.0;
pub(crate) const DEBUG_TEXT_POS_X: f32 = -140.0;
pub(crate) const DEBUG_TEXT_POS_Y: f32 = 285.0;
pub(crate) const DEBUG_TEXT_FONT_SIZE: f32 = 16.0;
pub(crate) const DEBUG_TEXT_LINE_HEIGHT: f32 = 19.0;

pub(crate) const OVERLAY_LINE_TEXT_PANES: [&[u8]; 18] = [
    b"set_txt_00\0",
    b"set_txt_01\0",
    b"set_txt_02\0",
    b"set_txt_03\0",
    b"set_txt_04\0",
    b"set_txt_name_00\0",
    b"set_txt_name_01\0",
    b"txt_name_00\0",
    b"txt_name_01\0",
    b"txt_name_02\0",
    b"txt_name_03\0",
    b"set_txt_name\0",
    b"set_txt_player_name\0",
    b"txt_player_name\0",
    b"txt_name\0",
    b"name\0",
    b"set_txt\0",
    b"txt\0",
];
