//! Which buttons and sticks the overlay draws from Smash's final mapped input.
//!
//! The game publishes one `MappedInputs` word per player after button mapping,
//! A+B smash, and the right-stick mode. Tap jump only sets `FLICK_JUMP`; it
//! does not put `JUMP` in that word. This crate turns the word, the player's
//! mapping bytes, and the physical pad into the HID-shaped view the overlay
//! already draws.
//!
//! Injected actions light one physical button, in this order: X, Y, A, B, R,
//! ZR, L, ZL, D-pad up, D-pad left, D-pad right, D-pad down. The first button
//! in that list whose mapping produces the action is the one that lights.
//! `CSTICK_ON`, `FLICK_JUMP`, and `STOCK_SHARE` never light a button.

/// `nn::hid` button bits the overlay already uses.
pub const HID_A: u64 = 1 << 0;
pub const HID_B: u64 = 1 << 1;
pub const HID_X: u64 = 1 << 2;
pub const HID_Y: u64 = 1 << 3;
pub const HID_LSTICK: u64 = 1 << 4;
pub const HID_RSTICK: u64 = 1 << 5;
pub const HID_L: u64 = 1 << 6;
pub const HID_R: u64 = 1 << 7;
pub const HID_ZL: u64 = 1 << 8;
pub const HID_ZR: u64 = 1 << 9;
pub const HID_PLUS: u64 = 1 << 10;
pub const HID_MINUS: u64 = 1 << 11;
pub const HID_DLEFT: u64 = 1 << 12;
pub const HID_DUP: u64 = 1 << 13;
pub const HID_DRIGHT: u64 = 1 << 14;
pub const HID_DDOWN: u64 = 1 << 15;
pub const HID_SL_LEFT: u64 = 1 << 24;
pub const HID_SR_LEFT: u64 = 1 << 25;
pub const HID_SL_RIGHT: u64 = 1 << 26;
pub const HID_SR_RIGHT: u64 = 1 << 27;

pub const HID_STICK_MAX: i32 = 32_767;
pub const MAPPING_LEN: usize = 0x50;

/// Visual GC trigger value that the overlay treats as a press (`3200`).
pub const GC_TRIGGER_LIGHT: u32 = 3200;

/// `fcvtzs` scale in final-input mapping. The constant is `127.0` at main+`0x4472fac`.
pub const STICK_SCALE: i32 = 127;

/// One count of truncation slack around `trunc(axis * 127)`.
pub const STICK_AGREE_TOLERANCE: i32 = 1;

const ATTACK: u32 = 0x1;
const SPECIAL: u32 = 0x2;
const JUMP: u32 = 0x4;
const GUARD: u32 = 0x8;
const CATCH: u32 = 0x10;
const SMASH: u32 = 0x20;
const JUMP_MINI: u32 = 0x40;
const CSTICK_ON: u32 = 0x80;
const ATTACK_RAW: u32 = 0x200;
const APPEAL_HI: u32 = 0x400;
const SPECIAL_RAW: u32 = 0x800;
const APPEAL_LW: u32 = 0x1000;
const APPEAL_SL: u32 = 0x2000;
const APPEAL_SR: u32 = 0x4000;
const GUARD_HOLD: u32 = 0x1_0000;
const SPECIAL_RAW2: u32 = 0x2_0000;

const KIND_ATTACK: u8 = 0;
const KIND_SPECIAL: u8 = 1;
const KIND_JUMP: u8 = 2;
const KIND_GUARD: u8 = 3;
const KIND_GRAB: u8 = 4;
const KIND_SMASH: u8 = 5;
const KIND_APPEAL_HI: u8 = 0xA;
const KIND_APPEAL_S: u8 = 0xB;
const KIND_APPEAL_LW: u8 = 0xC;

const STYLE_LEFT: u32 = 3;
const STYLE_RIGHT: u32 = 4;
const STYLE_GC: u32 = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pad {
    A,
    B,
    X,
    Y,
    L,
    R,
    Zl,
    Zr,
    DpadLeft,
    DpadUp,
    DpadRight,
    DpadDown,
}

const PRIORITY: [Pad; 12] = [
    Pad::X,
    Pad::Y,
    Pad::A,
    Pad::B,
    Pad::R,
    Pad::Zr,
    Pad::L,
    Pad::Zl,
    Pad::DpadUp,
    Pad::DpadLeft,
    Pad::DpadRight,
    Pad::DpadDown,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StickSide {
    Left,
    Right,
}

/// Physical pad sample, in the overlay's HID shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Physical {
    pub buttons: u64,
    pub left_stick: (i32, i32),
    pub right_stick: (i32, i32),
    pub gc_triggers: Option<(u32, u32)>,
}

/// One player's controller as final-input mapping read it.
#[derive(Clone, Copy, Debug)]
pub struct LiveController {
    pub style: u32,
    pub mapping: [u8; MAPPING_LEN],
    pub left_stick: (f32, f32),
    pub right_stick: (f32, f32),
    /// Game bit 8 reads `joy_shoulder` only when main+`0x52b92f0` is zero.
    pub joy_l_is_shoulder: bool,
}

/// `MappedInputs` for the local player. Sticks are the i8 fields (`/127`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FinalInput {
    pub buttons: u32,
    pub left_stick: (i8, i8),
    pub right_stick: (i8, i8),
}

/// HID-shaped view to draw. `*_from_final` is true when that stick is the game's value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shown {
    pub buttons: u64,
    pub left_stick: (i32, i32),
    pub right_stick: (i32, i32),
    pub gc_triggers: Option<(u32, u32)>,
    pub left_from_final: bool,
    pub right_from_final: bool,
}

/// Draw `final_input` on `live`'s layout. Unknown styles return the physical pad unchanged.
pub fn display(physical: &Physical, live: &LiveController, final_input: &FinalInput) -> Shown {
    if !style_known(live.style) {
        return passthrough(physical);
    }
    let explained = explained_bits(physical, live, final_input);
    let mut buttons = unmapped_buttons(physical, live);
    let mut injected_guard = None;
    for pad in PRIORITY {
        if !pressed(physical, pad) {
            continue;
        }
        let Some(kind) = kind_of(live, pad) else {
            continue;
        };
        if press_reached(kind, pad, final_input.buttons, explained.absmash) {
            buttons |= hid_bit(pad);
        }
    }
    light_injected(
        live,
        final_input.buttons,
        explained.mask,
        &mut buttons,
        &mut injected_guard,
    );
    let mut triggers = physical.gc_triggers;
    if live.style == STYLE_GC {
        raise_injected_trigger(injected_guard, &mut triggers);
    }
    let (left_stick, right_stick, left_from_final, right_from_final) =
        show_sticks(physical, live, final_input);
    Shown {
        buttons,
        left_stick,
        right_stick,
        gc_triggers: triggers,
        left_from_final,
        right_from_final,
    }
}

fn passthrough(physical: &Physical) -> Shown {
    Shown {
        buttons: physical.buttons,
        left_stick: physical.left_stick,
        right_stick: physical.right_stick,
        gc_triggers: physical.gc_triggers,
        left_from_final: false,
        right_from_final: false,
    }
}

fn style_known(style: u32) -> bool {
    (1..=7).contains(&style)
}

struct Explained {
    mask: u32,
    absmash: bool,
}

fn explained_bits(
    physical: &Physical,
    live: &LiveController,
    final_input: &FinalInput,
) -> Explained {
    let mut mask = 0u32;
    let mut attack = false;
    let mut special = false;
    for pad in PRIORITY {
        if !pressed(physical, pad) {
            continue;
        }
        let Some(kind) = kind_of(live, pad) else {
            continue;
        };
        mask |= produced_bits(kind, pad);
        attack |= kind == KIND_ATTACK;
        special |= kind == KIND_SPECIAL;
    }
    mask |= analog_guard_bits(physical, live);
    let absmash = absmash_on(live) && attack && special;
    if absmash && final_input.buttons & SMASH != 0 {
        mask |= SMASH;
    }
    if let Some(kind) = cstick_explains(physical, live, final_input) {
        mask |= produced_bits(kind, Pad::A);
    }
    Explained { mask, absmash }
}

fn light_injected(
    live: &LiveController,
    final_buttons: u32,
    mut explained: u32,
    buttons: &mut u64,
    injected_guard: &mut Option<Pad>,
) {
    if final_buttons & CATCH != 0 && explained & CATCH == 0 {
        if let Some(pad) = first_mapped(live, KIND_GRAB, None) {
            *buttons |= hid_bit(pad);
            explained |= CATCH | GUARD | GUARD_HOLD;
        }
    }
    for group in GROUPS {
        if final_buttons & group.bits == 0 || explained & group.bits != 0 {
            continue;
        }
        let Some(pad) = first_mapped(live, group.kind, group.prefer) else {
            continue;
        };
        let already = *buttons & hid_bit(pad) != 0;
        *buttons |= hid_bit(pad);
        if group.kind == KIND_GUARD && !already {
            *injected_guard = Some(pad);
        }
    }
}

struct Group {
    kind: u8,
    bits: u32,
    prefer: Option<Pad>,
}

const GROUPS: [Group; 9] = [
    Group {
        kind: KIND_ATTACK,
        bits: ATTACK | ATTACK_RAW,
        prefer: None,
    },
    Group {
        kind: KIND_SPECIAL,
        bits: SPECIAL | SPECIAL_RAW | SPECIAL_RAW2,
        prefer: None,
    },
    Group {
        kind: KIND_JUMP,
        bits: JUMP | JUMP_MINI,
        prefer: None,
    },
    Group {
        kind: KIND_GUARD,
        bits: GUARD | GUARD_HOLD,
        prefer: None,
    },
    Group {
        kind: KIND_SMASH,
        bits: SMASH,
        prefer: None,
    },
    Group {
        kind: KIND_APPEAL_HI,
        bits: APPEAL_HI,
        prefer: None,
    },
    Group {
        kind: KIND_APPEAL_LW,
        bits: APPEAL_LW,
        prefer: None,
    },
    Group {
        kind: KIND_APPEAL_S,
        bits: APPEAL_SL,
        prefer: Some(Pad::DpadLeft),
    },
    Group {
        kind: KIND_APPEAL_S,
        bits: APPEAL_SR,
        prefer: Some(Pad::DpadRight),
    },
];

fn press_reached(kind: u8, pad: Pad, final_buttons: u32, absmash: bool) -> bool {
    if produced_bits(kind, pad) & final_buttons != 0 {
        return true;
    }
    absmash
        && (kind == KIND_ATTACK || kind == KIND_SPECIAL)
        && final_buttons & ATTACK != 0
        && final_buttons & SMASH != 0
        && final_buttons & SPECIAL == 0
}

fn produced_bits(kind: u8, pad: Pad) -> u32 {
    match kind {
        KIND_ATTACK => ATTACK | ATTACK_RAW,
        KIND_SPECIAL => SPECIAL | SPECIAL_RAW | SPECIAL_RAW2,
        KIND_JUMP => JUMP | JUMP_MINI,
        KIND_GUARD => GUARD | GUARD_HOLD,
        KIND_GRAB => CATCH | GUARD,
        KIND_SMASH => SMASH,
        KIND_APPEAL_HI => APPEAL_HI,
        KIND_APPEAL_LW => APPEAL_LW,
        KIND_APPEAL_S => match pad {
            Pad::DpadLeft => APPEAL_SL,
            Pad::DpadRight => APPEAL_SR,
            _ => APPEAL_SL | APPEAL_SR,
        },
        _ => 0,
    }
}

fn first_mapped(live: &LiveController, kind: u8, prefer: Option<Pad>) -> Option<Pad> {
    if kind == KIND_APPEAL_S {
        return appeal_s_pad(live, prefer);
    }
    if let Some(pad) = prefer {
        if kind_of(live, pad) == Some(kind) {
            return Some(pad);
        }
    }
    PRIORITY
        .into_iter()
        .find(|pad| kind_of(live, *pad) == Some(kind))
}

fn appeal_s_pad(live: &LiveController, prefer: Option<Pad>) -> Option<Pad> {
    let left = kind_of(live, Pad::DpadLeft) == Some(KIND_APPEAL_S);
    let right = kind_of(live, Pad::DpadRight) == Some(KIND_APPEAL_S);
    match prefer {
        Some(Pad::DpadLeft) if left => Some(Pad::DpadLeft),
        Some(Pad::DpadRight) if right => Some(Pad::DpadRight),
        _ if left => Some(Pad::DpadLeft),
        _ if right => Some(Pad::DpadRight),
        _ => PRIORITY
            .into_iter()
            .find(|pad| kind_of(live, *pad) == Some(KIND_APPEAL_S)),
    }
}

fn analog_guard_bits(physical: &Physical, live: &LiveController) -> u32 {
    if live.style != STYLE_GC {
        return 0;
    }
    let Some((left, right)) = physical.gc_triggers else {
        return 0;
    };
    let mut bits = 0;
    if left > 0 && kind_of(live, Pad::L) == Some(KIND_GUARD) {
        bits |= GUARD | GUARD_HOLD;
    }
    if right > 0 && kind_of(live, Pad::R) == Some(KIND_GUARD) {
        bits |= GUARD | GUARD_HOLD;
    }
    bits
}

fn raise_injected_trigger(pad: Option<Pad>, triggers: &mut Option<(u32, u32)>) {
    let Some(pad) = pad else {
        return;
    };
    let Some((left, right)) = triggers.as_mut() else {
        return;
    };
    match pad {
        Pad::L => *left = (*left).max(GC_TRIGGER_LIGHT),
        Pad::R => *right = (*right).max(GC_TRIGGER_LIGHT),
        _ => {}
    }
}

/// C-stick mode explains its action when `CSTICK_ON` is set and the physical
/// right stick accounts for the published right stick.
///
/// `0x1752870` gates that bit on `x² + y²` of the post-curve right-stick i8s
/// compared with a parameter-file byte (`ldrb` / `cmp` / `b.gt` at
/// `0x17530e4`). That byte is not an instruction immediate, so the explanation
/// uses the same agreement rule as the stick draw.
fn cstick_explains(
    physical: &Physical,
    live: &LiveController,
    final_input: &FinalInput,
) -> Option<u8> {
    if final_input.buttons & CSTICK_ON == 0 {
        return None;
    }
    let kind = cstick_kind(live)?;
    if produced_bits(kind, Pad::A) == 0 {
        return None;
    }
    let (expected, _, hid) = other_stick(live, physical);
    if stick_follows(expected.0, expected.1, final_input.right_stick, hid) {
        Some(kind)
    } else {
        None
    }
}

fn unmapped_buttons(physical: &Physical, live: &LiveController) -> u64 {
    let mut mapped = 0u64;
    for pad in PRIORITY {
        if kind_of(live, pad).is_some() {
            mapped |= hid_bit(pad);
        }
    }
    physical.buttons & !mapped
}

fn pressed(physical: &Physical, pad: Pad) -> bool {
    physical.buttons & hid_bit(pad) != 0
}

fn hid_bit(pad: Pad) -> u64 {
    match pad {
        Pad::A => HID_A,
        Pad::B => HID_B,
        Pad::X => HID_X,
        Pad::Y => HID_Y,
        Pad::L => HID_L,
        Pad::R => HID_R,
        Pad::Zl => HID_ZL,
        Pad::Zr => HID_ZR,
        Pad::DpadLeft => HID_DLEFT,
        Pad::DpadUp => HID_DUP,
        Pad::DpadRight => HID_DRIGHT,
        Pad::DpadDown => HID_DDOWN,
    }
}

fn kind_of(live: &LiveController, pad: Pad) -> Option<u8> {
    let offset = field_offset(live.style, pad, live.joy_l_is_shoulder)?;
    let kind = live.mapping[offset];
    if is_action_kind(kind) {
        Some(kind)
    } else {
        None
    }
}

fn is_action_kind(kind: u8) -> bool {
    matches!(
        kind,
        KIND_ATTACK
            | KIND_SPECIAL
            | KIND_JUMP
            | KIND_GUARD
            | KIND_GRAB
            | KIND_SMASH
            | KIND_APPEAL_HI
            | KIND_APPEAL_S
            | KIND_APPEAL_LW
    )
}

fn field_offset(style: u32, pad: Pad, joy_l_is_shoulder: bool) -> Option<usize> {
    match style {
        1 | 2 | 5 | 6 => pro_field(pad),
        STYLE_GC => gc_field(pad),
        STYLE_LEFT => joy_field(pad, true, joy_l_is_shoulder),
        STYLE_RIGHT => joy_field(pad, false, joy_l_is_shoulder),
        _ => None,
    }
}

fn pro_field(pad: Pad) -> Option<usize> {
    Some(match pad {
        Pad::A => 0x16,
        Pad::B => 0x17,
        Pad::X => 0x19,
        Pad::Y => 0x1A,
        Pad::L => 0x0F,
        Pad::R => 0x10,
        Pad::Zl => 0x11,
        Pad::Zr => 0x12,
        Pad::DpadUp => 0x13,
        Pad::DpadLeft | Pad::DpadRight => 0x14,
        Pad::DpadDown => 0x15,
    })
}

fn gc_field(pad: Pad) -> Option<usize> {
    Some(match pad {
        Pad::A => 0x06,
        Pad::B => 0x07,
        Pad::X => 0x0A,
        Pad::Y => 0x09,
        Pad::L => 0x00,
        Pad::R => 0x01,
        Pad::Zr => 0x02,
        Pad::DpadUp => 0x03,
        Pad::DpadLeft | Pad::DpadRight => 0x04,
        Pad::DpadDown => 0x05,
        Pad::Zl => return None,
    })
}

fn joy_field(pad: Pad, left_joycon: bool, joy_l_is_shoulder: bool) -> Option<usize> {
    Some(match pad {
        Pad::Y if !left_joycon => 0x23,
        Pad::X if !left_joycon => 0x24,
        Pad::B if !left_joycon => 0x25,
        Pad::A if !left_joycon => 0x26,
        Pad::DpadRight if left_joycon => 0x23,
        Pad::DpadDown if left_joycon => 0x24,
        Pad::DpadUp if left_joycon => 0x25,
        Pad::DpadLeft if left_joycon => 0x26,
        Pad::L if joy_l_is_shoulder => 0x1F,
        Pad::R => 0x1F,
        Pad::Zl | Pad::Zr => 0x20,
        _ => return None,
    })
}

fn cstick_kind(live: &LiveController) -> Option<u8> {
    let offset = match live.style {
        1 | 2 | 5 | 6 => 0x18,
        STYLE_GC => 0x08,
        _ => return None,
    };
    let kind = live.mapping[offset];
    if matches!(
        kind,
        KIND_ATTACK | KIND_SPECIAL | KIND_JUMP | KIND_GUARD | KIND_SMASH
    ) {
        Some(kind)
    } else {
        None
    }
}

fn absmash_on(live: &LiveController) -> bool {
    flag_byte(live, 0x0C, 0x1C, 0x28) != 0
}

fn flag_byte(live: &LiveController, gc: usize, pro: usize, joy: usize) -> u8 {
    let offset = match live.style {
        STYLE_GC => gc,
        STYLE_LEFT | STYLE_RIGHT => joy,
        1 | 2 | 5 | 6 => pro,
        _ => return 0,
    };
    live.mapping[offset]
}

fn show_sticks(
    physical: &Physical,
    live: &LiveController,
    final_input: &FinalInput,
) -> ((i32, i32), (i32, i32), bool, bool) {
    let mut left = physical.left_stick;
    let mut right = physical.right_stick;
    let mut left_final = false;
    let mut right_final = false;
    let (expected_x, expected_y, side) = fighter_movement(live);
    let movement_hid = movement_source(physical, live.style).1;
    if !stick_follows(expected_x, expected_y, final_input.left_stick, movement_hid) {
        let shown = physical_from_final(live.style, side, final_input.left_stick);
        match side {
            StickSide::Left => {
                left = shown;
                left_final = true;
            }
            StickSide::Right => {
                right = shown;
                right_final = true;
            }
        }
    }
    let (other_expected, other_side, other_hid) = other_stick(live, physical);
    if !stick_follows(
        other_expected.0,
        other_expected.1,
        final_input.right_stick,
        other_hid,
    ) {
        let shown = physical_from_final(live.style, other_side, final_input.right_stick);
        match other_side {
            StickSide::Left => {
                left = shown;
                left_final = true;
            }
            StickSide::Right => {
                right = shown;
                right_final = true;
            }
        }
    }
    (left, right, left_final, right_final)
}

fn movement_source(physical: &Physical, style: u32) -> (StickSide, (i32, i32)) {
    if style == STYLE_RIGHT {
        (StickSide::Right, physical.right_stick)
    } else {
        (StickSide::Left, physical.left_stick)
    }
}

fn other_stick(live: &LiveController, physical: &Physical) -> ((f32, f32), StickSide, (i32, i32)) {
    if live.style == STYLE_RIGHT {
        (live.left_stick, StickSide::Left, physical.left_stick)
    } else {
        (live.right_stick, StickSide::Right, physical.right_stick)
    }
}

/// Fighter-space axes FIM multiplies by 127, and which physical stick they came from.
fn fighter_movement(live: &LiveController) -> (f32, f32, StickSide) {
    match live.style {
        STYLE_LEFT => (-live.left_stick.1, live.left_stick.0, StickSide::Left),
        STYLE_RIGHT => (live.right_stick.1, live.right_stick.0, StickSide::Right),
        _ => (live.left_stick.0, live.left_stick.1, StickSide::Left),
    }
}

fn physical_from_final(style: u32, side: StickSide, fighter: (i8, i8)) -> (i32, i32) {
    let (x, y) = (i32::from(fighter.0), i32::from(fighter.1));
    let (ox, oy) = match (style, side) {
        (STYLE_LEFT, StickSide::Left) => (y, -x),
        (STYLE_RIGHT, StickSide::Right) => (y, x),
        _ => (x, y),
    };
    (axis_to_hid(ox), axis_to_hid(oy))
}

fn stick_follows(expected_x: f32, expected_y: f32, final_stick: (i8, i8), hid: (i32, i32)) -> bool {
    let expected_x = if expected_x.is_finite() {
        quantize_axis(expected_x)
    } else {
        quantize_axis(hid.0 as f32 / HID_STICK_MAX as f32)
    };
    let expected_y = if expected_y.is_finite() {
        quantize_axis(expected_y)
    } else {
        quantize_axis(hid.1 as f32 / HID_STICK_MAX as f32)
    };
    axis_follows(expected_x, final_stick.0) && axis_follows(expected_y, final_stick.1)
}

fn axis_follows(expected: i32, actual: i8) -> bool {
    let actual = i32::from(actual);
    if expected != 0 && actual != 0 && expected.signum() != actual.signum() {
        return false;
    }
    actual.abs() <= expected.abs().saturating_add(STICK_AGREE_TOLERANCE)
}

fn quantize_axis(axis: f32) -> i32 {
    if !axis.is_finite() {
        return 0;
    }
    (axis.clamp(-1.0, 1.0) * STICK_SCALE as f32).trunc() as i32
}

fn axis_to_hid(axis: i32) -> i32 {
    let axis = axis.clamp(-STICK_SCALE, STICK_SCALE);
    ((axis as f32 / STICK_SCALE as f32) * HID_STICK_MAX as f32).round() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    const KIND_UNSET: u8 = 0xD;

    fn live(style: u32, mapping: [u8; MAPPING_LEN]) -> LiveController {
        LiveController {
            style,
            mapping,
            left_stick: (0.0, 0.0),
            right_stick: (0.0, 0.0),
            joy_l_is_shoulder: true,
        }
    }

    fn physical(buttons: u64) -> Physical {
        Physical {
            buttons,
            left_stick: (0, 0),
            right_stick: (0, 0),
            gc_triggers: None,
        }
    }

    fn final_buttons(buttons: u32) -> FinalInput {
        FinalInput {
            buttons,
            left_stick: (0, 0),
            right_stick: (0, 0),
        }
    }

    fn pro_default() -> [u8; MAPPING_LEN] {
        let mut mapping = [KIND_UNSET; MAPPING_LEN];
        mapping[0x0F] = KIND_GUARD;
        mapping[0x10] = KIND_GUARD;
        mapping[0x11] = KIND_GRAB;
        mapping[0x12] = KIND_GUARD;
        mapping[0x13] = KIND_APPEAL_HI;
        mapping[0x14] = KIND_APPEAL_S;
        mapping[0x15] = KIND_APPEAL_LW;
        mapping[0x16] = KIND_ATTACK;
        mapping[0x17] = KIND_SPECIAL;
        mapping[0x18] = KIND_SMASH;
        mapping[0x19] = KIND_JUMP;
        mapping[0x1A] = KIND_JUMP;
        mapping[0x1C] = 1;
        mapping[0x1D] = 1;
        mapping
    }

    fn shown(physical: &Physical, mapping: [u8; MAPPING_LEN], buttons: u32) -> Shown {
        display(physical, &live(5, mapping), &final_buttons(buttons))
    }

    #[test]
    fn human_default_layout_matches_raw() {
        let mapping = pro_default();
        let cases = [
            (HID_A, ATTACK | 0x8000, (0, 0)),
            (HID_X, JUMP | 0x8000, (0, 0)),
            (HID_X | HID_Y, JUMP | JUMP_MINI | 0x8000, (0, 0)),
            (0, 0x8000, (0, 30_000)),
            (HID_A | HID_B, ATTACK | SMASH | 0x8000, (0, 0)),
            (0, SMASH | CSTICK_ON | 0x8000, (0, 0)),
            (HID_PLUS | HID_LSTICK, 0x8000, (0, 0)),
            (HID_L | HID_R, GUARD | GUARD_HOLD | 0x8000, (0, 0)),
            (HID_ZL, GUARD | CATCH | 0x8000, (0, 0)),
        ];
        for (buttons, word, left) in cases {
            let mut pad = physical(buttons);
            pad.left_stick = left;
            let mut game = live(5, mapping);
            game.left_stick = (
                left.0 as f32 / HID_STICK_MAX as f32,
                left.1 as f32 / HID_STICK_MAX as f32,
            );
            let mut word_in = final_buttons(word);
            word_in.left_stick = (
                quantize_axis(game.left_stick.0) as i8,
                quantize_axis(game.left_stick.1) as i8,
            );
            if buttons == 0 && word & CSTICK_ON != 0 {
                pad.right_stick = (30_000, 0);
                game.right_stick = (30_000.0 / HID_STICK_MAX as f32, 0.0);
                word_in.right_stick = (quantize_axis(game.right_stick.0) as i8, 0);
            }
            let view = display(&pad, &game, &word_in);
            assert_eq!(view.buttons, buttons, "word {word:#x}");
            assert_eq!(view.left_stick, pad.left_stick);
            assert_eq!(view.right_stick, pad.right_stick);
            assert!(!view.left_from_final && !view.right_from_final);
        }
    }

    #[test]
    fn custom_layout_two_jump_buttons() {
        let mut mapping = pro_default();
        mapping[0x16] = KIND_JUMP;
        mapping[0x17] = KIND_JUMP;
        mapping[0x19] = KIND_ATTACK;
        mapping[0x1A] = KIND_SPECIAL;
        let pad = physical(0);
        let idle = shown(&pad, mapping, JUMP);
        assert_eq!(idle.buttons, HID_A);
        let both = physical(HID_A | HID_B);
        assert_eq!(
            shown(&both, mapping, JUMP | JUMP_MINI).buttons,
            HID_A | HID_B
        );
        let only_b = physical(HID_B);
        assert_eq!(shown(&only_b, mapping, JUMP).buttons, HID_B);
    }

    #[test]
    fn stick_up_does_not_explain_jump() {
        let mapping = pro_default();
        let mut pad = physical(0);
        pad.left_stick = (0, 30_000);
        let mut game = live(5, mapping);
        game.left_stick = (0.0, 30_000.0 / HID_STICK_MAX as f32);
        let mut word = final_buttons(0x8000);
        word.left_stick = (0, quantize_axis(game.left_stick.1) as i8);
        assert_eq!(display(&pad, &game, &word).buttons, 0);

        word.buttons = JUMP | 0x8000;
        assert_eq!(display(&pad, &game, &word).buttons, HID_X);

        pad.buttons = HID_X;
        assert_eq!(display(&pad, &game, &word).buttons, HID_X);

        game.mapping[0x1D] = 0;
        pad.buttons = 0;
        assert_eq!(display(&pad, &game, &word).buttons, HID_X);
    }

    #[test]
    fn absmash_keeps_both_pressed_buttons_and_no_third() {
        let mut mapping = pro_default();
        mapping[0x13] = KIND_SMASH;
        let pressed = physical(HID_A | HID_B);
        let view = shown(&pressed, mapping, ATTACK | SMASH);
        assert_eq!(view.buttons, HID_A | HID_B);

        mapping[0x1C] = 0;
        let separate = shown(&pressed, mapping, ATTACK | SPECIAL);
        assert_eq!(separate.buttons, HID_A | HID_B);

        let only_a = physical(HID_A);
        assert_eq!(shown(&only_a, mapping, ATTACK).buttons, HID_A);
        let swallowed = physical(HID_B);
        assert_eq!(shown(&swallowed, mapping, 0).buttons, 0);
    }

    #[test]
    fn right_stick_modes_do_not_light_face_buttons() {
        let modes = [
            (KIND_ATTACK, ATTACK, HID_A),
            (KIND_SPECIAL, SPECIAL, HID_B),
            (KIND_JUMP, JUMP, HID_X),
            (KIND_GUARD, GUARD, HID_R),
            (KIND_SMASH, SMASH, HID_DUP),
        ];
        for (kind, bits, face) in modes {
            let mut mapping = pro_default();
            mapping[0x18] = kind;
            if kind == KIND_SMASH {
                mapping[0x13] = KIND_SMASH;
            }
            let mut pad = physical(0);
            pad.right_stick = (28_000, 0);
            let mut game = live(5, mapping);
            game.right_stick = (28_000.0 / HID_STICK_MAX as f32, 0.0);
            let mut word = final_buttons(bits | CSTICK_ON);
            word.right_stick = (quantize_axis(game.right_stick.0) as i8, 0);
            let held = display(&pad, &game, &word);
            assert_eq!(held.buttons & face, 0, "mode {kind}");
            assert_eq!(held.right_stick, pad.right_stick);

            pad.right_stick = (0, 0);
            game.right_stick = (0.0, 0.0);
            word.right_stick = (0, 0);
            let agreed_idle = display(&pad, &game, &word);
            assert_eq!(agreed_idle.buttons & face, 0, "agreed idle mode {kind}");

            word.right_stick = (80, 0);
            let injected = display(&pad, &game, &word);
            assert_eq!(injected.buttons, face, "injected mode {kind}");
        }
    }

    #[test]
    fn injected_action_while_idle_lights_priority_button() {
        let view = shown(&physical(0), pro_default(), JUMP);
        assert_eq!(view.buttons, HID_X);
        let guard = shown(&physical(0), pro_default(), GUARD | CATCH);
        assert_eq!(guard.buttons, HID_ZL);
        let shield = shown(&physical(0), pro_default(), GUARD);
        assert_eq!(shield.buttons, HID_R);
    }

    #[test]
    fn injected_action_while_unrelated_button_stays_dark() {
        let pad = physical(HID_B);
        let view = shown(&pad, pro_default(), JUMP);
        assert_eq!(view.buttons, HID_X);
        assert_eq!(view.buttons & HID_B, 0);
    }

    #[test]
    fn gc_triggers_keep_raw_and_show_injected_guard() {
        let mut mapping = [KIND_UNSET; MAPPING_LEN];
        mapping[0x00] = KIND_GUARD;
        mapping[0x01] = KIND_GUARD;
        mapping[0x02] = KIND_GRAB;
        mapping[0x06] = KIND_ATTACK;
        mapping[0x07] = KIND_SPECIAL;
        mapping[0x0A] = KIND_JUMP;
        mapping[0x09] = KIND_JUMP;
        mapping[0x0C] = 1;
        mapping[0x0D] = 1;
        let mut pad = physical(HID_A);
        pad.gc_triggers = Some((0, 0));
        let game = live(STYLE_GC, mapping);
        let injected = display(&pad, &game, &final_buttons(GUARD));
        assert_eq!(injected.buttons & HID_A, 0);
        assert_eq!(injected.buttons & HID_R, HID_R);
        assert_eq!(injected.gc_triggers, Some((0, GC_TRIGGER_LIGHT)));

        pad.buttons = 0;
        pad.gc_triggers = Some((0, 8_000));
        let held = display(&pad, &game, &final_buttons(GUARD | 0x8000));
        assert_eq!(held.buttons, 0);
        assert_eq!(held.gc_triggers, Some((0, 8_000)));

        pad.buttons = HID_R;
        pad.gc_triggers = Some((100, 0));
        let digital = display(&pad, &game, &final_buttons(GUARD));
        assert_eq!(digital.buttons, HID_R);
        assert_eq!(digital.gc_triggers, Some((100, 0)));
    }

    #[test]
    fn joycon_single_style() {
        let mut right = [KIND_UNSET; MAPPING_LEN];
        right[0x23] = KIND_JUMP;
        right[0x26] = KIND_ATTACK;
        let game = live(STYLE_RIGHT, right);
        let pressed = display(&physical(HID_Y), &game, &final_buttons(JUMP));
        assert_eq!(pressed.buttons, HID_Y);
        let idle = display(&physical(0), &game, &final_buttons(JUMP));
        assert_eq!(idle.buttons, HID_Y);
        let unrelated = display(&physical(HID_A), &game, &final_buttons(JUMP));
        assert_eq!(unrelated.buttons, HID_Y);

        let mut left_map = [KIND_UNSET; MAPPING_LEN];
        left_map[0x23] = KIND_JUMP;
        left_map[0x26] = KIND_ATTACK;
        let left = live(STYLE_LEFT, left_map);
        let face = display(&physical(HID_DRIGHT), &left, &final_buttons(JUMP));
        assert_eq!(face.buttons, HID_DRIGHT);
        let injected = display(&physical(HID_A), &left, &final_buttons(JUMP));
        assert_eq!(injected.buttons, HID_DRIGHT | HID_A);
    }

    #[test]
    fn stick_agreement_keeps_physical_and_replacement_uses_final() {
        let mapping = pro_default();
        let mut pad = physical(0);
        pad.left_stick = (1_000, -400);
        let mut game = live(5, mapping);
        game.left_stick = (
            1_000.0 / HID_STICK_MAX as f32,
            -400.0 / HID_STICK_MAX as f32,
        );
        let mut word = final_buttons(0x8000);
        word.left_stick = (0, 0);
        let deadzone = display(&pad, &game, &word);
        assert_eq!(deadzone.left_stick, pad.left_stick);
        assert!(!deadzone.left_from_final);

        word.left_stick = (120, 0);
        let injected = display(&pad, &game, &word);
        assert!(injected.left_from_final);
        assert_eq!(injected.left_stick.0, axis_to_hid(120));
        assert_eq!(injected.left_stick.1, 0);

        word.left_stick = (-30, 0);
        let opposite = display(&pad, &game, &word);
        assert!(opposite.left_from_final);
        assert!(opposite.left_stick.0 < 0);
    }

    #[test]
    fn right_joycon_rotation_still_shows_the_physical_stick() {
        let mut game = live(STYLE_RIGHT, [KIND_UNSET; MAPPING_LEN]);
        game.right_stick = (1.0, 0.0);
        let mut pad = physical(0);
        pad.right_stick = (HID_STICK_MAX, 0);
        let mut word = final_buttons(0);
        word.left_stick = (0, STICK_SCALE as i8);
        let view = display(&pad, &game, &word);
        assert_eq!(view.right_stick, pad.right_stick);
        assert!(!view.right_from_final);

        game.right_stick = (0.0, 0.0);
        pad.right_stick = (0, 0);
        word.left_stick = (120, 0);
        let injected = display(&pad, &game, &word);
        assert!(injected.right_from_final);
        assert_eq!(injected.right_stick, (0, axis_to_hid(120)));
    }

    #[test]
    fn flick_jump_and_stock_share_light_nothing() {
        let view = shown(&physical(0), pro_default(), 0x8000 | 0x100 | CSTICK_ON);
        assert_eq!(view.buttons, 0);
    }

    #[test]
    fn unknown_style_keeps_the_physical_pad() {
        let pad = physical(HID_A | HID_PLUS);
        let view = display(&pad, &live(0, pro_default()), &final_buttons(JUMP));
        assert_eq!(view.buttons, pad.buttons);
        assert!(!view.left_from_final);
    }
}
