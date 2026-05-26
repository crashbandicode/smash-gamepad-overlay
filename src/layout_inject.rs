use std::sync::atomic::{AtomicBool, Ordering};

use crate::logger::trace;
use crate::offsets::{layout_arc_malloc_hook_offset_for_install, OFFSET_LAYOUT_ARC_MALLOC};

include!(concat!(env!("OUT_DIR"), "/sgpo_layout_arc.rs"));

const INFO_MELEE_BFLYT_ENTRY: &[u8] = b"blyt/info_melee.bflyt";

static LAYOUT_ARC_HOOK_LOGGED: AtomicBool = AtomicBool::new(false);
static LAYOUT_ARC_INJECTED_LOGGED: AtomicBool = AtomicBool::new(false);

#[skyline::hook(offset = layout_arc_malloc_hook_offset_for_install(), inline)]
unsafe fn handle_layout_arc_malloc(ctx: &mut skyline::hooks::InlineCtx) {
    if !INJECTED_LAYOUT_ARC_AVAILABLE || INJECTED_LAYOUT_ARC.is_empty() {
        return;
    }

    let decompressed_file = ctx.registers[21].x() as *const u8;
    let decompressed_size = ctx.registers[1].x() as usize;
    if decompressed_file.is_null() || decompressed_size == 0 {
        return;
    }

    let decompressed_arc = std::slice::from_raw_parts(decompressed_file, decompressed_size);
    if !contains_bytes(decompressed_arc, INFO_MELEE_BFLYT_ENTRY) {
        return;
    }

    let injected_arc = INJECTED_LAYOUT_ARC.as_ptr();
    let injected_arc_size = INJECTED_LAYOUT_ARC.len() as u64;

    ctx.registers[21].set_x(injected_arc as u64);
    ctx.registers[1].set_x(injected_arc_size);
    ctx.registers[23].set_x(injected_arc_size);
    ctx.registers[24].set_x(injected_arc_size);

    if !LAYOUT_ARC_INJECTED_LOGGED.swap(true, Ordering::Relaxed) {
        trace(&format!(
            "injected modified info_melee layout.arc ({} bytes)",
            INJECTED_LAYOUT_ARC.len()
        ));
    }
}

pub(crate) fn install_layout_injection_hook() {
    if !INJECTED_LAYOUT_ARC_AVAILABLE {
        trace("modified info_melee layout.arc not embedded; skipping layout injection hook");
        return;
    }

    match *OFFSET_LAYOUT_ARC_MALLOC {
        Some(offset) => {
            if !LAYOUT_ARC_HOOK_LOGGED.swap(true, Ordering::Relaxed) {
                trace(&format!(
                    "installing layout injection hook at .text+0x{offset:x}"
                ));
            }
            skyline::install_hooks!(handle_layout_arc_malloc);
        }
        None => {
            trace("layout injection hook not installed because layout.arc offset was not found");
        }
    }
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if haystack.len() < needle.len() {
        return false;
    }

    haystack
        .windows(needle.len())
        .any(|candidate| candidate == needle)
}
