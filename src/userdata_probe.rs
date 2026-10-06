//! Diagnostic observer for `Pane::GetExtUserDataArray`.
//!
//! The hook reads integer fields from the live pane argument and does not
//! change registers or the return value. It does not follow `pane+0xa8` or
//! the parent pointer. Suspect is a heuristic, not a proof.

use sgpo_ui_cache_metadata::userdata_probe::{
    ext_userdata_is_suspect, format_probe_record, format_probe_startup, ProbeRecord,
    ProbeRecordKind, NAME_OCTETS, PROBE_LINE_CAP,
};
use skyline::hooks::{getRegionAddress, InlineCtx, Region};
use skyline::nn::os::GetCurrentThread;
use std::cell::UnsafeCell;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::{
    GET_EXT_USER_DATA_ARRAY_OFFSET, GET_EXT_USER_DATA_ARRAY_PROLOGUE, PROBE_LOG_PATH,
    PROBE_ONLY_FLAG_PATH,
};
use crate::logger::{hex_bytes, trace};
use crate::pane_lifetime::read_installed_prologue;

const SUPPORTED_DISPLAY_VERSION: &str = "13.0.5";

// Closed until startup has stored the file and written the header.
static SINK_GATE: AtomicBool = AtomicBool::new(true);
// Release-published only after that setup, so the callback's Acquire CAS
// observes the initialized file.
static SINK_PUBLISHED: AtomicBool = AtomicBool::new(false);
static SAMPLE_RECORDED: AtomicBool = AtomicBool::new(false);
static SUSPECT_RECORDED: AtomicBool = AtomicBool::new(false);

struct ProbeSink(UnsafeCell<Option<File>>);
// Startup stores the file before the hook is installed. Later writes take
// SINK_GATE and do not use a cache lock.
unsafe impl Sync for ProbeSink {}
static PROBE_SINK: ProbeSink = ProbeSink(UnsafeCell::new(None));

struct SinkGuard;
impl Drop for SinkGuard {
    fn drop(&mut self) {
        SINK_GATE.store(false, Ordering::Release);
    }
}

pub(crate) fn run_probe_only_if_marked(build_id: &str) -> bool {
    if !Path::new(PROBE_ONLY_FLAG_PATH).exists() {
        return false;
    }
    crate::logger::reset_trace_file();
    match arm(build_id) {
        ArmResult::Armed => trace(
            "probe-only: userdata observer only, no live gamepad display; pane finalize, HUD, draw, scene, match, and skin config hooks are not installed",
        ),
        ArmResult::Refused(reason) => {
            trace(reason);
            trace(
                "probe-only: userdata observer was not armed, and no live gamepad display is installed",
            );
        }
    }
    true
}

pub(crate) fn arm_with_overlay(build_id: &str, display_version: &str) {
    if let ArmResult::Refused(reason) = arm_with_version(build_id, display_version) {
        trace(reason);
    }
}

fn arm(build_id: &str) -> ArmResult {
    let display_version = crate::offsets::display_version();
    arm_with_version(build_id, &display_version)
}

fn arm_with_version(build_id: &str, display_version: &str) -> ArmResult {
    if display_version != SUPPORTED_DISPLAY_VERSION {
        return ArmResult::Refused(
            "userdata probe not armed: Smash display version is not 13.0.5",
        );
    }
    if let Err(reason) = guard_matches() {
        trace(reason);
        return ArmResult::Refused(
            "userdata probe not armed: GetExtUserDataArray byte guard failed",
        );
    }
    if !open_sink() {
        return ArmResult::Refused(
            "userdata probe not armed: could not open sd:/smash-gamepad-overlay-probe.log",
        );
    }
    write_startup(build_id, display_version);
    SINK_PUBLISHED.store(true, Ordering::Release);
    SINK_GATE.store(false, Ordering::Release);
    skyline::install_hooks!(observe_get_ext_user_data_array);
    trace(
        "userdata probe armed at .text+0x59890; suspect lines are a heuristic, not proof of a bad pointer",
    );
    ArmResult::Armed
}

enum ArmResult {
    Armed,
    Refused(&'static str),
}

fn guard_matches() -> Result<(), &'static str> {
    match read_installed_prologue(GET_EXT_USER_DATA_ARRAY_OFFSET) {
        Ok(actual) if actual == GET_EXT_USER_DATA_ARRAY_PROLOGUE => Ok(()),
        Ok(actual) => {
            trace(&format!(
                "userdata probe guard failed at .text+0x{GET_EXT_USER_DATA_ARRAY_OFFSET:x}; expected {}",
                hex_bytes(&GET_EXT_USER_DATA_ARRAY_PROLOGUE)
            ));
            trace(&format!(
                "userdata probe prologue actual {}",
                hex_bytes(&actual)
            ));
            Err("guard mismatch")
        }
        Err(reason) => {
            trace(&format!(
                "userdata probe guard failed at .text+0x{GET_EXT_USER_DATA_ARRAY_OFFSET:x}: {reason}"
            ));
            Err(reason)
        }
    }
}

fn open_sink() -> bool {
    match OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(PROBE_LOG_PATH)
    {
        Ok(file) => {
            unsafe {
                *PROBE_SINK.0.get() = Some(file);
            }
            true
        }
        Err(_) => false,
    }
}

fn write_startup(build_id: &str, display_version: &str) {
    let main_base = unsafe { getRegionAddress(Region::Text) as u64 };
    let mut buf = [0u8; PROBE_LINE_CAP];
    let len = format_probe_startup(&mut buf, build_id, display_version, main_base);
    write_buf(&buf[..len]);
}

fn write_buf(bytes: &[u8]) {
    let Some(file) = (unsafe { (*PROBE_SINK.0.get()).as_mut() }) else {
        return;
    };
    let _ = file.write_all(bytes);
    let _ = file.sync_all();
}

#[skyline::hook(offset = GET_EXT_USER_DATA_ARRAY_OFFSET, inline)]
unsafe fn observe_get_ext_user_data_array(ctx: &InlineCtx) {
    record_getter(ctx.registers[0].x(), ctx.registers[30].x());
}

fn record_getter(pane: u64, lr: u64) {
    if pane == 0
        || (SAMPLE_RECORDED.load(Ordering::Relaxed) && SUSPECT_RECORDED.load(Ordering::Relaxed))
    {
        return;
    }
    let Some(ext_user_data) = read_u64(pane, 0xa8) else {
        return;
    };
    let suspect = ext_userdata_is_suspect(ext_user_data);
    if SAMPLE_RECORDED.load(Ordering::Relaxed)
        && (!suspect || SUSPECT_RECORDED.load(Ordering::Relaxed))
    {
        return;
    }
    let Some(fields) = read_fields(pane, lr, ext_user_data) else {
        return;
    };
    if SINK_GATE
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }
    let _guard = SinkGuard;
    if !SINK_PUBLISHED.load(Ordering::Acquire) {
        return;
    }
    if !SAMPLE_RECORDED.swap(true, Ordering::Relaxed) {
        write_record(ProbeRecordKind::Sample, &fields);
    }
    if suspect && !SUSPECT_RECORDED.swap(true, Ordering::Relaxed) {
        write_record(ProbeRecordKind::Suspect, &fields);
    }
}

struct FieldSnapshot {
    pane: u64,
    lr_text: u64,
    vtable_text: u64,
    parent: u64,
    flags: u8,
    flag_ex: u8,
    flag_word: u32,
    ext_user_data: u64,
    name: [u8; NAME_OCTETS],
    thread: u64,
}

fn read_fields(pane: u64, lr: u64, ext_user_data: u64) -> Option<FieldSnapshot> {
    let text = unsafe { getRegionAddress(Region::Text) as u64 };
    let mut name = [0u8; NAME_OCTETS];
    let name_src = field_ptr(pane, 0xb0)?;
    unsafe {
        std::ptr::copy_nonoverlapping(name_src, name.as_mut_ptr(), NAME_OCTETS);
    }
    Some(FieldSnapshot {
        pane,
        lr_text: text_relative(lr, text),
        vtable_text: text_relative(read_u64(pane, 0)?, text),
        parent: read_u64(pane, 0x18)?,
        flags: read_u8(pane, 0x58)?,
        flag_ex: read_u8(pane, 0x5c)?,
        flag_word: read_u32(pane, 0x60)?,
        ext_user_data,
        name,
        thread: unsafe { GetCurrentThread() as u64 },
    })
}

fn write_record(kind: ProbeRecordKind, fields: &FieldSnapshot) {
    let record = ProbeRecord {
        kind,
        pane: fields.pane,
        lr_text: fields.lr_text,
        vtable_text: fields.vtable_text,
        parent: fields.parent,
        flags: fields.flags,
        flag_ex: fields.flag_ex,
        flag_word: fields.flag_word,
        ext_user_data: fields.ext_user_data,
        name: &fields.name,
        thread: fields.thread,
    };
    let mut buf = [0u8; PROBE_LINE_CAP];
    let len = format_probe_record(&mut buf, &record);
    write_buf(&buf[..len]);
}

fn text_relative(addr: u64, text: u64) -> u64 {
    if text != 0 && addr >= text {
        addr - text
    } else {
        addr
    }
}

fn field_ptr(base: u64, offset: usize) -> Option<*const u8> {
    (base as usize).checked_add(offset).map(|addr| addr as *const u8)
}

fn read_u8(base: u64, offset: usize) -> Option<u8> {
    let ptr = field_ptr(base, offset)?;
    Some(unsafe { std::ptr::read_unaligned(ptr) })
}

fn read_u32(base: u64, offset: usize) -> Option<u32> {
    let ptr = field_ptr(base, offset)?;
    Some(unsafe { std::ptr::read_unaligned(ptr as *const u32) })
}

fn read_u64(base: u64, offset: usize) -> Option<u64> {
    let ptr = field_ptr(base, offset)?;
    Some(unsafe { std::ptr::read_unaligned(ptr as *const u64) })
}
