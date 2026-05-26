use skyline::nn::ui2d::{
    HorizontalPosition, Pane, PaneFlag, TextBox, TextBoxFlag, VerticalPosition,
};
use std::ffi::CStr;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::{
    DEBUG_TEXT_FONT_SIZE, DEBUG_TEXT_LINE_COUNT, DEBUG_TEXT_LINE_HEIGHT, DEBUG_TEXT_POS_X,
    DEBUG_TEXT_POS_Y, DEBUG_TEXT_WIDTH, MATCH_HUD_LAYOUT, MAX_OVERLAY_TEXT_PANES, OVERLAY_CONFIG,
    OVERLAY_LINE_TEXT_PANES, P1_PARTS_PANE_NAME,
};
use crate::input::{button_names, gc_trigger_text, npad_id_name, style_name, ControllerSnapshot};
use crate::logger::trace;
use crate::ui::{find_pane_by_name, set_textbox_text};

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

pub(crate) unsafe fn draw_debug_text_overlay(
    root_pane: *mut Pane,
    snapshot: Option<ControllerSnapshot>,
) {
    let text_panes = find_debug_text_panes(root_pane);
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

fn cstr_bytes_to_str(bytes: &[u8]) -> &str {
    CStr::from_bytes_with_nul(bytes)
        .ok()
        .and_then(|name| name.to_str().ok())
        .unwrap_or("<invalid>")
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
