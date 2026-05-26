use crate::input::ControlId;

#[derive(Debug, Copy, Clone)]
pub(crate) struct SkinElement {
    pub control_id: ControlId,
    pub pane_name: &'static [u8],
    pub base_x: f32,
    pub base_y: f32,
    pub size_x: f32,
    pub size_y: f32,
    pub released_alpha: u8,
    pub pressed_alpha: u8,
    pub released_scale: f32,
    pub pressed_scale: f32,
    pub released_visible: bool,
    pub pressed_visible: bool,
    pub stick_movement_radius: Option<f32>,
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct BuiltInSkin {
    pub name: &'static str,
    pub root_pane_name: &'static [u8],
    pub elements: &'static [SkinElement],
}

pub(crate) const PRO_CONTROLLER_STATIC_SKIN: BuiltInSkin = BuiltInSkin {
    name: "minimal_pro_controller_a_button",
    root_pane_name: b"sgpo_root\0",
    elements: &PRO_CONTROLLER_STATIC_ELEMENTS,
};

const PRO_CONTROLLER_STATIC_ELEMENTS: [SkinElement; 1] = [SkinElement {
    control_id: ControlId::A,
    pane_name: b"sgpo_pro_a_marker\0",
    base_x: 0.0,
    base_y: 0.0,
    size_x: 28.0,
    size_y: 28.0,
    released_alpha: 70,
    pressed_alpha: 255,
    released_scale: 1.0,
    pressed_scale: 1.35,
    released_visible: true,
    pressed_visible: true,
    stick_movement_radius: None,
}];
