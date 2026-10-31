//! Pure translation from ssbu-cpu-pilot's packed `MappedInputs` word to the
//! Pro Controller fields SGPO already draws.
//!
//! `last_input` is the little-endian image of
//! `{ buttons: u32, lstick_x: i8, lstick_y: i8, rstick_x: i8, rstick_y: i8 }`
//! with sticks in -120..=120. Button bits are the game's semantic actions
//! (Training Modpack `Buttons`). HID stick axes use the same full-scale
//! magnitude as `config::STICK_AXIS_MAX` (32767).
//!
//! Face-button choice: the buttons a default Pro Controller player presses.
//! `ATTACK_RAW` and `SMASH` fold into A, `SPECIAL_RAW` and `SPECIAL_RAW2`
//! fold into B, and `JUMP_MINI` folds into X, so a flag that means that action
//! still lights the same button. `FLICK_JUMP`, `CSTICK_ON` and `STOCK_SHARE`
//! are ignored: `FLICK_JUMP` (0x8000) is set in the mapped input even at rest
//! (Eden g01/g02, 2026-10-31: X lit in every CPU frame while it was mapped),
//! the c-stick is the right-stick axes (the CPU leaves them at 0), and stock
//! share is not a default Pro face button.

use std::fmt::{self, Write};

/// Full-scale nn::hid stick axis. Matches `config::STICK_AXIS_MAX`.
pub(crate) const HID_STICK_MAX: i32 = 32767;
const MAPPED_STICK_MAX: i32 = 120;

const ATTACK: u32 = 0x1;
const SPECIAL: u32 = 0x2;
const JUMP: u32 = 0x4;
const GUARD: u32 = 0x8;
const CATCH: u32 = 0x10;
const SMASH: u32 = 0x20;
const JUMP_MINI: u32 = 0x40;
const CSTICK_ON: u32 = 0x80;
const STOCK_SHARE: u32 = 0x100;
const ATTACK_RAW: u32 = 0x200;
const APPEAL_HI: u32 = 0x400;
const SPECIAL_RAW: u32 = 0x800;
const APPEAL_LW: u32 = 0x1000;
const APPEAL_SL: u32 = 0x2000;
const APPEAL_SR: u32 = 0x4000;
/// Set even at rest; deliberately not mapped (see the module docs). Kept for
/// the test that proves it lights nothing.
#[cfg(test)]
const FLICK_JUMP: u32 = 0x8000;
const GUARD_HOLD: u32 = 0x1_0000;
const SPECIAL_RAW2: u32 = 0x2_0000;

const HID_A: u64 = 1 << 0;
const HID_B: u64 = 1 << 1;
const HID_X: u64 = 1 << 2;
const HID_R: u64 = 1 << 7;
const HID_ZR: u64 = 1 << 9;
const HID_DLEFT: u64 = 1 << 12;
const HID_DUP: u64 = 1 << 13;
const HID_DRIGHT: u64 = 1 << 14;
const HID_DDOWN: u64 = 1 << 15;

/// Pro Controller fields SGPO can draw without reading HID.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) struct PilotSample {
    pub buttons: u64,
    pub left_stick: (i32, i32),
    pub right_stick: (i32, i32),
    pub cpu_level: u8,
}

/// `None` when `armed_level` is 0, which means the player still drives P1.
pub(crate) fn sample_from_pilot(last_input: u64, armed_level: u8) -> Option<PilotSample> {
    if armed_level == 0 {
        return None;
    }
    let buttons = last_input as u32;
    let left_stick = (
        scale_stick((last_input >> 32) as u8 as i8),
        scale_stick((last_input >> 40) as u8 as i8),
    );
    let right_stick = (
        scale_stick((last_input >> 48) as u8 as i8),
        scale_stick((last_input >> 56) as u8 as i8),
    );
    Some(PilotSample {
        buttons: pro_buttons(buttons),
        left_stick,
        right_stick,
        cpu_level: armed_level,
    })
}

pub(crate) fn write_cpu_label(level: u8, out: &mut impl Write) -> fmt::Result {
    write!(out, "CPU Lv{level}")
}

fn pro_buttons(semantic: u32) -> u64 {
    // CSTICK_ON is the right stick, which is already scaled. STOCK_SHARE is
    // not a default Pro face button.
    let semantic = semantic & !(CSTICK_ON | STOCK_SHARE);
    let mut buttons = 0u64;
    if semantic & (ATTACK | ATTACK_RAW | SMASH) != 0 {
        buttons |= HID_A;
    }
    if semantic & (SPECIAL | SPECIAL_RAW | SPECIAL_RAW2) != 0 {
        buttons |= HID_B;
    }
    if semantic & (JUMP | JUMP_MINI) != 0 {
        buttons |= HID_X;
    }
    if semantic & (GUARD | GUARD_HOLD) != 0 {
        buttons |= HID_R;
    }
    if semantic & CATCH != 0 {
        buttons |= HID_ZR;
    }
    if semantic & APPEAL_HI != 0 {
        buttons |= HID_DUP;
    }
    if semantic & APPEAL_LW != 0 {
        buttons |= HID_DDOWN;
    }
    if semantic & APPEAL_SL != 0 {
        buttons |= HID_DLEFT;
    }
    if semantic & APPEAL_SR != 0 {
        buttons |= HID_DRIGHT;
    }
    buttons
}

fn scale_stick(axis: i8) -> i32 {
    let axis = i64::from(axis).clamp(-i64::from(MAPPED_STICK_MAX), i64::from(MAPPED_STICK_MAX));
    (axis * i64::from(HID_STICK_MAX) / i64::from(MAPPED_STICK_MAX)) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack(buttons: u32, lx: i8, ly: i8, rx: i8, ry: i8) -> u64 {
        u64::from(buttons)
            | (u64::from(lx as u8) << 32)
            | (u64::from(ly as u8) << 40)
            | (u64::from(rx as u8) << 48)
            | (u64::from(ry as u8) << 56)
    }

    #[test]
    fn player_level_keeps_the_physical_pad() {
        assert_eq!(sample_from_pilot(pack(ATTACK, 120, 0, 0, 0), 0), None);
    }

    #[test]
    fn armed_neutral_input_still_replaces_the_pad() {
        let sample = sample_from_pilot(0, 3).unwrap();
        assert_eq!(sample.cpu_level, 3);
        assert_eq!(sample.buttons, 0);
        assert_eq!(sample.left_stick, (0, 0));
        assert_eq!(sample.right_stick, (0, 0));
    }

    #[test]
    fn default_pro_buttons_and_full_scale_sticks() {
        let semantic =
            ATTACK | SPECIAL | JUMP | GUARD | CATCH | APPEAL_HI | APPEAL_LW | APPEAL_SL | APPEAL_SR;
        let sample = sample_from_pilot(pack(semantic, 120, -120, 60, -30), 9).unwrap();
        assert_eq!(
            sample.buttons,
            HID_A | HID_B | HID_X | HID_R | HID_ZR | HID_DUP | HID_DDOWN | HID_DLEFT | HID_DRIGHT
        );
        assert_eq!(sample.left_stick, (HID_STICK_MAX, -HID_STICK_MAX));
        assert_eq!(sample.right_stick, (16383, -8191));
        assert_eq!(sample.cpu_level, 9);
    }

    #[test]
    fn raw_smash_and_mini_jump_fold_into_the_same_buttons() {
        let folded = ATTACK_RAW | SPECIAL_RAW | SPECIAL_RAW2 | SMASH | JUMP_MINI | GUARD_HOLD;
        let sample = sample_from_pilot(pack(folded, 0, 0, 0, 0), 1).unwrap();
        assert_eq!(sample.buttons, HID_A | HID_B | HID_X | HID_R);
    }

    #[test]
    fn the_at_rest_flick_jump_flag_lights_nothing() {
        // The mapped input carries FLICK_JUMP even with no action pressed.
        let sample = sample_from_pilot(pack(FLICK_JUMP, 0, 0, 0, 0), 1).unwrap();
        assert_eq!(sample.buttons, 0);
        let attack = sample_from_pilot(pack(FLICK_JUMP | 0x1, 0, 0, 0, 0), 1).unwrap();
        assert_eq!(attack.buttons, HID_A);
    }

    #[test]
    fn cstick_and_stock_share_do_not_light_a_button() {
        let sample = sample_from_pilot(pack(CSTICK_ON | STOCK_SHARE, 0, 0, 0, 0), 1).unwrap();
        assert_eq!(sample.buttons, 0);
    }

    #[test]
    fn stick_bytes_outside_the_mapped_range_clamp() {
        let sample = sample_from_pilot(pack(0, i8::MAX, i8::MIN, 0, 0), 1).unwrap();
        assert_eq!(sample.left_stick, (HID_STICK_MAX, -HID_STICK_MAX));
    }

    #[test]
    fn label_names_the_armed_level() {
        let mut line = String::new();
        write_cpu_label(3, &mut line).unwrap();
        assert_eq!(line, "CPU Lv3");
    }
}
