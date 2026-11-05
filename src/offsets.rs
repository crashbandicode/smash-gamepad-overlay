use skyline::nn::oe;

pub(crate) fn display_version() -> String {
    let mut version = oe::DisplayVersion { name: [0; 16] };
    unsafe {
        oe::GetDisplayVersion(&mut version);
    }

    let end = version
        .name
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(version.name.len());

    std::str::from_utf8(&version.name[..end])
        .unwrap_or("<non-utf8>")
        .to_string()
}
