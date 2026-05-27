use std::fs::OpenOptions;
use std::io::Write as IoWrite;

use crate::config::{LOG_PATH, PLUGIN_NAME};

pub(crate) fn reset_trace_file() {
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(LOG_PATH)
    {
        let _ = writeln!(file, "{PLUGIN_NAME}: log start");
    }
}

pub(crate) fn trace(message: &str) {
    println!("{PLUGIN_NAME}: {message}");

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(LOG_PATH) {
        let _ = writeln!(file, "{PLUGIN_NAME}: {message}");
    }
}

pub(crate) fn hex_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) struct StartupBanner<'a> {
    pub build_id: &'a str,
    pub git_change_count: u32,
    pub local_build_number: u64,
    pub embedded_layout_enabled: bool,
    pub display_version: &'a str,
    pub logical_control_count: usize,
    pub active_skin: &'a str,
    pub built_in_skin_count: usize,
    pub asset_metadata_count: usize,
}

pub(crate) fn log_startup_banner(banner: StartupBanner<'_>) {
    trace("starting P1 input overlay");
    trace(&format!("build {}", banner.build_id));
    trace(&format!(
        "git change count {} local build {}",
        banner.git_change_count, banner.local_build_number
    ));
    trace(&format!(
        "embedded layout injection {}",
        if banner.embedded_layout_enabled {
            "enabled"
        } else {
            "disabled"
        }
    ));
    trace(&format!("Smash display version {}", banner.display_version));
    trace(&format!(
        "registered {} logical controls",
        banner.logical_control_count
    ));
    trace(&format!(
        "active skin '{}' ({} built-in skins available)",
        banner.active_skin, banner.built_in_skin_count
    ));
    trace(&format!(
        "registered {} skin asset metadata entries",
        banner.asset_metadata_count
    ));
}
