//! The patched `info_melee` layout with no `layout.arc` on the SD card (prototype; `docs/NRO-ONLY-DESIGN.md`).
//!
//! The plugin registers an ARCropolis file callback for `ui/layout/info/info_melee/info_melee/layout.arc`. When the
//! game loads that file, the callback asks ARCropolis for the next layer (vanilla, another mod's file, or another
//! plugin's callback), adds SGPO's panes to it in place (`sgpo_layout_merge`) and hands it back. Another mod's
//! changes stay. Any failure returns `false`, and ARCropolis gives the game the archive unchanged: the overlay then
//! finds no panes and draws nothing.
//!
//! The callback runs inside ARCropolis's file load: it allocates nothing, takes no lock and writes no file. It
//! publishes its outcome to atomics, and the scene update writes the log line.
//!
//! When the build embeds the textured RetroSpy skins (`SGPO_SKIN_BLOB`, see `build.rs`), the merge also adds their
//! panes, materials and textures (`merge_with`), using a work space allocated once at boot.

use std::sync::atomic::{AtomicPtr, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::OnceLock;

use sgpo_layout_merge::{
    merge_in_place, merge_with, Error, Scratch, SkinBlob, LAYOUT_HASH40, LAYOUT_PATH, MAX_GROWTH, VANILLA_LEN_13_0_5,
};
use skyline::nn::ro::LookupSymbol;

use crate::logger::trace;

type FileCallback = extern "C" fn(u64, *mut u8, usize, &mut usize) -> bool;
type RegisterCallback = extern "C" fn(u64, usize, FileCallback);
type LoadFile = extern "C" fn(u64, *mut u8, usize, &mut usize) -> bool;

const MODS_ROOT: &str = "sd:/ultimate/mods";
/// Codes beyond `sgpo_layout_merge::Error::code`.
const CODE_OK: u8 = 0;
const CODE_BASE_NOT_LOADED: u8 = 100;

static LOAD_FILE: AtomicUsize = AtomicUsize::new(0);
/// The last load: sequence (16 bits) | code (8) | patched (4) | already (4) | merged length (32).
static OUTCOME: AtomicU64 = AtomicU64::new(0);
/// The last load's merge time in microseconds and the base length, written before `OUTCOME`.
static MERGE_US: AtomicU32 = AtomicU32::new(0);
static BASE_LEN: AtomicU32 = AtomicU32::new(0);
static LOGGED_SEQ: AtomicU32 = AtomicU32::new(0);
/// The last load's skins, written before `OUTCOME`: textures (16 bits) | skin error (8) | layouts with skins (8).
static SKIN_OUTCOME: AtomicU32 = AtomicU32::new(0);

static SKIN_BLOB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/sgpo-skins.bin"));
static SKINS: OnceLock<Option<SkinBlob<'static>>> = OnceLock::new();
/// The skins' work space; the callback takes it for the merge and puts it back.
static SCRATCH: AtomicPtr<Scratch> = AtomicPtr::new(core::ptr::null_mut());

fn lookup(name: &core::ffi::CStr) -> usize {
    let mut address = 0usize;
    // SAFETY: a plain symbol lookup with a NUL-terminated name.
    unsafe { LookupSymbol(&mut address, name.as_ptr().cast()) };
    address
}

/// Boot: register the callback, sized for the largest base it may be handed.
pub(crate) fn install(version: &str) -> bool {
    if version != "13.0.5" {
        trace("layout merge: not installed: the game is not 13.0.5");
        return false;
    }
    let (register, load) = (lookup(c"arcrop_register_callback"), lookup(c"arcrop_load_file"));
    if register == 0 || load == 0 {
        trace("layout merge: not installed: ARCropolis's callback API was not found (ARCropolis is required)");
        return false;
    }
    let skins = *SKINS.get_or_init(|| SkinBlob::parse(SKIN_BLOB));
    match skins {
        Some(s) => {
            SCRATCH.store(Box::into_raw(Box::<Scratch>::default()), Ordering::Release);
            trace(&format!(
                "layout merge: textured skins embedded: {} textures, {} B of pixels, blob {} B",
                s.count,
                s.data_len,
                SKIN_BLOB.len()
            ));
        }
        None if SKIN_BLOB.is_empty() => trace("layout merge: no textured skins embedded (default skins only)"),
        None => trace("layout merge: the embedded skin blob is invalid; default skins only"),
    }
    let base = largest_mod_layout().max(VANILLA_LEN_13_0_5);
    let max_size = base + MAX_GROWTH + skins.map_or(0, |s| s.max_growth());
    LOAD_FILE.store(load, Ordering::Release);
    // SAFETY: `register` is ARCropolis's exported `arcrop_register_callback(Hash40, usize, CallbackFn)`.
    let register: RegisterCallback = unsafe { core::mem::transmute::<usize, RegisterCallback>(register) };
    register(LAYOUT_HASH40, max_size, on_layout_load);
    trace(&format!(
        "layout merge: ARCropolis callback registered for {LAYOUT_PATH} hash={LAYOUT_HASH40:#x} max_size={max_size} (largest base {base})"
    ));
    true
}

/// The largest `info_melee` layout another mod folder holds: the callback's buffer must fit it plus SGPO's panes.
fn largest_mod_layout() -> usize {
    let Ok(dirs) = std::fs::read_dir(MODS_ROOT) else { return 0 };
    dirs.flatten()
        .filter_map(|dir| std::fs::metadata(dir.path().join(LAYOUT_PATH)).ok())
        .map(|meta| meta.len() as usize)
        .max()
        .unwrap_or(0)
}

fn ticks() -> u64 {
    // SAFETY: reads the system tick counter.
    unsafe { skyline::nn::os::GetSystemTick() }
}

fn publish(code: u8, patched: u8, already: u8, len: usize, base: usize, started: u64) {
    let us = ticks().saturating_sub(started) / 19; // 19.2 MHz
    MERGE_US.store(us.min(u32::MAX as u64) as u32, Ordering::Relaxed);
    BASE_LEN.store(base.min(u32::MAX as usize) as u32, Ordering::Relaxed);
    let seq = ((OUTCOME.load(Ordering::Relaxed) >> 48) + 1) & 0xFFFF;
    let word = (seq << 48)
        | (u64::from(code) << 40)
        | (u64::from(patched & 0xF) << 36)
        | (u64::from(already & 0xF) << 32)
        | (len.min(u32::MAX as usize) as u64);
    OUTCOME.store(word, Ordering::Release);
}

extern "C" fn on_layout_load(hash: u64, out: *mut u8, capacity: usize, out_size: &mut usize) -> bool {
    let started = ticks();
    let load = LOAD_FILE.load(Ordering::Acquire);
    let mut got = 0usize;
    // SAFETY: `load` is ARCropolis's exported `arcrop_load_file`, meant to be called from inside a file callback,
    // where it serves the next layer of this file into `out` (`capacity` bytes, owned by ARCropolis).
    let loaded = load != 0
        && !out.is_null()
        && unsafe { core::mem::transmute::<usize, LoadFile>(load)(hash, out, capacity, &mut got) }
        && got <= capacity;
    if !loaded {
        publish(CODE_BASE_NOT_LOADED, 0, 0, 0, got, started);
        return false;
    }
    // SAFETY: ARCropolis's buffer of `capacity` bytes, the first `got` of them the base archive.
    let buffer = unsafe { core::slice::from_raw_parts_mut(out, capacity) };
    let scratch = SCRATCH.swap(core::ptr::null_mut(), Ordering::AcqRel);
    // SAFETY: the pointer came from `Box::into_raw` at boot, and the swap gives this call sole use of it.
    let result = match (SKINS.get().copied().flatten(), unsafe { scratch.as_mut() }) {
        (Some(skins), Some(work)) => merge_with(buffer, got, &skins, work),
        _ => merge_in_place(buffer, got),
    };
    if !scratch.is_null() {
        SCRATCH.store(scratch, Ordering::Release);
    }
    match result {
        Ok(merged) => {
            *out_size = merged.len;
            let skins = (u32::from(merged.textures) << 16) | (u32::from(merged.skin_error) << 8) | u32::from(merged.skins);
            SKIN_OUTCOME.store(skins, Ordering::Relaxed);
            publish(CODE_OK, merged.patched, merged.already, merged.len, got, started);
            true
        }
        Err(error) => {
            SKIN_OUTCOME.store(0, Ordering::Relaxed);
            publish(error.code(), 0, 0, 0, got, started);
            false
        }
    }
}

/// From the scene update (not the file load): one line per new outcome.
pub(crate) fn log_new_outcome() {
    let word = OUTCOME.load(Ordering::Acquire);
    let seq = (word >> 48) as u32;
    if seq == 0 || LOGGED_SEQ.swap(seq, Ordering::Relaxed) == seq {
        return;
    }
    let code = ((word >> 40) & 0xFF) as u8;
    let (patched, already, len) = ((word >> 36) & 0xF, (word >> 32) & 0xF, word & 0xFFFF_FFFF);
    let (us, base) = (MERGE_US.load(Ordering::Relaxed), BASE_LEN.load(Ordering::Relaxed));
    let result = match code {
        CODE_OK => "merged",
        CODE_BASE_NOT_LOADED => "kept: ARCropolis did not serve the base archive",
        code => Error::describe(code),
    };
    let skin_word = SKIN_OUTCOME.load(Ordering::Relaxed);
    let (textures, skin_error, skins) = (skin_word >> 16, ((skin_word >> 8) & 0xFF) as u8, skin_word & 0xFF);
    let skin_note = if skin_error == 0 { "" } else { Error::describe(skin_error) };
    trace(&format!(
        "layout merge: load={seq} result={result} code={code} patched={patched} already={already} skins={skins} textures={textures} skin_error={skin_error} {skin_note} base={base} len={len} merge_us={us}"
    ));
}
