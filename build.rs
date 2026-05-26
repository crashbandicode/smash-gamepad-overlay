use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const MODIFIED_LAYOUT_ARC: &str = "local-assets/modified/info_melee/layout.arc";

fn main() {
    println!("cargo:rerun-if-changed={MODIFIED_LAYOUT_ARC}");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let generated = out_dir.join("sgpo_layout_arc.rs");
    let source = Path::new(MODIFIED_LAYOUT_ARC);

    if source.exists() {
        let copied = out_dir.join("info_melee_layout.arc");
        fs::copy(source, &copied).expect("failed to copy modified info_melee layout.arc");
        fs::write(
            generated,
            format!(
                "pub(crate) const INJECTED_LAYOUT_ARC_AVAILABLE: bool = true;\n\
                 pub(crate) static INJECTED_LAYOUT_ARC: &[u8] = include_bytes!(r#\"{}\"#);\n",
                copied.display()
            ),
        )
        .expect("failed to write generated layout include");
    } else {
        fs::write(
            generated,
            "pub(crate) const INJECTED_LAYOUT_ARC_AVAILABLE: bool = false;\n\
             pub(crate) static INJECTED_LAYOUT_ARC: &[u8] = &[];\n",
        )
        .expect("failed to write generated empty layout include");
    }
}
