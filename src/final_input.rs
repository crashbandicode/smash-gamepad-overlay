//! Read Smash 13.0.5's published final input and draw it on the local player's layout.
//!
//! No hooks. A mismatch in any guarded instruction window leaves the overlay on raw HID.

use std::sync::atomic::{AtomicBool, Ordering};

use skyline::hooks::{getRegionAddress, Region};

use sgpo_final_input::{display, FinalInput, LiveController, Physical, Shown, MAPPING_LEN};

use crate::config::{
    NPAD_STYLE_FULL_KEY, NPAD_STYLE_GAMECUBE, NPAD_STYLE_HANDHELD, NPAD_STYLE_JOY_DUAL,
    NPAD_STYLE_JOY_LEFT, NPAD_STYLE_JOY_RIGHT,
};
use crate::input::ControllerSnapshot;
use crate::logger::{hex_bytes, trace};
use crate::pane_lifetime::read_installed_prologue;

const SUPPORTED_VERSION: &str = "13.0.5";
const MANAGER_GLOBAL: usize = 0x52c50f0;
const MANAGER_VTABLE: usize = 0x509e5a8;
const CONTROLLER_TABLE: usize = 0x5339860;
const CONTROLLER_FALLBACK: usize = 0x5339830;
const CONTROLLER_MODE: usize = 0x53398b0;
const JOY_L_GATE: usize = 0x52b92f0;

static ENABLED: AtomicBool = AtomicBool::new(false);

struct Guard {
    offset: usize,
    bytes: &'static [u8; 64],
    name: &'static str,
}

// Bytes from main `21450C647B8C5940ED5B68218EA128FD3C5D597D`, file offset VA+0x100.
const GUARDS: [Guard; 9] = [
    Guard {
        offset: 0x17545f0,
        name: "input-manager vtable store",
        bytes: &[
            0xf3, 0x0f, 0x1e, 0xf8, 0xfd, 0x7b, 0x01, 0xa9, 0xfd, 0x43, 0x00, 0x91, 0x48, 0xca,
            0x01, 0xd0, 0x08, 0xa1, 0x16, 0x91, 0xf3, 0x03, 0x00, 0xaa, 0x08, 0x00, 0x00, 0xf9,
            0x00, 0xc0, 0x16, 0x91, 0xf8, 0x0b, 0x00, 0x94, 0x60, 0xca, 0x42, 0xf9, 0x48, 0xca,
            0x01, 0xd0, 0x08, 0xe1, 0x23, 0x91, 0x68, 0xc6, 0x02, 0xf9, 0x7f, 0xca, 0x02, 0xf9,
            0x80, 0x00, 0x00, 0xb4, 0x08, 0x00, 0x40, 0xf9,
        ],
    },
    Guard {
        offset: 0x1754650,
        name: "input-manager update setup",
        bytes: &[
            0xfa, 0x67, 0xbb, 0xa9, 0xf8, 0x5f, 0x01, 0xa9, 0xf6, 0x57, 0x02, 0xa9, 0xf4, 0x4f,
            0x03, 0xa9, 0xfd, 0x7b, 0x04, 0xa9, 0xfd, 0x03, 0x01, 0x91, 0xf3, 0x03, 0x00, 0xaa,
            0xf6, 0x03, 0x1f, 0xaa, 0xf4, 0x03, 0x1f, 0xaa, 0x15, 0x60, 0x00, 0x91, 0x17, 0xe0,
            0x0a, 0x91, 0x38, 0xdf, 0x01, 0xb0, 0x18, 0xc3, 0x20, 0x91, 0x39, 0xdf, 0x01, 0xb0,
            0x3a, 0xdf, 0x01, 0xb0, 0x5a, 0x83, 0x21, 0x91,
        ],
    },
    Guard {
        offset: 0x1754690,
        name: "input-manager player loop",
        bytes: &[
            0x68, 0x0a, 0x14, 0x8b, 0x08, 0x99, 0x82, 0xb9, 0x1f, 0x25, 0x00, 0x71, 0x08, 0x04,
            0x00, 0x54, 0x29, 0x5b, 0x44, 0xf9, 0x29, 0x21, 0x40, 0x39, 0x89, 0x00, 0x00, 0x34,
            0x43, 0x7b, 0x68, 0xf8, 0x63, 0x00, 0x00, 0xb5, 0x1a, 0x00, 0x00, 0x14, 0xe3, 0x03,
            0x18, 0xaa, 0x69, 0x08, 0x40, 0xf9, 0x29, 0xe1, 0x42, 0x39, 0x68, 0x02, 0x14, 0x8b,
            0xa9, 0x01, 0x00, 0x34, 0x1f, 0xe1, 0x14, 0x39,
        ],
    },
    Guard {
        offset: 0x1754730,
        name: "input-manager override copy",
        bytes: &[
            0x68, 0x02, 0x56, 0x39, 0x68, 0x00, 0x00, 0x34, 0x68, 0xa2, 0x42, 0xf9, 0x68, 0x5e,
            0x01, 0xf9, 0x68, 0x06, 0x56, 0x39, 0x68, 0x00, 0x00, 0x34, 0x68, 0xa6, 0x42, 0xf9,
            0x68, 0x62, 0x01, 0xf9, 0x68, 0x0a, 0x56, 0x39, 0x68, 0x00, 0x00, 0x34, 0x68, 0xaa,
            0x42, 0xf9, 0x68, 0x66, 0x01, 0xf9, 0x68, 0x0e, 0x56, 0x39, 0x68, 0x00, 0x00, 0x34,
            0x68, 0xae, 0x42, 0xf9, 0x68, 0x6a, 0x01, 0xf9,
        ],
    },
    Guard {
        offset: 0x1750e08,
        name: "final-input style dispatch",
        bytes: &[
            0x74, 0x08, 0x40, 0xf9, 0x82, 0x9e, 0x40, 0xb9, 0x48, 0x04, 0x00, 0x51, 0xf5, 0x03,
            0x00, 0xaa, 0x1f, 0x15, 0x00, 0x71, 0x28, 0x04, 0x00, 0x54, 0x69, 0x06, 0x80, 0x52,
            0x28, 0x25, 0xc8, 0x1a, 0x68, 0x06, 0x00, 0x37, 0x48, 0x0c, 0x00, 0x51, 0x1f, 0x05,
            0x00, 0x71, 0xa8, 0x75, 0x00, 0x54, 0x5f, 0x0c, 0x00, 0x71, 0xe8, 0x03, 0x19, 0x32,
            0xe9, 0x03, 0x1f, 0x32, 0x28, 0x01, 0x88, 0x1a,
        ],
    },
    Guard {
        offset: 0x1750f10,
        name: "final-input pro attack mapping",
        bytes: &[
            0x08, 0x0a, 0x80, 0x52, 0xe8, 0x56, 0x08, 0x9b, 0x09, 0x59, 0x40, 0x39, 0x3f, 0x35,
            0x00, 0x71, 0xa0, 0x01, 0x00, 0x54, 0x3f, 0x11, 0x00, 0x71, 0x61, 0x03, 0x00, 0x54,
            0xe9, 0x03, 0x1f, 0x2a, 0xe8, 0x03, 0x1f, 0x2a, 0xea, 0x07, 0x1d, 0x32, 0x8b, 0x15,
            0x30, 0x37, 0xe4, 0x00, 0x00, 0x14, 0xe9, 0x03, 0x1f, 0x2a, 0xe8, 0x03, 0x1f, 0x2a,
            0xea, 0x03, 0x1f, 0x2a, 0xcb, 0x10, 0x30, 0x37,
        ],
    },
    Guard {
        offset: 0x1750ec0,
        name: "final-input gamecube attack mapping",
        bytes: &[
            0x0b, 0x15, 0x28, 0x36, 0x08, 0x0a, 0x80, 0x52, 0xe8, 0x56, 0x08, 0x9b, 0x09, 0x19,
            0x40, 0x39, 0x3f, 0x35, 0x00, 0x71, 0x60, 0x03, 0x00, 0x54, 0x3f, 0x11, 0x00, 0x71,
            0x61, 0x04, 0x00, 0x54, 0xe9, 0x03, 0x1f, 0x2a, 0xe8, 0x03, 0x1f, 0x2a, 0xea, 0x07,
            0x1d, 0x32, 0xcb, 0x13, 0x30, 0x37, 0xda, 0x00, 0x00, 0x14, 0xff, 0x23, 0x00, 0xb9,
            0xbf, 0xc3, 0x1d, 0xb8, 0x8b, 0x0a, 0x40, 0xb9,
        ],
    },
    Guard {
        offset: 0x1751050,
        name: "final-input joy-con shoulder mapping",
        bytes: &[
            0x2b, 0x8a, 0x00, 0x36, 0x4b, 0x7d, 0x00, 0x91, 0x4a, 0xdb, 0x01, 0x90, 0x4c, 0xc1,
            0x4b, 0x39, 0x8a, 0x0a, 0x40, 0xb9, 0x6c, 0x6b, 0x00, 0x35, 0x4a, 0x6b, 0x40, 0x36,
            0x6c, 0x01, 0x40, 0x39, 0x9f, 0x35, 0x00, 0x71, 0xe0, 0x04, 0x00, 0x54, 0x9f, 0x11,
            0x00, 0x71, 0x21, 0x05, 0x00, 0x54, 0xec, 0x07, 0x1d, 0x32, 0x89, 0x01, 0x09, 0x2a,
            0x6a, 0x6a, 0x48, 0x37, 0x6d, 0x03, 0x00, 0x14,
        ],
    },
    Guard {
        offset: 0x1751a68,
        name: "final-input stick scale",
        bytes: &[
            0x08, 0x69, 0x01, 0xb0, 0xe0, 0x0f, 0xc0, 0x3d, 0x01, 0xad, 0x4f, 0xbd, 0x22, 0x90,
            0x80, 0x5f, 0x20, 0x90, 0xa0, 0x5f, 0x48, 0x00, 0x38, 0x1e, 0x68, 0x12, 0x00, 0x39,
            0x09, 0x00, 0x38, 0x1e, 0x08, 0x0a, 0x80, 0x52, 0xe8, 0x56, 0x08, 0x9b, 0x69, 0x16,
            0x00, 0x39, 0x03, 0x39, 0x40, 0x39, 0x02, 0x21, 0x40, 0x39, 0x8d, 0x00, 0x00, 0x14,
            0x08, 0x05, 0x00, 0x11, 0xa8, 0xc3, 0x1d, 0xb8,
        ],
    },
];

pub(crate) fn install(display_version: &str) {
    if display_version != SUPPORTED_VERSION {
        trace(&format!(
            "final-input view disabled for Smash {display_version}; using raw HID"
        ));
        return;
    }
    for guard in &GUARDS {
        match read_installed_prologue(guard.offset) {
            Ok(actual) if actual == *guard.bytes => {}
            Ok(actual) => {
                trace(&format!(
                    "final-input guard failed at .text+0x{:x} ({}); expected {}",
                    guard.offset,
                    guard.name,
                    hex_bytes(guard.bytes)
                ));
                trace(&format!(
                    "final-input guard actual {}; final-input view disabled, using raw HID",
                    hex_bytes(&actual)
                ));
                return;
            }
            Err(reason) => {
                trace(&format!(
                    "final-input guard failed at .text+0x{:x} ({}): {reason}; final-input view disabled, using raw HID",
                    guard.offset, guard.name
                ));
                return;
            }
        }
    }
    ENABLED.store(true, Ordering::Release);
    trace("final-input view enabled; nine instruction windows matched 13.0.5");
}

pub(crate) unsafe fn apply(snapshot: ControllerSnapshot) -> ControllerSnapshot {
    if !ENABLED.load(Ordering::Acquire) {
        return snapshot;
    }
    match read_local(&snapshot) {
        Ok(found) => {
            observe(&snapshot, &found);
            ControllerSnapshot {
                buttons: found.shown.buttons,
                left_stick: found.shown.left_stick,
                right_stick: found.shown.right_stick,
                gc_triggers: found.shown.gc_triggers,
                ..snapshot
            }
        }
        Err(miss) => {
            observe_miss(snapshot.npad_id, miss);
            snapshot
        }
    }
}

struct Found {
    shown: Shown,
    #[cfg_attr(not(feature = "diag"), allow(dead_code))]
    player: u32,
    #[cfg_attr(not(feature = "diag"), allow(dead_code))]
    controller: i32,
    #[cfg_attr(not(feature = "diag"), allow(dead_code))]
    style: u32,
}

enum Miss {
    Unreadable,
    NoPlayer,
    StyleMismatch { style: u32 },
}

unsafe fn read_local(snapshot: &ControllerSnapshot) -> Result<Found, Miss> {
    let base = getRegionAddress(Region::Text) as usize;
    if base == 0 {
        return Err(Miss::Unreadable);
    }
    let obj = read_ptr(base + MANAGER_GLOBAL).ok_or(Miss::Unreadable)?;
    if read_u64(obj).ok_or(Miss::Unreadable)? != (base + MANAGER_VTABLE) as u64 {
        return Err(Miss::Unreadable);
    }
    let mode = read_ptr(base + CONTROLLER_MODE).ok_or(Miss::Unreadable)?;
    let use_table = read_u8(mode + 8) != 0;
    let joy_l_is_shoulder = read_u8(base + JOY_L_GATE) == 0;
    let mut mismatch_style = None;
    for player in 0..8u32 {
        let Some(found) = read_player(base, obj, player, use_table, joy_l_is_shoulder, snapshot)
        else {
            continue;
        };
        match found {
            PlayerRead::Hit(found) => return Ok(found),
            PlayerRead::StyleMismatch { style } => mismatch_style = Some(style),
        }
    }
    match mismatch_style {
        Some(style) => Err(Miss::StyleMismatch { style }),
        None => Err(Miss::NoPlayer),
    }
}

enum PlayerRead {
    Hit(Found),
    StyleMismatch { style: u32 },
}

unsafe fn read_player(
    base: usize,
    obj: usize,
    player: u32,
    use_table: bool,
    joy_l_is_shoulder: bool,
    snapshot: &ControllerSnapshot,
) -> Option<PlayerRead> {
    let index_addr = obj + 0x298 + (player as usize) * 4;
    let controller_index = read_i32(index_addr)?;
    if !(0..=9).contains(&controller_index) {
        return None;
    }
    let struct_ptr = if use_table {
        let slot = base + CONTROLLER_TABLE + (controller_index as usize) * 8;
        read_ptr(slot)?
    } else {
        base + CONTROLLER_FALLBACK
    };
    let controller = read_ptr(struct_ptr + 0x10)?;
    if read_u32(controller + 0xc4)? != snapshot.npad_id {
        return None;
    }
    let style = read_u32(controller + 0x9c)?;
    if !style_matches_hid(style, snapshot.style_flags) {
        return Some(PlayerRead::StyleMismatch { style });
    }
    let mut mapping = [0u8; MAPPING_LEN];
    let mapping_addr = obj + 0x18 + (player as usize) * MAPPING_LEN;
    std::ptr::copy_nonoverlapping(mapping_addr as *const u8, mapping.as_mut_ptr(), MAPPING_LEN);
    let packed = read_u64(obj + 0x2b8 + (player as usize) * 8)?;
    let shown = display(
        &Physical {
            buttons: snapshot.buttons,
            left_stick: snapshot.left_stick,
            right_stick: snapshot.right_stick,
            gc_triggers: snapshot.gc_triggers,
        },
        &LiveController {
            style,
            mapping,
            left_stick: (read_f32(controller + 0x10)?, read_f32(controller + 0x14)?),
            right_stick: (read_f32(controller + 0x20)?, read_f32(controller + 0x24)?),
            joy_l_is_shoulder,
        },
        &decode_mapped(packed),
    );
    Some(PlayerRead::Hit(Found {
        shown,
        player,
        controller: controller_index,
        style,
    }))
}

fn style_matches_hid(style: u32, style_flags: u32) -> bool {
    let gc = style_flags & NPAD_STYLE_GAMECUBE != 0;
    match style {
        1 => !gc && style_flags & NPAD_STYLE_HANDHELD != 0,
        2 => !gc && style_flags & NPAD_STYLE_JOY_DUAL != 0,
        3 => !gc && style_flags & NPAD_STYLE_JOY_LEFT != 0,
        4 => !gc && style_flags & NPAD_STYLE_JOY_RIGHT != 0,
        5 | 6 => !gc && style_flags & NPAD_STYLE_FULL_KEY != 0,
        7 => gc,
        _ => false,
    }
}

fn decode_mapped(word: u64) -> FinalInput {
    let bytes = word.to_le_bytes();
    FinalInput {
        buttons: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        left_stick: (bytes[4] as i8, bytes[5] as i8),
        right_stick: (bytes[6] as i8, bytes[7] as i8),
    }
}

unsafe fn read_ptr(addr: usize) -> Option<usize> {
    let value = read_u64(addr)?;
    let ptr = value as usize;
    if ptr == 0 || !ptr.is_multiple_of(8) {
        None
    } else {
        Some(ptr)
    }
}

unsafe fn read_u64(addr: usize) -> Option<u64> {
    if addr == 0 || !addr.is_multiple_of(8) {
        return None;
    }
    Some((addr as *const u64).read_volatile())
}

unsafe fn read_u32(addr: usize) -> Option<u32> {
    if addr == 0 || !addr.is_multiple_of(4) {
        return None;
    }
    Some((addr as *const u32).read_volatile())
}

unsafe fn read_i32(addr: usize) -> Option<i32> {
    if addr == 0 || !addr.is_multiple_of(4) {
        return None;
    }
    Some((addr as *const i32).read_volatile())
}

unsafe fn read_f32(addr: usize) -> Option<f32> {
    if addr == 0 || !addr.is_multiple_of(4) {
        return None;
    }
    Some((addr as *const f32).read_volatile())
}

unsafe fn read_u8(addr: usize) -> u8 {
    (addr as *const u8).read_volatile()
}

fn observe(snapshot: &ControllerSnapshot, found: &Found) {
    #[cfg(feature = "diag")]
    diag::observe(snapshot, found);
    #[cfg(not(feature = "diag"))]
    {
        let _ = (snapshot, found);
    }
}

fn observe_miss(npad: u32, miss: Miss) {
    #[cfg(feature = "diag")]
    diag::observe_miss(npad, miss);
    #[cfg(not(feature = "diag"))]
    log_miss_once(npad, miss);
}

#[cfg(not(feature = "diag"))]
fn log_miss_once(npad: u32, miss: Miss) {
    static UNREADABLE: AtomicBool = AtomicBool::new(false);
    static NO_PLAYER: AtomicBool = AtomicBool::new(false);
    static STYLE: AtomicBool = AtomicBool::new(false);
    match miss {
        Miss::Unreadable => {
            if !UNREADABLE.swap(true, Ordering::Relaxed) {
                trace("final-input manager unreadable; using raw HID");
            }
        }
        Miss::NoPlayer => {
            if !NO_PLAYER.swap(true, Ordering::Relaxed) {
                trace(&format!(
                    "final-input no player for npad {npad}; using raw HID"
                ));
            }
        }
        Miss::StyleMismatch { style } => {
            if !STYLE.swap(true, Ordering::Relaxed) {
                trace(&format!(
                    "final-input npad {npad} style {style} disagreed with HID; using raw HID"
                ));
            }
        }
    }
}

#[cfg(feature = "diag")]
mod diag {
    use super::{Found, Miss};
    use crate::input::ControllerSnapshot;
    use crate::logger::trace;
    use std::fmt::Write;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    static LAST: AtomicU64 = AtomicU64::new(0);
    static MISSED: AtomicBool = AtomicBool::new(false);

    pub(super) fn observe(snapshot: &ControllerSnapshot, found: &Found) {
        MISSED.store(false, Ordering::Relaxed);
        let diff = snapshot.buttons ^ found.shown.buttons;
        let triggers_differ = snapshot.gc_triggers != found.shown.gc_triggers;
        let signature = mix(0x46, u64::from(found.player))
            .wrapping_add(mix(
                u64::from(found.controller as u32),
                u64::from(found.style),
            ))
            .wrapping_add(diff)
            .wrapping_add(u64::from(found.shown.left_from_final))
            .wrapping_add(u64::from(found.shown.right_from_final) << 1)
            .wrapping_add(u64::from(triggers_differ) << 2);
        if LAST.swap(signature, Ordering::AcqRel) == signature {
            return;
        }
        let mut line = StackLine::new();
        let left = source_name(found.shown.left_from_final);
        let right = source_name(found.shown.right_from_final);
        let _ = write!(
            line,
            "final-input p={} idx={} style={} npad={} diff={diff:#x} L={left} R={right} triggers={triggers_differ}",
            found.player, found.controller, found.style, snapshot.npad_id
        );
        trace(line.as_str());
    }

    pub(super) fn observe_miss(npad: u32, miss: Miss) {
        if MISSED.swap(true, Ordering::Relaxed) {
            return;
        }
        LAST.store(0, Ordering::Relaxed);
        let mut line = StackLine::new();
        match miss {
            Miss::Unreadable => {
                let _ = write!(line, "final-input unreadable for npad {npad}; raw HID");
            }
            Miss::NoPlayer => {
                let _ = write!(line, "final-input no player for npad {npad}; raw HID");
            }
            Miss::StyleMismatch { style } => {
                let _ = write!(
                    line,
                    "final-input npad {npad} style {style} disagreed with HID; raw HID"
                );
            }
        }
        trace(line.as_str());
    }

    fn source_name(from_final: bool) -> &'static str {
        if from_final {
            "final"
        } else {
            "physical"
        }
    }

    fn mix(left: u64, right: u64) -> u64 {
        left.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ right
    }

    struct StackLine {
        buf: [u8; 180],
        len: usize,
    }

    impl StackLine {
        fn new() -> Self {
            Self {
                buf: [0; 180],
                len: 0,
            }
        }

        fn as_str(&self) -> &str {
            core::str::from_utf8(&self.buf[..self.len]).unwrap_or("final-input")
        }
    }

    impl Write for StackLine {
        fn write_str(&mut self, text: &str) -> std::fmt::Result {
            let bytes = text.as_bytes();
            let next = self.len.checked_add(bytes.len()).ok_or(std::fmt::Error)?;
            if next > self.buf.len() {
                return Err(std::fmt::Error);
            }
            self.buf[self.len..next].copy_from_slice(bytes);
            self.len = next;
            Ok(())
        }
    }
}
