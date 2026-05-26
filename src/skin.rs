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
    name: "minimal_pro_controller_full",
    root_pane_name: b"sgpo_root\0",
    elements: &PRO_CONTROLLER_STATIC_ELEMENTS,
};

const PRO_CONTROLLER_STATIC_ELEMENTS: [SkinElement; 24] = [
    button(ControlId::ZL, b"sgpo_pro_lt\0", -145.0, 122.0, 54.0, 20.0),
    button(ControlId::L, b"sgpo_pro_lb\0", -145.0, 95.0, 54.0, 20.0),
    button(ControlId::ZR, b"sgpo_pro_rt\0", 65.0, 122.0, 54.0, 20.0),
    button(ControlId::R, b"sgpo_pro_rb\0", 65.0, 95.0, 54.0, 20.0),
    button(
        ControlId::Minus,
        b"sgpo_pro_minus\0",
        -38.0,
        45.0,
        20.0,
        20.0,
    ),
    button(ControlId::Plus, b"sgpo_pro_plus\0", 20.0, 45.0, 20.0, 20.0),
    button(ControlId::L3, b"sgpo_pro_l3\0", -68.0, -30.0, 18.0, 18.0),
    button(ControlId::R3, b"sgpo_pro_r3\0", 62.0, -100.0, 18.0, 18.0),
    static_marker(
        ControlId::LeftStickGate,
        b"sgpo_pro_ls_gate\0",
        -105.0,
        -30.0,
        58.0,
        58.0,
        60,
    ),
    stick_dot(
        ControlId::LeftStickDot,
        b"sgpo_pro_ls_dot\0",
        -105.0,
        -30.0,
        14.0,
        14.0,
        22.0,
    ),
    static_marker(
        ControlId::RightStickGate,
        b"sgpo_pro_rs_gate\0",
        30.0,
        -100.0,
        58.0,
        58.0,
        60,
    ),
    stick_dot(
        ControlId::RightStickDot,
        b"sgpo_pro_rs_dot\0",
        30.0,
        -100.0,
        14.0,
        14.0,
        22.0,
    ),
    button(
        ControlId::DpadUp,
        b"sgpo_pro_du\0",
        -105.0,
        -78.0,
        16.0,
        16.0,
    ),
    button(
        ControlId::DpadDown,
        b"sgpo_pro_dd\0",
        -105.0,
        -128.0,
        16.0,
        16.0,
    ),
    button(
        ControlId::DpadLeft,
        b"sgpo_pro_dl\0",
        -130.0,
        -103.0,
        16.0,
        16.0,
    ),
    button(
        ControlId::DpadRight,
        b"sgpo_pro_dr\0",
        -80.0,
        -103.0,
        16.0,
        16.0,
    ),
    button(
        ControlId::DpadUpLeft,
        b"sgpo_pro_dul\0",
        -130.0,
        -78.0,
        13.0,
        13.0,
    ),
    button(
        ControlId::DpadUpRight,
        b"sgpo_pro_dur\0",
        -80.0,
        -78.0,
        13.0,
        13.0,
    ),
    button(
        ControlId::DpadDownLeft,
        b"sgpo_pro_ddl\0",
        -130.0,
        -128.0,
        13.0,
        13.0,
    ),
    button(
        ControlId::DpadDownRight,
        b"sgpo_pro_ddr\0",
        -80.0,
        -128.0,
        13.0,
        13.0,
    ),
    button(ControlId::Y, b"sgpo_pro_btn_y\0", 15.0, -20.0, 24.0, 24.0),
    button(ControlId::X, b"sgpo_pro_btn_x\0", 45.0, 10.0, 24.0, 24.0),
    button(ControlId::B, b"sgpo_pro_btn_b\0", 45.0, -50.0, 24.0, 24.0),
    button(
        ControlId::A,
        b"sgpo_pro_a_marker\0",
        75.0,
        -20.0,
        28.0,
        28.0,
    ),
];

const fn button(
    control_id: ControlId,
    pane_name: &'static [u8],
    base_x: f32,
    base_y: f32,
    size_x: f32,
    size_y: f32,
) -> SkinElement {
    SkinElement {
        control_id,
        pane_name,
        base_x,
        base_y,
        size_x,
        size_y,
        released_alpha: 70,
        pressed_alpha: 255,
        released_scale: 1.0,
        pressed_scale: 1.22,
        released_visible: true,
        pressed_visible: true,
        stick_movement_radius: None,
    }
}

const fn static_marker(
    control_id: ControlId,
    pane_name: &'static [u8],
    base_x: f32,
    base_y: f32,
    size_x: f32,
    size_y: f32,
    alpha: u8,
) -> SkinElement {
    SkinElement {
        control_id,
        pane_name,
        base_x,
        base_y,
        size_x,
        size_y,
        released_alpha: alpha,
        pressed_alpha: alpha,
        released_scale: 1.0,
        pressed_scale: 1.0,
        released_visible: true,
        pressed_visible: true,
        stick_movement_radius: None,
    }
}

const fn stick_dot(
    control_id: ControlId,
    pane_name: &'static [u8],
    base_x: f32,
    base_y: f32,
    size_x: f32,
    size_y: f32,
    stick_movement_radius: f32,
) -> SkinElement {
    SkinElement {
        control_id,
        pane_name,
        base_x,
        base_y,
        size_x,
        size_y,
        released_alpha: 190,
        pressed_alpha: 255,
        released_scale: 1.0,
        pressed_scale: 1.0,
        released_visible: true,
        pressed_visible: true,
        stick_movement_radius: Some(stick_movement_radius),
    }
}
