use skyline::nn::ui2d::{
    HorizontalPosition, Pane, PaneFlag, TextBox, TextBoxFlag, VerticalPosition,
};
use std::cell::UnsafeCell;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::{
    DEBUG_TEXT_FONT_SIZE, DEBUG_TEXT_LINE_COUNT, DEBUG_TEXT_LINE_HEIGHT, DEBUG_TEXT_POS_X,
    DEBUG_TEXT_POS_Y, DEBUG_TEXT_WIDTH, MATCH_HUD_LAYOUT, MAX_OVERLAY_TEXT_PANES, OVERLAY_CONFIG,
    OVERLAY_LINE_TEXT_PANES, P1_PARTS_PANE_NAME,
};
use crate::input::{button_names, gc_trigger_text, npad_id_name, style_name, ControllerSnapshot};
use crate::logger::trace;
use crate::pane_utils::{cstr_bytes_to_str, pane_name_matches};
use crate::ui::{find_pane_by_name, set_textbox_text};

const EMPTY_TEXT_PANE_NAME: &[u8] = b"\0";

#[derive(Debug, Copy, Clone)]
struct TextPaneSlots {
    panes: [*mut Pane; MAX_OVERLAY_TEXT_PANES],
    names: [&'static [u8]; MAX_OVERLAY_TEXT_PANES],
    count: usize,
}

#[derive(Debug, Copy, Clone)]
enum DebugTextCache {
    Empty,
    Resolved {
        root_pane: *mut Pane,
        slots: TextPaneSlots,
    },
    Missing {
        root_pane: *mut Pane,
    },
}

struct DebugTextRuntime {
    cache: DebugTextCache,
}

struct DebugTextRuntimeCell(UnsafeCell<DebugTextRuntime>);

unsafe impl Sync for DebugTextRuntimeCell {}

impl TextPaneSlots {
    fn new() -> Self {
        Self {
            panes: [ptr::null_mut(); MAX_OVERLAY_TEXT_PANES],
            names: [EMPTY_TEXT_PANE_NAME; MAX_OVERLAY_TEXT_PANES],
            count: 0,
        }
    }

    fn push(&mut self, pane: *mut Pane, name: &'static [u8]) {
        if pane.is_null() || self.count >= MAX_OVERLAY_TEXT_PANES || self.contains(pane) {
            return;
        }

        self.panes[self.count] = pane;
        self.names[self.count] = name;
        self.count += 1;
    }

    fn contains(&self, pane: *mut Pane) -> bool {
        self.panes[..self.count].iter().any(|found| *found == pane)
    }

    fn drawable_count(&self) -> usize {
        self.count.min(DEBUG_TEXT_LINE_COUNT)
    }
}

static MISSING_TEXT_PANE_LOGGED: AtomicBool = AtomicBool::new(false);
static P1_PARTS_PANE_MISSING_LOGGED: AtomicBool = AtomicBool::new(false);
static P1_PARTS_LAYOUT_MISSING_LOGGED: AtomicBool = AtomicBool::new(false);
static TEXT_PANES_LOGGED: AtomicBool = AtomicBool::new(false);
static DEBUG_TEXT_RUNTIME: DebugTextRuntimeCell =
    DebugTextRuntimeCell(UnsafeCell::new(DebugTextRuntime {
        cache: DebugTextCache::Empty,
    }));

pub(crate) unsafe fn draw_debug_text_overlay(
    root_pane: *mut Pane,
    snapshot: Option<ControllerSnapshot>,
) {
    let text_panes = debug_text_panes_for_root(root_pane);
    if text_panes.count == 0 {
        if !MISSING_TEXT_PANE_LOGGED.swap(true, Ordering::Relaxed) {
            trace(&format!(
                "could not find a reusable text pane in {MATCH_HUD_LAYOUT}"
            ));
        }
        return;
    }

    if !TEXT_PANES_LOGGED.swap(true, Ordering::Relaxed) {
        trace(&format!(
            "using {} text pane(s) for debug overlay",
            text_panes.drawable_count()
        ));
    }

    let line_count = text_panes.drawable_count();
    let lines = format_debug_text_lines(snapshot, line_count);
    for (line_index, line) in lines.iter().enumerate().take(line_count) {
        let textbox = (*text_panes.panes[line_index]).as_textbox();
        position_debug_text(textbox, line_index);
        set_textbox_text(textbox, line);
    }
}

unsafe fn debug_text_panes_for_root(root_pane: *mut Pane) -> TextPaneSlots {
    (*DEBUG_TEXT_RUNTIME.0.get()).panes_for_root(root_pane)
}

impl DebugTextRuntime {
    unsafe fn panes_for_root(&mut self, root_pane: *mut Pane) -> TextPaneSlots {
        match self.cache {
            DebugTextCache::Resolved {
                root_pane: cached_root,
                slots,
            } if cached_root == root_pane && text_pane_slots_are_valid(&slots) => slots,
            DebugTextCache::Missing {
                root_pane: cached_root,
            } if cached_root == root_pane => TextPaneSlots::new(),
            _ => {
                let slots = find_debug_text_panes(root_pane);
                self.cache = if slots.count == 0 {
                    DebugTextCache::Missing { root_pane }
                } else {
                    DebugTextCache::Resolved { root_pane, slots }
                };
                slots
            }
        }
    }
}

unsafe fn find_debug_text_panes(root_pane: *mut Pane) -> TextPaneSlots {
    let mut panes = TextPaneSlots::new();

    if let Some(p1_root_pane) = find_p1_parts_root_pane(root_pane) {
        collect_debug_text_panes(&mut panes, p1_root_pane, "p1 parts layout");
    }

    if panes.count < MAX_OVERLAY_TEXT_PANES {
        collect_debug_text_panes(&mut panes, root_pane, "info_melee root");
    }

    panes
}

unsafe fn find_p1_parts_root_pane(root_pane: *mut Pane) -> Option<*mut Pane> {
    let p1_pane = find_pane_by_name(root_pane, P1_PARTS_PANE_NAME);
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

unsafe fn collect_debug_text_panes(panes: &mut TextPaneSlots, root_pane: *mut Pane, source: &str) {
    for name in OVERLAY_LINE_TEXT_PANES {
        if panes.count >= MAX_OVERLAY_TEXT_PANES {
            return;
        }

        let pane = find_pane_by_name(root_pane, name);
        if pane.is_null() {
            continue;
        }
        if !is_usable_textbox_pane(pane) {
            continue;
        }

        if !TEXT_PANES_LOGGED.load(Ordering::Relaxed) {
            trace(&format!(
                "found {} from {source} for debug overlay line {}",
                cstr_bytes_to_str(name),
                panes.count
            ));
        }
        panes.push(pane, name);
    }
}

unsafe fn text_pane_slots_are_valid(slots: &TextPaneSlots) -> bool {
    slots.panes[..slots.count]
        .iter()
        .zip(slots.names[..slots.count].iter())
        .all(|(pane, name)| is_usable_textbox_pane(*pane) && pane_name_matches(*pane, name))
}

unsafe fn is_usable_textbox_pane(pane: *mut Pane) -> bool {
    if pane.is_null() {
        return false;
    }

    let textbox = &*(pane as *const TextBox);
    (2..=512).contains(&textbox.text_buf_len)
        && textbox.text_len <= textbox.text_buf_len
        && !textbox.text_buf.is_null()
        && textbox.font_size_x.is_finite()
        && textbox.font_size_y.is_finite()
}

unsafe fn position_debug_text(textbox: &mut TextBox, line_index: usize) {
    textbox.pane.set_visible(true);
    textbox.pane.pos_x = DEBUG_TEXT_POS_X;
    textbox.pane.pos_y = DEBUG_TEXT_POS_Y - scaled(DEBUG_TEXT_LINE_HEIGHT * line_index as f32);
    textbox.pane.pos_z = 0.0;
    textbox.pane.scale_x = OVERLAY_CONFIG.scale;
    textbox.pane.scale_y = OVERLAY_CONFIG.scale;
    textbox.pane.size_x = DEBUG_TEXT_WIDTH;
    textbox.pane.size_y = DEBUG_TEXT_LINE_HEIGHT + 4.0;
    textbox.pane.alpha = OVERLAY_CONFIG.opacity;
    textbox.pane.global_alpha = OVERLAY_CONFIG.opacity;
    textbox.pane.flags |= 1 << PaneFlag::IsGlobalMatrixDirty as u8;

    textbox.font_size_x = DEBUG_TEXT_FONT_SIZE;
    textbox.font_size_y = DEBUG_TEXT_FONT_SIZE;
    textbox.line_space = 1.0;
    textbox.char_space = 0.0;
    textbox.set_text_alignment(HorizontalPosition::Left, VerticalPosition::Top);
    textbox.text_outline_enable(true);
    textbox.text_shadow_enable(true);
    textbox.set_color(255, 255, 255, OVERLAY_CONFIG.opacity);
    textbox.bits |= 1 << TextBoxFlag::IsPTDirty as u8;
}

fn scaled(value: f32) -> f32 {
    value * OVERLAY_CONFIG.scale
}

fn p1_header(snapshot: &ControllerSnapshot) -> String {
    format!(
        "P1 {} {}",
        npad_id_name(snapshot.npad_id),
        style_name(snapshot.style_flags)
    )
}

fn stick_line(snapshot: &ControllerSnapshot, trailing: Option<&str>) -> String {
    let (lx, ly) = snapshot.left_stick;
    let (rx, ry) = snapshot.right_stick;
    match trailing {
        Some(extra) => format!("LS {lx:+05},{ly:+05}  RS {rx:+05},{ry:+05}  {extra}"),
        None => format!("LS {lx:+05},{ly:+05}  RS {rx:+05},{ry:+05}"),
    }
}

fn format_debug_text_lines(
    snapshot: Option<ControllerSnapshot>,
    pane_count: usize,
) -> [String; DEBUG_TEXT_LINE_COUNT] {
    let Some(snapshot) = snapshot else {
        return [
            String::from("P1 controller not ready"),
            String::from("BTN -"),
            String::from("LS -----,-----"),
            String::from("RS -----,-----"),
        ];
    };

    let trigger_text =
        gc_trigger_text(snapshot.gc_triggers).unwrap_or_else(|| String::from("GC LT --- RT ---"));
    let header = p1_header(&snapshot);
    let buttons = button_names(snapshot.buttons);

    let (lx, ly) = snapshot.left_stick;
    let (rx, ry) = snapshot.right_stick;
    match pane_count {
        // Case 1 uses single-space separators because it must fit on one line.
        1 => [
            format!(
                "{header} BTN {buttons} LS {lx:+05},{ly:+05} RS {rx:+05},{ry:+05} {trigger_text}"
            ),
            String::new(),
            String::new(),
            String::new(),
        ],
        2 => [
            format!("{header}  BTN {buttons}"),
            stick_line(&snapshot, Some(&trigger_text)),
            String::new(),
            String::new(),
        ],
        3 => [
            header,
            format!("BTN {buttons}"),
            stick_line(&snapshot, Some(&trigger_text)),
            String::new(),
        ],
        _ => [
            header,
            format!("BTN {buttons}"),
            stick_line(&snapshot, None),
            trigger_text,
        ],
    }
}
