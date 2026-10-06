use skyline::nn::ui2d::{Pane, PaneFlag};
use std::ffi::CStr;

/// Engine Finalize loops skip panes with `PaneFlag::UserAllocated` (bit 3 at pane+0x58).
pub(crate) unsafe fn pane_is_user_allocated(pane: *mut Pane) -> bool {
    !pane.is_null() && ((*pane).flags & (1 << PaneFlag::UserAllocated as u8)) != 0
}

pub(crate) unsafe fn pane_name_matches(pane: *mut Pane, expected_name: &[u8]) -> bool {
    if pane.is_null() {
        return false;
    }

    let Ok(expected) = CStr::from_bytes_with_nul(expected_name) else {
        return false;
    };

    let name = &(*pane).name;
    let len = name
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(name.len());
    let actual = std::slice::from_raw_parts(name.as_ptr(), len);

    actual == expected.to_bytes()
}

pub(crate) fn cstr_bytes_to_str(bytes: &[u8]) -> &str {
    CStr::from_bytes_with_nul(bytes)
        .ok()
        .and_then(|name| name.to_str().ok())
        .unwrap_or("<invalid>")
}
