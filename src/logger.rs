use std::fmt::Write as FmtWrite;
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
    let mut output = String::new();
    for (index, byte) in bytes.iter().enumerate() {
        if index != 0 {
            output.push(' ');
        }
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}
