use skyline::nn::ui2d::{
    HorizontalPosition, Pane, PaneFlag, TextBox, TextBoxFlag, VerticalPosition,
};
use std::fmt::{self, Write};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::{
    DEBUG_TEXT_FONT_SIZE, DEBUG_TEXT_LINE_COUNT, DEBUG_TEXT_LINE_HEIGHT, DEBUG_TEXT_POS_X,
    DEBUG_TEXT_POS_Y, DEBUG_TEXT_WIDTH, MATCH_HUD_LAYOUT, MAX_OVERLAY_TEXT_PANES, OVERLAY_CONFIG,
    OVERLAY_LINE_TEXT_PANES, P1_PARTS_PANE_NAME,
};
use crate::input::{
    npad_id_name, style_name, write_button_names, write_gc_trigger_text, ControllerSnapshot,
};
use crate::logger::trace;
use crate::pane_utils::cstr_bytes_to_str;
use crate::ui::{find_pane_by_name, set_textbox_text, TEXTBOX_TEXT_CAP};

#[derive(Debug, Copy, Clone)]
struct TextPaneSlots {
    panes: [*mut Pane; MAX_OVERLAY_TEXT_PANES],
    count: usize,
}

impl TextPaneSlots {
    fn new() -> Self {
        Self {
            panes: [ptr::null_mut(); MAX_OVERLAY_TEXT_PANES],
            count: 0,
        }
    }

    fn push(&mut self, pane: *mut Pane) {
        if pane.is_null() || self.count >= MAX_OVERLAY_TEXT_PANES || self.contains(pane) {
            return;
        }

        self.panes[self.count] = pane;
        self.count += 1;
    }

    fn contains(&self, pane: *mut Pane) -> bool {
        self.panes[..self.count].contains(&pane)
    }

    fn drawable_count(&self) -> usize {
        self.count.min(DEBUG_TEXT_LINE_COUNT)
    }
}

static MISSING_TEXT_PANE_LOGGED: AtomicBool = AtomicBool::new(false);
static P1_PARTS_PANE_MISSING_LOGGED: AtomicBool = AtomicBool::new(false);
static P1_PARTS_LAYOUT_MISSING_LOGGED: AtomicBool = AtomicBool::new(false);
static TEXT_PANES_LOGGED: AtomicBool = AtomicBool::new(false);
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
        set_textbox_text(textbox, line.as_str());
    }
}

unsafe fn debug_text_panes_for_root(root_pane: *mut Pane) -> TextPaneSlots {
    find_debug_text_panes(root_pane)
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
        panes.push(pane);
    }
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

const DEBUG_TEXT_LINE_CAP: usize = TEXTBOX_TEXT_CAP;

struct DebugLine {
    bytes: [u8; DEBUG_TEXT_LINE_CAP],
    len: usize,
}

impl DebugLine {
    const fn new() -> Self {
        Self {
            bytes: [0; DEBUG_TEXT_LINE_CAP],
            len: 0,
        }
    }

    fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
    }

    fn clear(&mut self) {
        self.len = 0;
    }
}

impl Write for DebugLine {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let bytes = text.as_bytes();
        let next = self.len.checked_add(bytes.len()).ok_or(fmt::Error)?;
        if next > DEBUG_TEXT_LINE_CAP {
            return Err(fmt::Error);
        }
        self.bytes[self.len..next].copy_from_slice(bytes);
        self.len = next;
        Ok(())
    }
}

fn write_or_clear(line: &mut DebugLine, write_line: impl FnOnce(&mut DebugLine) -> fmt::Result) {
    line.clear();
    if write_line(line).is_err() {
        line.clear();
    }
}

fn write_p1_header(line: &mut DebugLine, snapshot: &ControllerSnapshot) -> fmt::Result {
    write!(
        line,
        "P1 {} {}",
        npad_id_name(snapshot.npad_id),
        style_name(snapshot.style_flags)
    )
}

fn write_stick_line(
    line: &mut DebugLine,
    snapshot: &ControllerSnapshot,
    trailing: Option<&str>,
) -> fmt::Result {
    let (lx, ly) = snapshot.left_stick;
    let (rx, ry) = snapshot.right_stick;
    match trailing {
        Some(extra) => write!(line, "LS {lx:+05},{ly:+05}  RS {rx:+05},{ry:+05}  {extra}"),
        None => write!(line, "LS {lx:+05},{ly:+05}  RS {rx:+05},{ry:+05}"),
    }
}

fn write_fixed(line: &mut DebugLine, text: &str) {
    write_or_clear(line, |line| line.write_str(text));
}

fn format_debug_text_lines(
    snapshot: Option<ControllerSnapshot>,
    pane_count: usize,
) -> [DebugLine; DEBUG_TEXT_LINE_COUNT] {
    let mut lines = [
        DebugLine::new(),
        DebugLine::new(),
        DebugLine::new(),
        DebugLine::new(),
    ];
    let Some(snapshot) = snapshot else {
        write_fixed(&mut lines[0], "P1 controller not ready");
        write_fixed(&mut lines[1], "BTN -");
        write_fixed(&mut lines[2], "LS -----,-----");
        write_fixed(&mut lines[3], "RS -----,-----");
        return lines;
    };

    let (lx, ly) = snapshot.left_stick;
    let (rx, ry) = snapshot.right_stick;
    match pane_count {
        // Case 1 uses single-space separators because it must fit on one line.
        1 => {
            write_or_clear(&mut lines[0], |line| {
                write_p1_header(line, &snapshot)?;
                line.write_str(" BTN ")?;
                write_button_names(snapshot.buttons, line)?;
                write!(line, " LS {lx:+05},{ly:+05} RS {rx:+05},{ry:+05} ")?;
                write_gc_trigger_text(snapshot.gc_triggers, line)
            });
        }
        2 => {
            write_or_clear(&mut lines[0], |line| {
                write_p1_header(line, &snapshot)?;
                line.write_str("  BTN ")?;
                write_button_names(snapshot.buttons, line)
            });
            write_or_clear(&mut lines[1], |line| {
                let mut trigger = DebugLine::new();
                write_gc_trigger_text(snapshot.gc_triggers, &mut trigger)?;
                write_stick_line(line, &snapshot, Some(trigger.as_str()))
            });
        }
        3 => {
            write_or_clear(&mut lines[0], |line| write_p1_header(line, &snapshot));
            write_or_clear(&mut lines[1], |line| {
                line.write_str("BTN ")?;
                write_button_names(snapshot.buttons, line)
            });
            write_or_clear(&mut lines[2], |line| {
                let mut trigger = DebugLine::new();
                write_gc_trigger_text(snapshot.gc_triggers, &mut trigger)?;
                write_stick_line(line, &snapshot, Some(trigger.as_str()))
            });
        }
        _ => {
            write_or_clear(&mut lines[0], |line| write_p1_header(line, &snapshot));
            write_or_clear(&mut lines[1], |line| {
                line.write_str("BTN ")?;
                write_button_names(snapshot.buttons, line)
            });
            write_or_clear(&mut lines[2], |line| {
                write_stick_line(line, &snapshot, None)
            });
            write_or_clear(&mut lines[3], |line| {
                write_gc_trigger_text(snapshot.gc_triggers, line)
            });
        }
    }
    lines
}
