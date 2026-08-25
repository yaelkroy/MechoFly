#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod brain_lab;
mod compute;
mod pet;
mod runtime;
mod self_test;
mod tray;

use std::{path::PathBuf, str::FromStr};

use app::{AppConfig, MechoFlyApp};
use compute::ComputePreference;
use pet::Skin;
use serde::Deserialize;

fn main() {
    if let Err(error) = run() {
        eprintln!("MechoFly: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(path) = option_value(&args, "--self-test") {
        self_test::run(PathBuf::from(path).as_path())?;
        return Ok(());
    }
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .try_init()
        .ok();
    let config = AppConfig::from_profile_and_args(&args)?;
    let icon = app_icon();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("MechoFly")
            .with_inner_size([248.0, 166.0])
            .with_min_inner_size([248.0, 166.0])
            .with_max_inner_size([248.0, 166.0])
            .with_position([96.0, 640.0])
            .with_resizable(false)
            .with_decorations(false)
            .with_transparent(true)
            .with_taskbar(false)
            .with_window_level(eframe::egui::WindowLevel::AlwaysOnTop)
            .with_has_shadow(false)
            .with_icon(icon),
        multisampling: 4,
        ..Default::default()
    };
    eframe::run_native(
        "MechoFly",
        options,
        Box::new(move |cc| Ok(Box::new(MechoFlyApp::new(cc, config.clone())))),
    )?;
    Ok(())
}

#[derive(Default, Deserialize)]
struct RuntimeProfile {
    skin: Option<String>,
    compute: Option<ComputePreference>,
    reduced_motion: Option<bool>,
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
