use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const MODIFIED_LAYOUT_ARC: &str = "local-assets/modified/info_melee/layout.arc";
const EMBED_LAYOUT_ENV: &str = "SMASH_GAMEPAD_OVERLAY_EMBED_LAYOUT";
const CHANGE_NUMBER_PATH: &str = "CHANGE_NUMBER";

fn main() {
    println!("cargo:rustc-check-cfg=cfg(sgpo_embed_layout)");
    println!("cargo:rerun-if-changed={MODIFIED_LAYOUT_ARC}");
    println!("cargo:rerun-if-changed={CHANGE_NUMBER_PATH}");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=SMASH_GAMEPAD_OVERLAY_BUILD_ID");
    println!("cargo:rerun-if-env-changed={EMBED_LAYOUT_ENV}");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let generated = out_dir.join("sgpo_layout_arc.rs");
    let generated_build_info = out_dir.join("sgpo_build_info.rs");
    let source = Path::new(MODIFIED_LAYOUT_ARC);
    let embed_layout = env_flag_enabled(EMBED_LAYOUT_ENV);
    let change_number = read_change_number();
    let build_number = increment_build_number();

    if source.exists() && embed_layout {
        println!("cargo:rustc-cfg=sgpo_embed_layout");

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

    fs::write(
        generated_build_info,
        format!(
            "pub(crate) const BUILD_ID: &str = {:?};\n\
             pub(crate) const CHANGE_NUMBER: u32 = {};\n\
             pub(crate) const LOCAL_BUILD_NUMBER: u64 = {};\n\
             pub(crate) const EMBEDDED_LAYOUT_ENABLED: bool = {};\n",
            build_id(change_number, build_number),
            change_number,
            build_number,
            embed_layout
        ),
    )
    .expect("failed to write generated build info");
}

fn env_flag_enabled(name: &str) -> bool {
    matches!(
        env::var(name).as_deref(),
        Ok("1") | Ok("true") | Ok("yes") | Ok("on")
    )
}

fn read_change_number() -> u32 {
    fs::read_to_string(CHANGE_NUMBER_PATH)
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(0)
}

fn increment_build_number() -> u64 {
    let target_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| String::from(".")))
            .join("target");
    let path = target_dir.join("sgpo-local-build-number");

    let previous = fs::read_to_string(&path)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(0);
    let current = previous.saturating_add(1);

    let _ = fs::create_dir_all(target_dir);
    fs::write(path, current.to_string()).expect("failed to update local build number");

    current
}

fn build_id(change_number: u32, build_number: u64) -> String {
    if let Ok(value) = env::var("SMASH_GAMEPAD_OVERLAY_BUILD_ID") {
        if !value.trim().is_empty() {
            return value;
        }
    }

    let rev = command_output("git", &["rev-parse", "--short", "HEAD"])
        .unwrap_or_else(|| String::from("unknown"));
    let dirty = command_output("git", &["status", "--short"])
        .map(|status| !status.trim().is_empty())
        .unwrap_or(false);

    if dirty {
        let fingerprint = dirty_tree_fingerprint();
        format!("c{change_number}-b{build_number}-{rev}-dirty-{fingerprint:08x}")
    } else {
        format!("c{change_number}-b{build_number}-{rev}")
    }
}

fn dirty_tree_fingerprint() -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for input in [
        command_output("git", &["status", "--short"]).unwrap_or_default(),
        command_output(
            "git",
            &[
                "diff",
                "--",
                "src",
                "tools",
                "docs",
                "README.md",
                "AGENTSUMMARY.md",
                "build.rs",
                "Cargo.toml",
                "Cargo.lock",
                CHANGE_NUMBER_PATH,
            ],
        )
        .unwrap_or_default(),
    ] {
        for byte in input.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }

    hash
}

fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }

    Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
