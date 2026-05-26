use skyline::hooks::{getRegionAddress, Region};
use skyline::nn::oe;
use std::path::Path;
use std::sync::LazyLock;

use crate::config::{
    BEST_MATCHES_TO_LOG, LEGACY_DRAW_OFFSET, TEXT_SCAN_ALIGNMENT, TRAINING_MODPACK_PLUGIN_PATH,
};
use crate::logger::{hex_bytes, trace};

pub(crate) static OFFSET_DRAW: LazyLock<Option<usize>> = LazyLock::new(resolve_draw_offset);

// First instructions of nn::ui2d::Layout::Draw in Smash 13.0.1.
static NEEDLE_DRAW: &[u8] = &[
    0x08, 0x0c, 0x40, 0xf9, 0xc8, 0x03, 0x00, 0xb4, 0xff, 0x83, 0x01, 0xd1, 0xf5, 0x1b, 0x00, 0xf9,
    0xf4, 0x4f, 0x04, 0xa9, 0xfd, 0x7b, 0x05, 0xa9, 0xfd, 0x43, 0x01, 0x91, 0xf4, 0x03, 0x00, 0xaa,
];

pub(crate) fn draw_hook_offset_for_install() -> usize {
    (*OFFSET_DRAW).expect("Layout::Draw offset checked before installing draw hook")
}

fn resolve_draw_offset() -> Option<usize> {
    trace(&format!("Smash display version {}", display_version()));

    if training_modpack_plugin_present() {
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
