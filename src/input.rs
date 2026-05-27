use skyline::nn::hid;

use crate::config::{
    NPAD_ID_HANDHELD, NPAD_ID_NO1, NPAD_STYLE_FULL_KEY, NPAD_STYLE_GAMECUBE, NPAD_STYLE_HANDHELD,
    NPAD_STYLE_JOY_DUAL, NPAD_STYLE_JOY_LEFT, NPAD_STYLE_JOY_RIGHT, STICK_AXIS_MAX,
    VISUAL_TRIGGER_ACTIVE_THRESHOLD,
};

const BUTTON_A: u64 = 1 << 0;
const BUTTON_B: u64 = 1 << 1;
const BUTTON_X: u64 = 1 << 2;
const BUTTON_Y: u64 = 1 << 3;
const BUTTON_LSTICK: u64 = 1 << 4;
const BUTTON_RSTICK: u64 = 1 << 5;
const BUTTON_L: u64 = 1 << 6;
const BUTTON_R: u64 = 1 << 7;
const BUTTON_ZL: u64 = 1 << 8;
const BUTTON_ZR: u64 = 1 << 9;
const BUTTON_PLUS: u64 = 1 << 10;
const BUTTON_MINUS: u64 = 1 << 11;
const BUTTON_DLEFT: u64 = 1 << 12;
const BUTTON_DUP: u64 = 1 << 13;
const BUTTON_DRIGHT: u64 = 1 << 14;
const BUTTON_DDOWN: u64 = 1 << 15;
const BUTTON_SL_LEFT: u64 = 1 << 24;
const BUTTON_SR_LEFT: u64 = 1 << 25;
const BUTTON_SL_RIGHT: u64 = 1 << 26;
const BUTTON_SR_RIGHT: u64 = 1 << 27;

#[derive(Debug, Copy, Clone)]
pub(crate) struct ControllerSnapshot {
    pub npad_id: u32,
    pub style_flags: u32,
    pub buttons: u64,
    pub left_stick: (i32, i32),
    pub right_stick: (i32, i32),
    pub gc_triggers: Option<(u32, u32)>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum ControlId {
    A,
    B,
    X,
    Y,
    L,
    R,
    ZL,
    ZR,
    L3,
    R3,
    Plus,
    Minus,
    Home,
    Capture,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
    DpadUpLeft,
    DpadUpRight,
    DpadDownLeft,
    DpadDownRight,
    LeftStickGate,
    LeftStickDot,
    RightStickGate,
    RightStickDot,
    GcLTrigger,
    GcRTrigger,
}

impl ControlId {
    const ALL: [Self; 28] = [
        Self::A,
        Self::B,
        Self::X,
        Self::Y,
        Self::L,
        Self::R,
        Self::ZL,
        Self::ZR,
        Self::L3,
        Self::R3,
        Self::Plus,
        Self::Minus,
        Self::Home,
        Self::Capture,
        Self::DpadUp,
        Self::DpadDown,
        Self::DpadLeft,
        Self::DpadRight,
        Self::DpadUpLeft,
        Self::DpadUpRight,
        Self::DpadDownLeft,
        Self::DpadDownRight,
        Self::LeftStickGate,
        Self::LeftStickDot,
        Self::RightStickGate,
        Self::RightStickDot,
        Self::GcLTrigger,
        Self::GcRTrigger,
    ];
}

pub(crate) fn logical_control_count() -> usize {
    ControlId::ALL.len()
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct ControlValue {
    pub pressed: bool,
    pub analog: f32,
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct ControllerViewState {
    buttons: u64,
    left_stick: (f32, f32),
    right_stick: (f32, f32),
    gc_left_trigger: f32,
    gc_right_trigger: f32,
}

impl ControllerViewState {
    pub(crate) fn neutral() -> Self {
        Self {
            buttons: 0,
            left_stick: (0.0, 0.0),
            right_stick: (0.0, 0.0),
            gc_left_trigger: 0.0,
            gc_right_trigger: 0.0,
        }
    }

    pub(crate) fn from_snapshot(snapshot: ControllerSnapshot) -> Self {
        let (gc_left_trigger, gc_right_trigger) = snapshot
            .gc_triggers
            .map(|(left, right)| (normalize_trigger(left), normalize_trigger(right)))
            .unwrap_or((0.0, 0.0));

        Self {
            buttons: snapshot.buttons,
            left_stick: normalize_stick(snapshot.left_stick),
            right_stick: normalize_stick(snapshot.right_stick),
            gc_left_trigger,
            gc_right_trigger,
        }
    }

    pub(crate) fn control_value(&self, control_id: ControlId) -> ControlValue {
        match control_id {
            ControlId::A => self.button_value(BUTTON_A),
            ControlId::B => self.button_value(BUTTON_B),
            ControlId::X => self.button_value(BUTTON_X),
            ControlId::Y => self.button_value(BUTTON_Y),
            ControlId::L => self.button_value(BUTTON_L),
            ControlId::R => self.button_value(BUTTON_R),
            ControlId::ZL => self.button_value(BUTTON_ZL),
            ControlId::ZR => self.button_value(BUTTON_ZR),
            ControlId::L3 => self.button_value(BUTTON_LSTICK),
            ControlId::R3 => self.button_value(BUTTON_RSTICK),
            ControlId::Plus => self.button_value(BUTTON_PLUS),
            ControlId::Minus => self.button_value(BUTTON_MINUS),
            // Home/Capture are represented in some skins but are not exposed as match input.
            ControlId::Home | ControlId::Capture => ControlValue {
                pressed: false,
                analog: 0.0,
            },
            ControlId::DpadUp => self.button_value(BUTTON_DUP),
            ControlId::DpadDown => self.button_value(BUTTON_DDOWN),
            ControlId::DpadLeft => self.button_value(BUTTON_DLEFT),
            ControlId::DpadRight => self.button_value(BUTTON_DRIGHT),
            ControlId::DpadUpLeft => self.combined_button_value(BUTTON_DUP, BUTTON_DLEFT),
            ControlId::DpadUpRight => self.combined_button_value(BUTTON_DUP, BUTTON_DRIGHT),
            ControlId::DpadDownLeft => self.combined_button_value(BUTTON_DDOWN, BUTTON_DLEFT),
            ControlId::DpadDownRight => self.combined_button_value(BUTTON_DDOWN, BUTTON_DRIGHT),
            ControlId::LeftStickGate | ControlId::RightStickGate => ControlValue {
                pressed: false,
                analog: 0.0,
            },
            ControlId::LeftStickDot => stick_value(self.left_stick),
            ControlId::RightStickDot => stick_value(self.right_stick),
            ControlId::GcLTrigger => trigger_value(self.gc_left_trigger),
            ControlId::GcRTrigger => trigger_value(self.gc_right_trigger),
        }
    }

    pub(crate) fn stick_position(&self, control_id: ControlId) -> Option<(f32, f32)> {
        match control_id {
            ControlId::LeftStickDot => Some(self.left_stick),
            ControlId::RightStickDot => Some(self.right_stick),
            _ => None,
        }
    }

    fn button_value(&self, mask: u64) -> ControlValue {
        let pressed = self.buttons & mask != 0;
        ControlValue {
            pressed,
            analog: if pressed { 1.0 } else { 0.0 },
        }
    }

    fn combined_button_value(&self, first_mask: u64, second_mask: u64) -> ControlValue {
        let pressed = self.buttons & first_mask != 0 && self.buttons & second_mask != 0;
        ControlValue {
            pressed,
            analog: if pressed { 1.0 } else { 0.0 },
        }
    }
}

pub(crate) unsafe fn poll_p1_controller() -> Option<ControllerSnapshot> {
    poll_controller(NPAD_ID_NO1).or_else(|| poll_controller(NPAD_ID_HANDHELD))
}

unsafe fn poll_controller(npad_id: u32) -> Option<ControllerSnapshot> {
    let style_flags = hid::GetNpadStyleSet(&npad_id).flags;
    if style_flags == 0 {
        return None;
    }

    if style_flags & NPAD_STYLE_GAMECUBE != 0 {
        let mut state = hid::NpadGcState::default();
        hid::GetNpadGcState(&mut state, &npad_id);
        return Some(ControllerSnapshot {
            npad_id,
            style_flags,
            buttons: state.Buttons,
            left_stick: (state.LStickX, state.LStickY),
            right_stick: (state.RStickX, state.RStickY),
            gc_triggers: Some((state.LTrigger, state.RTrigger)),
        });
    }

    let mut state = hid::NpadHandheldState::default();
    match first_supported_standard_style(style_flags) {
        NPAD_STYLE_HANDHELD => hid::GetNpadHandheldState(&mut state, &npad_id),
        NPAD_STYLE_JOY_DUAL => hid::GetNpadJoyDualState(&mut state, &npad_id),
        NPAD_STYLE_JOY_LEFT => hid::GetNpadJoyLeftState(&mut state, &npad_id),
        NPAD_STYLE_JOY_RIGHT => hid::GetNpadJoyRightState(&mut state, &npad_id),
        _ => hid::GetNpadFullKeyState(&mut state, &npad_id),
    }

    Some(ControllerSnapshot {
        npad_id,
        style_flags,
        buttons: state.Buttons,
        left_stick: (state.LStickX, state.LStickY),
        right_stick: (state.RStickX, state.RStickY),
        gc_triggers: None,
    })
}

fn first_supported_standard_style(style_flags: u32) -> u32 {
    for style in [
        NPAD_STYLE_FULL_KEY,
        NPAD_STYLE_HANDHELD,
        NPAD_STYLE_JOY_DUAL,
        NPAD_STYLE_JOY_LEFT,
        NPAD_STYLE_JOY_RIGHT,
    ] {
        if style_flags & style != 0 {
            return style;
        }
    }

    NPAD_STYLE_FULL_KEY
}

pub(crate) fn npad_id_name(npad_id: u32) -> &'static str {
    match npad_id {
        NPAD_ID_NO1 => "No1",
        NPAD_ID_HANDHELD => "Handheld",
        _ => "Npad",
    }
}

pub(crate) fn gc_trigger_text(triggers: Option<(u32, u32)>) -> Option<String> {
    triggers.map(|(left, right)| format!("GC LT {left:03} RT {right:03}"))
}

pub(crate) fn style_name(style_flags: u32) -> &'static str {
    if style_flags & NPAD_STYLE_GAMECUBE != 0 {
        "GC"
    } else if style_flags & NPAD_STYLE_FULL_KEY != 0 {
        "FullKey"
    } else if style_flags & NPAD_STYLE_HANDHELD != 0 {
        "Handheld"
    } else if style_flags & NPAD_STYLE_JOY_DUAL != 0 {
        "JoyDual"
    } else if style_flags & NPAD_STYLE_JOY_LEFT != 0 {
        "JoyLeft"
    } else if style_flags & NPAD_STYLE_JOY_RIGHT != 0 {
        "JoyRight"
    } else {
        "Unknown"
    }
}

pub(crate) fn button_names(buttons: u64) -> String {
    let mut names = String::new();

    for (mask, name) in [
        (BUTTON_A, "A"),
        (BUTTON_B, "B"),
        (BUTTON_X, "X"),
        (BUTTON_Y, "Y"),
        (BUTTON_LSTICK, "LS"),
        (BUTTON_RSTICK, "RS"),
        (BUTTON_L, "L"),
        (BUTTON_R, "R"),
        (BUTTON_ZL, "ZL"),
        (BUTTON_ZR, "ZR"),
        (BUTTON_PLUS, "+"),
        (BUTTON_MINUS, "-"),
        (BUTTON_DLEFT, "DL"),
        (BUTTON_DUP, "DU"),
        (BUTTON_DRIGHT, "DR"),
        (BUTTON_DDOWN, "DD"),
        (BUTTON_SL_LEFT, "SL-L"),
        (BUTTON_SR_LEFT, "SR-L"),
        (BUTTON_SL_RIGHT, "SL-R"),
        (BUTTON_SR_RIGHT, "SR-R"),
    ] {
        if buttons & mask != 0 {
            if !names.is_empty() {
                names.push(' ');
            }
            names.push_str(name);
        }
    }

    if names.is_empty() {
        names.push('-');
    }

    names
}

fn normalize_stick(stick: (i32, i32)) -> (f32, f32) {
    (normalize_stick_axis(stick.0), normalize_stick_axis(stick.1))
}

fn normalize_stick_axis(value: i32) -> f32 {
    (value as f32 / STICK_AXIS_MAX).clamp(-1.0, 1.0)
}

fn normalize_trigger(value: u32) -> f32 {
    (value as f32 / 255.0).clamp(0.0, 1.0)
}

fn stick_value(stick: (f32, f32)) -> ControlValue {
    let analog = (stick.0 * stick.0 + stick.1 * stick.1)
        .sqrt()
        .clamp(0.0, 1.0);
    ControlValue {
        pressed: analog > 0.08,
        analog,
    }
}

fn trigger_value(value: f32) -> ControlValue {
    ControlValue {
        pressed: value > normalize_trigger(VISUAL_TRIGGER_ACTIVE_THRESHOLD),
        analog: value,
    }
}
