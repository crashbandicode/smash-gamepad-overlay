use skyline::nn::ui2d::{Pane, PaneFlag};

use crate::config::OVERLAY_CONFIG;
use crate::input::{ControllerSnapshot, ControllerViewState};
use crate::skin::{BuiltInSkin, SkinElement, MINIMAL_GAMECUBE_SKIN};
use crate::ui::find_pane_by_name;

#[derive(Debug, Copy, Clone)]
pub(crate) enum VisualRenderError {
    ControllerNotReady,
    MissingSkinPane { skin_name: &'static str },
}

pub(crate) unsafe fn render_visual_overlay(
    root_pane: *mut Pane,
    snapshot: Option<ControllerSnapshot>,
) -> Result<(), VisualRenderError> {
    let snapshot = snapshot.ok_or(VisualRenderError::ControllerNotReady)?;
    let view_state = ControllerViewState::from_snapshot(snapshot);
    render_skin(root_pane, &MINIMAL_GAMECUBE_SKIN, &view_state)
}

unsafe fn render_skin(
    root_pane: *mut Pane,
    skin: &BuiltInSkin,
    state: &ControllerViewState,
) -> Result<(), VisualRenderError> {
    if !skin_panes_present(root_pane, skin) {
        return Err(VisualRenderError::MissingSkinPane {
            skin_name: skin.name,
        });
    }

    for element in skin.elements {
        let pane = find_pane_by_name(root_pane, element.pane_name);
        update_skin_pane(pane, element, state);
    }

    Ok(())
}

unsafe fn skin_panes_present(root_pane: *mut Pane, skin: &BuiltInSkin) -> bool {
    skin.elements
        .iter()
        .all(|element| !find_pane_by_name(root_pane, element.pane_name).is_null())
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
    let (stick_x, stick_y) = state
        .stick_position(element.control_id)
        .zip(element.stick_movement_radius)
        .map(|((x, y), radius)| (x * radius, y * radius))
        .unwrap_or((0.0, 0.0));

    (*pane).set_visible(visible);
    (*pane).pos_x = OVERLAY_CONFIG.x + scaled(element.base_x + stick_x);
    (*pane).pos_y = OVERLAY_CONFIG.y + scaled(element.base_y + stick_y);
    (*pane).pos_z = 0.0;
    (*pane).scale_x = OVERLAY_CONFIG.scale;
    (*pane).scale_y = OVERLAY_CONFIG.scale;
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

fn scaled(value: f32) -> f32 {
    value * OVERLAY_CONFIG.scale
}
