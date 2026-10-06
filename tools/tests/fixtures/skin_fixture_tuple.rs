// Tuple-form companion to skin_fixture.rs. Current src/skin.rs uses this shape
// for image_stick. The flat fixture stays so historical sources still parse.

const SWITCH_PRO_ALT_ELEMENTS: [SkinElement; 1] = [
    image_stick(
        ControlId::LeftStickDot,
        b"sgpo_tuple_left_stick\0",
        "stick_Left.png",
        (-347.0, 137.5),
        (164.0, 164.0),
        (41.0, 41.0),
    ),
];
