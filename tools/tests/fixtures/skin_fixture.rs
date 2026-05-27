// Synthetic skin source used only by tools/tests/test_analyze_retrospy_skin.py.
// It contains a SWITCH_PRO_ALT_ELEMENTS block whose shape matches the real
// builder call sites in src/skin.rs. Two image_button entries and one
// image_stick entry exercise both regex paths plus the constant-bracketing
// logic that locates the `= [ ... ];` body in parse_switch_pro_alt_builtin.
//
// The literal `= [ ... ];` sequence on the previous line is intentional: it
// regression-tests strip_rust_comments by sitting inside a line comment.
// A correctly comment-stripped parser must ignore it and still find the real
// array below.
//
// This file is NOT compiled into the plugin; it is read as text by the test.

// Coordinates and dimensions below are arbitrary; tests pin exact values.
const SWITCH_PRO_ALT_ELEMENTS: [SkinElement; 3] = [
    image_button(
        ControlId::A,
        b"sgpo_fixture_face_a\0",
        "face_A.png",
        10.0,
        20.0,
        50.0,
        60.0,
    ),
    image_button(
        ControlId::B,
        b"sgpo_fixture_face_b\0",
        "face_B.png",
        -10.5,
        -20.5,
        51.0,
        61.0,
    ),
    image_stick(
        ControlId::LeftStickDot,
        b"sgpo_fixture_left_stick\0",
        "stick_Left.png",
        100.0,
        200.0,
        80.0,
        80.0,
        15.5,
        25.5,
    ),
];
