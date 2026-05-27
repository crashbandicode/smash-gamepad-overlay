// Synthetic ControlId enum used only by tools/tests/test_analyze_retrospy_skin.py.
// It exercises parse_control_ids against:
//   - `//` line comments
//   - `#[allow(dead_code)]` attribute lines
//   - blank lines between variants
//   - variants that include digits and uppercase characters
//
// This file is NOT compiled into the plugin; it is read as text by the test.

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub enum ControlId {
    // First face button.
    A,
    B,
    X,
    Y,

    #[allow(dead_code)]
    ZL,
    ZR,
    L3,
    R3,

    // Stick gates and dots are runtime-neutral except for the dots.
    LeftStickGate,
    LeftStickDot,
}
