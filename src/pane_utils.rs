use skyline::nn::ui2d::Pane;
use std::ffi::CStr;

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
    let actual = std::slice::from_raw_parts(name.as_ptr() as *const u8, len);

    actual == expected.to_bytes()
}

pub(crate) fn cstr_bytes_to_str(bytes: &[u8]) -> &str {
    CStr::from_bytes_with_nul(bytes)
        .ok()
        .and_then(|name| name.to_str().ok())
        .unwrap_or("<invalid>")
}
