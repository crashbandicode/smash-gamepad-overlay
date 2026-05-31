use anyhow::{bail, Context, Result};
use clap::{Parser, ValueEnum};
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};
use nx_layout_toolbox::bflyt::{read_bflyt, write_bflyt, ClonePaneSpec, BFLYT};
use nx_layout_toolbox::bntx::pipeline::ImportTextureFormat;
use nx_layout_toolbox::layout::{apply_manifest, validate_manifest, ApplyOptions, ValidateOptions};
use nx_layout_toolbox::manifest as nx_manifest;
use nx_layout_toolbox::sarc;
use nx_layout_toolbox::texpipe::Bc7Quality;
use serde::Serialize;
use smash_arc::{ArcFile, ArcLookup, Region};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

const INFO_MELEE_LAYOUT_ARC_PATH: &str = "ui/layout/info/info_melee/info_melee/layout.arc";
const MOD_FOLDER_NAME: &str = "smash-gamepad-overlay";
const SMASH_TITLE_ID: &str = "01006A800016E000";

const DEFAULT_BASE_UNPACKED: &str = "local-assets/modified/info_melee/unpacked";
const DEFAULT_BASE_WORK_DIR: &str = "local-assets/generated/base-info-melee-rs";
const DEFAULT_OUTPUT_DIR: &str = "local-assets/generated/sgpo-skins-rs";
const DEFAULT_NRO: &str = "target/aarch64-skyline-switch/release/libsmash_gamepad_overlay.nro";
const DEFAULT_SWITCH_PRO_ALT_DIR: &str = "/mnt/c/Program Files/RetroSpy/skins/switch-pro-alt";
const DEFAULT_GAMECUBE_TRON_DIR: &str = "/mnt/c/Program Files/RetroSpy/skins/gamecube-tron";

const ROOT_BFLYT: &str = "blyt/info_melee.bflyt";
const PLAYER_PARTS_BFLYTS: [&str; 2] = [
    "blyt/info_melee_lct_player_00.bflyt",
    "blyt/info_melee_lct_player_01.bflyt",
];

const AUTO_SKIN_NAME: &str = "auto";
const DEFAULT_SIMPLE_SKIN_NAME: &str = "default_simple";
const DEFAULT_SIMPLE_GAMECUBE_SKIN_NAME: &str = "default_simple_gamecube";

const SGPO_ROOT: &str = "sgpo_root";
const SGPO_ROOT_POS: (f32, f32, f32) = (760.0, -330.0, 0.0);
const SGPO_ROOT_SIZE: (f32, f32) = (360.0, 300.0);
const INITIAL_PANE_ALPHA: u8 = 0;

const ROOT_MARKER_TEMPLATE: &str = "set_rep_stock_01";
const PLAYER_MARKER_TEMPLATE: &str = "set_rep_01";
const PLAYER_MATERIAL_SOURCE_PANE: &str = "set_rep_stock_01";
const GENERATED_PANE_TEMPLATE: &str = "sgpo_pro_a_marker";
const GENERATED_MATERIAL_TEMPLATE: &str = "set_rep_stock_01";

const IMAGE_BUTTON_RELEASED_ALPHA: u8 = 70;
const IMAGE_BUTTON_PRESSED_ALPHA: u8 = 255;
const IMAGE_STATIC_ALPHA: u8 = 255;
const IMAGE_STICK_ALPHA: u8 = 255;
const RELEASED_SCALE: f32 = 1.0;
const BUTTON_PRESSED_SCALE: f32 = 1.05;
const ZIP_MOD_TIME: u16 = 0;
const ZIP_MOD_DATE: u16 = 0x0021; // 1980-01-01, the earliest valid DOS ZIP date.

const PANE_SPECS: [PaneSpec; 24] = [
    PaneSpec::new("sgpo_pro_lt", -145.0, 122.0, 54.0, 20.0),
    PaneSpec::new("sgpo_pro_lb", -145.0, 95.0, 54.0, 20.0),
    PaneSpec::new("sgpo_pro_rt", 65.0, 122.0, 54.0, 20.0),
    PaneSpec::new("sgpo_pro_rb", 65.0, 95.0, 54.0, 20.0),
    PaneSpec::new("sgpo_pro_minus", -38.0, 45.0, 20.0, 20.0),
    PaneSpec::new("sgpo_pro_plus", 20.0, 45.0, 20.0, 20.0),
    PaneSpec::new("sgpo_pro_l3", -68.0, -30.0, 18.0, 18.0),
    PaneSpec::new("sgpo_pro_r3", 62.0, -100.0, 18.0, 18.0),
    PaneSpec::new("sgpo_pro_ls_gate", -105.0, -30.0, 58.0, 58.0),
    PaneSpec::new("sgpo_pro_ls_dot", -105.0, -30.0, 14.0, 14.0),
    PaneSpec::new("sgpo_pro_rs_gate", 30.0, -100.0, 58.0, 58.0),
    PaneSpec::new("sgpo_pro_rs_dot", 30.0, -100.0, 14.0, 14.0),
    PaneSpec::new("sgpo_pro_du", -105.0, -78.0, 16.0, 16.0),
    PaneSpec::new("sgpo_pro_dd", -105.0, -128.0, 16.0, 16.0),
    PaneSpec::new("sgpo_pro_dl", -130.0, -103.0, 16.0, 16.0),
    PaneSpec::new("sgpo_pro_dr", -80.0, -103.0, 16.0, 16.0),
    PaneSpec::new("sgpo_pro_dul", -130.0, -78.0, 13.0, 13.0),
    PaneSpec::new("sgpo_pro_dur", -80.0, -78.0, 13.0, 13.0),
    PaneSpec::new("sgpo_pro_ddl", -130.0, -128.0, 13.0, 13.0),
    PaneSpec::new("sgpo_pro_ddr", -80.0, -128.0, 13.0, 13.0),
    PaneSpec::new("sgpo_pro_btn_y", 15.0, -20.0, 24.0, 24.0),
    PaneSpec::new("sgpo_pro_btn_x", 45.0, 10.0, 24.0, 24.0),
    PaneSpec::new("sgpo_pro_btn_b", 45.0, -50.0, 24.0, 24.0),
    PaneSpec::new("sgpo_pro_a_marker", 75.0, -20.0, 28.0, 28.0),
];

#[derive(Debug, Parser)]
#[command(
    name = "sgpo-installer",
    about = "Build and stage Smash Gamepad Overlay layout/config/NRO assets"
)]
struct Cli {
    #[arg(long, default_value = ".env")]
    env_file: PathBuf,
    #[arg(long)]
    sd_root: Option<PathBuf>,
    #[arg(long)]
    data_arc: Option<PathBuf>,
    #[arg(long)]
    layout_arc: Option<PathBuf>,
    #[arg(long)]
    base_unpacked: Option<PathBuf>,
    #[arg(long)]
    base_work_dir: Option<PathBuf>,
    #[arg(long)]
    output_dir: Option<PathBuf>,
    #[arg(long)]
    nro: Option<PathBuf>,
    #[arg(long, default_value = AUTO_SKIN_NAME)]
    active_skin: String,
    #[arg(long, value_enum)]
    include_skin: Vec<PresetKey>,
    #[arg(long)]
    include_all_presets: bool,
    #[arg(long)]
    switch_pro_alt_dir: Option<PathBuf>,
    #[arg(long)]
    gamecube_tron_dir: Option<PathBuf>,
    #[arg(long, default_value = "fast")]
    quality: String,
    #[arg(
        long,
        default_value = "bc7-srgb",
        help = "Texture format for generated PNG imports: bc7-srgb, bc7, rgba8, or rgba8-srgb"
    )]
    texture_format: String,
    #[arg(long)]
    align: Option<String>,
    #[arg(long)]
    force: bool,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    list_presets: bool,
    #[arg(long)]
    no_backup: bool,
    #[arg(long)]
    skip_nro: bool,
    #[arg(
        long,
        help = "Write an SD-root overlay ZIP containing layout/config/NRO outputs"
    )]
    sd_zip: Option<PathBuf>,
    #[arg(long, help = "Build outputs without copying directly to --sd-root")]
    no_sd_stage: bool,
    #[arg(
        long,
        default_value = "target/arcropolis/smash-gamepad-overlay-generated-rs"
    )]
    stage_target: PathBuf,
    #[arg(long)]
    no_stage_target: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, ValueEnum)]
enum PresetKey {
    #[value(name = "switch-pro-alt")]
    SwitchProAlt,
    #[value(name = "gamecube-tron")]
    GamecubeTron,
}

#[derive(Debug, Clone)]
struct Config {
    sd_root: Option<PathBuf>,
    data_arc: Option<PathBuf>,
    layout_arc: Option<PathBuf>,
    base_unpacked: Option<PathBuf>,
    base_work_dir: PathBuf,
    output_dir: PathBuf,
    nro: PathBuf,
    active_skin: String,
    selected_presets: Vec<SkinPreset>,
    quality: Bc7Quality,
    texture_format: ImportTextureFormat,
    srgb: bool,
    texture_format_label: String,
    align: Option<u32>,
    force: bool,
    dry_run: bool,
    no_backup: bool,
    skip_nro: bool,
    sd_zip: Option<PathBuf>,
    no_sd_stage: bool,
    stage_target: PathBuf,
    no_stage_target: bool,
}

impl Config {
    fn should_stage_sd(&self) -> bool {
        self.sd_root.is_some() && !self.no_sd_stage
    }
}

#[derive(Debug, Clone)]
struct SkinPreset {
    key: PresetKey,
    key_name: &'static str,
    skin_name: &'static str,
    section: &'static str,
    pane_prefix: &'static str,
    expected_layout_flavor: &'static str,
    skin_dir: PathBuf,
    manifest_dir: PathBuf,
    report: PathBuf,
    validate_builtin: bool,
}

#[derive(Debug, Clone, Copy)]
struct PaneSpec {
    name: &'static str,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl PaneSpec {
    const fn new(name: &'static str, x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            name,
            x,
            y,
            width,
            height,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct GeneratedManifest {
    schema_version: u32,
    skin_name: String,
    root_pane_name: String,
    expected_layout_flavor: String,
    source: ManifestSource,
    coordinate_space: CoordinateSpace,
    defaults: ManifestDefaults,
    elements: Vec<GeneratedElement>,
}

#[derive(Debug, Clone, Serialize)]
struct ManifestSource {
    skin_xml: String,
    retrospy_skin_name: String,
    retrospy_author: String,
    retrospy_types: Vec<String>,
    section: String,
    background_image: String,
    background_width: u32,
    background_height: u32,
}

#[derive(Debug, Clone, Serialize)]
struct CoordinateSpace {
    origin: &'static str,
    base_x_formula: &'static str,
    base_y_formula: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct ManifestDefaults {
    image_static: ElementDefaults,
    image_button: ElementDefaults,
    image_stick: ElementDefaults,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct ElementDefaults {
    released_alpha: u8,
    pressed_alpha: u8,
    released_scale: f32,
    pressed_scale: f32,
}

#[derive(Debug, Clone, Serialize)]
struct GeneratedElement {
    control_id: String,
    pane_name: String,
    image_filename: String,
    material_name: String,
    source: ElementSource,
    base_x: f32,
    base_y: f32,
    width: f32,
    height: f32,
    released_alpha: u8,
    pressed_alpha: u8,
    released_scale: f32,
    pressed_scale: f32,
    stick_movement: Option<StickMovement>,
}

#[derive(Debug, Clone, Serialize)]
struct ElementSource {
    retrospy_kind: String,
    retrospy_section: String,
    retrospy_name: String,
    x: f32,
    y: f32,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct StickMovement {
    x: f32,
    y: f32,
}

#[derive(Debug, Clone)]
struct SkinMetadata {
    name: String,
    author: String,
    types: Vec<String>,
    background_image: String,
    background_width: u32,
    background_height: u32,
}

#[derive(Debug, Clone)]
struct ParsedControl {
    kind: ControlKind,
    section: String,
    retrospy_name: String,
    control_id: Option<String>,
    image: String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    base_x: f32,
    base_y: f32,
    movement_x: Option<f32>,
    movement_y: Option<f32>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ControlKind {
    Background,
    Button,
    Stick,
}

#[derive(Debug, Clone)]
struct XmlTag {
    attrs: HashMap<String, String>,
    start: usize,
    end: usize,
    inner: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let env = load_dotenv(&cli.env_file)?;
    let config = build_config(cli, &env)?;

    if config.dry_run {
        print_install_plan(&config);
        return Ok(());
    }

    print_install_plan(&config);
    validate_request(&config)?;

    let mut base_unpacked = config.base_unpacked.clone();
    if base_unpacked.is_none() {
        base_unpacked = Some(prepare_base_unpacked(&config)?);
    }
    let base_unpacked = base_unpacked.expect("base layout selected");

    let mut manifests = Vec::new();
    for preset in &config.selected_presets {
        manifests.push((preset.clone(), generate_manifest(preset)?));
    }

    if !config.no_backup && config.should_stage_sd() {
        backup_installed_files(&config)?;
    }

    build_layout(&config, &base_unpacked, &manifests)?;

    if !config.skip_nro && config.should_stage_sd() {
        stage_nro(&config)?;
    }

    print_install_result(&config);
    Ok(())
}

fn build_config(cli: Cli, env: &HashMap<String, String>) -> Result<Config> {
    let all_keys = if cli.include_all_presets {
        vec![PresetKey::SwitchProAlt, PresetKey::GamecubeTron]
    } else {
        cli.include_skin.clone()
    };

    let sd_root = cli
        .sd_root
        .or_else(|| env_path(env, "SGPO_SD_ROOT"))
        .or_else(|| {
            env_path(env, "SGPO_EMU_MODS_DIR")
                .and_then(|mods| mods.parent()?.parent().map(Path::to_path_buf))
        });

    if sd_root.is_none() && cli.sd_zip.is_none() {
        bail!(
            "--sd-root, SGPO_SD_ROOT, or SGPO_EMU_MODS_DIR is required unless --sd-zip is passed"
        );
    }
    if cli.no_sd_stage && cli.sd_zip.is_none() {
        bail!("--no-sd-stage requires --sd-zip");
    }

    let data_arc = cli.data_arc.or_else(|| env_path(env, "SGPO_DATA_ARC"));
    let layout_arc = cli
        .layout_arc
        .or_else(|| env_path(env, "SGPO_INFO_MELEE_LAYOUT_ARC"));
    let base_unpacked = cli.base_unpacked.or_else(|| {
        if data_arc.is_none() && layout_arc.is_none() {
            Some(env_path_or(
                env,
                "SGPO_BASE_UNPACKED",
                DEFAULT_BASE_UNPACKED,
            ))
        } else {
            None
        }
    });
    let base_work_dir = cli
        .base_work_dir
        .unwrap_or_else(|| env_path_or(env, "SGPO_BASE_WORK_DIR", DEFAULT_BASE_WORK_DIR));
    let output_dir = cli
        .output_dir
        .unwrap_or_else(|| env_path_or(env, "SGPO_SKIN_OUTPUT_DIR", DEFAULT_OUTPUT_DIR));
    let nro = cli
        .nro
        .unwrap_or_else(|| env_path_or(env, "SGPO_NRO", DEFAULT_NRO));
    let switch_dir = cli
        .switch_pro_alt_dir
        .unwrap_or_else(|| env_path_or(env, "SGPO_SWITCH_PRO_ALT_DIR", DEFAULT_SWITCH_PRO_ALT_DIR));
    let gamecube_dir = cli
        .gamecube_tron_dir
        .unwrap_or_else(|| env_path_or(env, "SGPO_GAMECUBE_TRON_DIR", DEFAULT_GAMECUBE_TRON_DIR));

    let presets = available_presets(&switch_dir, &gamecube_dir);
    if cli.list_presets {
        print_presets(&presets);
        std::process::exit(0);
    }

    let selected_presets = all_keys
        .iter()
        .map(|key| {
            presets
                .iter()
                .find(|preset| preset.key == *key)
                .cloned()
                .expect("all preset keys are known")
        })
        .collect();

    let quality = cli
        .quality
        .parse::<Bc7Quality>()
        .map_err(|error| anyhow::anyhow!(error))?;
    let (texture_format, srgb, texture_format_label) =
        parse_import_texture_format(&cli.texture_format)?;

    Ok(Config {
        sd_root,
        data_arc,
        layout_arc,
        base_unpacked,
        base_work_dir,
        output_dir,
        nro,
        active_skin: cli.active_skin,
        selected_presets,
        quality,
        texture_format,
        srgb,
        texture_format_label,
        align: parse_optional_u32(cli.align.as_deref())?,
        force: cli.force,
        dry_run: cli.dry_run,
        no_backup: cli.no_backup,
        skip_nro: cli.skip_nro,
        sd_zip: cli.sd_zip,
        no_sd_stage: cli.no_sd_stage,
        stage_target: cli.stage_target,
        no_stage_target: cli.no_stage_target,
    })
}

fn available_presets(switch_dir: &Path, gamecube_dir: &Path) -> Vec<SkinPreset> {
    vec![
        SkinPreset {
            key: PresetKey::SwitchProAlt,
            key_name: "switch-pro-alt",
            skin_name: "switch_pro_alt_builtin",
            section: "switch",
            pane_prefix: "sgpo_alt",
            expected_layout_flavor:
                "switch_pro_alt_builtin generated asset panes from the future skin converter",
            skin_dir: switch_dir.to_path_buf(),
            manifest_dir: PathBuf::from("target/skin-build/switch-pro-alt"),
            report: PathBuf::from("target/skin-analysis/switch-pro-alt.md"),
            validate_builtin: true,
        },
        SkinPreset {
            key: PresetKey::GamecubeTron,
            key_name: "gamecube-tron",
            skin_name: "gamecube_tron_builtin",
            section: "gamecube",
            pane_prefix: "sgpo_gct",
            expected_layout_flavor:
                "gamecube_tron_builtin generated asset panes from the future skin converter",
            skin_dir: gamecube_dir.to_path_buf(),
            manifest_dir: PathBuf::from("target/skin-build/gamecube-tron"),
            report: PathBuf::from("target/skin-analysis/gamecube-tron.md"),
            validate_builtin: false,
        },
    ]
}

fn print_presets(presets: &[SkinPreset]) {
    println!("built-in no-asset skins:");
    println!("- {DEFAULT_SIMPLE_SKIN_NAME}: square Switch/Pro/Joy-Con layout");
    println!("- {DEFAULT_SIMPLE_GAMECUBE_SKIN_NAME}: square GameCube layout subset");
    println!();
    println!("available generated skin presets:");
    for preset in presets {
        println!("- {}: {}", preset.key_name, preset.skin_name);
        println!("  section: {}", preset.section);
        println!("  pane prefix: {}", preset.pane_prefix);
        println!("  default dir: {}", preset.skin_dir.display());
    }
}

fn validate_request(config: &Config) -> Result<()> {
    if let Some(sd_root) = &config.sd_root {
        require_path(sd_root, "SD root")?;
    }
    if let Some(base_unpacked) = &config.base_unpacked {
        require_path(base_unpacked, "base unpacked info_melee layout directory")?;
    } else if let Some(layout_arc) = &config.layout_arc {
        require_path(layout_arc, "info_melee layout.arc")?;
    } else if let Some(data_arc) = &config.data_arc {
        require_path(data_arc, "Smash data.arc")?;
    } else {
        bail!("provide --data-arc, --layout-arc, or --base-unpacked");
    }

    if !config.skip_nro {
        require_path(&config.nro, "built SGPO NRO")?;
    }
    for preset in &config.selected_presets {
        require_path(&preset.skin_dir.join("skin.xml"), "RetroSpy skin.xml")?;
    }

    let selected_names: HashSet<&str> = config
        .selected_presets
        .iter()
        .map(|preset| preset.skin_name)
        .collect();
    let built_in_names = [
        AUTO_SKIN_NAME,
        DEFAULT_SIMPLE_SKIN_NAME,
        DEFAULT_SIMPLE_GAMECUBE_SKIN_NAME,
        "minimal_debug",
    ];
    if !built_in_names
        .iter()
        .any(|name| config.active_skin.eq_ignore_ascii_case(name))
        && !selected_names
            .iter()
            .any(|name| config.active_skin.eq_ignore_ascii_case(name))
    {
        bail!(
            "active skin '{}' is not included in the generated layout",
            config.active_skin
        );
    }

    Ok(())
}

fn print_install_plan(config: &Config) {
    let (switch_default, gamecube_default) = default_skin_names(&config.selected_presets);
    println!("SGPO Rust installer plan");
    println!(
        "  SD root:        {}",
        optional_path(config.sd_root.as_deref())
    );
    println!(
        "  data.arc:       {}",
        optional_path(config.data_arc.as_deref())
    );
    println!(
        "  layout.arc:     {}",
        optional_path(config.layout_arc.as_deref())
    );
    println!(
        "  base layout:    {}",
        optional_path(config.base_unpacked.as_deref())
    );
    println!("  base work dir:  {}", config.base_work_dir.display());
    println!("  output dir:     {}", config.output_dir.display());
    println!(
        "  NRO:            {}",
        if config.skip_nro {
            "<skipped>".to_string()
        } else {
            config.nro.display().to_string()
        }
    );
    println!("  active skin:    {}", config.active_skin);
    println!("  switch default: {switch_default}");
    println!("  GC default:     {gamecube_default}");
    println!("  texture format: {}", config.texture_format_label);
    println!(
        "  SD ZIP:         {}",
        optional_path(config.sd_zip.as_deref())
    );
    println!(
        "  SD stage:       {}",
        if config.should_stage_sd() {
            "yes"
        } else {
            "no"
        }
    );
    println!(
        "  backup:         {}",
        yes_no(!config.no_backup && config.should_stage_sd())
    );
    println!("  dry run:        {}", yes_no(config.dry_run));
    println!("  generated skins:");
    if config.selected_presets.is_empty() {
        println!("    - <none>; using built-in square default_simple skins");
    } else {
        for preset in &config.selected_presets {
            println!(
                "    - {}: {} from {}",
                preset.key_name,
                preset.skin_name,
                preset.skin_dir.display()
            );
        }
    }
}

fn print_install_result(config: &Config) {
    if config.should_stage_sd() {
        println!(
            "installed SGPO skin pack with active skin '{}' to {}",
            config.active_skin,
            config.sd_root.as_ref().expect("staged SD root").display()
        );
    }
    if let Some(sd_zip) = &config.sd_zip {
        println!(
            "wrote SGPO SD archive with active skin '{}' to {}",
            config.active_skin,
            sd_zip.display()
        );
    }
}

fn optional_path(path: Option<&Path>) -> String {
    path.map(|path| path.display().to_string())
        .unwrap_or_else(|| "<not used>".to_string())
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn default_skin_names(presets: &[SkinPreset]) -> (&'static str, &'static str) {
    let switch_preset = presets
        .iter()
        .find(|preset| preset.key == PresetKey::SwitchProAlt);
    let gamecube_preset = presets
        .iter()
        .find(|preset| preset.key == PresetKey::GamecubeTron);
    (
        switch_preset
            .map(|preset| preset.skin_name)
            .unwrap_or(DEFAULT_SIMPLE_SKIN_NAME),
        gamecube_preset
            .map(|preset| preset.skin_name)
            .unwrap_or(DEFAULT_SIMPLE_GAMECUBE_SKIN_NAME),
    )
}

fn prepare_base_unpacked(config: &Config) -> Result<PathBuf> {
    let original_layout = config.base_work_dir.join("original/layout.arc");
    let original_unpacked = config.base_work_dir.join("original/unpacked");
    let patched_unpacked = config.base_work_dir.join("patched/unpacked");

    if config.base_work_dir.exists() {
        if !config.force {
            if patched_unpacked.exists() {
                println!(
                    "reusing prepared base layout -> {}",
                    patched_unpacked.display()
                );
                return Ok(patched_unpacked);
            }
            bail!(
                "base work dir already exists: {} (pass --force to replace)",
                config.base_work_dir.display()
            );
        }
        fs::remove_dir_all(&config.base_work_dir)
            .with_context(|| format!("removing {}", config.base_work_dir.display()))?;
    }

    if let Some(parent) = original_layout.parent() {
        fs::create_dir_all(parent)?;
    }

    if let Some(layout_arc) = &config.layout_arc {
        fs::copy(layout_arc, &original_layout).with_context(|| {
            format!(
                "copying {} -> {}",
                layout_arc.display(),
                original_layout.display()
            )
        })?;
    } else if let Some(data_arc) = &config.data_arc {
        extract_info_melee_layout_arc(data_arc, &original_layout)?;
    } else {
        bail!("internal error: no base layout source selected");
    }

    let layout_bytes = fs::read(&original_layout)?;
    let count = sarc::unpack_to_dir(&layout_bytes, &original_unpacked)?;
    println!(
        "unpacked {} files -> {}",
        count,
        original_unpacked.display()
    );

    copy_dir_all(&original_unpacked, &patched_unpacked)?;
    patch_info_melee_layout(&patched_unpacked)?;
    Ok(patched_unpacked)
}

fn extract_info_melee_layout_arc(data_arc: &Path, output_layout: &Path) -> Result<()> {
    let arc = ArcFile::open(data_arc)?;
    let layout = arc.get_file_contents(INFO_MELEE_LAYOUT_ARC_PATH, Region::UsEnglish)?;
    if let Some(parent) = output_layout.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output_layout, layout)?;
    println!(
        "extracted {INFO_MELEE_LAYOUT_ARC_PATH} from {} -> {}",
        data_arc.display(),
        output_layout.display()
    );
    Ok(())
}

fn patch_info_melee_layout(unpacked: &Path) -> Result<()> {
    patch_bflyt(
        &unpacked.join(ROOT_BFLYT),
        ROOT_MARKER_TEMPLATE,
        None,
        "root HUD",
    )?;
    for bflyt in PLAYER_PARTS_BFLYTS {
        patch_bflyt(
            &unpacked.join(bflyt),
            PLAYER_MARKER_TEMPLATE,
            Some(PLAYER_MATERIAL_SOURCE_PANE),
            "player parts",
        )?;
    }
    Ok(())
}

fn patch_bflyt(
    path: &Path,
    marker_template: &str,
    material_source_pane: Option<&str>,
    label: &str,
) -> Result<()> {
    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let mut bflyt = read_bflyt(&bytes).with_context(|| format!("parsing {}", path.display()))?;

    let expected: Vec<&str> = std::iter::once(SGPO_ROOT)
        .chain(PANE_SPECS.iter().map(|spec| spec.name))
        .collect();
    let present = expected
        .iter()
        .filter(|name| bflyt.pane_exists(name))
        .count();
    if present == expected.len() {
        println!(
            "all SGPO Pro Controller panes already present; leaving {} unchanged",
            path.display()
        );
        return Ok(());
    }
    if present != 0 {
        bail!(
            "{} already contains a partial SGPO patch; use an unpatched source",
            path.display()
        );
    }

    let marker_material = if let Some(source_pane) = material_source_pane {
        Some(
            pane_material_name(&bflyt, source_pane)
                .with_context(|| format!("finding material for pane '{source_pane}'"))?,
        )
    } else {
        None
    };

    bflyt.clone_pane(&ClonePaneSpec {
        template: "RootPane".to_string(),
        new_name: SGPO_ROOT.to_string(),
        parent: Some("RootPane".to_string()),
        translate_x: Some(SGPO_ROOT_POS.0),
        translate_y: Some(SGPO_ROOT_POS.1),
        translate_z: Some(SGPO_ROOT_POS.2),
        width: Some(SGPO_ROOT_SIZE.0),
        height: Some(SGPO_ROOT_SIZE.1),
        alpha: Some(INITIAL_PANE_ALPHA),
        visible: Some(true),
        bind_material: None,
    })?;

    for spec in PANE_SPECS {
        bflyt.clone_pane(&ClonePaneSpec {
            template: marker_template.to_string(),
            new_name: spec.name.to_string(),
            parent: Some(SGPO_ROOT.to_string()),
            translate_x: Some(spec.x),
            translate_y: Some(spec.y),
            translate_z: Some(0.0),
            width: Some(spec.width),
            height: Some(spec.height),
            alpha: Some(INITIAL_PANE_ALPHA),
            visible: Some(true),
            bind_material: marker_material.clone(),
        })?;
    }

    let written = write_bflyt(&bflyt).with_context(|| format!("writing {}", path.display()))?;
    fs::write(path, written)?;
    println!(
        "inserted {SGPO_ROOT} with {} visual panes into {} ({label})",
        PANE_SPECS.len(),
        path.display()
    );
    Ok(())
}

fn pane_material_name(bflyt: &BFLYT, pane_name: &str) -> Result<String> {
    let pane = bflyt
        .find_pane(pane_name)
        .with_context(|| format!("pane '{pane_name}' not found"))?;
    let material_index = pane
        .picture
        .as_ref()
        .map(|picture| picture.material_index as usize)
        .or_else(|| pane.text.as_ref().map(|text| text.material_index as usize))
        .with_context(|| format!("pane '{pane_name}' is not material-backed"))?;
    let material = bflyt
        .materials
        .get(material_index)
        .with_context(|| format!("material index {material_index} is out of range"))?;
    Ok(material.name.clone())
}

fn generate_manifest(preset: &SkinPreset) -> Result<GeneratedManifest> {
    let skin_xml = preset.skin_dir.join("skin.xml");
    let (metadata, controls) = parse_retrospy_skin(&skin_xml, preset.section)?;
    let manifest = build_manifest(preset, &skin_xml, &metadata, &controls);
    let validation_errors = if preset.validate_builtin {
        validate_switch_pro_alt_builtin(&manifest)
    } else {
        Vec::new()
    };

    fs::create_dir_all(&preset.manifest_dir)?;
    fs::create_dir_all(
        preset
            .report
            .parent()
            .unwrap_or_else(|| Path::new("target/skin-analysis")),
    )?;
    fs::write(
        preset.manifest_dir.join("skin_manifest.json"),
        serde_json::to_string_pretty(&manifest)? + "\n",
    )?;
    fs::write(
        preset.manifest_dir.join("skin_manifest.md"),
        manifest_markdown(&manifest, &validation_errors, preset.validate_builtin),
    )?;
    fs::write(
        &preset.report,
        analysis_report(
            &skin_xml,
            &metadata,
            &controls,
            &manifest,
            &validation_errors,
        ),
    )?;

    println!("Wrote {}", preset.report.display());
    println!(
        "Wrote {}",
        preset.manifest_dir.join("skin_manifest.json").display()
    );
    println!(
        "Wrote {}",
        preset.manifest_dir.join("skin_manifest.md").display()
    );
    let mapped = controls
        .iter()
        .filter(|control| control.control_id.is_some())
        .count();
    println!(
        "Parsed {} controls ({} mapped, {} unsupported/unmapped) from {}",
        controls.len(),
        mapped,
        controls.len() - mapped,
        skin_xml.display()
    );
    if preset.validate_builtin {
        if validation_errors.is_empty() {
            println!("Generated manifest exactly matches switch_pro_alt_builtin");
        } else {
            bail!(
                "generated manifest does not match switch_pro_alt_builtin:\n{}",
                validation_errors.join("\n")
            );
        }
    } else {
        println!("Generated manifest without built-in comparison");
    }

    Ok(manifest)
}

fn parse_retrospy_skin(
    skin_xml: &Path,
    selected_section: &str,
) -> Result<(SkinMetadata, Vec<ParsedControl>)> {
    let xml =
        fs::read_to_string(skin_xml).with_context(|| format!("reading {}", skin_xml.display()))?;
    let skin_dir = skin_xml
        .parent()
        .with_context(|| format!("{} has no parent directory", skin_xml.display()))?;
    let root_attrs = scan_tags(&xml, "skin")
        .into_iter()
        .next()
        .map(|tag| tag.attrs)
        .unwrap_or_default();
    let background_tag = scan_tags(&xml, "background")
        .into_iter()
        .next()
        .context("skin.xml is missing <background>")?;
    let background_image = background_tag
        .attrs
        .get("image")
        .cloned()
        .context("<background> is missing image=")?;
    let (background_width, background_height) =
        image::image_dimensions(skin_dir.join(&background_image)).with_context(|| {
            format!("reading background image dimensions for {background_image}")
        })?;
    let metadata = SkinMetadata {
        name: root_attrs.get("name").cloned().unwrap_or_default(),
        author: root_attrs.get("author").cloned().unwrap_or_default(),
        types: root_attrs
            .get("type")
            .map(|value| {
                value
                    .split(';')
                    .filter(|part| !part.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
        background_image: background_image.clone(),
        background_width,
        background_height,
    };

    let mut controls = vec![ParsedControl {
        kind: ControlKind::Background,
        section: "skin".to_string(),
        retrospy_name: background_tag
            .attrs
            .get("name")
            .cloned()
            .unwrap_or_else(|| "background".to_string()),
        control_id: Some("SkinBackground".to_string()),
        image: background_image,
        x: 0.0,
        y: 0.0,
        width: background_width as f32,
        height: background_height as f32,
        base_x: 0.0,
        base_y: 0.0,
        movement_x: None,
        movement_y: None,
    }];

    let mut section_ranges = Vec::new();
    for section in scan_sections(&xml) {
        section_ranges.push((section.start, section.end));
        let section_types = section
            .attrs
            .get("type")
            .map(|value| value.split(';').collect::<HashSet<_>>())
            .unwrap_or_default();
        if !section_types.contains(selected_section) {
            continue;
        }
        let section_map = section_button_map(selected_section);
        let inner = section.inner.unwrap_or_default();
        for button in scan_tags(&inner, "button") {
            let name = button.attrs.get("name").cloned().unwrap_or_default();
            controls.push(parsed_button(
                &button.attrs,
                selected_section,
                section_map.get(name.as_str()).copied(),
                background_width,
                background_height,
            )?);
        }
    }

    let root_without_sections = remove_ranges(&xml, &section_ranges);
    let root_map = section_button_map(selected_section);
    for button in scan_tags(&root_without_sections, "button") {
        let name = button.attrs.get("name").cloned().unwrap_or_default();
        let control_id =
            global_button_map(name.as_str()).or_else(|| root_map.get(name.as_str()).copied());
        controls.push(parsed_button(
            &button.attrs,
            "global",
            control_id,
            background_width,
            background_height,
        )?);
    }
    for stick in scan_tags(&root_without_sections, "stick") {
        controls.push(parsed_stick(
            &stick.attrs,
            "global",
            background_width,
            background_height,
        )?);
    }

    Ok((metadata, controls))
}

fn parsed_button(
    attrs: &HashMap<String, String>,
    section: &str,
    control_id: Option<&'static str>,
    background_width: u32,
    background_height: u32,
) -> Result<ParsedControl> {
    let (x, y, width, height) = rect_attrs(attrs)?;
    let (base_x, base_y) =
        centered_position(x, y, width, height, background_width, background_height);
    Ok(ParsedControl {
        kind: ControlKind::Button,
        section: section.to_string(),
        retrospy_name: attrs.get("name").cloned().unwrap_or_default(),
        control_id: control_id.map(str::to_string),
        image: attrs.get("image").cloned().unwrap_or_default(),
        x,
        y,
        width,
        height,
        base_x,
        base_y,
        movement_x: None,
        movement_y: None,
    })
}

fn parsed_stick(
    attrs: &HashMap<String, String>,
    section: &str,
    background_width: u32,
    background_height: u32,
) -> Result<ParsedControl> {
    let (x, y, width, height) = rect_attrs(attrs)?;
    let xname = attrs.get("xname").map(String::as_str).unwrap_or_default();
    let yname = attrs.get("yname").map(String::as_str).unwrap_or_default();
    let (base_x, base_y) =
        centered_position(x, y, width, height, background_width, background_height);
    Ok(ParsedControl {
        kind: ControlKind::Stick,
        section: section.to_string(),
        retrospy_name: format!("{xname}/{yname}"),
        control_id: stick_map(xname, yname).map(str::to_string),
        image: attrs.get("image").cloned().unwrap_or_default(),
        x,
        y,
        width,
        height,
        base_x,
        base_y,
        movement_x: optional_f32_attr(attrs, "xrange")?,
        movement_y: optional_f32_attr(attrs, "yrange")?,
    })
}

fn rect_attrs(attrs: &HashMap<String, String>) -> Result<(f32, f32, f32, f32)> {
    Ok((
        required_f32_attr(attrs, "x")?,
        required_f32_attr(attrs, "y")?,
        required_f32_attr(attrs, "width")?,
        required_f32_attr(attrs, "height")?,
    ))
}

fn centered_position(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    background_width: u32,
    background_height: u32,
) -> (f32, f32) {
    (
        x + width / 2.0 - background_width as f32 / 2.0,
        background_height as f32 / 2.0 - (y + height / 2.0),
    )
}

fn build_manifest(
    preset: &SkinPreset,
    skin_xml: &Path,
    metadata: &SkinMetadata,
    controls: &[ParsedControl],
) -> GeneratedManifest {
    let pane_overrides = pane_overrides(preset.key);
    let mut mapped: Vec<_> = controls
        .iter()
        .filter(|control| control.control_id.is_some())
        .cloned()
        .collect();
    mapped.sort_by_key(|control| control_sort_key(control.control_id.as_deref().unwrap_or("")));

    let elements = mapped
        .iter()
        .map(|control| {
            let control_id = control.control_id.as_ref().expect("mapped control");
            let pane_name = pane_overrides
                .get(control_id.as_str())
                .cloned()
                .unwrap_or_else(|| suggested_pane_name(control, preset.pane_prefix));
            let defaults = defaults_for(control.kind);
            GeneratedElement {
                control_id: control_id.clone(),
                pane_name: pane_name.clone(),
                image_filename: control.image.clone(),
                material_name: format!("mat_{pane_name}"),
                source: ElementSource {
                    retrospy_kind: match control.kind {
                        ControlKind::Background => "background",
                        ControlKind::Button => "button",
                        ControlKind::Stick => "stick",
                    }
                    .to_string(),
                    retrospy_section: control.section.clone(),
                    retrospy_name: control.retrospy_name.clone(),
                    x: control.x,
                    y: control.y,
                },
                base_x: control.base_x,
                base_y: control.base_y,
                width: control.width,
                height: control.height,
                released_alpha: defaults.released_alpha,
                pressed_alpha: defaults.pressed_alpha,
                released_scale: defaults.released_scale,
                pressed_scale: defaults.pressed_scale,
                stick_movement: control
                    .movement_x
                    .zip(control.movement_y)
                    .map(|(x, y)| StickMovement { x, y }),
            }
        })
        .collect();

    GeneratedManifest {
        schema_version: 1,
        skin_name: preset.skin_name.to_string(),
        root_pane_name: SGPO_ROOT.to_string(),
        expected_layout_flavor: preset.expected_layout_flavor.to_string(),
        source: ManifestSource {
            skin_xml: skin_xml.display().to_string(),
            retrospy_skin_name: metadata.name.clone(),
            retrospy_author: metadata.author.clone(),
            retrospy_types: metadata.types.clone(),
            section: preset.section.to_string(),
            background_image: metadata.background_image.clone(),
            background_width: metadata.background_width,
            background_height: metadata.background_height,
        },
        coordinate_space: CoordinateSpace {
            origin: "background_center",
            base_x_formula: "x + width / 2 - background_width / 2",
            base_y_formula: "background_height / 2 - (y + height / 2)",
        },
        defaults: ManifestDefaults {
            image_static: defaults_for(ControlKind::Background),
            image_button: defaults_for(ControlKind::Button),
            image_stick: defaults_for(ControlKind::Stick),
        },
        elements,
    }
}

fn defaults_for(kind: ControlKind) -> ElementDefaults {
    match kind {
        ControlKind::Background => ElementDefaults {
            released_alpha: IMAGE_STATIC_ALPHA,
            pressed_alpha: IMAGE_STATIC_ALPHA,
            released_scale: RELEASED_SCALE,
            pressed_scale: RELEASED_SCALE,
        },
        ControlKind::Button => ElementDefaults {
            released_alpha: IMAGE_BUTTON_RELEASED_ALPHA,
            pressed_alpha: IMAGE_BUTTON_PRESSED_ALPHA,
            released_scale: RELEASED_SCALE,
            pressed_scale: BUTTON_PRESSED_SCALE,
        },
        ControlKind::Stick => ElementDefaults {
            released_alpha: IMAGE_STICK_ALPHA,
            pressed_alpha: IMAGE_STICK_ALPHA,
            released_scale: RELEASED_SCALE,
            pressed_scale: RELEASED_SCALE,
        },
    }
}

fn build_layout(
    config: &Config,
    base_unpacked: &Path,
    manifests: &[(SkinPreset, GeneratedManifest)],
) -> Result<()> {
    let output_unpacked = config.output_dir.join("unpacked");
    let output_layout = config.output_dir.join("layout.arc");

    require_path(&base_unpacked.join(ROOT_BFLYT), "base root BFLYT")?;
    for bflyt in PLAYER_PARTS_BFLYTS {
        require_path(&base_unpacked.join(bflyt), "base player-parts BFLYT")?;
    }
    require_path(&base_unpacked.join("timg/__Combined.bntx"), "base BNTX")?;

    if output_unpacked.exists() {
        if !config.force {
            bail!(
                "output already exists: {} (pass --force to replace)",
                output_unpacked.display()
            );
        }
        fs::remove_dir_all(&output_unpacked)?;
    }
    fs::create_dir_all(&config.output_dir)?;
    copy_dir_all(base_unpacked, &output_unpacked)?;

    for (preset, manifest) in manifests {
        println!(
            "building skin '{}' from {}",
            manifest.skin_name,
            preset.manifest_dir.join("skin_manifest.json").display()
        );
        let prepared_skin_dir = prepare_skin_assets(preset, manifest, &config.output_dir)?;
        apply_generated_skin(config, &output_unpacked, manifest, &prepared_skin_dir)?;
    }

    if output_layout.exists() {
        fs::remove_file(&output_layout)?;
    }
    let packed = sarc::pack_directory(&output_unpacked)?;
    fs::write(&output_layout, packed)?;
    println!("built layout -> {}", output_layout.display());

    if !config.no_stage_target {
        let staged = copy_layout(&output_layout, &config.stage_target)?;
        println!("staged local ARCropolis layout -> {}", staged.display());
    }

    let (switch_default, gamecube_default) = default_skin_names(&config.selected_presets);
    let config_bytes = skin_config_bytes(&config.active_skin, switch_default, gamecube_default)?;

    if config.should_stage_sd() {
        let sd_root = config.sd_root.as_ref().expect("checked should_stage_sd");
        let sd_mod_root = sd_root.join("ultimate/mods").join(MOD_FOLDER_NAME);
        let staged = copy_layout(&output_layout, &sd_mod_root)?;
        println!("staged SD layout -> {}", staged.display());
        let config_path = write_skin_config_bytes(&sd_mod_root, &config_bytes)?;
        println!("wrote SD skin config -> {}", config_path.display());
    }

    if let Some(sd_zip) = &config.sd_zip {
        write_sd_archive(config, &output_layout, &config_bytes, sd_zip)?;
        println!("wrote SD archive -> {}", sd_zip.display());
    }

    Ok(())
}

fn apply_generated_skin(
    config: &Config,
    output_unpacked: &Path,
    manifest: &GeneratedManifest,
    prepared_skin_dir: &Path,
) -> Result<()> {
    let mut nx_manifest = to_nx_manifest(manifest)?;
    for element in &mut nx_manifest.elements {
        element.released_alpha = 0;
    }

    let opts = ApplyOptions {
        bflyt_rel: ROOT_BFLYT.to_string(),
        bntx_rel: "timg/__Combined.bntx".to_string(),
        pane_template: GENERATED_PANE_TEMPLATE.to_string(),
        material_template: GENERATED_MATERIAL_TEMPLATE.to_string(),
        quality: config.quality,
        srgb: config.srgb,
        align: config.align,
        texture_format: config.texture_format,
        skip_existing: false,
    };
    let report = apply_manifest(output_unpacked, &nx_manifest, prepared_skin_dir, &opts)?;
    println!(
        "applied {} element(s); skipped {}; BFLYT now {} bytes; BNTX now {} bytes",
        report.applied, report.skipped, report.bflyt_bytes, report.bntx_bytes
    );

    for bflyt in PLAYER_PARTS_BFLYTS {
        let opts = ApplyOptions {
            bflyt_rel: bflyt.to_string(),
            bntx_rel: "timg/__Combined.bntx".to_string(),
            pane_template: GENERATED_PANE_TEMPLATE.to_string(),
            material_template: GENERATED_MATERIAL_TEMPLATE.to_string(),
            quality: config.quality,
            srgb: config.srgb,
            align: config.align,
            texture_format: config.texture_format,
            skip_existing: true,
        };
        let report = apply_manifest(output_unpacked, &nx_manifest, prepared_skin_dir, &opts)?;
        println!(
            "added {} manifest pane(s) to {}; skipped {}",
            report.applied, bflyt, report.skipped
        );
    }

    for bflyt in std::iter::once(ROOT_BFLYT).chain(PLAYER_PARTS_BFLYTS) {
        let opts = ValidateOptions {
            bflyt_rel: bflyt.to_string(),
            bntx_rel: "timg/__Combined.bntx".to_string(),
            strict_dimensions: false,
        };
        let report = validate_manifest(output_unpacked, &nx_manifest, &opts)?;
        if !report.all_passed() {
            let failures = report
                .results
                .into_iter()
                .filter(|result| !result.ok)
                .map(|result| format!("{}: {}", result.pane_name, result.failures.join("; ")))
                .collect::<Vec<_>>()
                .join("\n");
            bail!("manifest validation failed for {bflyt}:\n{failures}");
        }
        println!("validated {} manifest pane(s) in {}", report.passed, bflyt);
    }

    Ok(())
}

fn to_nx_manifest(manifest: &GeneratedManifest) -> Result<nx_manifest::SkinManifest> {
    let value = serde_json::to_value(manifest)?;
    Ok(serde_json::from_value(value)?)
}

fn prepare_skin_assets(
    preset: &SkinPreset,
    manifest: &GeneratedManifest,
    output_dir: &Path,
) -> Result<PathBuf> {
    let prepared_skin_dir = output_dir.join("skin-assets").join(&manifest.skin_name);
    if prepared_skin_dir.exists() {
        fs::remove_dir_all(&prepared_skin_dir)?;
    }
    copy_dir_all(&preset.skin_dir, &prepared_skin_dir)?;
    make_tree_writable(&prepared_skin_dir)?;

    if preset.key == PresetKey::SwitchProAlt {
        for element in &manifest.elements {
            if element.control_id == "SkinBackground" {
                chroma_key_green_to_alpha(&prepared_skin_dir.join(&element.image_filename))?;
            }
        }
    }

    Ok(prepared_skin_dir)
}

fn chroma_key_green_to_alpha(path: &Path) -> Result<()> {
    let image = image::open(path)
        .with_context(|| format!("opening {} for chroma-key processing", path.display()))?;
    let (width, height) = image.dimensions();
    let mut rgba = image.to_rgba8();
    let mut changed = false;
    for pixel in rgba.pixels_mut() {
        let [red, green, blue, _alpha] = pixel.0;
        if is_chroma_green(red, green, blue) {
            pixel.0[3] = 0;
            changed = true;
        }
    }

    if changed {
        bleed_transparent_pixel_rgb(&mut rgba, width, height, 6);
        for pixel in rgba.pixels_mut() {
            let [red, green, blue, alpha] = pixel.0;
            if alpha == 0 && is_chroma_green(red, green, blue) {
                pixel.0[0] = 0;
                pixel.0[1] = 0;
                pixel.0[2] = 0;
            }
        }
        DynamicImage::ImageRgba8(rgba).save(path)?;
    }
    Ok(())
}

fn is_chroma_green(red: u8, green: u8, blue: u8) -> bool {
    green >= 110 && green.saturating_sub(red.max(blue)) >= 35
}

fn bleed_transparent_pixel_rgb(
    image: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    width: u32,
    height: u32,
    passes: usize,
) {
    for _ in 0..passes {
        let mut updates = Vec::new();
        for y in 0..height {
            for x in 0..width {
                let pixel = image.get_pixel(x, y);
                if pixel.0[3] != 0 {
                    continue;
                }
                let colors = neighboring_opaque_colors(image, width, height, x, y);
                if colors.is_empty() {
                    continue;
                }
                let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
                for [red, green, blue] in &colors {
                    r += *red as u32;
                    g += *green as u32;
                    b += *blue as u32;
                }
                let count = colors.len() as u32;
                updates.push((
                    x,
                    y,
                    [(r / count) as u8, (g / count) as u8, (b / count) as u8],
                ));
            }
        }
        if updates.is_empty() {
            break;
        }
        for (x, y, [red, green, blue]) in updates {
            let pixel = image.get_pixel_mut(x, y);
            pixel.0[0] = red;
            pixel.0[1] = green;
            pixel.0[2] = blue;
        }
    }
}

fn neighboring_opaque_colors(
    image: &ImageBuffer<Rgba<u8>, Vec<u8>>,
    width: u32,
    height: u32,
    x: u32,
    y: u32,
) -> Vec<[u8; 3]> {
    let mut colors = Vec::new();
    for ny in y.saturating_sub(1)..=(y + 1).min(height - 1) {
        for nx in x.saturating_sub(1)..=(x + 1).min(width - 1) {
            if nx == x && ny == y {
                continue;
            }
            let pixel = image.get_pixel(nx, ny).0;
            if pixel[3] != 0 {
                colors.push([pixel[0], pixel[1], pixel[2]]);
            }
        }
    }
    colors
}

fn copy_layout(layout: &Path, mod_root: &Path) -> Result<PathBuf> {
    let staged_layout = mod_root.join(INFO_MELEE_LAYOUT_ARC_PATH);
    if let Some(parent) = staged_layout.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(layout, &staged_layout).with_context(|| {
        format!(
            "copying {} -> {}",
            layout.display(),
            staged_layout.display()
        )
    })?;
    Ok(staged_layout)
}

fn skin_config_bytes(
    active_skin: &str,
    default_switch_skin: &str,
    default_gamecube_skin: &str,
) -> Result<Vec<u8>> {
    let config = serde_json::json!({
        "active_skin": active_skin,
        "default_skins": {
            "switch": default_switch_skin,
            "gamecube": default_gamecube_skin,
        }
    });
    Ok((serde_json::to_string_pretty(&config)? + "\n").into_bytes())
}

fn write_skin_config_bytes(mod_root: &Path, config_bytes: &[u8]) -> Result<PathBuf> {
    let config_path = mod_root.join("config.json");
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&config_path, config_bytes)?;
    Ok(config_path)
}

fn stage_nro(config: &Config) -> Result<()> {
    let sd_root = config
        .sd_root
        .as_ref()
        .context("cannot stage NRO without an SD root")?;
    let plugin_dir = config
        .sd_root
        .as_ref()
        .unwrap_or(sd_root)
        .join("atmosphere/contents")
        .join(SMASH_TITLE_ID)
        .join("romfs/skyline/plugins");
    fs::create_dir_all(&plugin_dir)?;
    let dest = plugin_dir.join(
        config
            .nro
            .file_name()
            .context("NRO path has no file name")?,
    );
    fs::copy(&config.nro, &dest)
        .with_context(|| format!("copying {} -> {}", config.nro.display(), dest.display()))?;
    println!("staged NRO -> {}", dest.display());
    Ok(())
}

fn backup_installed_files(config: &Config) -> Result<()> {
    let timestamp = timestamp();
    let nro_name = config
        .nro
        .file_name()
        .context("NRO path has no file name")?;
    let sd_root = config
        .sd_root
        .as_ref()
        .context("cannot back up installed files without an SD root")?;
    let paths = [
        sd_root
            .join("ultimate/mods")
            .join(MOD_FOLDER_NAME)
            .join(INFO_MELEE_LAYOUT_ARC_PATH),
        sd_root
            .join("ultimate/mods")
            .join(MOD_FOLDER_NAME)
            .join("config.json"),
        sd_root
            .join("atmosphere/contents")
            .join(SMASH_TITLE_ID)
            .join("romfs/skyline/plugins")
            .join(nro_name),
    ];
    for path in paths {
        if !path.exists() {
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .context("backup path has invalid file name")?;
        let backup = path.with_file_name(format!("{file_name}.bak.{timestamp}"));
        fs::copy(&path, &backup)
            .with_context(|| format!("backing up {} -> {}", path.display(), backup.display()))?;
        println!("backed up {} -> {}", path.display(), backup.display());
    }
    Ok(())
}

struct ZipEntryInput {
    name: String,
    bytes: Vec<u8>,
}

struct ZipCentralEntry {
    name: String,
    crc32: u32,
    size: u32,
    local_header_offset: u32,
}

fn write_sd_archive(config: &Config, layout: &Path, config_bytes: &[u8], out: &Path) -> Result<()> {
    let mut entries = vec![
        ZipEntryInput {
            name: sd_layout_archive_path(),
            bytes: fs::read(layout).with_context(|| format!("reading {}", layout.display()))?,
        },
        ZipEntryInput {
            name: sd_config_archive_path(),
            bytes: config_bytes.to_vec(),
        },
    ];

    if !config.skip_nro {
        let nro_name = config
            .nro
            .file_name()
            .and_then(|name| name.to_str())
            .context("NRO path has invalid file name")?;
        entries.push(ZipEntryInput {
            name: sd_nro_archive_path(nro_name),
            bytes: fs::read(&config.nro)
                .with_context(|| format!("reading {}", config.nro.display()))?,
        });
    }

    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    write_stored_zip(out, &entries)
}

fn sd_layout_archive_path() -> String {
    format!("ultimate/mods/{MOD_FOLDER_NAME}/{INFO_MELEE_LAYOUT_ARC_PATH}")
}

fn sd_config_archive_path() -> String {
    format!("ultimate/mods/{MOD_FOLDER_NAME}/config.json")
}

fn sd_nro_archive_path(nro_name: &str) -> String {
    format!("atmosphere/contents/{SMASH_TITLE_ID}/romfs/skyline/plugins/{nro_name}")
}

fn write_stored_zip(out: &Path, entries: &[ZipEntryInput]) -> Result<()> {
    let mut file = fs::File::create(out).with_context(|| format!("creating {}", out.display()))?;
    let mut central_entries = Vec::with_capacity(entries.len());

    for entry in entries {
        let name_bytes = entry.name.as_bytes();
        if name_bytes.len() > u16::MAX as usize {
            bail!("ZIP entry name is too long: {}", entry.name);
        }
        if entry.bytes.len() > u32::MAX as usize {
            bail!(
                "ZIP entry is too large for non-Zip64 output: {}",
                entry.name
            );
        }
        let offset = file.stream_position()?;
        if offset > u32::MAX as u64 {
            bail!("ZIP output is too large for non-Zip64 central directory");
        }
        let crc32 = crc32(&entry.bytes);
        let size = entry.bytes.len() as u32;

        write_u32_le(&mut file, 0x0403_4b50)?;
        write_u16_le(&mut file, 20)?;
        write_u16_le(&mut file, 0)?;
        write_u16_le(&mut file, 0)?;
        write_u16_le(&mut file, ZIP_MOD_TIME)?;
        write_u16_le(&mut file, ZIP_MOD_DATE)?;
        write_u32_le(&mut file, crc32)?;
        write_u32_le(&mut file, size)?;
        write_u32_le(&mut file, size)?;
        write_u16_le(&mut file, name_bytes.len() as u16)?;
        write_u16_le(&mut file, 0)?;
        file.write_all(name_bytes)?;
        file.write_all(&entry.bytes)?;

        central_entries.push(ZipCentralEntry {
            name: entry.name.clone(),
            crc32,
            size,
            local_header_offset: offset as u32,
        });
    }

    let central_offset = file.stream_position()?;
    if central_offset > u32::MAX as u64 {
        bail!("ZIP output is too large for non-Zip64 central directory");
    }

    for entry in &central_entries {
        let name_bytes = entry.name.as_bytes();
        write_u32_le(&mut file, 0x0201_4b50)?;
        write_u16_le(&mut file, 20)?;
        write_u16_le(&mut file, 20)?;
        write_u16_le(&mut file, 0)?;
        write_u16_le(&mut file, 0)?;
        write_u16_le(&mut file, ZIP_MOD_TIME)?;
        write_u16_le(&mut file, ZIP_MOD_DATE)?;
        write_u32_le(&mut file, entry.crc32)?;
        write_u32_le(&mut file, entry.size)?;
        write_u32_le(&mut file, entry.size)?;
        write_u16_le(&mut file, name_bytes.len() as u16)?;
        write_u16_le(&mut file, 0)?;
        write_u16_le(&mut file, 0)?;
        write_u16_le(&mut file, 0)?;
        write_u16_le(&mut file, 0)?;
        write_u32_le(&mut file, 0)?;
        write_u32_le(&mut file, entry.local_header_offset)?;
        file.write_all(name_bytes)?;
    }

    let central_end = file.stream_position()?;
    let central_size = central_end - central_offset;
    if central_entries.len() > u16::MAX as usize
        || central_size > u32::MAX as u64
        || central_offset > u32::MAX as u64
    {
        bail!("ZIP output is too large for non-Zip64 end-of-central-directory");
    }

    write_u32_le(&mut file, 0x0605_4b50)?;
    write_u16_le(&mut file, 0)?;
    write_u16_le(&mut file, 0)?;
    write_u16_le(&mut file, central_entries.len() as u16)?;
    write_u16_le(&mut file, central_entries.len() as u16)?;
    write_u32_le(&mut file, central_size as u32)?;
    write_u32_le(&mut file, central_offset as u32)?;
    write_u16_le(&mut file, 0)?;
    file.flush()?;
    Ok(())
}

fn write_u16_le(mut writer: impl Write, value: u16) -> Result<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn write_u32_le(mut writer: impl Write, value: u32) -> Result<()> {
    writer.write_all(&value.to_le_bytes())?;
    Ok(())
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in bytes {
        crc ^= *byte as u32;
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format_unix_timestamp(seconds)
}

fn format_unix_timestamp(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let day_seconds = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = day_seconds / 3_600;
    let minute = (day_seconds % 3_600) / 60;
    let second = day_seconds % 60;
    format!("{year:04}{month:02}{day:02}-{hour:02}{minute:02}{second:02}")
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i32, u32, u32) {
    // Howard Hinnant's civil-from-days algorithm, shifted from Unix epoch.
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year as i32, month as u32, day as u32)
}

fn manifest_markdown(
    manifest: &GeneratedManifest,
    validation_errors: &[String],
    validation_enabled: bool,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Skin Manifest: {}\n\n", manifest.skin_name));
    out.push_str("Generated by `tools/sgpo_installer`.\n\n");
    out.push_str("## Source\n\n");
    out.push_str(&format!("- Skin XML: `{}`\n", manifest.source.skin_xml));
    out.push_str(&format!(
        "- RetroSpy skin: `{}`\n",
        manifest.source.retrospy_skin_name
    ));
    out.push_str(&format!("- Section: `{}`\n", manifest.source.section));
    out.push_str(&format!(
        "- Background: `{}` ({}x{})\n",
        manifest.source.background_image,
        manifest.source.background_width,
        manifest.source.background_height
    ));
    out.push_str(&format!("- Root pane: `{}`\n", manifest.root_pane_name));
    out.push_str(&format!(
        "- Expected layout flavor: `{}`\n\n",
        manifest.expected_layout_flavor
    ));
    out.push_str("## Validation\n\n");
    if !validation_enabled {
        out.push_str("- Built-in validation skipped for this generated skin.\n\n");
    } else if validation_errors.is_empty() {
        out.push_str("- Generated manifest exactly matches `switch_pro_alt_builtin`.\n\n");
    } else {
        for error in validation_errors {
            out.push_str(&format!("- Mismatch: {error}\n"));
        }
        out.push('\n');
    }
    out.push_str("## Elements\n\n");
    out.push_str("| ControlId | Pane | Image | Material | Base x/y | Size | Alpha released/pressed | Scale released/pressed | Stick movement |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for element in &manifest.elements {
        let movement = element
            .stick_movement
            .map(|movement| format!("{}, {}", fmt_f32(movement.x), fmt_f32(movement.y)))
            .unwrap_or_else(|| "-".to_string());
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | {}, {} | {} x {} | {} / {} | {} / {} | {} |\n",
            element.control_id,
            element.pane_name,
            element.image_filename,
            element.material_name,
            fmt_f32(element.base_x),
            fmt_f32(element.base_y),
            fmt_f32(element.width),
            fmt_f32(element.height),
            element.released_alpha,
            element.pressed_alpha,
            fmt_f32(element.released_scale),
            fmt_f32(element.pressed_scale),
            movement
        ));
    }
    out
}

fn analysis_report(
    skin_xml: &Path,
    metadata: &SkinMetadata,
    controls: &[ParsedControl],
    manifest: &GeneratedManifest,
    validation_errors: &[String],
) -> String {
    let mapped = controls
        .iter()
        .filter(|control| control.control_id.is_some())
        .count();
    let mut out = String::new();
    out.push_str("# RetroSpy Skin Analysis\n\n");
    out.push_str(&format!("- Skin XML: `{}`\n", skin_xml.display()));
    out.push_str(&format!("- RetroSpy skin: `{}`\n", metadata.name));
    out.push_str(&format!("- Author: `{}`\n", metadata.author));
    out.push_str(&format!(
        "- Background: `{}` ({}x{})\n",
        metadata.background_image, metadata.background_width, metadata.background_height
    ));
    out.push_str(&format!("- Parsed controls: {}\n", controls.len()));
    out.push_str(&format!("- Mapped controls: {mapped}\n"));
    out.push_str(&format!(
        "- Manifest elements: {}\n\n",
        manifest.elements.len()
    ));
    if validation_errors.is_empty() {
        out.push_str("Validation: no mismatches for enabled built-in comparisons.\n\n");
    } else {
        out.push_str("Validation mismatches:\n");
        for error in validation_errors {
            out.push_str(&format!("- {error}\n"));
        }
        out.push('\n');
    }
    out.push_str("## Parsed Controls\n\n");
    out.push_str(
        "| Kind | Section | RetroSpy | ControlId | Image | x/y | Size | Base x/y | Movement |\n",
    );
    out.push_str("| --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for control in controls {
        let movement = control
            .movement_x
            .zip(control.movement_y)
            .map(|(x, y)| format!("{}, {}", fmt_f32(x), fmt_f32(y)))
            .unwrap_or_else(|| "-".to_string());
        out.push_str(&format!(
            "| {} | {} | `{}` | `{}` | `{}` | {}, {} | {} x {} | {}, {} | {} |\n",
            match control.kind {
                ControlKind::Background => "background",
                ControlKind::Button => "button",
                ControlKind::Stick => "stick",
            },
            control.section,
            control.retrospy_name,
            control.control_id.as_deref().unwrap_or("UNMAPPED"),
            control.image,
            fmt_f32(control.x),
            fmt_f32(control.y),
            fmt_f32(control.width),
            fmt_f32(control.height),
            fmt_f32(control.base_x),
            fmt_f32(control.base_y),
            movement
        ));
    }
    out
}

fn validate_switch_pro_alt_builtin(manifest: &GeneratedManifest) -> Vec<String> {
    let expected = switch_pro_alt_expected();
    let manifest_by_control: BTreeMap<&str, &GeneratedElement> = manifest
        .elements
        .iter()
        .map(|element| (element.control_id.as_str(), element))
        .collect();
    let expected_by_control: BTreeMap<&str, &ExpectedElement> = expected
        .iter()
        .map(|element| (element.control_id, element))
        .collect();
    let mut errors = Vec::new();

    for control in manifest_by_control.keys() {
        if !expected_by_control.contains_key(control) {
            errors.push(format!(
                "{control}: present in manifest but missing from built-in"
            ));
        }
    }
    for control in expected_by_control.keys() {
        if !manifest_by_control.contains_key(control) {
            errors.push(format!(
                "{control}: present in built-in but missing from manifest"
            ));
        }
    }
    for (control, actual) in manifest_by_control {
        let Some(expected) = expected_by_control.get(control) else {
            continue;
        };
        compare_string(
            &mut errors,
            control,
            "pane_name",
            &actual.pane_name,
            expected.pane_name,
        );
        compare_string(
            &mut errors,
            control,
            "image_filename",
            &actual.image_filename,
            expected.image,
        );
        compare_string(
            &mut errors,
            control,
            "material_name",
            &actual.material_name,
            &format!("mat_{}", expected.pane_name),
        );
        compare_f32(
            &mut errors,
            control,
            "base_x",
            actual.base_x,
            expected.base_x,
        );
        compare_f32(
            &mut errors,
            control,
            "base_y",
            actual.base_y,
            expected.base_y,
        );
        compare_f32(&mut errors, control, "width", actual.width, expected.width);
        compare_f32(
            &mut errors,
            control,
            "height",
            actual.height,
            expected.height,
        );
    }

    errors
}

fn compare_string(
    errors: &mut Vec<String>,
    control: &str,
    field: &str,
    actual: &str,
    expected: &str,
) {
    if actual != expected {
        errors.push(format!(
            "{control}.{field}: manifest `{actual}` != built-in `{expected}`"
        ));
    }
}

fn compare_f32(errors: &mut Vec<String>, control: &str, field: &str, actual: f32, expected: f32) {
    if (actual - expected).abs() > 0.001 {
        errors.push(format!(
            "{control}.{field}: manifest {} != built-in {}",
            fmt_f32(actual),
            fmt_f32(expected)
        ));
    }
}

#[derive(Debug, Clone, Copy)]
struct ExpectedElement {
    control_id: &'static str,
    pane_name: &'static str,
    image: &'static str,
    base_x: f32,
    base_y: f32,
    width: f32,
    height: f32,
}

fn switch_pro_alt_expected() -> Vec<ExpectedElement> {
    vec![
        ExpectedElement::new(
            "SkinBackground",
            "sgpo_alt_background",
            "background.png",
            0.0,
            0.0,
            1280.0,
            965.0,
        ),
        ExpectedElement::new(
            "A",
            "sgpo_alt_face_a",
            "face_A.png",
            431.5,
            137.5,
            99.0,
            100.0,
        ),
        ExpectedElement::new(
            "B",
            "sgpo_alt_face_b",
            "face_B.png",
            333.0,
            52.0,
            100.0,
            99.0,
        ),
        ExpectedElement::new(
            "X",
            "sgpo_alt_face_x",
            "face_X.png",
            332.5,
            222.5,
            99.0,
            100.0,
        ),
        ExpectedElement::new(
            "Y",
            "sgpo_alt_face_y",
            "face_Y.png",
            235.0,
            137.5,
            100.0,
            100.0,
        ),
        ExpectedElement::new(
            "L",
            "sgpo_alt_l",
            "trigger_L.png",
            -334.5,
            347.0,
            327.0,
            113.0,
        ),
        ExpectedElement::new(
            "R",
            "sgpo_alt_r",
            "trigger_R.png",
            335.5,
            347.0,
            327.0,
            113.0,
        ),
        ExpectedElement::new(
            "ZL",
            "sgpo_alt_zl",
            "trigger_ZL.png",
            -364.5,
            408.0,
            227.0,
            133.0,
        ),
        ExpectedElement::new(
            "ZR",
            "sgpo_alt_zr",
            "trigger_ZR.png",
            364.5,
            408.0,
            227.0,
            133.0,
        ),
        ExpectedElement::new(
            "L3",
            "sgpo_alt_l3",
            "stick_LS_Press.png",
            -347.0,
            137.5,
            206.0,
            204.0,
        ),
        ExpectedElement::new(
            "R3",
            "sgpo_alt_r3",
            "stick_RS_Press.png",
            165.5,
            -36.5,
            205.0,
            204.0,
        ),
        ExpectedElement::new(
            "Plus",
            "sgpo_alt_plus",
            "center_Plus.png",
            155.5,
            232.5,
            61.0,
            62.0,
        ),
        ExpectedElement::new(
            "Minus",
            "sgpo_alt_minus",
            "center_Minus.png",
            -155.5,
            232.5,
            61.0,
            62.0,
        ),
        ExpectedElement::new(
            "Home",
            "sgpo_alt_home",
            "center_Home.png",
            89.5,
            137.0,
            63.0,
            63.0,
        ),
        ExpectedElement::new(
            "Capture",
            "sgpo_alt_capture",
            "center_Capture.png",
            -89.0,
            137.5,
            58.0,
            58.0,
        ),
        ExpectedElement::new(
            "DpadUp",
            "sgpo_alt_dpad_up",
            "dpad_Up.png",
            -194.0,
            11.0,
            68.0,
            97.0,
        ),
        ExpectedElement::new(
            "DpadDown",
            "sgpo_alt_dpad_down",
            "dpad_Down.png",
            -194.0,
            -83.0,
            68.0,
            97.0,
        ),
        ExpectedElement::new(
            "DpadLeft",
            "sgpo_alt_dpad_left",
            "dpad_Left.png",
            -241.0,
            -36.5,
            98.0,
            68.0,
        ),
        ExpectedElement::new(
            "DpadRight",
            "sgpo_alt_dpad_right",
            "dpad_Right.png",
            -147.0,
            -36.0,
            98.0,
            67.0,
        ),
        ExpectedElement::new(
            "LeftStickDot",
            "sgpo_alt_left_stick",
            "stick_Left.png",
            -347.0,
            137.5,
            164.0,
            164.0,
        ),
        ExpectedElement::new(
            "RightStickDot",
            "sgpo_alt_right_stick",
            "stick_Right.png",
            165.0,
            -36.5,
            164.0,
            164.0,
        ),
    ]
}

impl ExpectedElement {
    const fn new(
        control_id: &'static str,
        pane_name: &'static str,
        image: &'static str,
        base_x: f32,
        base_y: f32,
        width: f32,
        height: f32,
    ) -> Self {
        Self {
            control_id,
            pane_name,
            image,
            base_x,
            base_y,
            width,
            height,
        }
    }
}

fn pane_overrides(key: PresetKey) -> HashMap<&'static str, String> {
    match key {
        PresetKey::SwitchProAlt => switch_pro_alt_expected()
            .into_iter()
            .map(|element| (element.control_id, element.pane_name.to_string()))
            .collect(),
        PresetKey::GamecubeTron => HashMap::new(),
    }
}

fn suggested_pane_name(control: &ParsedControl, pane_prefix: &str) -> String {
    let suffix = if control.kind == ControlKind::Background {
        "background".to_string()
    } else {
        control_suffix(
            control
                .control_id
                .as_deref()
                .unwrap_or(&control.retrospy_name),
        )
    };
    format!("{pane_prefix}_{suffix}")
}

fn control_suffix(value: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = value.chars().collect();
    for (index, ch) in chars.iter().copied().enumerate() {
        if ch.is_ascii_alphanumeric() {
            let previous = index.checked_sub(1).and_then(|i| chars.get(i)).copied();
            let next = chars.get(index + 1).copied();
            let split_before_upper = ch.is_ascii_uppercase()
                && !out.is_empty()
                && !out.ends_with('_')
                && (previous
                    .is_some_and(|prev| prev.is_ascii_lowercase() || prev.is_ascii_digit())
                    || next.is_some_and(|next| next.is_ascii_lowercase()));
            if split_before_upper {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

fn control_sort_key(control_id: &str) -> (usize, String) {
    const CONTROL_ORDER: [&str; 29] = [
        "SkinBackground",
        "A",
        "B",
        "X",
        "Y",
        "L",
        "R",
        "ZL",
        "ZR",
        "L3",
        "R3",
        "Plus",
        "Minus",
        "Home",
        "Capture",
        "DpadUp",
        "DpadDown",
        "DpadLeft",
        "DpadRight",
        "DpadUpLeft",
        "DpadUpRight",
        "DpadDownLeft",
        "DpadDownRight",
        "LeftStickGate",
        "LeftStickDot",
        "RightStickGate",
        "RightStickDot",
        "GcLTrigger",
        "GcRTrigger",
    ];
    (
        CONTROL_ORDER
            .iter()
            .position(|value| *value == control_id)
            .unwrap_or(CONTROL_ORDER.len()),
        control_id.to_string(),
    )
}

fn section_button_map(section: &str) -> HashMap<&'static str, &'static str> {
    match section {
        "switch" => HashMap::from([
            ("a", "A"),
            ("b", "B"),
            ("x", "X"),
            ("y", "Y"),
            ("l", "L"),
            ("r", "R"),
            ("zl", "ZL"),
            ("zr", "ZR"),
            ("ls", "L3"),
            ("rs", "R3"),
            ("+", "Plus"),
            ("-", "Minus"),
            ("home", "Home"),
            ("capture", "Capture"),
        ]),
        "gamecube" => HashMap::from([
            ("a", "A"),
            ("b", "B"),
            ("x", "X"),
            ("y", "Y"),
            ("z", "ZR"),
            ("l", "GcLTrigger"),
            ("r", "GcRTrigger"),
            ("start", "Plus"),
        ]),
        "pc360" => HashMap::from([
            ("a", "A"),
            ("b", "B"),
            ("x", "X"),
            ("y", "Y"),
            ("l", "L"),
            ("r", "R"),
            ("trig_l_d", "ZL"),
            ("trig_r_d", "ZR"),
            ("l3", "L3"),
            ("r3", "R3"),
            ("start", "Plus"),
            ("back", "Minus"),
        ]),
        _ => HashMap::new(),
    }
}

fn global_button_map(name: &str) -> Option<&'static str> {
    match name {
        "up" => Some("DpadUp"),
        "down" => Some("DpadDown"),
        "left" => Some("DpadLeft"),
        "right" => Some("DpadRight"),
        _ => None,
    }
}

fn stick_map(xname: &str, yname: &str) -> Option<&'static str> {
    match (xname, yname) {
        ("lstick_x", "lstick_y") => Some("LeftStickDot"),
        ("rstick_x", "rstick_y") | ("cstick_x", "cstick_y") => Some("RightStickDot"),
        _ => None,
    }
}

fn scan_sections(xml: &str) -> Vec<XmlTag> {
    let mut sections = Vec::new();
    let mut cursor = 0usize;
    while let Some(rel_start) = xml[cursor..].find("<section") {
        let start = cursor + rel_start;
        let Some(start_end_rel) = xml[start..].find('>') else {
            break;
        };
        let start_end = start + start_end_rel + 1;
        let start_tag = &xml[start + "<section".len()..start_end - 1];
        let attrs = parse_attrs(start_tag);
        let Some(close_rel) = xml[start_end..].find("</section>") else {
            break;
        };
        let close_start = start_end + close_rel;
        let end = close_start + "</section>".len();
        sections.push(XmlTag {
            attrs,
            start,
            end,
            inner: Some(xml[start_end..close_start].to_string()),
        });
        cursor = end;
    }
    sections
}

fn scan_tags(xml: &str, name: &str) -> Vec<XmlTag> {
    let mut tags = Vec::new();
    let needle = format!("<{name}");
    let mut cursor = 0usize;
    while let Some(rel_start) = xml[cursor..].find(&needle) {
        let start = cursor + rel_start;
        let after_name = start + needle.len();
        if xml[after_name..]
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
        {
            cursor = after_name;
            continue;
        }
        let Some(end_rel) = xml[start..].find('>') else {
            break;
        };
        let end = start + end_rel + 1;
        let tag_body = &xml[after_name..end - 1];
        tags.push(XmlTag {
            attrs: parse_attrs(tag_body.trim_end_matches('/')),
            start,
            end,
            inner: None,
        });
        cursor = end;
    }
    tags
}

fn parse_attrs(input: &str) -> HashMap<String, String> {
    let mut attrs = HashMap::new();
    let bytes = input.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        let key_start = index;
        while index < bytes.len()
            && (bytes[index].is_ascii_alphanumeric()
                || bytes[index] == b'_'
                || bytes[index] == b'-')
        {
            index += 1;
        }
        if key_start == index {
            index += 1;
            continue;
        }
        let key = &input[key_start..index];
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() || bytes[index] != b'=' {
            continue;
        }
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index >= bytes.len() || (bytes[index] != b'"' && bytes[index] != b'\'') {
            continue;
        }
        let quote = bytes[index];
        index += 1;
        let value_start = index;
        while index < bytes.len() && bytes[index] != quote {
            index += 1;
        }
        if index > value_start {
            attrs.insert(key.to_string(), html_unescape(&input[value_start..index]));
        } else {
            attrs.insert(key.to_string(), String::new());
        }
        index += 1;
    }
    attrs
}

fn remove_ranges(input: &str, ranges: &[(usize, usize)]) -> String {
    let mut output = String::new();
    let mut cursor = 0usize;
    for (start, end) in ranges {
        if cursor < *start {
            output.push_str(&input[cursor..*start]);
        }
        cursor = *end;
    }
    if cursor < input.len() {
        output.push_str(&input[cursor..]);
    }
    output
}

fn html_unescape(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn required_f32_attr(attrs: &HashMap<String, String>, key: &str) -> Result<f32> {
    attrs
        .get(key)
        .with_context(|| format!("missing {key}= attribute"))?
        .parse()
        .with_context(|| format!("invalid {key}= attribute"))
}

fn optional_f32_attr(attrs: &HashMap<String, String>, key: &str) -> Result<Option<f32>> {
    attrs
        .get(key)
        .map(|value| {
            value
                .parse()
                .with_context(|| format!("invalid {key}= attribute"))
        })
        .transpose()
}

fn load_dotenv(path: &Path) -> Result<HashMap<String, String>> {
    let mut values = HashMap::new();
    if !path.exists() {
        return Ok(values);
    }
    for line in fs::read_to_string(path)?.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        let value = value.trim().trim_matches('"').trim_matches('\'');
        values.insert(key.to_string(), value.to_string());
    }
    Ok(values)
}

fn env_path(env: &HashMap<String, String>, key: &str) -> Option<PathBuf> {
    env.get(key)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn env_path_or(env: &HashMap<String, String>, key: &str, fallback: &str) -> PathBuf {
    env_path(env, key).unwrap_or_else(|| PathBuf::from(fallback))
}

fn parse_optional_u32(value: Option<&str>) -> Result<Option<u32>> {
    value
        .map(|value| {
            if let Some(hex) = value.strip_prefix("0x") {
                u32::from_str_radix(hex, 16).with_context(|| format!("invalid hex value {value}"))
            } else {
                value
                    .parse::<u32>()
                    .with_context(|| format!("invalid value {value}"))
            }
        })
        .transpose()
}

fn parse_import_texture_format(value: &str) -> Result<(ImportTextureFormat, bool, String)> {
    let normalized = value.trim().to_ascii_lowercase().replace('_', "-");
    match normalized.as_str() {
        "bc7" | "bc7-srgb" | "bc7-unorm-srgb" => {
            Ok((ImportTextureFormat::Bc7, true, "bc7-srgb".to_string()))
        }
        "bc7-unorm" => Ok((ImportTextureFormat::Bc7, false, "bc7".to_string())),
        "rgba8" | "rgba8-unorm" | "r8g8b8a8" | "r8g8b8a8-unorm" => {
            Ok((ImportTextureFormat::Rgba8, false, "rgba8".to_string()))
        }
        "rgba8-srgb" | "rgba8-unorm-srgb" | "r8g8b8a8-srgb" | "r8g8b8a8-unorm-srgb" => Ok((
            ImportTextureFormat::Rgba8Srgb,
            true,
            "rgba8-srgb".to_string(),
        )),
        _ => bail!(
            "unsupported texture format '{value}'; expected bc7-srgb, bc7, rgba8, or rgba8-srgb"
        ),
    }
}

fn require_path(path: &Path, label: &str) -> Result<()> {
    if !path.exists() {
        bail!("{label} does not exist: {}", path.display());
    }
    Ok(())
}

fn copy_dir_all(source: &Path, dest: &Path) -> Result<()> {
    if dest.exists() {
        fs::remove_dir_all(dest)?;
    }
    fs::create_dir_all(dest)?;
    for entry in WalkDir::new(source) {
        let entry = entry?;
        let relative = entry.path().strip_prefix(source)?;
        let target = dest.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

fn make_tree_writable(path: &Path) -> Result<()> {
    for entry in WalkDir::new(path) {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let mut perms = metadata.permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = if metadata.is_dir() { 0o700 } else { 0o600 };
            perms.set_mode(perms.mode() | mode);
        }
        #[cfg(not(unix))]
        {
            perms.set_readonly(false);
        }
        fs::set_permissions(entry.path(), perms)?;
    }
    Ok(())
}

fn fmt_f32(value: f32) -> String {
    let text = format!("{value:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_timestamp_format_matches_python_style_suffix() {
        assert_eq!(format_unix_timestamp(0), "19700101-000000");
        assert_eq!(format_unix_timestamp(1_704_067_200), "20240101-000000");
    }

    #[test]
    fn control_suffix_matches_existing_manifest_names() {
        assert_eq!(control_suffix("DpadUp"), "dpad_up");
        assert_eq!(control_suffix("GcLTrigger"), "gc_l_trigger");
        assert_eq!(control_suffix("LeftStickDot"), "left_stick_dot");
    }

    #[test]
    fn texture_format_aliases_keep_existing_default_srgb() {
        assert_eq!(
            parse_import_texture_format("bc7").unwrap(),
            (ImportTextureFormat::Bc7, true, "bc7-srgb".to_string())
        );
        assert_eq!(
            parse_import_texture_format("bc7-unorm").unwrap(),
            (ImportTextureFormat::Bc7, false, "bc7".to_string())
        );
        assert_eq!(
            parse_import_texture_format("rgba8").unwrap(),
            (ImportTextureFormat::Rgba8, false, "rgba8".to_string())
        );
        assert_eq!(
            parse_import_texture_format("r8g8b8a8_srgb").unwrap(),
            (
                ImportTextureFormat::Rgba8Srgb,
                true,
                "rgba8-srgb".to_string()
            )
        );
    }
}
