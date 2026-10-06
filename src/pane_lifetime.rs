//! Shared Pane::Finalize observer for both UI pane caches.
//!
//! The inline hook reads `x0` as the address being finalized and does not
//! change registers. Eviction compares integers and does not load the pane.
//! Later scene-update and draw code may still write panes that have not been
//! finalized. Only the eviction path is free of pane reads and writes.
//!
//! The finalizer itself does not allocate, format text, or write the log.
//! Counters and a short ring are flushed from capture, scene update, draw,
//! and match reset after those paths have released their cache locks.

use sgpo_ui_cache_metadata::CoverageWatch;
use skyline::hooks::{getRegionAddress, InlineCtx, Region};
use skyline::nn::os::GetCurrentThread;
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::config::{
    LAYOUT_FINALIZE_OFFSET, LAYOUT_FINALIZE_PROLOGUE, PANE_FINALIZE_OFFSET, PANE_FINALIZE_PROLOGUE,
};
use crate::hud::finalize_retired_pane as finalize_hud_pane;
use crate::logger::{hex_bytes, trace};
use crate::visual::finalize_retired_pane as finalize_visual_pane;

const SUPPORTED_DISPLAY_VERSION: &str = "13.0.5";
const PROLOGUE_LEN: usize = 64;
const RING_CAP: usize = 4;
const LOCK_SPINS: u32 = 96;

static HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);
static CACHE_ACCESS: AtomicBool = AtomicBool::new(false);
static PANE_GUARD_OK: AtomicBool = AtomicBool::new(false);
static LAYOUT_GUARD_OK: AtomicBool = AtomicBool::new(false);
static RETIRE_EPOCH: AtomicU64 = AtomicU64::new(0);
static CAPTURE_THREAD: AtomicU64 = AtomicU64::new(0);
static UPDATE_THREAD: AtomicU64 = AtomicU64::new(0);
static RENDER_THREAD: AtomicU64 = AtomicU64::new(0);
static LAST_FINALIZER_THREAD: AtomicU64 = AtomicU64::new(0);
static THREAD_SWITCHES: AtomicU64 = AtomicU64::new(0);
static THREAD_SWITCHES_INTERVAL: AtomicU64 = AtomicU64::new(0);
static LAST_LAYOUT_ADDRESS: AtomicU64 = AtomicU64::new(0);
static PANE_FINALIZE_ENTRIES: AtomicU64 = AtomicU64::new(0);
static PANE_FINALIZE_INTERVAL: AtomicU64 = AtomicU64::new(0);
static LAYOUT_FINALIZE_ENTRIES: AtomicU64 = AtomicU64::new(0);
static LAYOUT_FINALIZE_INTERVAL: AtomicU64 = AtomicU64::new(0);
static MATCHING_EVICTIONS: AtomicU64 = AtomicU64::new(0);
static MATCHING_EVICTIONS_INTERVAL: AtomicU64 = AtomicU64::new(0);
static INACTIVE_LOGGED: AtomicBool = AtomicBool::new(false);
static CAPTURE_LOGGED: AtomicBool = AtomicBool::new(false);
static UPDATE_LOGGED: AtomicBool = AtomicBool::new(false);
static RENDER_LOGGED: AtomicBool = AtomicBool::new(false);
static CAPTURE_SWITCH_LOGGED: AtomicBool = AtomicBool::new(false);
static UPDATE_SWITCH_LOGGED: AtomicBool = AtomicBool::new(false);
static RENDER_SWITCH_LOGGED: AtomicBool = AtomicBool::new(false);
static DIAG_LOCK: AtomicBool = AtomicBool::new(false);

struct DiagState {
    watch: CoverageWatch,
    ring: [RetireNote; RING_CAP],
    ring_len: u8,
    ring_flushed: u8,
    overflow_logged: bool,
}

struct DiagCell(UnsafeCell<DiagState>);

unsafe impl Sync for DiagCell {}

#[derive(Clone, Copy)]
struct RetireNote {
    address: u64,
    hud_slots: u8,
    visual_slots: u8,
    thread: u64,
}

static DIAG: DiagCell = DiagCell(UnsafeCell::new(DiagState {
    watch: CoverageWatch::new(),
    ring: [RetireNote {
        address: 0,
        hud_slots: 0,
        visual_slots: 0,
        thread: 0,
    }; RING_CAP],
    ring_len: 0,
    ring_flushed: 0,
    overflow_logged: false,
}));

struct DiagGuard;

impl DiagGuard {
    fn acquire() -> Self {
        acquire_flag(&DIAG_LOCK);
        Self
    }
}

impl Drop for DiagGuard {
    fn drop(&mut self) {
        DIAG_LOCK.store(false, Ordering::Release);
    }
}

#[derive(Clone, Copy)]
pub(crate) enum RoutineKind {
    Capture,
    Update,
    Render,
}

pub(crate) fn cache_access_enabled() -> bool {
    HOOK_INSTALLED.load(Ordering::Acquire) && CACHE_ACCESS.load(Ordering::Acquire)
}

pub(crate) fn retire_epoch() -> u64 {
    RETIRE_EPOCH.load(Ordering::Acquire)
}

/// Bounded spin, then a positive 1us sleep so a lower-priority lock holder can run.
pub(crate) fn acquire_flag(flag: &AtomicBool) {
    let mut spins = 0u32;
    while flag
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        if spins < LOCK_SPINS {
            spins += 1;
            std::hint::spin_loop();
        } else {
            pause_for_lock_holder();
        }
    }
}

fn pause_for_lock_holder() {
    unsafe {
        skyline::nn::os::SleepThread(skyline::nn::TimeSpan::nano(1_000));
    }
}

pub(crate) fn install_pane_finalize_observer(display_version: &str) -> bool {
    if display_version != SUPPORTED_DISPLAY_VERSION {
        trace(&format!(
            "pane finalize observer not installed at .text+0x{PANE_FINALIZE_OFFSET:x}: display version {display_version} is not {SUPPORTED_DISPLAY_VERSION}"
        ));
        return false;
    }

    if !guard_prologue(
        PANE_FINALIZE_OFFSET,
        &PANE_FINALIZE_PROLOGUE,
        "pane finalize",
    ) {
        return false;
    }

    let layout_guard = guard_prologue(
        LAYOUT_FINALIZE_OFFSET,
        &LAYOUT_FINALIZE_PROLOGUE,
        "layout finalize",
    );
    LAYOUT_GUARD_OK.store(layout_guard, Ordering::Release);
    if layout_guard {
        trace(&format!(
            "layout finalize diagnostic observer guard matched 16 instructions at .text+0x{LAYOUT_FINALIZE_OFFSET:x}; counts only, no cache invalidation"
        ));
        skyline::install_hooks!(observe_pane_finalize, observe_layout_finalize);
    } else {
        trace(
            "layout finalize diagnostic observer not installed; pane cache still uses the pane finalize guard",
        );
        skyline::install_hooks!(observe_pane_finalize);
    }

    PANE_GUARD_OK.store(true, Ordering::Release);
    HOOK_INSTALLED.store(true, Ordering::Release);
    CACHE_ACCESS.store(true, Ordering::Release);
    true
}

/// Fail closed when the finalize guard is inactive. Thread identity is recorded
/// and is not an access gate.
pub(crate) fn begin_routine(kind: RoutineKind) -> bool {
    if !cache_access_enabled() {
        log_inactive_once();
        return false;
    }
    note_routine_thread(kind);
    true
}

/// Pane finalizer entry. No logging and no allocation on this path.
///
/// The epoch increments before either cache lock so a capture that is still
/// resolving can see the new epoch and refuse to publish.
pub(crate) fn on_pane_finalizing(retiring: u64) {
    RETIRE_EPOCH.fetch_add(1, Ordering::Release);
    let thread = current_thread_token();
    PANE_FINALIZE_ENTRIES.fetch_add(1, Ordering::Relaxed);
    PANE_FINALIZE_INTERVAL.fetch_add(1, Ordering::Relaxed);
    note_thread_change(&LAST_FINALIZER_THREAD, thread);

    let hud = finalize_hud_pane(retiring);
    let visual = finalize_visual_pane(retiring);
    note_after_caches(retiring, hud.cleared, visual.cleared, thread);
}

/// Match start/end drops live cache metadata elsewhere. This log does not
/// clear the address watchlist, so captured identities stay until Finalize.
pub(crate) fn note_match_reset(reason: &'static str) {
    flush_diagnostics(Some(reason));
}

pub(crate) fn flush_diagnostics(reset_reason: Option<&'static str>) {
    let force = reset_reason.is_some();
    let pane_interval = if force {
        PANE_FINALIZE_INTERVAL.swap(0, Ordering::Relaxed)
    } else {
        PANE_FINALIZE_INTERVAL.load(Ordering::Relaxed)
    };
    let layout_interval = if force {
        LAYOUT_FINALIZE_INTERVAL.swap(0, Ordering::Relaxed)
    } else {
        LAYOUT_FINALIZE_INTERVAL.load(Ordering::Relaxed)
    };
    let eviction_interval = if force {
        MATCHING_EVICTIONS_INTERVAL.swap(0, Ordering::Relaxed)
    } else {
        MATCHING_EVICTIONS_INTERVAL.load(Ordering::Relaxed)
    };
    let switch_interval = if force {
        THREAD_SWITCHES_INTERVAL.swap(0, Ordering::Relaxed)
    } else {
        THREAD_SWITCHES_INTERVAL.load(Ordering::Relaxed)
    };

    let copied = {
        let _guard = DiagGuard::acquire();
        let diag = unsafe { &mut *DIAG.0.get() };
        let new_ring = diag.ring_len > diag.ring_flushed;
        let log_overflow = diag.watch.summary().overflow && !diag.overflow_logged;
        if !force && !new_ring && !log_overflow {
            return;
        }
        if log_overflow {
            diag.overflow_logged = true;
        }
        let summary = diag.watch.summary();
        let mut notes = [RetireNote {
            address: 0,
            hud_slots: 0,
            visual_slots: 0,
            thread: 0,
        }; RING_CAP];
        let pending = usize::from(diag.ring_len.saturating_sub(diag.ring_flushed));
        let start = usize::from(diag.ring_flushed);
        for (index, note) in notes.iter_mut().enumerate().take(pending) {
            *note = diag.ring[start + index];
        }
        diag.ring_flushed = diag.ring_len;
        CopiedDiag {
            summary,
            notes,
            pending,
            log_overflow,
        }
    };
    if let Some(reason) = reset_reason {
        trace(&format!(
            "ui cache reset reason={reason} pane_finalize_interval={pane_interval} layout_finalize_interval={layout_interval} matching_evictions_interval={eviction_interval} thread_switches_interval={switch_interval} {}",
            snapshot_fields()
        ));
    } else {
        trace(&format!(
            "ui cache diagnostic pane_finalize_interval={pane_interval} layout_finalize_interval={layout_interval} matching_evictions_interval={eviction_interval} thread_switches_interval={switch_interval} {}",
            snapshot_fields()
        ));
    }

    if copied.log_overflow {
        trace(
            "ui cache coverage watch overflowed; do not treat captured-pane finalization as complete",
        );
    }
    for note in copied.notes.iter().take(copied.pending) {
        trace(&format!(
            "pane finalize retired cached ui addr=0x{:x} hud_slots={} visual_slots={} thread=0x{:x}",
            note.address, note.hud_slots, note.visual_slots, note.thread
        ));
    }
    if force || copied.log_overflow {
        trace_watch(&copied.summary);
    }
}

pub(crate) fn remember_captured(root: u64, layout_root: u64, children: &[u64], owner_token: u64) {
    let _guard = DiagGuard::acquire();
    let watch = unsafe { &mut (*DIAG.0.get()).watch };
    let _ = watch.note_capture(root, owner_token);
    let _ = watch.note_capture(layout_root, owner_token);
    for child in children {
        let _ = watch.note_capture(*child, owner_token);
    }
}

struct CopiedDiag {
    summary: sgpo_ui_cache_metadata::WatchSummary,
    notes: [RetireNote; RING_CAP],
    pending: usize,
    log_overflow: bool,
}

#[skyline::hook(offset = PANE_FINALIZE_OFFSET, inline)]
unsafe fn observe_pane_finalize(ctx: &InlineCtx) {
    on_pane_finalizing(ctx.registers[0].x());
}

#[skyline::hook(offset = LAYOUT_FINALIZE_OFFSET, inline)]
unsafe fn observe_layout_finalize(ctx: &InlineCtx) {
    LAYOUT_FINALIZE_ENTRIES.fetch_add(1, Ordering::Relaxed);
    LAYOUT_FINALIZE_INTERVAL.fetch_add(1, Ordering::Relaxed);
    LAST_LAYOUT_ADDRESS.store(ctx.registers[0].x(), Ordering::Relaxed);
}

fn note_after_caches(retiring: u64, hud_slots: usize, visual_slots: usize, thread: u64) {
    if hud_slots + visual_slots > 0 {
        let slots = (hud_slots + visual_slots) as u64;
        MATCHING_EVICTIONS.fetch_add(slots, Ordering::Relaxed);
        MATCHING_EVICTIONS_INTERVAL.fetch_add(slots, Ordering::Relaxed);
    }

    let _guard = DiagGuard::acquire();
    let diag = unsafe { &mut *DIAG.0.get() };
    let _ = diag.watch.note_finalize(retiring);
    if hud_slots + visual_slots > 0 && usize::from(diag.ring_len) < RING_CAP {
        diag.ring[usize::from(diag.ring_len)] = RetireNote {
            address: retiring,
            hud_slots: u8::try_from(hud_slots).unwrap_or(u8::MAX),
            visual_slots: u8::try_from(visual_slots).unwrap_or(u8::MAX),
            thread,
        };
        diag.ring_len = diag.ring_len.saturating_add(1);
    }
}

fn snapshot_fields() -> String {
    format!(
        "pane_finalize_total={} layout_finalize_total={} matching_evictions_total={} thread_switches={} capture_thread=0x{:x} update_thread=0x{:x} render_thread=0x{:x} last_finalizer=0x{:x} last_layout=0x{:x} pane_guard={} layout_guard={} retire_epoch={}",
        PANE_FINALIZE_ENTRIES.load(Ordering::Relaxed),
        LAYOUT_FINALIZE_ENTRIES.load(Ordering::Relaxed),
        MATCHING_EVICTIONS.load(Ordering::Relaxed),
        THREAD_SWITCHES.load(Ordering::Relaxed),
        CAPTURE_THREAD.load(Ordering::Relaxed),
        UPDATE_THREAD.load(Ordering::Relaxed),
        RENDER_THREAD.load(Ordering::Relaxed),
        LAST_FINALIZER_THREAD.load(Ordering::Relaxed),
        LAST_LAYOUT_ADDRESS.load(Ordering::Relaxed),
        PANE_GUARD_OK.load(Ordering::Relaxed),
        LAYOUT_GUARD_OK.load(Ordering::Relaxed),
        RETIRE_EPOCH.load(Ordering::Relaxed)
    )
}

fn trace_watch(summary: &sgpo_ui_cache_metadata::WatchSummary) {
    let balanced = !summary.overflow && summary.live == 0;
    trace(&format!(
        "ui cache coverage stored={} retired={} live={} alias_observations={} overflow={} watch_balanced={}",
        summary.stored,
        summary.retired_matches,
        summary.live,
        summary.alias_observations,
        summary.overflow,
        balanced
    ));
    if summary.live_sample_len > 0 {
        let mut sample = String::new();
        for address in summary.live_sample.iter().take(summary.live_sample_len) {
            sample.push_str(&format!("0x{address:x} "));
        }
        trace(&format!(
            "ui cache coverage live addresses not yet finalized: {sample}"
        ));
    }
}

fn note_routine_thread(kind: RoutineKind) {
    let thread = current_thread_token();
    if thread == 0 {
        return;
    }
    let (slot, first_logged, switch_logged) = match kind {
        RoutineKind::Capture => (&CAPTURE_THREAD, &CAPTURE_LOGGED, &CAPTURE_SWITCH_LOGGED),
        RoutineKind::Update => (&UPDATE_THREAD, &UPDATE_LOGGED, &UPDATE_SWITCH_LOGGED),
        RoutineKind::Render => (&RENDER_THREAD, &RENDER_LOGGED, &RENDER_SWITCH_LOGGED),
    };
    let previous = slot.swap(thread, Ordering::Relaxed);
    if previous == 0 {
        if !first_logged.swap(true, Ordering::Relaxed) {
            trace(&format!(
                "ui cache {} thread=0x{thread:x}",
                kind_name(kind)
            ));
        }
        return;
    }
    if previous != thread {
        note_switch_count();
        if !switch_logged.swap(true, Ordering::Relaxed) {
            trace(&format!(
                "ui cache {} thread changed 0x{previous:x} -> 0x{thread:x}; cache access continues",
                kind_name(kind)
            ));
        }
    }
}

fn note_thread_change(slot: &AtomicU64, thread: u64) {
    let previous = slot.swap(thread, Ordering::Relaxed);
    if previous != 0 && previous != thread {
        note_switch_count();
    }
}

fn note_switch_count() {
    THREAD_SWITCHES.fetch_add(1, Ordering::Relaxed);
    THREAD_SWITCHES_INTERVAL.fetch_add(1, Ordering::Relaxed);
}

fn log_inactive_once() {
    if !INACTIVE_LOGGED.swap(true, Ordering::Relaxed) {
        trace("ui pane cache inactive; cached visual panes will not be used");
    }
}

fn kind_name(kind: RoutineKind) -> &'static str {
    match kind {
        RoutineKind::Capture => "capture",
        RoutineKind::Update => "update",
        RoutineKind::Render => "render",
    }
}

fn current_thread_token() -> u64 {
    unsafe { GetCurrentThread() as u64 }
}

fn guard_prologue(offset: usize, expected: &[u8; PROLOGUE_LEN], label: &str) -> bool {
    match read_installed_prologue(offset) {
        Ok(actual) if actual == *expected => {
            trace(&format!(
                "{label} observer guard matched 16 instructions at .text+0x{offset:x}"
            ));
            true
        }
        Ok(actual) => {
            trace(&format!(
                "{label} observer guard failed at .text+0x{offset:x}; expected {}",
                hex_bytes(expected)
            ));
            trace(&format!(
                "{label} observer prologue actual {}",
                hex_bytes(&actual)
            ));
            false
        }
        Err(reason) => {
            trace(&format!(
                "{label} observer guard failed at .text+0x{offset:x}: {reason}"
            ));
            false
        }
    }
}

pub(crate) fn read_installed_prologue(offset: usize) -> Result<[u8; PROLOGUE_LEN], &'static str> {
    let text = unsafe { getRegionAddress(Region::Text) } as usize;
    let rodata = unsafe { getRegionAddress(Region::Rodata) } as usize;
    if text == 0 || rodata == 0 {
        return Err("module region address is null");
    }
    let text_len = rodata
        .checked_sub(text)
        .ok_or("text region end is below its start")?;
    let end = offset
        .checked_add(PROLOGUE_LEN)
        .ok_or("prologue range overflows")?;
    if end > text_len {
        return Err("text region does not contain the prologue");
    }
    let src = text
        .checked_add(offset)
        .ok_or("prologue address overflows")?;
    let mut actual = [0u8; PROLOGUE_LEN];
    unsafe {
        std::ptr::copy_nonoverlapping(src as *const u8, actual.as_mut_ptr(), PROLOGUE_LEN);
    }
    Ok(actual)
}
