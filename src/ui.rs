use skyline::libc::c_char;
use skyline::nn::ui2d::Pane;

// 13.0.5: early nn::ui2d helper, unmoved from 13.0.4.
#[skyline::from_offset(0x59970)]
unsafe fn find_pane_by_name_recursive(pane: *const Pane, name: *const c_char) -> *mut Pane;

pub(crate) unsafe fn find_pane_by_name(root_pane: *mut Pane, name: &[u8]) -> *mut Pane {
    find_pane_by_name_recursive(root_pane, name.as_ptr() as *const c_char)
}
