//! Review-only evidence emitted from MechoFly's own presentation buffer.
//!
//! No screen API is used: captures contain only the 420 x 280 pet raster
//! composited over a constant collector-owned color.

use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    time::Instant,
};

use eframe::egui::{Pos2, Vec2};
use mechofly_core::{
    Behavior,
    grooming_program::{GroomingMotorSubstate, grooming_program_at},
};
use serde::Serialize;

use crate::pet::{PET_HEIGHT, PET_WIDTH, PetMotion, Skin, render_pet_bgra_at_age_with_altitude};

const BACKDROP_RGB: [u8; 3] = [28, 24, 36];

#[derive(Serialize)]
struct ReviewTraceRecord {
    sequence: u64,
    wall_elapsed_ms: u128,
    model_frame: u64,
    modeled_ms: u64,
    behavior: Behavior,
    behavior_age_frames: u32,
    grooming_substate: Option<GroomingMotorSubstate>,
    screen_x: f32,
    screen_y: f32,
    movement_left: f32,
    movement_top: f32,
    movement_right: f32,
    movement_bottom: f32,
    heading_radians: f32,
    speed_pixels_per_second: f32,
    altitude_pixels: f32,
    natural_flight_motion: bool,
    cursor_hovered: bool,
    dragging: bool,
    evidence_hold: bool,
}

pub struct ReviewEvidence {
    trace: BufWriter<File>,
    captures: PathBuf,
    started: Instant,
    sequence: u64,
    boundary_sequence: u32,
    previous_hold: bool,
    previous_behavior: Option<Behavior>,
    captured_grooming: BTreeSet<GroomingMotorSubstate>,
    captured_flight_phases: BTreeSet<&'static str>,
}

impl ReviewEvidence {
    pub fn new(data_directory: &Path) -> Result<Self, String> {
        fs::create_dir_all(data_directory).map_err(|error| error.to_string())?;
        let captures = data_directory.join("review-captures");
        fs::create_dir_all(&captures).map_err(|error| error.to_string())?;
        let trace_path = data_directory.join("review-trace.jsonl");
        let trace = BufWriter::new(
            File::create(&trace_path)
                .map_err(|error| format!("cannot create {}: {error}", trace_path.display()))?,
        );
        Ok(Self {
            trace,
            captures,
            started: Instant::now(),
            sequence: 0,
            boundary_sequence: 0,
            previous_hold: false,
            previous_behavior: None,
            captured_grooming: BTreeSet::new(),
            captured_flight_phases: BTreeSet::new(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn observe(
        &mut self,
        model_frame: u64,
        behavior_age_frames: u32,
        behavior: Behavior,
        pet: &PetMotion,
        screen_origin: Pos2,
        screen_size: Vec2,
        skin: Skin,
        cursor_hovered: bool,
        dragging: bool,
        evidence_hold: bool,
    ) -> Result<(), String> {
        self.sequence = self.sequence.saturating_add(1);
        let grooming =
            (behavior == Behavior::Groom).then(|| grooming_program_at(behavior_age_frames));
        let width = screen_size.x.max(480.0);
        let height = screen_size.y.max(320.0);
        let movement_left = screen_origin.x + 8.0;
        let movement_top = screen_origin.y + 8.0;
        let movement_right = (screen_origin.x + width - PET_WIDTH as f32 - 8.0).max(movement_left);
        let movement_bottom =
            (screen_origin.y + height - PET_HEIGHT as f32 - 8.0).max(movement_top);
        let record = ReviewTraceRecord {
            sequence: self.sequence,
            wall_elapsed_ms: self.started.elapsed().as_millis(),
            model_frame,
            modeled_ms: model_frame.saturating_mul(u64::from(mechofly_core::MODEL_STEP_MS)),
            behavior,
            behavior_age_frames,
            grooming_substate: grooming.map(|program| program.substate),
            screen_x: pet.screen_position.x,
            screen_y: pet.screen_position.y,
            movement_left,
            movement_top,
            movement_right,
            movement_bottom,
            heading_radians: pet.heading_radians,
            speed_pixels_per_second: pet.speed_pixels_per_second(),
            altitude_pixels: pet.altitude_pixels,
            natural_flight_motion: pet.natural_flight_motion,
            cursor_hovered,
            dragging,
            evidence_hold,
        };
        serde_json::to_writer(&mut self.trace, &record).map_err(|error| error.to_string())?;
        self.trace
            .write_all(b"\n")
            .map_err(|error| error.to_string())?;

        if evidence_hold && !self.previous_hold {
            self.boundary_sequence = self.boundary_sequence.saturating_add(1);
            let name = format!("boundary-{:04}.png", self.boundary_sequence);
            self.capture(&name, skin, behavior, pet)?;
            self.trace.flush().map_err(|error| error.to_string())?;
        }
        self.previous_hold = evidence_hold;

        if let Some(program) = grooming
            && program.segment_progress >= 0.35
            && !matches!(
                program.substate,
                GroomingMotorSubstate::Prepare | GroomingMotorSubstate::Reset
            )
            && self.captured_grooming.insert(program.substate)
        {
            let name = format!(
                "groom-{}.png",
                program
                    .substate
                    .label()
                    .to_ascii_lowercase()
                    .replace(' ', "-")
            );
            self.capture(&name, skin, behavior, pet)?;
        }
        for (name, should_capture) in [
            (
                "flight-takeoff.png",
                behavior == Behavior::PreEscape && behavior_age_frames >= 2,
            ),
            (
                "flight-early.png",
                behavior == Behavior::Flight && behavior_age_frames >= 12,
            ),
            (
                "flight-maneuver.png",
                behavior == Behavior::Flight && pet.flight_maneuver_active(),
            ),
            (
                "flight-landing.png",
                behavior == Behavior::Landing && behavior_age_frames >= 4,
            ),
            (
                "flight-touchdown.png",
                matches!(behavior, Behavior::Rest | Behavior::Quiet)
                    && self.previous_behavior == Some(Behavior::Landing),
            ),
        ] {
            if should_capture && self.captured_flight_phases.insert(name) {
                self.capture(name, skin, behavior, pet)?;
            }
        }
        self.previous_behavior = Some(behavior);
        Ok(())
    }

    fn capture(
        &self,
        name: &str,
        skin: Skin,
        behavior: Behavior,
        pet: &PetMotion,
    ) -> Result<(), String> {
        let mut bgra = vec![0_u8; PET_WIDTH * PET_HEIGHT * 4];
        render_pet_bgra_at_age_with_altitude(
            &mut bgra,
            skin,
            behavior,
            pet.animation_seconds,
            pet.behavior_age_seconds,
            pet.heading_radians,
            pet.altitude_pixels,
            pet.reduced_motion,
        );
        write_composited_png(&self.captures.join(name), &bgra)
    }
}

fn composite_bgra_pixel(pixel: [u8; 4]) -> [u8; 4] {
    let inverse_alpha = 255_u16 - u16::from(pixel[3]);
    let blend = |premultiplied: u8, background: u8| {
        (u16::from(premultiplied) + (u16::from(background) * inverse_alpha + 127) / 255).min(255)
            as u8
    };
    [
        blend(pixel[2], BACKDROP_RGB[0]),
        blend(pixel[1], BACKDROP_RGB[1]),
        blend(pixel[0], BACKDROP_RGB[2]),
        255,
    ]
}

fn write_composited_png(path: &Path, bgra: &[u8]) -> Result<(), String> {
    if bgra.len() != PET_WIDTH * PET_HEIGHT * 4 {
        return Err("review capture BGRA buffer has the wrong dimensions".to_owned());
    }
    let mut rgba = Vec::with_capacity(bgra.len());
    for pixel in bgra.as_chunks::<4>().0 {
        rgba.extend_from_slice(&composite_bgra_pixel([
            pixel[0], pixel[1], pixel[2], pixel[3],
        ]));
    }
    let file = File::create(path)
        .map_err(|error| format!("cannot create review capture {}: {error}", path.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), PET_WIDTH as u32, PET_HEIGHT as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    writer
        .write_image_data(&rgba)
        .map_err(|error| error.to_string())?;
    writer.finish().map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compositor_never_exposes_pixels_behind_the_pet() {
        assert_eq!(composite_bgra_pixel([0, 0, 0, 0]), [28, 24, 36, 255]);
        assert_eq!(composite_bgra_pixel([3, 2, 1, 255]), [1, 2, 3, 255]);
        assert_eq!(composite_bgra_pixel([18, 12, 14, 128]), [28, 24, 36, 255]);
    }
}
