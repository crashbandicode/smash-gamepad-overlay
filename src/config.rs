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
pub(crate) const SKYLINE_PLUGIN_DIR: &str =
    "sd:/atmosphere/contents/01006A800016E000/romfs/skyline/plugins";
pub(crate) const TRAINING_MODPACK_PLUGIN_PATH: &str =
    "sd:/atmosphere/contents/01006A800016E000/romfs/skyline/plugins/libtraining_modpack.nro";
pub(crate) const HIDE_TRAINING_GAMEPAD_FLAG_PATH: &str =
    "sd:/ultimate/mods/smash-gamepad-overlay/HIDE_TRAINING_GAMEPAD";
pub(crate) const FORCE_TRAINING_MODPACK_COMPAT_FLAG_PATH: &str =
    "sd:/ultimate/mods/smash-gamepad-overlay/FORCE_TRAINING_MODPACK_COMPAT";
pub(crate) const SKIN_CONFIG_PATH: &str = "sd:/ultimate/mods/smash-gamepad-overlay/config.json";

#[cfg(feature = "diagnostic-pane-userdata")]
pub(crate) const PROBE_LOG_PATH: &str = "sd:/smash-gamepad-overlay-probe.log";
#[cfg(feature = "diagnostic-pane-userdata")]
pub(crate) const PROBE_ONLY_FLAG_PATH: &str =
    "sd:/ultimate/mods/smash-gamepad-overlay/PROBE_ONLY";
/// `Pane::GetExtUserDataArray` on Smash 13.0.5. The inline observer runs
/// before the original load of `pane+0xa8`. The Picture path that faulted in
/// Count calls this Array entry from main+0x5f540; Count's return is
/// main+0x5f54c and is not this hook's caller.
#[cfg(feature = "diagnostic-pane-userdata")]
pub(crate) const GET_EXT_USER_DATA_ARRAY_OFFSET: usize = 0x59890;
#[cfg(feature = "diagnostic-pane-userdata")]
pub(crate) static GET_EXT_USER_DATA_ARRAY_PROLOGUE: [u8; 64] = [
    0x08, 0x54, 0x40, 0xf9, 0x08, 0x01, 0x00, 0xb4, 0x09, 0x60, 0x40, 0xb9, 0x09, 0x01, 0x00, 0x34,
    0x09, 0x11, 0x40, 0x79, 0x3f, 0x05, 0x00, 0x71, 0xe1, 0x00, 0x00, 0x54, 0xe0, 0x03, 0x1f, 0xaa,
    0xc0, 0x03, 0x5f, 0xd6, 0xe0, 0x03, 0x1f, 0xaa, 0xc0, 0x03, 0x5f, 0xd6, 0x00, 0x31, 0x00, 0x91,
    0xc0, 0x03, 0x5f, 0xd6, 0x00, 0x61, 0x00, 0x91, 0xc0, 0x03, 0x5f, 0xd6, 0x00, 0x00, 0x00, 0x00,
];

pub(crate) const NPAD_ID_NO1: u32 = 0;
pub(crate) const NPAD_ID_HANDHELD: u32 = 0x20;

pub(crate) const NPAD_STYLE_FULL_KEY: u32 = 1 << 0;
pub(crate) const NPAD_STYLE_HANDHELD: u32 = 1 << 1;
pub(crate) const NPAD_STYLE_JOY_DUAL: u32 = 1 << 2;
pub(crate) const NPAD_STYLE_JOY_LEFT: u32 = 1 << 3;
pub(crate) const NPAD_STYLE_JOY_RIGHT: u32 = 1 << 4;
pub(crate) const NPAD_STYLE_GAMECUBE: u32 = 1 << 5;

// Smash 13.0.5 `.text` offsets. `Layout::Draw` is still found by signature;
// `LEGACY_DRAW_OFFSET` is only the diagnostic address.
//
// Ported from the 13.0.4 map using HewDraw-Remix ea1eb702 ("13.0.5 offsets")
// and smashline fdc151f. Match start, set-info-alpha, and match end sit in
// ranges whose neighboring hooks did not move. Scene update and the ui2d
// helpers from about `0x30f6e00` through `0x38f86a0` moved by `+0x5B0`.
// HDR updated the same pane lookup (`0x3776360 -> 0x3776910`) and
// `set_text_string` (`0x37a22f0 -> 0x37a28a0`).
pub(crate) const LEGACY_DRAW_OFFSET: usize = 0x4b620;
pub(crate) const HUD_SET_INFO_ALPHA_OFFSET: usize = 0x1b6cc08;
/// `nn::ui2d::Pane::Finalize` on Smash 13.0.5. Inline observer only; the
/// original prologue must still run.
pub(crate) const PANE_FINALIZE_OFFSET: usize = 0x57f10;
/// First 16 instructions at `PANE_FINALIZE_OFFSET` from main build
/// `21450C647B8C5940ED5B68218EA128FD3C5D597D` (NSO text at file `0x100+offset`).
pub(crate) static PANE_FINALIZE_PROLOGUE: [u8; 64] = [
    0xfc, 0x6f, 0xba, 0xa9, 0xfa, 0x67, 0x01, 0xa9, 0xf8, 0x5f, 0x02, 0xa9, 0xf6, 0x57, 0x03, 0xa9,
    0xf4, 0x4f, 0x04, 0xa9, 0xfd, 0x7b, 0x05, 0xa9, 0xfd, 0x43, 0x01, 0x91, 0x08, 0x60, 0x40, 0xb9,
    0xf4, 0x03, 0x01, 0xaa, 0xf3, 0x03, 0x00, 0xaa, 0x48, 0x03, 0x88, 0x36, 0x48, 0x02, 0x00, 0x34,
    0x69, 0x56, 0x40, 0xf9, 0x28, 0x31, 0x00, 0x91, 0x29, 0x11, 0x40, 0xb9, 0x2a, 0x11, 0x00, 0x91,
];
/// `nn::ui2d::Layout::Finalize` on Smash 13.0.5. Diagnostic counts only.
pub(crate) const LAYOUT_FINALIZE_OFFSET: usize = 0x48e50;
/// First 16 instructions at `LAYOUT_FINALIZE_OFFSET` from the same main build.
pub(crate) static LAYOUT_FINALIZE_PROLOGUE: [u8; 64] = [
    0xf8, 0x5f, 0xbc, 0xa9, 0xf6, 0x57, 0x01, 0xa9, 0xf4, 0x4f, 0x02, 0xa9, 0xfd, 0x7b, 0x03, 0xa9,
    0xfd, 0xc3, 0x00, 0x91, 0xf3, 0x03, 0x00, 0xaa, 0x00, 0x2c, 0x40, 0xf9, 0xf4, 0x03, 0x01, 0xaa,
    0xa0, 0x02, 0x00, 0xb4, 0x08, 0x04, 0x40, 0xf9, 0x68, 0x01, 0x00, 0xb4, 0xe9, 0x92, 0x02, 0x90,
    0x29, 0x1d, 0x43, 0xf9, 0xea, 0x92, 0x02, 0x90, 0x29, 0x01, 0x40, 0xf9, 0x4a, 0x21, 0x43, 0xf9,
];
pub(crate) const SCENE_UPDATE_OFFSET: usize = 0x374812c;
pub(crate) const HUD_MATCH_START_OFFSET: usize = 0x1345558;
pub(crate) const HUD_MATCH_END_OFFSET: usize = 0x1d68b94;
pub(crate) const LAYOUT_GET_PANE_BY_NAME_OFFSET: usize = 0x3776910;
pub(crate) const BEST_MATCHES_TO_LOG: usize = 3;
pub(crate) const TEXT_SCAN_ALIGNMENT: usize = 4;

pub(crate) const DEBUG_TEXT_LINE_COUNT: usize = 4;
pub(crate) const MAX_OVERLAY_TEXT_PANES: usize = 24;
pub(crate) const STICK_AXIS_MAX: f32 = 32767.0;
pub(crate) const GC_TRIGGER_AXIS_MAX: f32 = STICK_AXIS_MAX;
pub(crate) const VISUAL_TRIGGER_ACTIVE_THRESHOLD: u32 = 3200;

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

pub(crate) const OVERLAY_LINE_TEXT_PANES: [&[u8]; 15] = [
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
];
