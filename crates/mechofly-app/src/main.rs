#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod brain_lab;
mod compute;
#[cfg(windows)]
mod desktop_pet;
mod diagnostics;
mod live_brain;
mod pet;
mod runtime;
mod screen_ecotope;
mod self_test;
mod tray;

use std::{path::PathBuf, str::FromStr};

use app::{AppConfig, MechoFlyApp, RuntimeSourceIdentity};
use compute::ComputePreference;
use pet::Skin;
#[cfg(not(windows))]
use pet::{PET_HEIGHT, PET_WIDTH};
use screen_ecotope::EcotopeMode;
use serde::Deserialize;

fn main() {
    diagnostics::initialize();
    if let Err(error) = run() {
        diagnostics::record_fatal_error(&error.to_string());
        eprintln!("MechoFly: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(path) = option_value(&args, "--self-test") {
        diagnostics::mark("starting isolated deterministic self-test");
        self_test::run(PathBuf::from(path).as_path())?;
        diagnostics::mark("isolated deterministic self-test completed");
        return Ok(());
    }
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .try_init()
        .ok();
    let config = AppConfig::from_profile_and_args(&args)?;
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
    ecotope_mode: Option<String>,
    source_branch: Option<String>,
    source_commit: Option<String>,
    source_tree: Option<String>,
    executable_sha256: Option<String>,
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
            ecotope_mode: profile
                .ecotope_mode
                .as_deref()
                .and_then(|value| EcotopeMode::from_str(value).ok())
                .unwrap_or_default(),
            source_identity: RuntimeSourceIdentity {
                branch: profile.source_branch,
                commit: profile.source_commit,
                tree: profile.source_tree,
                executable_sha256: profile.executable_sha256,
            },
        };
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
        if let Some(value) = option_value(args, "--ecotope") {
            config.ecotope_mode = EcotopeMode::from_str(value)?;
        }
        Ok(config)
    }
}

fn load_runtime_profile() -> Option<RuntimeProfile> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    let path = PathBuf::from(local)
        .join("MechoFly")
        .join("runtime-profile.json");
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
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
