#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod behavior_baseline;
mod behavior_campaign;
mod behavior_inspector;
mod brain_lab;
mod compute;
#[cfg(windows)]
mod desktop_pet;
mod diagnostics;
mod live_brain;
mod pet;
#[cfg(feature = "n6-product-checkpoint")]
mod product_checkpoint;
#[cfg(feature = "n41-visual-review-b")]
mod review_evidence;
mod runtime;
#[cfg(feature = "n7-scientific-explanation")]
mod scientific_explanation;
mod self_test;
mod storage;
mod tray;

use std::{path::PathBuf, str::FromStr};

use app::{AppConfig, MechoFlyApp, RuntimeSourceIdentity};
use compute::ComputePreference;
use pet::Skin;
#[cfg(not(windows))]
use pet::{PET_HEIGHT, PET_WIDTH};
use serde::Deserialize;
#[cfg(feature = "n41-visual-review-b")]
use serde::Serialize;

const N41_B_VISUAL_REVIEW_FLAG: &str = "--n41-b-visual-review";
const N41_VISUAL_REVIEW_RECEIPT_OPTION: &str = "--n41-visual-review-receipt";
const N6_PRODUCT_CHECKPOINT_SELF_TEST_OPTION: &str = "--n6-product-checkpoint-self-test";
const N6_COUNTERFACTUAL_REPLAY_SELF_TEST_OPTION: &str = "--n6-counterfactual-replay-self-test";
const N7_SCIENTIFIC_EXPLANATION_SELF_TEST_OPTION: &str = "--n7-scientific-explanation-self-test";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if option_value(&args, N7_SCIENTIFIC_EXPLANATION_SELF_TEST_OPTION).is_some() {
        if let Err(error) = run_n7_scientific_explanation(&args) {
            eprintln!("MechoFly: {error}");
            std::process::exit(1);
        }
        return;
    }
    if option_value(&args, N6_COUNTERFACTUAL_REPLAY_SELF_TEST_OPTION).is_some() {
        if let Err(error) = run_n6_counterfactual_replay(&args) {
            eprintln!("MechoFly: {error}");
            std::process::exit(1);
        }
        return;
    }
    if option_value(&args, N6_PRODUCT_CHECKPOINT_SELF_TEST_OPTION).is_some() {
        if let Err(error) = run_n6_product_checkpoint(&args) {
            eprintln!("MechoFly: {error}");
            std::process::exit(1);
        }
        return;
    }
    diagnostics::initialize();
    if let Err(error) = run(&args) {
        diagnostics::record_fatal_error(&error.to_string());
        eprintln!("MechoFly: {error}");
        std::process::exit(1);
    }
}

#[cfg(feature = "n7-scientific-explanation")]
fn run_n7_scientific_explanation(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let path = option_value(args, N7_SCIENTIFIC_EXPLANATION_SELF_TEST_OPTION)
        .ok_or("N7 scientific-explanation receipt path is missing")?;
    scientific_explanation::run(PathBuf::from(path).as_path())
}

#[cfg(not(feature = "n7-scientific-explanation"))]
fn run_n7_scientific_explanation(_args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    Err("N7 scientific-explanation self-test requires feature n7-scientific-explanation".into())
}

#[cfg(feature = "n6-counterfactual-replay")]
fn run_n6_counterfactual_replay(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let path = option_value(args, N6_COUNTERFACTUAL_REPLAY_SELF_TEST_OPTION)
        .ok_or("N6 counterfactual-replay receipt path is missing")?;
    product_checkpoint::run_counterfactual_replay(PathBuf::from(path).as_path())
}

#[cfg(not(feature = "n6-counterfactual-replay"))]
fn run_n6_counterfactual_replay(_args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    Err("N6 counterfactual-replay self-test requires feature n6-counterfactual-replay".into())
}

#[cfg(feature = "n6-product-checkpoint")]
fn run_n6_product_checkpoint(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let path = option_value(args, N6_PRODUCT_CHECKPOINT_SELF_TEST_OPTION)
        .ok_or("N6 product-checkpoint receipt path is missing")?;
    product_checkpoint::run(PathBuf::from(path).as_path())
}

#[cfg(not(feature = "n6-product-checkpoint"))]
fn run_n6_product_checkpoint(_args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    Err("N6 product-checkpoint self-test requires feature n6-product-checkpoint".into())
}

fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = option_value(args, "--self-test") {
        diagnostics::mark("starting isolated deterministic self-test");
        self_test::run(PathBuf::from(path).as_path())?;
        diagnostics::mark("isolated deterministic self-test completed");
        return Ok(());
    }
    if let Some(path) = option_value(args, "--behavior-baseline") {
        diagnostics::mark("starting deterministic behavior telemetry baseline");
        behavior_baseline::run(PathBuf::from(path).as_path(), args)?;
        diagnostics::mark("deterministic behavior telemetry baseline completed");
        return Ok(());
    }
    if let Some(path) = option_value(args, "--behavior-campaign") {
        behavior_campaign::run(PathBuf::from(path).as_path(), args)?;
        return Ok(());
    }
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .try_init()
        .ok();
    let config = AppConfig::from_profile_and_args(args)?;
    write_n41_visual_review_launch_receipt(args, &config)?;
    diagnostics::mark("runtime profile and command line accepted");
    let icon = app_icon();
    let options = eframe::NativeOptions {
        viewport: root_viewport(icon),
        multisampling: 4,
        ..Default::default()
    };
    diagnostics::mark("entering eframe native event loop");
    eframe::run_native(
        "MechoFly",
        options,
        Box::new(move |cc| Ok(Box::new(MechoFlyApp::new(cc, config.clone())))),
    )?;
    diagnostics::mark("eframe native event loop returned normally");
    Ok(())
}

fn root_viewport(icon: eframe::egui::IconData) -> eframe::egui::ViewportBuilder {
    let builder = eframe::egui::ViewportBuilder::default()
        .with_title("MechoFly")
        .with_resizable(false)
        .with_decorations(false)
        .with_taskbar(false)
        .with_window_level(eframe::egui::WindowLevel::AlwaysOnTop)
        .with_has_shadow(false)
        .with_icon(icon);

    #[cfg(windows)]
    {
        // The Windows desktop pet is a native per-pixel-alpha layered window.
        // This hidden eframe root keeps the event loop, Brain Lab viewports,
        // and vendor-neutral wgpu compute alive without exposing a swap-chain
        // rectangle on the desktop.
        builder
            .with_inner_size([1.0, 1.0])
            .with_position([-32_000.0, -32_000.0])
            .with_visible(false)
    }
    #[cfg(not(windows))]
    {
        builder
            .with_inner_size([PET_WIDTH as f32, PET_HEIGHT as f32])
            .with_min_inner_size([PET_WIDTH as f32, PET_HEIGHT as f32])
            .with_max_inner_size([PET_WIDTH as f32, PET_HEIGHT as f32])
            .with_position([96.0, 640.0])
            .with_transparent(true)
    }
}

#[derive(Default, Deserialize)]
struct RuntimeProfile {
    skin: Option<String>,
    compute: Option<ComputePreference>,
    reduced_motion: Option<bool>,
    source_branch: Option<String>,
    source_commit: Option<String>,
    source_tree: Option<String>,
    executable_sha256: Option<String>,
}

#[cfg(feature = "n41-visual-review-b")]
#[derive(Serialize)]
struct N41VisualReviewLaunchReceipt {
    schema_version: u32,
    status: &'static str,
    classification: &'static str,
    review_feature: &'static str,
    required_explicit_flag: &'static str,
    canonical_default_profile: &'static str,
    active_profile: &'static str,
    parameter_sha256: &'static str,
    dynamics_version: &'static str,
    dynamics_claim: &'static str,
    process_id: u32,
    executable: String,
    executable_sha256: String,
    storage_override_active: bool,
    storage_directory: Option<String>,
    source_branch: Option<String>,
    source_commit: Option<String>,
    source_tree: Option<String>,
    source_executable_sha256: Option<String>,
    trace_path: String,
    capture_directory: String,
    capture_source: &'static str,
    promotion_authorized: bool,
    deployment_authorized: bool,
}

impl AppConfig {
    fn from_profile_and_args(args: &[String]) -> Result<Self, String> {
        let profile = load_runtime_profile().unwrap_or_default();
        let mut config = Self {
            skin: profile
                .skin
                .as_deref()
                .and_then(|value| Skin::from_str(value).ok())
                .unwrap_or_default(),
            compute: profile.compute.unwrap_or_default(),
            open_brain_lab: args.iter().any(|arg| arg == "--brain-lab"),
            reduced_motion: profile.reduced_motion.unwrap_or(false)
                || args.iter().any(|arg| arg == "--reduced-motion"),
            source_identity: RuntimeSourceIdentity {
                branch: profile.source_branch,
                commit: profile.source_commit,
                tree: profile.source_tree,
                executable_sha256: profile.executable_sha256,
            },
            #[cfg(feature = "n41-visual-review-b")]
            n41_visual_review_b: visual_review_enabled(args)?,
        };
        #[cfg(not(feature = "n41-visual-review-b"))]
        visual_review_enabled(args)?;
        if let Some(value) = option_value(args, "--skin") {
            config.skin = Skin::from_str(value)?;
        }
        if let Some(value) = option_value(args, "--compute") {
            config.compute = match value.to_ascii_lowercase().as_str() {
                "auto" => ComputePreference::Auto,
                "cpu" => ComputePreference::Cpu,
                "gpu" => ComputePreference::Gpu,
                _ => return Err(format!("unknown compute preference {value:?}")),
            };
        }
        Ok(config)
    }
}

fn load_runtime_profile() -> Option<RuntimeProfile> {
    let directory = if let Some(directory) = storage::override_directory() {
        directory
    } else {
        PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("MechoFly")
    };
    let path = directory.join("runtime-profile.json");
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

#[cfg(feature = "n41-visual-review-b")]
fn visual_review_enabled(args: &[String]) -> Result<bool, String> {
    let requested = args.iter().any(|arg| arg == N41_B_VISUAL_REVIEW_FLAG);
    if option_value(args, N41_VISUAL_REVIEW_RECEIPT_OPTION).is_some() && !requested {
        return Err(format!(
            "{N41_VISUAL_REVIEW_RECEIPT_OPTION} requires {N41_B_VISUAL_REVIEW_FLAG}"
        ));
    }
    Ok(requested)
}

#[cfg(not(feature = "n41-visual-review-b"))]
fn visual_review_enabled(args: &[String]) -> Result<bool, String> {
    if args.iter().any(|arg| arg == N41_B_VISUAL_REVIEW_FLAG)
        || option_value(args, N41_VISUAL_REVIEW_RECEIPT_OPTION).is_some()
    {
        return Err(
            "N4.1-D exploratory-flight review requires a build with feature n41-visual-review-b"
                .into(),
        );
    }
    Ok(false)
}

#[cfg(feature = "n41-visual-review-b")]
fn write_n41_visual_review_launch_receipt(
    args: &[String],
    config: &AppConfig,
) -> Result<(), String> {
    if !config.n41_visual_review_b {
        return Ok(());
    }
    use mechofly_core::behavior_parameters::{
        BehaviorParameterProfile, N41_NATURAL_FLIGHT_DYNAMICS_CLAIM,
        N41_NATURAL_FLIGHT_DYNAMICS_VERSION, artifact_sha256, parameter_sha256_for,
    };

    let destination = option_value(args, N41_VISUAL_REVIEW_RECEIPT_OPTION).ok_or_else(|| {
        format!("{N41_B_VISUAL_REVIEW_FLAG} requires {N41_VISUAL_REVIEW_RECEIPT_OPTION} PATH")
    })?;
    let destination = PathBuf::from(destination);
    if !destination.is_absolute() {
        return Err("N4.1 visual-review receipt path must be absolute".into());
    }
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let executable_bytes = std::fs::read(&executable).map_err(|error| error.to_string())?;
    let storage_directory = storage::override_directory();
    if storage_directory.is_none() {
        return Err("N4.1 visual review requires MECHOFLY_DATA_DIR".into());
    }
    let receipt = N41VisualReviewLaunchReceipt {
        schema_version: 5,
        status: "PASS",
        classification: "feature_gated_uncued_exploratory_flight_visual_review_candidate",
        review_feature: "n41-visual-review-b",
        required_explicit_flag: N41_B_VISUAL_REVIEW_FLAG,
        canonical_default_profile: "n4",
        active_profile: "n41-b-natural-flight",
        parameter_sha256: parameter_sha256_for(BehaviorParameterProfile::N41BNaturalFlight),
        dynamics_version: N41_NATURAL_FLIGHT_DYNAMICS_VERSION,
        dynamics_claim: N41_NATURAL_FLIGHT_DYNAMICS_CLAIM,
        process_id: std::process::id(),
        executable: executable.display().to_string(),
        executable_sha256: artifact_sha256(&executable_bytes),
        storage_override_active: true,
        storage_directory: storage_directory
            .as_ref()
            .map(|path| path.display().to_string()),
        source_branch: config.source_identity.branch.clone(),
        source_commit: config.source_identity.commit.clone(),
        source_tree: config.source_identity.tree.clone(),
        source_executable_sha256: config.source_identity.executable_sha256.clone(),
        trace_path: storage_directory
            .as_ref()
            .expect("storage override was checked")
            .join("review-trace.jsonl")
            .display()
            .to_string(),
        capture_directory: storage_directory
            .as_ref()
            .expect("storage override was checked")
            .join("review-captures")
            .display()
            .to_string(),
        capture_source: "direct pet BGRA buffer composited over a constant backdrop; no screen capture",
        promotion_authorized: false,
        deployment_authorized: false,
    };
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(&receipt).map_err(|error| error.to_string())?;
    std::fs::write(&destination, [bytes.as_slice(), b"\n"].concat())
        .map_err(|error| error.to_string())
}

#[cfg(not(feature = "n41-visual-review-b"))]
fn write_n41_visual_review_launch_receipt(
    args: &[String],
    _config: &AppConfig,
) -> Result<(), String> {
    visual_review_enabled(args).map(|_| ())
}

fn option_value<'a>(args: &'a [String], option: &str) -> Option<&'a str> {
    args.iter()
        .position(|arg| arg == option)
        .and_then(|index| args.get(index + 1))
        .map(String::as_str)
}

fn app_icon() -> eframe::egui::IconData {
    let mut rgba = vec![0_u8; 32 * 32 * 4];
    for y in 0..32_i32 {
        for x in 0..32_i32 {
            let offset = ((y * 32 + x) * 4) as usize;
            let body = ((x - 16) * (x - 16)) / 2 + (y - 17) * (y - 17) < 76;
            let head = (x - 8) * (x - 8) + (y - 17) * (y - 17) < 24;
            let lantern = (x - 25) * (x - 25) + (y - 17) * (y - 17) < 20;
            let color = if lantern {
                [202, 229, 70, 255]
            } else if head {
                [44, 49, 43, 255]
            } else if body {
                [41, 111, 72, 255]
            } else {
                [0, 0, 0, 0]
            };
            rgba[offset..offset + 4].copy_from_slice(&color);
        }
    }
    eframe::egui::IconData {
        rgba,
        width: 32,
        height: 32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_launch_does_not_enable_visual_review() {
        assert!(!visual_review_enabled(&[]).expect("ordinary launch must parse"));
    }

    #[cfg(feature = "n41-visual-review-b")]
    #[test]
    fn feature_build_still_requires_explicit_flag() {
        let args = vec![N41_B_VISUAL_REVIEW_FLAG.to_owned()];
        assert!(visual_review_enabled(&args).expect("feature-gated flag must parse"));

        let receipt_only = vec![
            N41_VISUAL_REVIEW_RECEIPT_OPTION.to_owned(),
            "C:\\review.json".to_owned(),
        ];
        assert!(visual_review_enabled(&receipt_only).is_err());
    }

    #[cfg(not(feature = "n41-visual-review-b"))]
    #[test]
    fn canonical_build_rejects_visual_review_flag() {
        let args = vec![N41_B_VISUAL_REVIEW_FLAG.to_owned()];
        assert!(visual_review_enabled(&args).is_err());
    }
}
