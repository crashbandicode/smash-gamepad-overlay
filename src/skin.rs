use crate::input::{ControlId, ControllerFamily, ControllerSnapshot};
use crate::logger::trace;
use std::fs;
use std::io;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::config::SKIN_CONFIG_PATH;

const DEFAULT_SIMPLE_SKIN_INDEX: usize = 0;
const DEFAULT_SIMPLE_GAMECUBE_SKIN_INDEX: usize = 1;
const FALLBACK_SKIN_INDEX: usize = DEFAULT_SIMPLE_SKIN_INDEX;
const AUTO_ACTIVE_SKIN_NAME: &str = "auto";
static ACTIVE_SKIN_INDEX: AtomicUsize = AtomicUsize::new(FALLBACK_SKIN_INDEX);
static AUTO_SKIN_ENABLED: AtomicBool = AtomicBool::new(false);
static AUTO_SWITCH_SKIN_INDEX: AtomicUsize = AtomicUsize::new(DEFAULT_SIMPLE_SKIN_INDEX);
static AUTO_GAMECUBE_SKIN_INDEX: AtomicUsize = AtomicUsize::new(DEFAULT_SIMPLE_GAMECUBE_SKIN_INDEX);

#[derive(Debug, Copy, Clone)]
pub(crate) struct StickMovementRange {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct SkinElement {
    pub control_id: ControlId,
    pub pane_name: &'static [u8],
    /// Source image name consumed by the future PC-side converter; ignored by the runtime plugin.
    pub image_name: Option<&'static str>,
    /// Generated/source material name consumed by the future PC-side converter; ignored by the runtime plugin.
    pub material_name: Option<&'static str>,
    pub base_x: f32,
    pub base_y: f32,
    pub size_x: f32,
    pub size_y: f32,
    pub released_alpha: u8,
    pub pressed_alpha: u8,
    pub released_scale: f32,
    pub pressed_scale: f32,
    pub released_visible: bool,
    pub pressed_visible: bool,
    pub stick_movement: Option<StickMovementRange>,
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct BuiltInSkin {
    pub name: &'static str,
    pub root_pane_name: &'static [u8],
    pub expected_layout_flavor: &'static str,
    pub root_scale: f32,
    pub elements: &'static [SkinElement],
}

static BUILT_IN_SKINS: [BuiltInSkin; 4] = [
    DEFAULT_SIMPLE_SKIN,
    DEFAULT_SIMPLE_GAMECUBE_SKIN,
    SWITCH_PRO_ALT_BUILTIN_SKIN,
    GAMECUBE_TRON_BUILTIN_SKIN,
];

pub(crate) fn active_skin() -> &'static BuiltInSkin {
    let index = ACTIVE_SKIN_INDEX.load(Ordering::Relaxed);
    BUILT_IN_SKINS
        .get(index)
        .unwrap_or(&BUILT_IN_SKINS[FALLBACK_SKIN_INDEX])
}

pub(crate) fn select_active_skin_for_snapshot(
    snapshot: Option<ControllerSnapshot>,
    reason: &str,
) -> &'static BuiltInSkin {
    if !AUTO_SKIN_ENABLED.load(Ordering::Relaxed) {
        return active_skin();
    }

    let Some(snapshot) = snapshot else {
        return active_skin();
    };

    let selected_index = match snapshot.controller_family() {
        ControllerFamily::Switch => AUTO_SWITCH_SKIN_INDEX.load(Ordering::Relaxed),
        ControllerFamily::GameCube => AUTO_GAMECUBE_SKIN_INDEX.load(Ordering::Relaxed),
    };

    store_active_skin_index(selected_index, "auto", reason);
    active_skin()
}

pub(crate) fn reload_active_skin_config(reason: &str) {
    match selected_skin_config() {
        SkinConfigSelection::Fixed {
            selected_index,
            source,
        } => {
            AUTO_SKIN_ENABLED.store(false, Ordering::Relaxed);
            store_active_skin_index(selected_index, source, reason);
        }
        SkinConfigSelection::Auto {
            switch_index,
            gamecube_index,
            source,
        } => {
            AUTO_SWITCH_SKIN_INDEX.store(switch_index, Ordering::Relaxed);
            AUTO_GAMECUBE_SKIN_INDEX.store(gamecube_index, Ordering::Relaxed);
            AUTO_SKIN_ENABLED.store(true, Ordering::Relaxed);
            store_active_skin_index(switch_index, source, reason);
            trace(&format!(
                "auto skin selection enabled from {source}: switch='{}' gamecube='{}' ({reason})",
                BUILT_IN_SKINS[switch_index].name, BUILT_IN_SKINS[gamecube_index].name
            ));
        }
    }
}

pub(crate) fn built_in_skin_count() -> usize {
    BUILT_IN_SKINS.len()
}

pub(crate) fn built_in_asset_metadata_count() -> usize {
    BUILT_IN_SKINS
        .iter()
        .flat_map(|skin| skin.elements.iter())
        .filter(|element| element.image_name.is_some() || element.material_name.is_some())
        .count()
}

enum SkinConfigSelection {
    Fixed {
        selected_index: usize,
        source: &'static str,
    },
    Auto {
        switch_index: usize,
        gamecube_index: usize,
        source: &'static str,
    },
}

fn selected_skin_config() -> SkinConfigSelection {
    let contents = match fs::read_to_string(SKIN_CONFIG_PATH) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return fixed_selection(FALLBACK_SKIN_INDEX, "missing config fallback");
        }
        Err(error) => {
            trace(&format!(
                "could not read skin config {SKIN_CONFIG_PATH}: {error}; using '{}'",
                BUILT_IN_SKINS[FALLBACK_SKIN_INDEX].name
            ));
            return fixed_selection(FALLBACK_SKIN_INDEX, "read-error fallback");
        }
    };

    let Some(active_skin_name) = json_string_field(&contents, "active_skin") else {
        trace(&format!(
            "skin config {SKIN_CONFIG_PATH} does not contain string field 'active_skin'; using '{}'",
            BUILT_IN_SKINS[FALLBACK_SKIN_INDEX].name
        ));
        return fixed_selection(FALLBACK_SKIN_INDEX, "invalid config fallback");
    };

    if active_skin_name
        .trim()
        .eq_ignore_ascii_case(AUTO_ACTIVE_SKIN_NAME)
    {
        return SkinConfigSelection::Auto {
            switch_index: configured_default_skin_index(
                &contents,
                "switch",
                DEFAULT_SIMPLE_SKIN_INDEX,
            ),
            gamecube_index: configured_default_skin_index(
                &contents,
                "gamecube",
                DEFAULT_SIMPLE_GAMECUBE_SKIN_INDEX,
            ),
            source: SKIN_CONFIG_PATH,
        };
    }

    match built_in_skin_index(&active_skin_name) {
        Some(index) => fixed_selection(index, SKIN_CONFIG_PATH),
        None => {
            trace(&format!(
                "skin config requested unknown skin '{active_skin_name}'; using '{}'",
                BUILT_IN_SKINS[FALLBACK_SKIN_INDEX].name
            ));
            fixed_selection(FALLBACK_SKIN_INDEX, "unknown-skin fallback")
        }
    }
}

fn fixed_selection(selected_index: usize, source: &'static str) -> SkinConfigSelection {
    SkinConfigSelection::Fixed {
        selected_index,
        source,
    }
}

fn configured_default_skin_index(contents: &str, field: &str, fallback_index: usize) -> usize {
    let Some(name) = json_string_field(contents, field) else {
        return fallback_index;
    };

    match built_in_skin_index(&name) {
        Some(index) => index,
        None => {
            trace(&format!(
                "skin config auto default '{field}' requested unknown skin '{name}'; using '{}'",
                BUILT_IN_SKINS[fallback_index].name
            ));
            fallback_index
        }
    }
}

fn store_active_skin_index(selected_index: usize, source: &'static str, reason: &str) {
    let previous_index = ACTIVE_SKIN_INDEX.swap(selected_index, Ordering::Relaxed);
    if previous_index != selected_index {
        trace(&format!(
            "active skin '{}' selected from {source} ({reason})",
            BUILT_IN_SKINS[selected_index].name
        ));
    }
}

fn built_in_skin_index(name: &str) -> Option<usize> {
    if name.trim().eq_ignore_ascii_case("minimal_debug") {
        return Some(DEFAULT_SIMPLE_SKIN_INDEX);
    }

    BUILT_IN_SKINS
        .iter()
        .position(|skin| skin.name.eq_ignore_ascii_case(name.trim()))
}

fn json_string_field(contents: &str, field: &str) -> Option<String> {
    let key = format!("\"{field}\"");
    let key_index = contents.find(&key)?;
    let after_key = &contents[key_index + key.len()..];
    let colon_index = after_key.find(':')?;
    let value = after_key[colon_index + 1..].trim_start();
    if !value.starts_with('"') {
        return None;
    }

    parse_json_string(value)
}

fn parse_json_string(value: &str) -> Option<String> {
    let mut output = String::new();
    let mut chars = value[1..].chars();

    while let Some(ch) = chars.next() {
        match ch {
            '"' => return Some(output),
            '\\' => {
                let escaped = chars.next()?;
                match escaped {
                    '"' | '\\' | '/' => output.push(escaped),
                    'n' => output.push('\n'),
                    'r' => output.push('\r'),
                    't' => output.push('\t'),
                    _ => return None,
                }
            }
            _ => output.push(ch),
        }
    }

    None
}

pub(crate) const DEFAULT_SIMPLE_SKIN: BuiltInSkin = BuiltInSkin {
    name: "default_simple",
    root_pane_name: b"sgpo_root\0",
    expected_layout_flavor:
        "default_simple sgpo_pro_* panes generated by tools/patch_info_melee_layout.py",
    root_scale: 1.0,
    elements: &DEFAULT_SIMPLE_ELEMENTS,
};

pub(crate) const DEFAULT_SIMPLE_GAMECUBE_SKIN: BuiltInSkin = BuiltInSkin {
    name: "default_simple_gamecube",
    root_pane_name: b"sgpo_root\0",
    expected_layout_flavor:
        "default_simple_gamecube sgpo_pro_* panes generated by tools/patch_info_melee_layout.py",
    root_scale: 1.0,
    elements: &DEFAULT_SIMPLE_GAMECUBE_ELEMENTS,
};

pub(crate) const SWITCH_PRO_ALT_BUILTIN_SKIN: BuiltInSkin = BuiltInSkin {
    name: "switch_pro_alt_builtin",
    root_pane_name: b"sgpo_root\0",
    expected_layout_flavor:
        "switch_pro_alt_builtin generated asset panes from the future skin converter",
    root_scale: SWITCH_PRO_ALT_ROOT_SCALE,
    elements: &SWITCH_PRO_ALT_ELEMENTS,
};

pub(crate) const GAMECUBE_TRON_BUILTIN_SKIN: BuiltInSkin = BuiltInSkin {
    name: "gamecube_tron_builtin",
    root_pane_name: b"sgpo_root\0",
    expected_layout_flavor:
        "gamecube_tron_builtin generated asset panes from the future skin converter",
    root_scale: GAMECUBE_TRON_ROOT_SCALE,
    elements: &GAMECUBE_TRON_ELEMENTS,
};

const DEFAULT_SIMPLE_ELEMENTS: [SkinElement; 24] = [
    minimal_button(ControlId::ZL, b"sgpo_pro_lt\0", -145.0, 122.0, 54.0, 20.0),
    minimal_button(ControlId::L, b"sgpo_pro_lb\0", -145.0, 95.0, 54.0, 20.0),
    minimal_button(ControlId::ZR, b"sgpo_pro_rt\0", 65.0, 122.0, 54.0, 20.0),
    minimal_button(ControlId::R, b"sgpo_pro_rb\0", 65.0, 95.0, 54.0, 20.0),
    minimal_button(
        ControlId::Minus,
        b"sgpo_pro_minus\0",
        -38.0,
        45.0,
        20.0,
        20.0,
    ),
    minimal_button(ControlId::Plus, b"sgpo_pro_plus\0", 20.0, 45.0, 20.0, 20.0),
    minimal_button(ControlId::L3, b"sgpo_pro_l3\0", -68.0, -30.0, 18.0, 18.0),
    minimal_button(ControlId::R3, b"sgpo_pro_r3\0", 62.0, -100.0, 18.0, 18.0),
    minimal_static_marker(
        ControlId::LeftStickGate,
        b"sgpo_pro_ls_gate\0",
        -105.0,
        -30.0,
        58.0,
        58.0,
        60,
    ),
    minimal_stick_dot(
        ControlId::LeftStickDot,
        b"sgpo_pro_ls_dot\0",
        -105.0,
        -30.0,
        14.0,
        14.0,
        22.0,
        22.0,
    ),
    minimal_static_marker(
        ControlId::RightStickGate,
        b"sgpo_pro_rs_gate\0",
        30.0,
        -100.0,
        58.0,
        58.0,
        60,
    ),
    minimal_stick_dot(
        ControlId::RightStickDot,
        b"sgpo_pro_rs_dot\0",
        30.0,
        -100.0,
        14.0,
        14.0,
        22.0,
        22.0,
    ),
    minimal_button(
        ControlId::DpadUp,
        b"sgpo_pro_du\0",
        -105.0,
        -78.0,
        16.0,
        16.0,
    ),
    minimal_button(
        ControlId::DpadDown,
        b"sgpo_pro_dd\0",
        -105.0,
        -128.0,
        16.0,
        16.0,
    ),
    minimal_button(
        ControlId::DpadLeft,
        b"sgpo_pro_dl\0",
        -130.0,
        -103.0,
        16.0,
        16.0,
    ),
    minimal_button(
        ControlId::DpadRight,
        b"sgpo_pro_dr\0",
        -80.0,
        -103.0,
        16.0,
        16.0,
    ),
    minimal_button(
        ControlId::DpadUpLeft,
        b"sgpo_pro_dul\0",
        -130.0,
        -78.0,
        13.0,
        13.0,
    ),
    minimal_button(
        ControlId::DpadUpRight,
        b"sgpo_pro_dur\0",
        -80.0,
        -78.0,
        13.0,
        13.0,
    ),
    minimal_button(
        ControlId::DpadDownLeft,
        b"sgpo_pro_ddl\0",
        -130.0,
        -128.0,
        13.0,
        13.0,
    ),
    minimal_button(
        ControlId::DpadDownRight,
        b"sgpo_pro_ddr\0",
        -80.0,
        -128.0,
        13.0,
        13.0,
    ),
    minimal_button(ControlId::Y, b"sgpo_pro_btn_y\0", 15.0, -20.0, 24.0, 24.0),
    minimal_button(ControlId::X, b"sgpo_pro_btn_x\0", 45.0, 10.0, 24.0, 24.0),
    minimal_button(ControlId::B, b"sgpo_pro_btn_b\0", 45.0, -50.0, 24.0, 24.0),
    minimal_button(
        ControlId::A,
        b"sgpo_pro_a_marker\0",
        75.0,
        -20.0,
        28.0,
        28.0,
    ),
];

const DEFAULT_SIMPLE_GAMECUBE_ELEMENTS: [SkinElement; 16] = [
    minimal_button(
        ControlId::GcLTrigger,
        b"sgpo_pro_lt\0",
        -145.0,
        122.0,
        54.0,
        20.0,
    ),
    minimal_button(
        ControlId::GcRTrigger,
        b"sgpo_pro_rt\0",
        65.0,
        122.0,
        54.0,
        20.0,
    ),
    minimal_button(ControlId::ZR, b"sgpo_pro_rb\0", 65.0, 95.0, 54.0, 20.0),
    minimal_button(ControlId::Plus, b"sgpo_pro_plus\0", 20.0, 45.0, 20.0, 20.0),
    minimal_static_marker(
        ControlId::LeftStickGate,
        b"sgpo_pro_ls_gate\0",
        -105.0,
        -30.0,
        58.0,
        58.0,
        60,
    ),
    minimal_stick_dot(
        ControlId::LeftStickDot,
        b"sgpo_pro_ls_dot\0",
        -105.0,
        -30.0,
        14.0,
        14.0,
        22.0,
        22.0,
    ),
    minimal_static_marker(
        ControlId::RightStickGate,
        b"sgpo_pro_rs_gate\0",
        30.0,
        -100.0,
        58.0,
        58.0,
        60,
    ),
    minimal_stick_dot(
        ControlId::RightStickDot,
        b"sgpo_pro_rs_dot\0",
        30.0,
        -100.0,
        14.0,
        14.0,
        22.0,
        22.0,
    ),
    minimal_button(
        ControlId::DpadUp,
        b"sgpo_pro_du\0",
        -105.0,
        -78.0,
        16.0,
        16.0,
    ),
    minimal_button(
        ControlId::DpadDown,
        b"sgpo_pro_dd\0",
        -105.0,
        -128.0,
        16.0,
        16.0,
    ),
    minimal_button(
        ControlId::DpadLeft,
        b"sgpo_pro_dl\0",
        -130.0,
        -103.0,
        16.0,
        16.0,
    ),
    minimal_button(
        ControlId::DpadRight,
        b"sgpo_pro_dr\0",
        -80.0,
        -103.0,
        16.0,
        16.0,
    ),
    minimal_button(ControlId::Y, b"sgpo_pro_btn_y\0", 15.0, -20.0, 24.0, 24.0),
    minimal_button(ControlId::X, b"sgpo_pro_btn_x\0", 45.0, 10.0, 24.0, 24.0),
    minimal_button(ControlId::B, b"sgpo_pro_btn_b\0", 45.0, -50.0, 24.0, 24.0),
    minimal_button(
        ControlId::A,
        b"sgpo_pro_a_marker\0",
        75.0,
        -20.0,
        28.0,
        28.0,
    ),
];

const SWITCH_PRO_ALT_ROOT_SCALE: f32 = 0.42;
const GAMECUBE_TRON_ROOT_SCALE: f32 = 0.62;

// Coordinates mirror RetroSpy's switch-pro-alt skin.xml against its 1280x965
// background. Runtime root_scale brings that source-space skin down to a
// practical in-game size while keeping the original coordinates readable.
const SWITCH_PRO_ALT_ELEMENTS: [SkinElement; 21] = [
    image_static(
        ControlId::SkinBackground,
        b"sgpo_alt_background\0",
        "background.png",
        0.0,
        0.0,
        1280.0,
        965.0,
    ),
    image_button(
        ControlId::B,
        b"sgpo_alt_face_b\0",
        "face_B.png",
        333.0,
        52.0,
        100.0,
        99.0,
    ),
    image_button(
        ControlId::A,
        b"sgpo_alt_face_a\0",
        "face_A.png",
        431.5,
        137.5,
        99.0,
        100.0,
    ),
    image_button(
        ControlId::X,
        b"sgpo_alt_face_x\0",
        "face_X.png",
        332.5,
        222.5,
        99.0,
        100.0,
    ),
    image_button(
        ControlId::Y,
        b"sgpo_alt_face_y\0",
        "face_Y.png",
        235.0,
        137.5,
        100.0,
        100.0,
    ),
    image_button(
        ControlId::Plus,
        b"sgpo_alt_plus\0",
        "center_Plus.png",
        155.5,
        232.5,
        61.0,
        62.0,
    ),
    image_button(
        ControlId::Minus,
        b"sgpo_alt_minus\0",
        "center_Minus.png",
        -155.5,
        232.5,
        61.0,
        62.0,
    ),
    image_button(
        ControlId::ZL,
        b"sgpo_alt_zl\0",
        "trigger_ZL.png",
        -364.5,
        408.0,
        227.0,
        133.0,
    ),
    image_button(
        ControlId::ZR,
        b"sgpo_alt_zr\0",
        "trigger_ZR.png",
        364.5,
        408.0,
        227.0,
        133.0,
    ),
    image_button(
        ControlId::L,
        b"sgpo_alt_l\0",
        "trigger_L.png",
        -334.5,
        347.0,
        327.0,
        113.0,
    ),
    image_button(
        ControlId::R,
        b"sgpo_alt_r\0",
        "trigger_R.png",
        335.5,
        347.0,
        327.0,
        113.0,
    ),
    image_button(
        ControlId::Home,
        b"sgpo_alt_home\0",
        "center_Home.png",
        89.5,
        137.0,
        63.0,
        63.0,
    ),
    image_button(
        ControlId::Capture,
        b"sgpo_alt_capture\0",
        "center_Capture.png",
        -89.0,
        137.5,
        58.0,
        58.0,
    ),
    image_button(
        ControlId::L3,
        b"sgpo_alt_l3\0",
        "stick_LS_Press.png",
        -347.0,
        137.5,
        206.0,
        204.0,
    ),
    image_button(
        ControlId::R3,
        b"sgpo_alt_r3\0",
        "stick_RS_Press.png",
        165.5,
        -36.5,
        205.0,
        204.0,
    ),
    image_button(
        ControlId::DpadUp,
        b"sgpo_alt_dpad_up\0",
        "dpad_Up.png",
        -194.0,
        11.0,
        68.0,
        97.0,
    ),
    image_button(
        ControlId::DpadDown,
        b"sgpo_alt_dpad_down\0",
        "dpad_Down.png",
        -194.0,
        -83.0,
        68.0,
        97.0,
    ),
    image_button(
        ControlId::DpadLeft,
        b"sgpo_alt_dpad_left\0",
        "dpad_Left.png",
        -241.0,
        -36.5,
        98.0,
        68.0,
    ),
    image_button(
        ControlId::DpadRight,
        b"sgpo_alt_dpad_right\0",
        "dpad_Right.png",
        -147.0,
        -36.0,
        98.0,
        67.0,
    ),
    image_stick(
        ControlId::LeftStickDot,
        b"sgpo_alt_left_stick\0",
        "stick_Left.png",
        -347.0,
        137.5,
        164.0,
        164.0,
        41.0,
        41.0,
    ),
    image_stick(
        ControlId::RightStickDot,
        b"sgpo_alt_right_stick\0",
        "stick_Right.png",
        165.0,
        -36.5,
        164.0,
        164.0,
        41.0,
        41.0,
    ),
];

// Coordinates mirror RetroSpy's gamecube-tron skin.xml against its 883x307
// background. Runtime root_scale keeps the source-space skin at match-HUD size.
const GAMECUBE_TRON_ELEMENTS: [SkinElement; 15] = [
    image_static(
        ControlId::SkinBackground,
        b"sgpo_gct_background\0",
        "background_0.png",
        0.0,
        0.0,
        883.0,
        307.0,
    ),
    image_button(
        ControlId::A,
        b"sgpo_gct_a\0",
        "a.png",
        286.0,
        -38.0,
        117.0,
        117.0,
    ),
    image_button(
        ControlId::B,
        b"sgpo_gct_b\0",
        "b.png",
        176.5,
        -75.5,
        74.0,
        74.0,
    ),
    image_button(
        ControlId::X,
        b"sgpo_gct_x\0",
        "x.png",
        393.5,
        -26.0,
        70.0,
        125.0,
    ),
    image_button(
        ControlId::Y,
        b"sgpo_gct_y\0",
        "y.png",
        266.5,
        63.0,
        124.0,
        73.0,
    ),
    image_button(
        ControlId::ZR,
        b"sgpo_gct_zr\0",
        "z.png",
        378.0,
        91.5,
        87.0,
        72.0,
    ),
    image_button(
        ControlId::Plus,
        b"sgpo_gct_plus\0",
        "start.png",
        94.5,
        85.5,
        58.0,
        58.0,
    ),
    image_button(
        ControlId::GcRTrigger,
        b"sgpo_gct_gc_r_trigger\0",
        "r.png",
        -107.0,
        107.0,
        207.0,
        39.0,
    ),
    image_button(
        ControlId::GcLTrigger,
        b"sgpo_gct_gc_l_trigger\0",
        "l.png",
        -324.5,
        107.0,
        208.0,
        39.0,
    ),
    image_button(
        ControlId::DpadUp,
        b"sgpo_gct_dpad_up\0",
        "up.png",
        60.5,
        -8.0,
        38.0,
        51.0,
    ),
    image_button(
        ControlId::DpadDown,
        b"sgpo_gct_dpad_down\0",
        "down.png",
        60.5,
        -79.0,
        38.0,
        51.0,
    ),
    image_button(
        ControlId::DpadLeft,
        b"sgpo_gct_dpad_left\0",
        "left.png",
        25.0,
        -43.5,
        51.0,
        38.0,
    ),
    image_button(
        ControlId::DpadRight,
        b"sgpo_gct_dpad_right\0",
        "right.png",
        96.0,
        -43.0,
        51.0,
        37.0,
    ),
    image_stick(
        ControlId::LeftStickDot,
        b"sgpo_gct_left_stick_dot\0",
        "lstick_xlstick_y.png",
        -326.0,
        -23.0,
        113.0,
        113.0,
        56.0,
        56.0,
    ),
    image_stick(
        ControlId::RightStickDot,
        b"sgpo_gct_right_stick_dot\0",
        "cstick_xcstick_y.png",
        -108.5,
        -24.5,
        80.0,
        80.0,
        50.0,
        50.0,
    ),
];

// Defaults used by all SkinElement builders. Each builder applies struct-update
// syntax on top of this base so only the fields that differ from a neutral,
// fully visible, non-stick element need to be spelled out at the call site.
const BLANK_ELEMENT: SkinElement = SkinElement {
    control_id: ControlId::A,
    pane_name: b"\0",
    image_name: None,
    material_name: None,
    base_x: 0.0,
    base_y: 0.0,
    size_x: 0.0,
    size_y: 0.0,
    released_alpha: 255,
    pressed_alpha: 255,
    released_scale: 1.0,
    pressed_scale: 1.0,
    released_visible: true,
    pressed_visible: true,
    stick_movement: None,
};

const fn minimal_button(
    control_id: ControlId,
    pane_name: &'static [u8],
    base_x: f32,
    base_y: f32,
    size_x: f32,
    size_y: f32,
) -> SkinElement {
    SkinElement {
        control_id,
        pane_name,
        base_x,
        base_y,
        size_x,
        size_y,
        released_alpha: 70,
        pressed_scale: 1.22,
        ..BLANK_ELEMENT
    }
}

const fn minimal_static_marker(
    control_id: ControlId,
    pane_name: &'static [u8],
    base_x: f32,
    base_y: f32,
    size_x: f32,
    size_y: f32,
    alpha: u8,
) -> SkinElement {
    SkinElement {
        control_id,
        pane_name,
        base_x,
        base_y,
        size_x,
        size_y,
        released_alpha: alpha,
        pressed_alpha: alpha,
        ..BLANK_ELEMENT
    }
}

const fn minimal_stick_dot(
    control_id: ControlId,
    pane_name: &'static [u8],
    base_x: f32,
    base_y: f32,
    size_x: f32,
    size_y: f32,
    movement_x: f32,
    movement_y: f32,
) -> SkinElement {
    SkinElement {
        control_id,
        pane_name,
        base_x,
        base_y,
        size_x,
        size_y,
        released_alpha: 190,
        stick_movement: Some(StickMovementRange {
            x: movement_x,
            y: movement_y,
        }),
        ..BLANK_ELEMENT
    }
}

const fn image_button(
    control_id: ControlId,
    pane_name: &'static [u8],
    image_name: &'static str,
    base_x: f32,
    base_y: f32,
    size_x: f32,
    size_y: f32,
) -> SkinElement {
    SkinElement {
        control_id,
        pane_name,
        image_name: Some(image_name),
        base_x,
        base_y,
        size_x,
        size_y,
        released_alpha: 70,
        pressed_scale: 1.05,
        ..BLANK_ELEMENT
    }
}

const fn image_stick(
    control_id: ControlId,
    pane_name: &'static [u8],
    image_name: &'static str,
    base_x: f32,
    base_y: f32,
    size_x: f32,
    size_y: f32,
    movement_x: f32,
    movement_y: f32,
) -> SkinElement {
    SkinElement {
        control_id,
        pane_name,
        image_name: Some(image_name),
        base_x,
        base_y,
        size_x,
        size_y,
        stick_movement: Some(StickMovementRange {
            x: movement_x,
            y: movement_y,
        }),
        ..BLANK_ELEMENT
    }
}

const fn image_static(
    control_id: ControlId,
    pane_name: &'static [u8],
    image_name: &'static str,
    base_x: f32,
    base_y: f32,
    size_x: f32,
    size_y: f32,
) -> SkinElement {
    SkinElement {
        control_id,
        pane_name,
        image_name: Some(image_name),
        base_x,
        base_y,
        size_x,
        size_y,
        released_alpha: 255,
        pressed_alpha: 255,
        ..BLANK_ELEMENT
    }
}
