use skyline::nn::ui2d::{Pane, PaneFlag};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::OVERLAY_CONFIG;
use crate::input::{ControllerSnapshot, ControllerViewState};
use crate::logger::trace;
use crate::skin::{BuiltInSkin, SkinElement, PRO_CONTROLLER_STATIC_SKIN};
use crate::ui::find_pane_by_name;

#[derive(Debug, Copy, Clone)]
pub(crate) enum VisualRenderError {
    MissingSkinPane {
        skin_name: &'static str,
        pane_name: &'static [u8],
    },
}

static VISUAL_PANES_LOGGED: AtomicBool = AtomicBool::new(false);

pub(crate) unsafe fn render_visual_overlay(
    root_pane: *mut Pane,
    snapshot: Option<ControllerSnapshot>,
) -> Result<(), VisualRenderError> {
    let view_state = snapshot
        .map(ControllerViewState::from_snapshot)
        .unwrap_or_else(ControllerViewState::neutral);
    render_skin(root_pane, &PRO_CONTROLLER_STATIC_SKIN, &view_state)
}

unsafe fn render_skin(
    root_pane: *mut Pane,
    skin: &BuiltInSkin,
    state: &ControllerViewState,
) -> Result<(), VisualRenderError> {
    let skin_root = find_named_pane(root_pane, skin, skin.root_pane_name)?;
    update_skin_root(skin_root);

    for element in skin.elements {
        let pane = find_skin_pane(skin_root, skin, element)?;
        update_skin_pane(pane, element, state);
    }

    if !VISUAL_PANES_LOGGED.swap(true, Ordering::Relaxed) {
        trace(&format!("visual mode using injected skin '{}'", skin.name));
    }

    Ok(())
}

unsafe fn find_skin_pane(
    root_pane: *mut Pane,
    skin: &BuiltInSkin,
    element: &SkinElement,
) -> Result<*mut Pane, VisualRenderError> {
    find_named_pane(root_pane, skin, element.pane_name)
}

unsafe fn find_named_pane(
    root_pane: *mut Pane,
    skin: &BuiltInSkin,
    pane_name: &'static [u8],
) -> Result<*mut Pane, VisualRenderError> {
    let pane = find_pane_by_name(root_pane, pane_name);
    if pane.is_null() {
        Err(VisualRenderError::MissingSkinPane {
            skin_name: skin.name,
            pane_name,
        })
    } else {
        Ok(pane)
    }
}

unsafe fn update_skin_root(pane: *mut Pane) {
    (*pane).set_visible(true);
    (*pane).pos_x = OVERLAY_CONFIG.x;
    (*pane).pos_y = OVERLAY_CONFIG.y;
    (*pane).pos_z = 0.0;
    (*pane).scale_x = OVERLAY_CONFIG.scale;
    (*pane).scale_y = OVERLAY_CONFIG.scale;
    (*pane).alpha = OVERLAY_CONFIG.opacity;
    (*pane).global_alpha = OVERLAY_CONFIG.opacity;
    (*pane).flags |= 1 << PaneFlag::IsGlobalMatrixDirty as u8;
}

unsafe fn update_skin_pane(pane: *mut Pane, element: &SkinElement, state: &ControllerViewState) {
    let value = state.control_value(element.control_id);
    let visible = if value.pressed {
        element.pressed_visible
    } else {
        element.released_visible
    };
    let alpha = if value.pressed {
        interpolate_alpha(element.released_alpha, element.pressed_alpha, value.analog)
    } else {
        element.released_alpha
    };
    let element_scale = if value.pressed {
        interpolate_scale(element.released_scale, element.pressed_scale, value.analog)
    } else {
        element.released_scale
    };
    let (stick_x, stick_y) = state
        .stick_position(element.control_id)
        .zip(element.stick_movement_radius)
        .map(|((x, y), radius)| (x * radius, y * radius))
        .unwrap_or((0.0, 0.0));

    (*pane).set_visible(visible);
    (*pane).pos_x = scaled(element.base_x + stick_x);
    (*pane).pos_y = scaled(element.base_y + stick_y);
    (*pane).pos_z = 0.0;
    (*pane).scale_x = element_scale;
    (*pane).scale_y = element_scale;
    (*pane).size_x = element.size_x;
    (*pane).size_y = element.size_y;
    (*pane).alpha = alpha;
    (*pane).global_alpha = alpha;
    (*pane).flags |= 1 << PaneFlag::IsGlobalMatrixDirty as u8;
}

fn interpolate_alpha(released_alpha: u8, pressed_alpha: u8, analog: f32) -> u8 {
    let analog = analog.clamp(0.0, 1.0);
    (released_alpha as f32 + (pressed_alpha as f32 - released_alpha as f32) * analog)
        .clamp(0.0, 255.0) as u8
}

fn interpolate_scale(released_scale: f32, pressed_scale: f32, analog: f32) -> f32 {
    let analog = analog.clamp(0.0, 1.0);
    released_scale + (pressed_scale - released_scale) * analog
}

fn scaled(value: f32) -> f32 {
    value * OVERLAY_CONFIG.scale
}
