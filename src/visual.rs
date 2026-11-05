//! Pane writes shared by the HUD cache: skin root placement, per-control updates and hiding.

use sgpo_ui_cache_metadata::placement::Placement;
use skyline::nn::ui2d::{Pane, PaneFlag};

use crate::input::ControllerViewState;
use crate::skin::{BuiltInSkin, SkinElement};

pub(crate) const MAX_RESOLVED_SKIN_ELEMENTS: usize = 32;

#[derive(Debug, Copy, Clone)]
pub(crate) enum VisualRenderError {
    MissingSkinPane {
        skin_name: &'static str,
        pane_name: &'static [u8],
    },
    SkinTooLarge {
        skin_name: &'static str,
        element_count: usize,
        max_elements: usize,
    },
    UserAllocatedPane {
        skin_name: &'static str,
        pane_name: &'static [u8],
    },
}

pub(crate) fn validate_skin_size(skin: &BuiltInSkin) -> Result<(), VisualRenderError> {
    if skin.elements.len() > MAX_RESOLVED_SKIN_ELEMENTS {
        Err(VisualRenderError::SkinTooLarge {
            skin_name: skin.name,
            element_count: skin.elements.len(),
            max_elements: MAX_RESOLVED_SKIN_ELEMENTS,
        })
    } else {
        Ok(())
    }
}

pub(crate) unsafe fn update_visual_skin_root_with_config(
    pane: *mut Pane,
    config: &Placement,
    skin: &BuiltInSkin,
) {
    (*pane).set_visible(true);
    (*pane).pos_x = config.x;
    (*pane).pos_y = config.y;
    (*pane).pos_z = 0.0;
    (*pane).scale_x = config.scale * skin.root_scale;
    (*pane).scale_y = config.scale * skin.root_scale;
    (*pane).alpha = config.opacity;
    (*pane).global_alpha = config.opacity;
    (*pane).flags |= 1 << PaneFlag::IsGlobalMatrixDirty as u8;
}

pub(crate) unsafe fn hide_visual_skin_root(pane: *mut Pane) {
    if pane.is_null() {
        return;
    }

    (*pane).alpha = 0;
    (*pane).global_alpha = 0;
    (*pane).set_visible(false);
    (*pane).flags |= 1 << PaneFlag::IsGlobalMatrixDirty as u8;
}

pub(crate) unsafe fn hide_visual_skin_pane(pane: *mut Pane) {
    if pane.is_null() {
        return;
    }

    (*pane).alpha = 0;
    (*pane).global_alpha = 0;
    (*pane).set_visible(false);
    (*pane).flags |= 1 << PaneFlag::IsGlobalMatrixDirty as u8;
}

pub(crate) unsafe fn update_visual_skin_pane(
    pane: *mut Pane,
    element: &SkinElement,
    state: &ControllerViewState,
) {
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
        .zip(element.stick_movement)
        .map(|((x, y), movement)| (x * movement.x, y * movement.y))
        .unwrap_or((0.0, 0.0));

    (*pane).set_visible(visible);
    (*pane).pos_x = element.base_x + stick_x;
    (*pane).pos_y = element.base_y + stick_y;
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
