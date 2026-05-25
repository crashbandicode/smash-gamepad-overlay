#![allow(non_snake_case)]

use skyline::hooks::{getRegionAddress, Region};
use skyline::libc::c_char;
use skyline::nn::ui2d::{
    HorizontalPosition, Layout, Pane, PaneFlag, TextBox, TextBoxFlag, VerticalPosition,
};
use skyline::nn::{hid, oe};
use std::ffi::{CStr, CString};
use std::fmt::Write as FmtWrite;
use std::fs::OpenOptions;
use std::io::Write as IoWrite;
use std::path::Path;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::LazyLock;

const PLUGIN_NAME: &str = "smash-gamepad-overlay";
const LOG_PATH: &str = "sd:/smash-gamepad-overlay.log";
const NPAD_ID_NO1: u32 = 0;
const NPAD_ID_HANDHELD: u32 = 0x20;

const NPAD_STYLE_FULL_KEY: u32 = 1 << 0;
const NPAD_STYLE_HANDHELD: u32 = 1 << 1;
const NPAD_STYLE_JOY_DUAL: u32 = 1 << 2;
const NPAD_STYLE_JOY_LEFT: u32 = 1 << 3;
const NPAD_STYLE_JOY_RIGHT: u32 = 1 << 4;
const NPAD_STYLE_GAMECUBE: u32 = 1 << 5;

const MATCH_HUD_LAYOUT: &str = "info_melee";
const LEGACY_DRAW_OFFSET: usize = 0x4b620;
const BEST_MATCHES_TO_LOG: usize = 3;
const TEXT_SCAN_ALIGNMENT: usize = 4;
const OVERLAY_POS_X: f32 = -140.0;
const OVERLAY_POS_Y: f32 = 285.0;
const OVERLAY_WIDTH: f32 = 760.0;
const OVERLAY_FONT_SIZE: f32 = 16.0;
const OVERLAY_LINE_COUNT: usize = 4;
const OVERLAY_LINE_HEIGHT: f32 = 19.0;
const TRAINING_MODPACK_PLUGIN_PATH: &str =
    "sd:/atmosphere/contents/01006A800016E000/romfs/skyline/plugins/libtraining_modpack.nro";
const P1_PARTS_PANE_NAME: &[u8] = b"p1\0";

const OVERLAY_LINE_TEXT_PANES: [&[u8]; 18] = [
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

static DRAW_HOOK_LOGGED: AtomicBool = AtomicBool::new(false);
static MATCH_HUD_LAYOUT_LOGGED: AtomicBool = AtomicBool::new(false);
static MISSING_TEXT_PANE_LOGGED: AtomicBool = AtomicBool::new(false);
static P1_PARTS_PANE_MISSING_LOGGED: AtomicBool = AtomicBool::new(false);
static P1_PARTS_LAYOUT_MISSING_LOGGED: AtomicBool = AtomicBool::new(false);
static OVERLAY_TEXT_PANES_LOGGED: AtomicBool = AtomicBool::new(false);

static OFFSET_DRAW: LazyLock<Option<usize>> = LazyLock::new(resolve_draw_offset);

// First instructions of nn::ui2d::Layout::Draw in Smash 13.0.1.
static NEEDLE_DRAW: &[u8] = &[
    0x08, 0x0c, 0x40, 0xf9, 0xc8, 0x03, 0x00, 0xb4, 0xff, 0x83, 0x01, 0xd1, 0xf5, 0x1b, 0x00, 0xf9,
    0xf4, 0x4f, 0x04, 0xa9, 0xfd, 0x7b, 0x05, 0xa9, 0xfd, 0x43, 0x01, 0x91, 0xf4, 0x03, 0x00, 0xaa,
];

#[skyline::from_offset(0x59970)]
unsafe fn find_pane_by_name_recursive(pane: *const Pane, name: *const c_char) -> *mut Pane;

#[skyline::from_offset(0x37a22f0)]
unsafe fn pane_set_text_string(pane: *mut TextBox, text: *const c_char);

#[derive(Debug, Copy, Clone)]
struct ControllerSnapshot {
    npad_id: u32,
    style_flags: u32,
    buttons: u64,
    left_stick: (i32, i32),
    right_stick: (i32, i32),
    gc_triggers: Option<(u32, u32)>,
}

struct OverlayTextPanes {
    panes: [*mut Pane; OVERLAY_LINE_COUNT],
    count: usize,
}

impl OverlayTextPanes {
    fn new() -> Self {
        Self {
            panes: [ptr::null_mut(); OVERLAY_LINE_COUNT],
            count: 0,
        }
    }

    fn push(&mut self, pane: *mut Pane) {
        if pane.is_null() || self.count >= OVERLAY_LINE_COUNT || self.contains(pane) {
            return;
        }

        self.panes[self.count] = pane;
        self.count += 1;
    }

    fn contains(&self, pane: *mut Pane) -> bool {
        self.panes[..self.count].iter().any(|found| *found == pane)
    }
}

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

            draw_overlay((*layout).root_pane, poll_p1_controller());
        }
    }

    original!()(layout, draw_info, cmd_buffer);
}

fn resolve_draw_offset() -> Option<usize> {
    trace(&format!("Smash display version {}", display_version()));

    let training_modpack_present = training_modpack_plugin_present();
    if training_modpack_present {
        trace(&format!(
            "detected Training Modpack plugin at {TRAINING_MODPACK_PLUGIN_PATH}"
        ));
        trace(
            "not installing draw hook because Training Modpack also scans and hooks Layout::Draw",
        );
        return None;
    }

    let offset = find_unique_text_offset("nn::ui2d::Layout::Draw", NEEDLE_DRAW);
    if offset.is_none() {
        log_draw_offset_diagnostics(NEEDLE_DRAW);
    }

    offset
}

fn draw_hook_offset_for_install() -> usize {
    (*OFFSET_DRAW).expect("Layout::Draw offset checked before installing draw hook")
}

fn find_unique_text_offset(name: &str, needle: &[u8]) -> Option<usize> {
    let haystack = unsafe { text_region_bytes() };

    if haystack.len() < needle.len() {
        trace(&format!("{name}: text region is smaller than signature"));
        return None;
    }

    let mut found = None;
    for offset in 0..=haystack.len() - needle.len() {
        if &haystack[offset..offset + needle.len()] != needle {
            continue;
        }
        if found.is_some() {
            trace(&format!("{name}: found multiple signature matches"));
            return None;
        }
        found = Some(offset);
    }

    if let Some(offset) = found {
        trace(&format!("{name}: found at .text+0x{offset:x}"));
    } else {
        trace(&format!("{name}: signature was not found"));
    }

    found
}

unsafe fn text_region_bytes() -> &'static [u8] {
    let start = getRegionAddress(Region::Text) as *const u8;
    let end = getRegionAddress(Region::Rodata) as *const u8;
    let len = end.offset_from(start) as usize;
    std::slice::from_raw_parts(start, len)
}

fn log_draw_offset_diagnostics(needle: &[u8]) {
    let text_start = unsafe { getRegionAddress(Region::Text) as usize };
    let rodata_start = unsafe { getRegionAddress(Region::Rodata) as usize };
    let haystack = unsafe { text_region_bytes() };

    trace(&format!(
        ".text=0x{text_start:x} .rodata=0x{rodata_start:x} scan_len=0x{:x}",
        haystack.len()
    ));

    if LEGACY_DRAW_OFFSET + needle.len() <= haystack.len() {
        let legacy_bytes = &haystack[LEGACY_DRAW_OFFSET..LEGACY_DRAW_OFFSET + needle.len()];
        trace(&format!(
            "bytes at legacy Layout::Draw .text+0x{LEGACY_DRAW_OFFSET:x}: {}",
            hex_bytes(legacy_bytes)
        ));

        let first_instruction = u32::from_le_bytes([
            legacy_bytes[0],
            legacy_bytes[1],
            legacy_bytes[2],
            legacy_bytes[3],
        ]);
        if looks_like_aarch64_branch(first_instruction) {
            trace(
                "legacy Layout::Draw starts with a branch; another plugin may have hooked it first",
            );
        }
    } else {
        trace(&format!(
            "legacy Layout::Draw .text+0x{LEGACY_DRAW_OFFSET:x} is outside scanned text"
        ));
    }

    log_best_signature_matches("nn::ui2d::Layout::Draw", haystack, needle);
}

fn log_best_signature_matches(name: &str, haystack: &[u8], needle: &[u8]) {
    if haystack.len() < needle.len() {
        return;
    }

    let mut best = [(usize::MAX, 0usize); BEST_MATCHES_TO_LOG];

    for offset in (0..=haystack.len() - needle.len()).step_by(TEXT_SCAN_ALIGNMENT) {
        let mismatches = needle
            .iter()
            .zip(&haystack[offset..offset + needle.len()])
            .filter(|(expected, actual)| expected != actual)
            .count();

        for index in 0..BEST_MATCHES_TO_LOG {
            if mismatches < best[index].0 {
                for move_index in (index + 1..BEST_MATCHES_TO_LOG).rev() {
                    best[move_index] = best[move_index - 1];
                }
                best[index] = (mismatches, offset);
                break;
            }
        }
    }

    trace(&format!("closest aligned {name} signature candidates:"));
    for (mismatches, offset) in best {
        if mismatches == usize::MAX {
            continue;
        }

        let candidate = &haystack[offset..offset + needle.len()];
        trace(&format!(
            "  .text+0x{offset:x}: {mismatches}/{} bytes differ: {}",
            needle.len(),
            hex_bytes(candidate)
        ));
    }
}

fn looks_like_aarch64_branch(instruction: u32) -> bool {
    instruction & 0x7c00_0000 == 0x1400_0000
}

fn training_modpack_plugin_present() -> bool {
    Path::new(TRAINING_MODPACK_PLUGIN_PATH).exists()
}

fn reset_trace_file() {
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(LOG_PATH)
    {
        let _ = writeln!(file, "{PLUGIN_NAME}: log start");
    }
}

fn trace(message: &str) {
    println!("{PLUGIN_NAME}: {message}");

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(LOG_PATH) {
        let _ = writeln!(file, "{PLUGIN_NAME}: {message}");
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut output = String::new();
    for (index, byte) in bytes.iter().enumerate() {
        if index != 0 {
            output.push(' ');
        }
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn display_version() -> String {
    let mut version = oe::DisplayVersion { name: [0; 16] };
    unsafe {
        oe::GetDisplayVersion(&mut version);
    }

    let end = version
        .name
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(version.name.len());

    std::str::from_utf8(&version.name[..end])
        .unwrap_or("<non-utf8>")
        .to_string()
}

unsafe fn layout_name_is(layout: *const Layout, expected: &str) -> bool {
    if (*layout).layout_name.is_null() {
        return false;
    }

    CStr::from_ptr((*layout).layout_name)
        .to_str()
        .map(|name| name == expected)
        .unwrap_or(false)
}

unsafe fn poll_p1_controller() -> Option<ControllerSnapshot> {
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

unsafe fn draw_overlay(root_pane: *mut Pane, snapshot: Option<ControllerSnapshot>) {
    if root_pane.is_null() {
        return;
    }

    let overlay_panes = find_overlay_text_panes(root_pane);
    if overlay_panes.count == 0 {
        if !MISSING_TEXT_PANE_LOGGED.swap(true, Ordering::Relaxed) {
            trace(&format!(
                "could not find a reusable text pane in {MATCH_HUD_LAYOUT}"
            ));
        }
        return;
    }

    if !OVERLAY_TEXT_PANES_LOGGED.swap(true, Ordering::Relaxed) {
        trace(&format!(
            "using {} text pane(s) for overlay",
            overlay_panes.count
        ));
    }

    let lines = format_overlay_lines(snapshot, overlay_panes.count);
    for (line_index, line) in lines.iter().enumerate().take(overlay_panes.count) {
        let textbox = (*overlay_panes.panes[line_index]).as_textbox();
        position_overlay_text(textbox, line_index);

        if let Ok(c_text) = CString::new(line.as_str()) {
            pane_set_text_string(textbox, c_text.as_ptr());
        }
    }
}

unsafe fn find_overlay_text_panes(root_pane: *mut Pane) -> OverlayTextPanes {
    let mut panes = OverlayTextPanes::new();

    if let Some(p1_root_pane) = find_p1_parts_root_pane(root_pane) {
        collect_overlay_text_panes(&mut panes, p1_root_pane, "p1 parts layout");
    }

    if panes.count < OVERLAY_LINE_COUNT {
        collect_overlay_text_panes(&mut panes, root_pane, "info_melee root");
    }

    panes
}

unsafe fn find_p1_parts_root_pane(root_pane: *mut Pane) -> Option<*mut Pane> {
    let p1_pane =
        find_pane_by_name_recursive(root_pane, P1_PARTS_PANE_NAME.as_ptr() as *const c_char);
    if p1_pane.is_null() {
        if !P1_PARTS_PANE_MISSING_LOGGED.swap(true, Ordering::Relaxed) {
            trace(&format!(
                "could not find p1 parts pane in {MATCH_HUD_LAYOUT}"
            ));
        }
        return None;
    }

    let p1_layout = (*p1_pane).as_parts().layout;
    if p1_layout.is_null() || (*p1_layout).root_pane.is_null() {
        if !P1_PARTS_LAYOUT_MISSING_LOGGED.swap(true, Ordering::Relaxed) {
            trace("p1 parts pane did not have a usable nested layout");
        }
        return None;
    }

    Some((*p1_layout).root_pane)
}

unsafe fn collect_overlay_text_panes(
    panes: &mut OverlayTextPanes,
    root_pane: *mut Pane,
    source: &str,
) {
    for name in OVERLAY_LINE_TEXT_PANES {
        if panes.count >= OVERLAY_LINE_COUNT {
            return;
        }

        let pane = find_pane_by_name_recursive(root_pane, name.as_ptr() as *const c_char);
        if pane.is_null() {
            continue;
        }

        if !OVERLAY_TEXT_PANES_LOGGED.load(Ordering::Relaxed) {
            trace(&format!(
                "found {} from {source} for overlay line {}",
                cstr_bytes_to_str(name),
                panes.count
            ));
        }
        panes.push(pane);
    }
}

fn cstr_bytes_to_str(bytes: &[u8]) -> &str {
    CStr::from_bytes_with_nul(bytes)
        .ok()
        .and_then(|name| name.to_str().ok())
        .unwrap_or("<invalid>")
}

unsafe fn position_overlay_text(textbox: &mut TextBox, line_index: usize) {
    textbox.pane.set_visible(true);
    textbox.pane.pos_x = OVERLAY_POS_X;
    textbox.pane.pos_y = OVERLAY_POS_Y - OVERLAY_LINE_HEIGHT * line_index as f32;
    textbox.pane.pos_z = 0.0;
    textbox.pane.size_x = OVERLAY_WIDTH;
    textbox.pane.size_y = OVERLAY_LINE_HEIGHT + 4.0;
    textbox.pane.alpha = 255;
    textbox.pane.global_alpha = 255;
    textbox.pane.flags |= 1 << PaneFlag::IsGlobalMatrixDirty as u8;

    textbox.font_size_x = OVERLAY_FONT_SIZE;
    textbox.font_size_y = OVERLAY_FONT_SIZE;
    textbox.line_space = 1.0;
    textbox.char_space = 0.0;
    textbox.set_text_alignment(HorizontalPosition::Left, VerticalPosition::Top);
    textbox.text_outline_enable(true);
    textbox.text_shadow_enable(true);
    textbox.set_color(255, 255, 255, 255);
    textbox.bits |= 1 << TextBoxFlag::IsPTDirty as u8;
}

fn format_overlay_lines(
    snapshot: Option<ControllerSnapshot>,
    pane_count: usize,
) -> [String; OVERLAY_LINE_COUNT] {
    let Some(snapshot) = snapshot else {
        return [
            String::from("P1 controller not ready"),
            String::from("BTN -"),
            String::from("LS -----,-----"),
            String::from("RS -----,-----"),
        ];
    };

    let trigger_text = match gc_trigger_text(snapshot.gc_triggers) {
        Some(text) => text,
        None => String::from("GC LT --- RT ---"),
    };

    if pane_count == 1 {
        return [
            format!(
                "P1 {} {} BTN {} LS {:+05},{:+05} RS {:+05},{:+05} {}",
                npad_id_name(snapshot.npad_id),
                style_name(snapshot.style_flags),
                button_names(snapshot.buttons),
                snapshot.left_stick.0,
                snapshot.left_stick.1,
                snapshot.right_stick.0,
                snapshot.right_stick.1,
                trigger_text,
            ),
            String::new(),
            String::new(),
            String::new(),
        ];
    }

    if pane_count == 2 {
        return [
            format!(
                "P1 {} {}  BTN {}",
                npad_id_name(snapshot.npad_id),
                style_name(snapshot.style_flags),
                button_names(snapshot.buttons)
            ),
            format!(
                "LS {:+05},{:+05}  RS {:+05},{:+05}  {}",
                snapshot.left_stick.0,
                snapshot.left_stick.1,
                snapshot.right_stick.0,
                snapshot.right_stick.1,
                trigger_text,
            ),
            String::new(),
            String::new(),
        ];
    }

    if pane_count == 3 {
        return [
            format!(
                "P1 {} {}",
                npad_id_name(snapshot.npad_id),
                style_name(snapshot.style_flags)
            ),
            format!("BTN {}", button_names(snapshot.buttons)),
            format!(
                "LS {:+05},{:+05}  RS {:+05},{:+05}  {}",
                snapshot.left_stick.0,
                snapshot.left_stick.1,
                snapshot.right_stick.0,
                snapshot.right_stick.1,
                trigger_text,
            ),
            String::new(),
        ];
    }

    [
        format!(
            "P1 {} {}",
            npad_id_name(snapshot.npad_id),
            style_name(snapshot.style_flags)
        ),
        format!("BTN {}", button_names(snapshot.buttons)),
        format!(
            "LS {:+05},{:+05}  RS {:+05},{:+05}",
            snapshot.left_stick.0,
            snapshot.left_stick.1,
            snapshot.right_stick.0,
            snapshot.right_stick.1
        ),
        trigger_text,
    ]
}

fn npad_id_name(npad_id: u32) -> &'static str {
    match npad_id {
        NPAD_ID_NO1 => "No1",
        NPAD_ID_HANDHELD => "Handheld",
        _ => "Npad",
    }
}

fn gc_trigger_text(triggers: Option<(u32, u32)>) -> Option<String> {
    triggers.map(|(left, right)| format!("GC LT {left:03} RT {right:03}"))
}

fn style_name(style_flags: u32) -> &'static str {
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

fn button_names(buttons: u64) -> String {
    let mut names = String::new();

    for (mask, name) in [
        (1 << 0, "A"),
        (1 << 1, "B"),
        (1 << 2, "X"),
        (1 << 3, "Y"),
        (1 << 4, "LS"),
        (1 << 5, "RS"),
        (1 << 6, "L"),
        (1 << 7, "R"),
        (1 << 8, "ZL"),
        (1 << 9, "ZR"),
        (1 << 10, "+"),
        (1 << 11, "-"),
        (1 << 12, "DL"),
        (1 << 13, "DU"),
        (1 << 14, "DR"),
        (1 << 15, "DD"),
        (1 << 24, "SL-L"),
        (1 << 25, "SR-L"),
        (1 << 26, "SL-R"),
        (1 << 27, "SR-R"),
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

#[skyline::main(name = "smash-gamepad-overlay")]
pub fn main() {
    reset_trace_file();
    trace("starting P1 input overlay");

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
