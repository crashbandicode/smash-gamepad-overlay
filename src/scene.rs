//! Read-only scene classification for overlay placement, never gameplay mode changes.

use skyline::hooks::{getRegionAddress, Region};
use skyline::nn::ro::LookupSymbol;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::logger::trace;
use sgpo_ui_cache_metadata::scene::is_waiting_practice;

static MODE_BASE: AtomicUsize = AtomicUsize::new(0);

extern "C" {
    #[link_name = "\u{1}_ZN3app9smashball16is_training_modeEv"]
    fn is_training_mode() -> bool;
}

pub(crate) fn init(version: &str) {
    if version != "13.0.5" {
        return;
    }
    // Same native predicate/data mapping verified by the Training Lobby adapter:
    // Smash main 21450C647B8C5940ED5B68218EA128FD3C5D597D. No hook is installed.
    const EXPECTED: [u8; 32] = [
        0xfd, 0x7b, 0xbf, 0xa9, 0xfd, 0x03, 0x00, 0x91, 0x08, 0xe5, 0x01, 0xf0, 0x08, 0x61, 0x13,
        0x91, 0x08, 0xfd, 0xdf, 0x08, 0x48, 0x01, 0x00, 0x36, 0xa8, 0xe2, 0x01, 0x90, 0x08, 0xc1,
        0x46, 0x39,
    ];
    unsafe {
        let base = getRegionAddress(Region::Text) as usize;
        let mut predicate = 0usize;
        LookupSymbol(
            &mut predicate,
            c"_ZN3app9smashball16is_training_modeEv".as_ptr().cast(),
        );
        if predicate == base + 0x1671650
            && std::slice::from_raw_parts(predicate as *const u8, EXPECTED.len()) == EXPECTED
        {
            MODE_BASE.store(base, Ordering::Release);
            trace("practice placement: waiting-room mode guard matched 13.0.5");
        } else {
            trace("practice placement: waiting-room mode guard rejected; native Training only");
        }
    }
}

pub(crate) fn practice_active() -> bool {
    if unsafe { is_training_mode() } {
        return true;
    }
    let base = MODE_BASE.load(Ordering::Acquire);
    if base == 0 {
        return false;
    }
    unsafe {
        is_waiting_practice(
            std::ptr::read_volatile((base + 0x53144d8) as *const u8) & 1 != 0,
            std::ptr::read_volatile((base + 0x52c51b0) as *const u8),
            std::ptr::read_volatile((base + 0x53050f0) as *const u32),
        )
    }
}
