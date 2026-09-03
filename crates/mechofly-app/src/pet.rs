use std::{
    f32::consts::{PI, TAU},
    str::FromStr,
};

use eframe::egui::{self, Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};
use mechofly_core::{
    Behavior,
    grooming_program::{GroomingMotorSubstate, GroomingProgramFrame, grooming_program_at},
};
use serde::{Deserialize, Serialize};

pub const PET_WIDTH: usize = 420;
pub const PET_HEIGHT: usize = 280;
const CENTER: [f32; 2] = [PET_WIDTH as f32 * 0.5, PET_HEIGHT as f32 * 0.5];
const RASTER_SCALE: usize = 2;
const REFERENCE_TICK_SECONDS: f32 = 0.033;
const WALK_SPEED_PIXELS_PER_SECOND: f32 = 1.85 / REFERENCE_TICK_SECONDS;
const WALK_SPEED_ONSET_SECONDS: f32 = 0.16;
const WALK_SPEED_MULTIPLIERS: [f32; 16] = [
    0.55, 0.62, 0.68, 0.75, 0.82, 0.89, 0.96, 1.03, 1.10, 1.17, 1.24, 1.31, 1.38, 1.45, 1.52, 1.60,
];
const FLIGHT_SPEED_MULTIPLIERS: [f32; 16] = [
    0.68, 0.73, 0.78, 0.83, 0.88, 0.93, 0.98, 1.03, 1.08, 1.13, 1.18, 1.23, 1.28, 1.33, 1.38, 1.43,
];
const REVERSE_SPEED_PIXELS_PER_SECOND: f32 = -2.2 / REFERENCE_TICK_SECONDS;
const ESCAPE_SPEED_PIXELS_PER_SECOND: f32 = 18.0 / REFERENCE_TICK_SECONDS;
const FLIGHT_SPEED_PIXELS_PER_SECOND: f32 = 8.4 / REFERENCE_TICK_SECONDS;
const FLIGHT_SPEED_BLEND_SECONDS: f32 = 0.22;
const FLIGHT_COURSE_SELECTION_MAX_RADIANS: f32 = 2.60;
const FLIGHT_COURSE_SELECTION_MIN_RADIANS: f32 = 0.45;
const FLIGHT_EDGE_AVOIDANCE_FRACTION: f32 = 0.10;
const FLIGHT_EDGE_AVOIDANCE_MIN_PIXELS: f32 = 48.0;
const FLIGHT_EDGE_AVOIDANCE_MAX_PIXELS: f32 = 160.0;
const FLIGHT_EDGE_AVOIDANCE_GAIN: f32 = 2.40;
const FLIGHT_EDGE_AVOIDANCE_MAX_TURN_RADIANS_PER_SECOND: f32 = 2.20;

const NERVOUS_SPEED_PIXELS_PER_SECOND: f32 = 3.0 / REFERENCE_TICK_SECONDS;
const LANDING_COMPLETION_SECONDS: f32 = 0.495;
const TAKEOFF_COMPLETION_SECONDS: f32 = 0.198;
const TAKEOFF_ALTITUDE_PIXELS: f32 = 24.0;
const CRUISE_ALTITUDE_PIXELS: f32 = 72.0;
const FLIGHT_ALTITUDE_RISE_SECONDS: f32 = 0.45;
const LANDING_APPROACH_MIN_PIXELS: f32 = 36.0;
const LANDING_APPROACH_MAX_PIXELS: f32 = 96.0;
const LANDING_SAFE_TOP_MARGIN_PIXELS: f32 = 32.0;
const LANDING_SAFE_BOTTOM_FRACTION: f32 = 0.12;
const LANDING_SAFE_BOTTOM_MIN_PIXELS: f32 = 56.0;
const LANDING_SAFE_BOTTOM_MAX_PIXELS: f32 = 112.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum GroundPathMotif {
    Straight,
    CurveLeft,
    CurveRight,
    SharpTurnLeft,
    SharpTurnRight,
}

impl GroundPathMotif {
    fn for_bout(sequence: u64) -> Self {
        match motor_mix64(sequence ^ 0x4D4F_5449_4F4E_5631) % 8 {
            0..=3 => Self::Straight,
            4 => Self::CurveLeft,
            5 => Self::CurveRight,
            6 => Self::SharpTurnLeft,
            _ => Self::SharpTurnRight,
        }
    }

    fn turn_rate_radians_per_second(self, age_seconds: f32) -> f32 {
        match self {
            Self::Straight => 0.0,
            Self::CurveLeft => -0.14,
            Self::CurveRight => 0.14,
            Self::SharpTurnLeft | Self::SharpTurnRight if age_seconds < 0.42 => {
                let direction = if self == Self::SharpTurnLeft {
                    -1.0
                } else {
                    1.0
                };
                direction * 1.7 * (PI * age_seconds / 0.42).sin().max(0.0)
            }
            Self::SharpTurnLeft | Self::SharpTurnRight => 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
struct GroundSpeedProfile {
    multiplier: f32,
    stride_phase: f32,
    drift_phase: f32,
    stride_radians_per_second: f32,
    drift_radians_per_second: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
struct FlightSaccade {
    start_seconds: f32,
    duration_seconds: f32,
    angle_radians: f32,
}

impl FlightSaccade {
    fn turn_rate_radians_per_second(self, age_seconds: f32) -> f32 {
        let local = age_seconds - self.start_seconds;
        if !(0.0..self.duration_seconds).contains(&local) || self.angle_radians == 0.0 {
            return 0.0;
        }
        let phase = local / self.duration_seconds;
        self.angle_radians * PI / (2.0 * self.duration_seconds) * (PI * phase).sin()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
struct FlightPathProfile {
    speed_multiplier: f32,
    speed_phase: f32,
    speed_radians_per_second: f32,
    drift_radians_per_second: f32,
    saccades: [FlightSaccade; 2],
}

impl Default for FlightPathProfile {
    fn default() -> Self {
        Self::for_bout(0)
    }
}

impl FlightPathProfile {
    fn for_bout(sequence: u64) -> Self {
        let key = motor_mix64(sequence ^ 0x464C_4947_4854_5631);
        let unit = |shift: u32| (key.rotate_right(shift) & 0xffff) as f32 / 65_535.0;
        let direction = |bit: u32| if (key >> bit) & 1 == 0 { -1.0 } else { 1.0 };
        // Every accepted Flight duration is at least 594 ms. Keep the first
        // saccade wholly inside even the shortest bout so "one guaranteed"
        // remains a tested motor contract, not a probabilistic observation.
        let first_start = 0.24 + unit(7) * 0.16;
        let first_duration = 0.060 + unit(23) * 0.050;
        let second_start = first_start + 0.55 + unit(39) * 1.20;
        let second_duration = 0.065 + unit(53) * 0.050;
        let second_present = (key >> 5) & 0b11 != 0;
        Self {
            speed_multiplier: FLIGHT_SPEED_MULTIPLIERS
                [(key as usize) % FLIGHT_SPEED_MULTIPLIERS.len()],
            speed_phase: unit(13) * TAU,
            speed_radians_per_second: 1.2 + unit(31) * 2.4,
            drift_radians_per_second: (unit(47) - 0.5) * 0.24,
            saccades: [
                FlightSaccade {
                    start_seconds: first_start,
                    duration_seconds: first_duration,
                    angle_radians: direction(3) * (0.55 + unit(17) * 0.90),
                },
                FlightSaccade {
                    start_seconds: second_start,
                    duration_seconds: second_duration,
                    angle_radians: if second_present {
                        direction(37) * (0.45 + unit(29) * 0.85)
                    } else {
                        0.0
                    },
                },
            ],
        }
    }

    fn turn_rate_radians_per_second(self, age_seconds: f32) -> f32 {
        self.drift_radians_per_second
            + self
                .saccades
                .iter()
                .map(|saccade| saccade.turn_rate_radians_per_second(age_seconds))
                .sum::<f32>()
    }

    fn saccade_active(self, age_seconds: f32) -> bool {
        self.saccades.iter().any(|saccade| {
            let local = age_seconds - saccade.start_seconds;
            (0.0..saccade.duration_seconds).contains(&local) && saccade.angle_radians != 0.0
        })
    }

    fn speed_pixels_per_second(self, age_seconds: f32) -> f32 {
        let cruise = FLIGHT_SPEED_PIXELS_PER_SECOND
            * self.speed_multiplier
            * (1.0
                + 0.055 * (age_seconds * self.speed_radians_per_second + self.speed_phase).sin());
        let blend = smootherstep((age_seconds / FLIGHT_SPEED_BLEND_SECONDS).clamp(0.0, 1.0));
        lerp_scalar(ESCAPE_SPEED_PIXELS_PER_SECOND, cruise, blend)
    }
}

fn uncued_exploratory_heading(sequence: u64) -> f32 {
    let key = motor_mix64(sequence ^ 0x554E_4355_4544_5632);
    let unit = (key & 0xffff_ffff) as f32 / u32::MAX as f32;
    -PI + unit * TAU
}

fn uncued_course_selection_saccade(
    sequence: u64,
    current_heading_radians: f32,
    fallback_angle_radians: f32,
) -> f32 {
    let desired_heading = uncued_exploratory_heading(sequence);
    let error = wrapped_angle(desired_heading - current_heading_radians);
    if error.abs() < FLIGHT_COURSE_SELECTION_MIN_RADIANS {
        fallback_angle_radians
    } else {
        error.clamp(
            -FLIGHT_COURSE_SELECTION_MAX_RADIANS,
            FLIGHT_COURSE_SELECTION_MAX_RADIANS,
        )
    }
}

fn flight_edge_avoidance_turn_rate(
    position: Pos2,
    heading_radians: f32,
    movement_bounds: Rect,
    age_seconds: f32,
    profile: FlightPathProfile,
) -> f32 {
    if profile.saccade_active(age_seconds) {
        return 0.0;
    }

    let horizontal_margin = (movement_bounds.width().max(1.0) * FLIGHT_EDGE_AVOIDANCE_FRACTION)
        .clamp(
            FLIGHT_EDGE_AVOIDANCE_MIN_PIXELS,
            FLIGHT_EDGE_AVOIDANCE_MAX_PIXELS,
        );
    let vertical_margin = (movement_bounds.height().max(1.0) * FLIGHT_EDGE_AVOIDANCE_FRACTION)
        .clamp(
            FLIGHT_EDGE_AVOIDANCE_MIN_PIXELS,
            FLIGHT_EDGE_AVOIDANCE_MAX_PIXELS,
        );
    let mut inward = Vec2::ZERO;
    if position.x < movement_bounds.left() + horizontal_margin {
        inward.x += smootherstep(
            ((movement_bounds.left() + horizontal_margin - position.x) / horizontal_margin)
                .clamp(0.0, 1.0),
        );
    } else if position.x > movement_bounds.right() - horizontal_margin {
        inward.x -= smootherstep(
            ((position.x - (movement_bounds.right() - horizontal_margin)) / horizontal_margin)
                .clamp(0.0, 1.0),
        );
    }
    if position.y < movement_bounds.top() + vertical_margin {
        inward.y += smootherstep(
            ((movement_bounds.top() + vertical_margin - position.y) / vertical_margin)
                .clamp(0.0, 1.0),
        );
    } else if position.y > movement_bounds.bottom() - vertical_margin {
        inward.y -= smootherstep(
            ((position.y - (movement_bounds.bottom() - vertical_margin)) / vertical_margin)
                .clamp(0.0, 1.0),
        );
    }
    if inward.length_sq() <= f32::EPSILON {
        return 0.0;
    }

    let target_heading = inward.y.atan2(inward.x);
    let heading_error = wrapped_angle(target_heading - heading_radians);
    let proximity = inward.length().min(1.0);
    (heading_error * FLIGHT_EDGE_AVOIDANCE_GAIN).clamp(
        -FLIGHT_EDGE_AVOIDANCE_MAX_TURN_RADIANS_PER_SECOND,
        FLIGHT_EDGE_AVOIDANCE_MAX_TURN_RADIANS_PER_SECOND,
    ) * proximity
}

impl Default for GroundSpeedProfile {
    fn default() -> Self {
        Self::for_bout(0)
    }
}

impl GroundSpeedProfile {
    fn for_bout(sequence: u64) -> Self {
        let key = motor_mix64(sequence ^ 0x5350_4545_445F_5631);
        let multiplier = WALK_SPEED_MULTIPLIERS[(key as usize) % WALK_SPEED_MULTIPLIERS.len()];
        let phase = |shift: u32| ((key.rotate_right(shift) & 0xffff) as f32 / 65_535.0) * TAU;
        Self {
            multiplier,
            stride_phase: phase(11),
            drift_phase: phase(37),
            stride_radians_per_second: 7.2 + ((key >> 16) & 0xff) as f32 / 255.0 * 5.4,
            drift_radians_per_second: 1.1 + ((key >> 40) & 0xff) as f32 / 255.0 * 1.8,
        }
    }

    fn onset(self, age_seconds: f32) -> f32 {
        smootherstep((age_seconds / WALK_SPEED_ONSET_SECONDS).clamp(0.0, 1.0))
    }

    fn speed_pixels_per_second(self, age_seconds: f32) -> f32 {
        let texture = 1.0
            + 0.09 * (age_seconds * self.stride_radians_per_second + self.stride_phase).sin()
            + 0.045 * (age_seconds * self.drift_radians_per_second + self.drift_phase).sin();
        WALK_SPEED_PIXELS_PER_SECOND * self.multiplier * self.onset(age_seconds) * texture.max(0.5)
    }
}

fn motor_mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Skin {
    Drosophila,
    #[default]
    Firefly,
}

impl Skin {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Drosophila => "Drosophila Natural",
            Self::Firefly => "MechoFly Prism",
        }
    }

    pub const fn cli(self) -> &'static str {
        match self {
            Self::Drosophila => "drosophila",
            Self::Firefly => "firefly",
        }
    }
}

impl FromStr for Skin {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "drosophila" => Ok(Self::Drosophila),
            // Preserve the historical CLI value while restoring Prism.
            "firefly" | "prism" => Ok(Self::Firefly),
            other => Err(format!("unknown skin {other:?}")),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PetMotion {
    pub screen_position: Pos2,
    /// Direction in screen coordinates. Zero points right; positive turns down.
    pub heading_radians: f32,
    speed_pixels_per_second: f32,
    pub animation_seconds: f32,
    pub behavior_age_seconds: f32,
    /// Height above the local virtual surface. This is depth (Z), not screen Y.
    pub altitude_pixels: f32,
    last_behavior: Behavior,
    behavior_start_altitude_pixels: f32,
    landing_start_position: Pos2,
    landing_target_position: Pos2,
    landing_start_velocity: Vec2,
    landing_start_heading: f32,
    landing_target_heading: f32,
    landing_start_altitude_pixels: f32,
    landing_active: bool,
    ground_bout_sequence: u64,
    ground_path_motif: GroundPathMotif,
    ground_speed_profile: GroundSpeedProfile,
    flight_bout_sequence: u64,
    flight_path_profile: FlightPathProfile,
    /// Opt-in presentation experiment. The canonical application and every
    /// frozen N4/N4.1 profile retain the recorded legacy flight path.
    pub natural_flight_motion: bool,
    pub paused: bool,
    pub reduced_motion: bool,
}

#[cfg(feature = "n6-product-checkpoint")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PetMotionCheckpoint {
    schema_version: u32,
    screen_position: [f32; 2],
    heading_radians: f32,
    speed_pixels_per_second: f32,
    animation_seconds: f32,
    behavior_age_seconds: f32,
    altitude_pixels: f32,
    last_behavior: Behavior,
    behavior_start_altitude_pixels: f32,
    landing_start_position: [f32; 2],
    landing_target_position: [f32; 2],
    landing_start_velocity: [f32; 2],
    landing_start_heading: f32,
    landing_target_heading: f32,
    landing_start_altitude_pixels: f32,
    landing_active: bool,
    ground_bout_sequence: u64,
    ground_path_motif: GroundPathMotif,
    ground_speed_profile: GroundSpeedProfile,
    flight_bout_sequence: u64,
    flight_path_profile: FlightPathProfile,
    natural_flight_motion: bool,
    paused: bool,
    reduced_motion: bool,
}

#[cfg(feature = "n6-product-checkpoint")]
impl PetMotionCheckpoint {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err("unsupported pet-motion checkpoint schema".to_owned());
        }
        let values = [
            self.screen_position[0],
            self.screen_position[1],
            self.heading_radians,
            self.speed_pixels_per_second,
            self.animation_seconds,
            self.behavior_age_seconds,
            self.altitude_pixels,
            self.behavior_start_altitude_pixels,
            self.landing_start_position[0],
            self.landing_start_position[1],
            self.landing_target_position[0],
            self.landing_target_position[1],
            self.landing_start_velocity[0],
            self.landing_start_velocity[1],
            self.landing_start_heading,
            self.landing_target_heading,
            self.landing_start_altitude_pixels,
            self.ground_speed_profile.multiplier,
            self.ground_speed_profile.stride_phase,
            self.ground_speed_profile.drift_phase,
            self.ground_speed_profile.stride_radians_per_second,
            self.ground_speed_profile.drift_radians_per_second,
            self.flight_path_profile.speed_multiplier,
            self.flight_path_profile.speed_phase,
            self.flight_path_profile.speed_radians_per_second,
            self.flight_path_profile.drift_radians_per_second,
            self.flight_path_profile.saccades[0].start_seconds,
            self.flight_path_profile.saccades[0].duration_seconds,
            self.flight_path_profile.saccades[0].angle_radians,
            self.flight_path_profile.saccades[1].start_seconds,
            self.flight_path_profile.saccades[1].duration_seconds,
            self.flight_path_profile.saccades[1].angle_radians,
        ];
        if values.iter().any(|value| !value.is_finite()) {
            return Err("pet-motion checkpoint contains a non-finite value".to_owned());
        }
        if self.behavior_age_seconds < 0.0 || self.altitude_pixels < 0.0 {
            return Err("pet-motion checkpoint contains a negative age or altitude".to_owned());
        }
        Ok(())
    }

    pub(crate) fn digest(&self) -> Result<String, String> {
        self.validate()?;
        let encoded = serde_json::to_vec(self)
            .map_err(|error| format!("cannot encode pet-motion checkpoint: {error}"))?;
        Ok(mechofly_core::sha256_hex([encoded]))
    }
}

impl Default for PetMotion {
    fn default() -> Self {
        Self {
            screen_position: Pos2::new(96.0, 640.0),
            heading_radians: 0.0,
            speed_pixels_per_second: 0.0,
            animation_seconds: 0.0,
            behavior_age_seconds: 0.0,
            altitude_pixels: 0.0,
            last_behavior: Behavior::Rest,
            behavior_start_altitude_pixels: 0.0,
            landing_start_position: Pos2::new(96.0, 640.0),
            landing_target_position: Pos2::new(96.0, 640.0),
            landing_start_velocity: Vec2::ZERO,
            landing_start_heading: 0.0,
            landing_target_heading: 0.0,
            landing_start_altitude_pixels: 0.0,
            landing_active: false,
            ground_bout_sequence: 0,
            ground_path_motif: GroundPathMotif::Straight,
            ground_speed_profile: GroundSpeedProfile::default(),
            flight_bout_sequence: 0,
            flight_path_profile: FlightPathProfile::default(),
            natural_flight_motion: false,
            paused: false,
            reduced_motion: false,
        }
    }
}

impl PetMotion {
    #[cfg(feature = "n6-product-checkpoint")]
    pub(crate) fn checkpoint(&self) -> PetMotionCheckpoint {
        PetMotionCheckpoint {
            schema_version: 1,
            screen_position: [self.screen_position.x, self.screen_position.y],
            heading_radians: self.heading_radians,
            speed_pixels_per_second: self.speed_pixels_per_second,
            animation_seconds: self.animation_seconds,
            behavior_age_seconds: self.behavior_age_seconds,
            altitude_pixels: self.altitude_pixels,
            last_behavior: self.last_behavior,
            behavior_start_altitude_pixels: self.behavior_start_altitude_pixels,
            landing_start_position: [self.landing_start_position.x, self.landing_start_position.y],
            landing_target_position: [
                self.landing_target_position.x,
                self.landing_target_position.y,
            ],
            landing_start_velocity: [self.landing_start_velocity.x, self.landing_start_velocity.y],
            landing_start_heading: self.landing_start_heading,
            landing_target_heading: self.landing_target_heading,
            landing_start_altitude_pixels: self.landing_start_altitude_pixels,
            landing_active: self.landing_active,
            ground_bout_sequence: self.ground_bout_sequence,
            ground_path_motif: self.ground_path_motif,
            ground_speed_profile: self.ground_speed_profile,
            flight_bout_sequence: self.flight_bout_sequence,
            flight_path_profile: self.flight_path_profile,
            natural_flight_motion: self.natural_flight_motion,
            paused: self.paused,
            reduced_motion: self.reduced_motion,
        }
    }

    #[cfg(feature = "n6-product-checkpoint")]
    pub(crate) fn from_checkpoint(checkpoint: &PetMotionCheckpoint) -> Result<Self, String> {
        checkpoint.validate()?;
        Ok(Self {
            screen_position: Pos2::new(
                checkpoint.screen_position[0],
                checkpoint.screen_position[1],
            ),
            heading_radians: checkpoint.heading_radians,
            speed_pixels_per_second: checkpoint.speed_pixels_per_second,
            animation_seconds: checkpoint.animation_seconds,
            behavior_age_seconds: checkpoint.behavior_age_seconds,
            altitude_pixels: checkpoint.altitude_pixels,
            last_behavior: checkpoint.last_behavior,
            behavior_start_altitude_pixels: checkpoint.behavior_start_altitude_pixels,
            landing_start_position: Pos2::new(
                checkpoint.landing_start_position[0],
                checkpoint.landing_start_position[1],
            ),
            landing_target_position: Pos2::new(
                checkpoint.landing_target_position[0],
                checkpoint.landing_target_position[1],
            ),
            landing_start_velocity: Vec2::new(
                checkpoint.landing_start_velocity[0],
                checkpoint.landing_start_velocity[1],
            ),
            landing_start_heading: checkpoint.landing_start_heading,
            landing_target_heading: checkpoint.landing_target_heading,
            landing_start_altitude_pixels: checkpoint.landing_start_altitude_pixels,
            landing_active: checkpoint.landing_active,
            ground_bout_sequence: checkpoint.ground_bout_sequence,
            ground_path_motif: checkpoint.ground_path_motif,
            ground_speed_profile: checkpoint.ground_speed_profile,
            flight_bout_sequence: checkpoint.flight_bout_sequence,
            flight_path_profile: checkpoint.flight_path_profile,
            natural_flight_motion: checkpoint.natural_flight_motion,
            paused: checkpoint.paused,
            reduced_motion: checkpoint.reduced_motion,
        })
    }

    /// Synchronize coordinates only while Windows is actively dragging the native window.
    /// Autonomous motion retains its floating-point accumulator between integer HWND updates.
    pub fn synchronize_native_drag(&mut self, dragging: bool, position: Option<Pos2>) {
        if dragging && let Some(position) = position {
            self.screen_position = position;
        }
    }

    #[cfg(feature = "n41-visual-review-b")]
    pub fn speed_pixels_per_second(&self) -> f32 {
        self.speed_pixels_per_second
    }

    #[cfg(feature = "n41-visual-review-b")]
    pub fn flight_maneuver_active(&self) -> bool {
        self.natural_flight_motion
            && self.last_behavior == Behavior::Flight
            && self
                .flight_path_profile
                .saccade_active(self.behavior_age_seconds)
    }

    pub fn advance(
        &mut self,
        dt: f32,
        behavior: Behavior,
        screen_origin: Pos2,
        screen_size: Vec2,
        held: bool,
        cursor_position: Option<Pos2>,
    ) {
        let dt = dt.clamp(0.0, 0.1);
        let width = screen_size.x.max(480.0);
        let height = screen_size.y.max(320.0);
        let left = screen_origin.x + 8.0;
        let top = screen_origin.y + 8.0;
        let right = (screen_origin.x + width - PET_WIDTH as f32 - 8.0).max(left);
        let bottom = (screen_origin.y + height - PET_HEIGHT as f32 - 8.0).max(top);
        let movement_bounds = Rect::from_min_max(Pos2::new(left, top), Pos2::new(right, bottom));

        if behavior != self.last_behavior {
            self.last_behavior = behavior;
            self.behavior_age_seconds = 0.0;
            self.behavior_start_altitude_pixels = self.altitude_pixels;
            if matches!(behavior, Behavior::Walk | Behavior::Reverse) {
                self.ground_bout_sequence = self.ground_bout_sequence.saturating_add(1);
                self.ground_path_motif = GroundPathMotif::for_bout(self.ground_bout_sequence);
                self.ground_speed_profile = GroundSpeedProfile::for_bout(self.ground_bout_sequence);
            }
            if behavior == Behavior::Flight && self.natural_flight_motion {
                self.flight_bout_sequence = self.flight_bout_sequence.saturating_add(1);
                let mut profile = FlightPathProfile::for_bout(self.flight_bout_sequence);
                profile.saccades[0].angle_radians = uncued_course_selection_saccade(
                    self.flight_bout_sequence,
                    self.heading_radians,
                    profile.saccades[0].angle_radians,
                );
                self.flight_path_profile = profile;
            }
            if behavior == Behavior::Landing {
                self.landing_start_position = self.screen_position;
                self.landing_start_velocity = if self.natural_flight_motion {
                    Vec2::angled(self.heading_radians) * self.speed_pixels_per_second
                } else {
                    Vec2::ZERO
                };
                self.landing_target_position = local_landing_target(
                    self.screen_position,
                    self.heading_radians,
                    if self.natural_flight_motion {
                        self.speed_pixels_per_second
                    } else {
                        0.0
                    },
                    left,
                    top,
                    right,
                    bottom,
                );
                self.landing_start_heading = self.heading_radians;
                self.landing_target_heading = landing_surface_heading(self.heading_radians);
                self.landing_start_altitude_pixels = self.altitude_pixels;
                self.landing_active = true;
            } else {
                self.landing_active = false;
            }
        }
        if self.paused || held {
            return;
        }
        self.animation_seconds += dt;
        self.behavior_age_seconds += dt;

        let center = self.screen_position + Vec2::new(PET_WIDTH as f32, PET_HEIGHT as f32) * 0.5;
        if behavior == Behavior::PreEscape
            && let Some(cursor) = cursor_position
        {
            let away = center - cursor;
            if away.length_sq() > 1.0 {
                self.heading_radians = wrapped_angle(away.y.atan2(away.x));
            }
        }

        self.speed_pixels_per_second = match behavior {
            Behavior::Walk => {
                self.altitude_pixels = 0.0;
                let age_end = self.behavior_age_seconds;
                let age_start = (age_end - dt).max(0.0);
                let turn_rate_start = self
                    .ground_path_motif
                    .turn_rate_radians_per_second(age_start)
                    * self.ground_speed_profile.onset(age_start);
                let turn_rate_end = self.ground_path_motif.turn_rate_radians_per_second(age_end)
                    * self.ground_speed_profile.onset(age_end);
                self.heading_radians = wrapped_angle(
                    self.heading_radians + (turn_rate_start + turn_rate_end) * 0.5 * dt,
                );
                (self.ground_speed_profile.speed_pixels_per_second(age_start)
                    + self.ground_speed_profile.speed_pixels_per_second(age_end))
                    * 0.5
            }
            Behavior::Reverse => {
                self.altitude_pixels = 0.0;
                REVERSE_SPEED_PIXELS_PER_SECOND
            }
            Behavior::PreEscape => {
                let progress =
                    (self.behavior_age_seconds / TAKEOFF_COMPLETION_SECONDS).clamp(0.0, 1.0);
                self.altitude_pixels = lerp_scalar(
                    self.behavior_start_altitude_pixels,
                    TAKEOFF_ALTITUDE_PIXELS,
                    smootherstep(progress),
                );
                ESCAPE_SPEED_PIXELS_PER_SECOND
            }
            Behavior::Flight => {
                let progress =
                    (self.behavior_age_seconds / FLIGHT_ALTITUDE_RISE_SECONDS).clamp(0.0, 1.0);
                self.altitude_pixels = lerp_scalar(
                    self.behavior_start_altitude_pixels,
                    CRUISE_ALTITUDE_PIXELS,
                    smootherstep(progress),
                );
                if self.natural_flight_motion {
                    let age_end = self.behavior_age_seconds;
                    let age_start = (age_end - dt).max(0.0);
                    let turn_rate_start = self
                        .flight_path_profile
                        .turn_rate_radians_per_second(age_start)
                        + flight_edge_avoidance_turn_rate(
                            self.screen_position,
                            self.heading_radians,
                            movement_bounds,
                            age_start,
                            self.flight_path_profile,
                        );
                    let turn_rate_end = self
                        .flight_path_profile
                        .turn_rate_radians_per_second(age_end)
                        + flight_edge_avoidance_turn_rate(
                            self.screen_position,
                            self.heading_radians,
                            movement_bounds,
                            age_end,
                            self.flight_path_profile,
                        );
                    self.heading_radians = wrapped_angle(
                        self.heading_radians + (turn_rate_start + turn_rate_end) * 0.5 * dt,
                    );
                    (self.flight_path_profile.speed_pixels_per_second(age_start)
                        + self.flight_path_profile.speed_pixels_per_second(age_end))
                        * 0.5
                } else {
                    self.heading_radians = wrapped_angle(
                        self.heading_radians
                            + (self.animation_seconds * 1.7).sin()
                                * (0.065 / REFERENCE_TICK_SECONDS)
                                * dt,
                    );
                    FLIGHT_SPEED_PIXELS_PER_SECOND
                }
            }
            Behavior::Landing => {
                if !self.landing_active {
                    self.landing_start_position = self.screen_position;
                    self.landing_start_velocity = if self.natural_flight_motion {
                        Vec2::angled(self.heading_radians) * self.speed_pixels_per_second
                    } else {
                        Vec2::ZERO
                    };
                    self.landing_target_position = local_landing_target(
                        self.screen_position,
                        self.heading_radians,
                        if self.natural_flight_motion {
                            self.speed_pixels_per_second
                        } else {
                            0.0
                        },
                        left,
                        top,
                        right,
                        bottom,
                    );
                    self.landing_start_heading = self.heading_radians;
                    self.landing_target_heading = landing_surface_heading(self.heading_radians);
                    self.landing_start_altitude_pixels = self.altitude_pixels;
                    self.landing_active = true;
                }
                let progress =
                    (self.behavior_age_seconds / LANDING_COMPLETION_SECONDS).clamp(0.0, 1.0);
                let eased = smootherstep(progress);
                self.screen_position = if self.natural_flight_motion {
                    hermite_position(
                        self.landing_start_position,
                        self.landing_target_position,
                        self.landing_start_velocity,
                        LANDING_COMPLETION_SECONDS,
                        progress,
                    )
                } else {
                    lerp_position(
                        self.landing_start_position,
                        self.landing_target_position,
                        eased,
                    )
                };
                self.altitude_pixels = lerp_scalar(self.landing_start_altitude_pixels, 0.0, eased);
                let heading_delta =
                    wrapped_angle(self.landing_target_heading - self.landing_start_heading);
                self.heading_radians =
                    wrapped_angle(self.landing_start_heading + heading_delta * eased);
                if self.natural_flight_motion {
                    hermite_velocity(
                        self.landing_start_position,
                        self.landing_target_position,
                        self.landing_start_velocity,
                        LANDING_COMPLETION_SECONDS,
                        progress,
                    )
                    .length()
                } else {
                    0.0
                }
            }
            Behavior::Alert => {
                self.altitude_pixels = 0.0;
                self.heading_radians = wrapped_angle(
                    self.heading_radians
                        + (self.animation_seconds * 19.0).sin()
                            * (0.18 / REFERENCE_TICK_SECONDS)
                            * dt,
                );
                NERVOUS_SPEED_PIXELS_PER_SECOND
            }
            Behavior::Rest | Behavior::Quiet | Behavior::Groom => {
                self.altitude_pixels = 0.0;
                0.0
            }
        };

        if behavior != Behavior::Landing {
            self.screen_position +=
                Vec2::angled(self.heading_radians) * self.speed_pixels_per_second * dt;
        }
        if behavior == Behavior::Landing && self.behavior_age_seconds >= LANDING_COMPLETION_SECONDS
        {
            self.screen_position = self.landing_target_position;
            self.heading_radians = self.landing_target_heading;
            self.altitude_pixels = 0.0;
            self.speed_pixels_per_second = 0.0;
        }

        let mut bounced_x = false;
        let mut bounced_y = false;
        if self.screen_position.x < left {
            self.screen_position.x = left;
            bounced_x = true;
        } else if self.screen_position.x > right {
            self.screen_position.x = right;
            bounced_x = true;
        }
        if self.screen_position.y < top {
            self.screen_position.y = top;
            bounced_y = true;
        } else if self.screen_position.y > bottom {
            self.screen_position.y = bottom;
            bounced_y = true;
        }
        if bounced_x {
            self.heading_radians = wrapped_angle(PI - self.heading_radians);
        }
        if bounced_y && behavior != Behavior::Landing {
            self.heading_radians = wrapped_angle(-self.heading_radians);
        }
    }
}

fn wrapped_angle(angle: f32) -> f32 {
    (angle + PI).rem_euclid(PI * 2.0) - PI
}

fn smootherstep(progress: f32) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    progress * progress * progress * (progress * (progress * 6.0 - 15.0) + 10.0)
}

fn lerp_position(start: Pos2, end: Pos2, progress: f32) -> Pos2 {
    Pos2::new(
        start.x + (end.x - start.x) * progress,
        start.y + (end.y - start.y) * progress,
    )
}

fn hermite_position(
    start: Pos2,
    end: Pos2,
    start_velocity: Vec2,
    duration_seconds: f32,
    progress: f32,
) -> Pos2 {
    let progress = progress.clamp(0.0, 1.0);
    let p2 = progress * progress;
    let p3 = p2 * progress;
    let h00 = 2.0 * p3 - 3.0 * p2 + 1.0;
    let h10 = p3 - 2.0 * p2 + progress;
    let h01 = -2.0 * p3 + 3.0 * p2;
    let tangent = start_velocity * duration_seconds.max(f32::EPSILON);
    Pos2::new(
        h00 * start.x + h10 * tangent.x + h01 * end.x,
        h00 * start.y + h10 * tangent.y + h01 * end.y,
    )
}

fn hermite_velocity(
    start: Pos2,
    end: Pos2,
    start_velocity: Vec2,
    duration_seconds: f32,
    progress: f32,
) -> Vec2 {
    let progress = progress.clamp(0.0, 1.0);
    let p2 = progress * progress;
    let dh00 = 6.0 * p2 - 6.0 * progress;
    let dh10 = 3.0 * p2 - 4.0 * progress + 1.0;
    let dh01 = -6.0 * p2 + 6.0 * progress;
    let duration = duration_seconds.max(f32::EPSILON);
    let tangent = start_velocity * duration;
    Vec2::new(
        (dh00 * start.x + dh10 * tangent.x + dh01 * end.x) / duration,
        (dh00 * start.y + dh10 * tangent.y + dh01 * end.y) / duration,
    )
}

fn lerp_scalar(start: f32, end: f32, progress: f32) -> f32 {
    start + (end - start) * progress
}

fn landing_surface_heading(heading: f32) -> f32 {
    if heading.cos() >= 0.0 { 0.0 } else { PI }
}

fn local_landing_target(
    position: Pos2,
    heading: f32,
    approach_speed_pixels_per_second: f32,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
) -> Pos2 {
    let vertical_span = (bottom - top).max(1.0);
    let bottom_margin = (vertical_span * LANDING_SAFE_BOTTOM_FRACTION).clamp(
        LANDING_SAFE_BOTTOM_MIN_PIXELS,
        LANDING_SAFE_BOTTOM_MAX_PIXELS,
    );
    let safe_top = (top + LANDING_SAFE_TOP_MARGIN_PIXELS).min(bottom);
    let safe_bottom = (bottom - bottom_margin).max(safe_top);
    let approach = (approach_speed_pixels_per_second.abs() * FLIGHT_SPEED_BLEND_SECONDS)
        .clamp(LANDING_APPROACH_MIN_PIXELS, LANDING_APPROACH_MAX_PIXELS);
    let candidate = position + Vec2::angled(heading) * approach;
    Pos2::new(
        candidate.x.clamp(left, right),
        candidate.y.clamp(safe_top, safe_bottom),
    )
}

fn visual_altitude_for_behavior(behavior: Behavior, age_seconds: f32) -> f32 {
    match behavior {
        Behavior::PreEscape => {
            TAKEOFF_ALTITUDE_PIXELS
                * smootherstep((age_seconds / TAKEOFF_COMPLETION_SECONDS).clamp(0.0, 1.0))
        }
        Behavior::Flight => lerp_scalar(
            TAKEOFF_ALTITUDE_PIXELS,
            CRUISE_ALTITUDE_PIXELS,
            smootherstep((age_seconds / FLIGHT_ALTITUDE_RISE_SECONDS).clamp(0.0, 1.0)),
        ),
        Behavior::Landing => {
            CRUISE_ALTITUDE_PIXELS
                * (1.0 - smootherstep((age_seconds / LANDING_COMPLETION_SECONDS).clamp(0.0, 1.0)))
        }
        _ => 0.0,
    }
}

#[derive(Clone, Debug)]
pub struct MotionSelfTest {
    pub passed: bool,
    pub walking_translation_pixels: f32,
    pub high_refresh_native_round_trip_translation_pixels: f32,
    pub native_drag_synchronization: bool,
    pub escape_translation_pixels: f32,
    pub flight_path_pixels: f32,
    pub flight_horizontal_pixels: f32,
    pub flight_vertical_pixels: f32,
    pub flight_altitude_pixels: f32,
    pub landing_descent_pixels: f32,
    pub landing_screen_vertical_displacement_pixels: f32,
    pub landing_bottom_clearance_pixels: f32,
    pub landing_reached_surface: bool,
    pub landing_target_local: bool,
    pub landing_avoids_bottom_edge: bool,
    pub touchdown_altitude_pixels: f32,
    pub landing_first_step_pixels: f32,
    pub landing_max_step_pixels: f32,
    pub landing_completion_step_pixels: f32,
    pub landing_to_rest_step_pixels: f32,
    pub landing_refresh_rate_position_error_pixels: f32,
    pub landing_refresh_rate_heading_error_radians: f32,
    pub landing_refresh_rate_altitude_error_pixels: f32,
    pub landing_refresh_rate_invariant: bool,
    pub landing_position_continuous: bool,
    pub screen_space_shadow_below_body: bool,
    pub shadow_altitude_monotonic: bool,
    pub shadow_center_heading_invariant: bool,
    pub shadow_heading_aligned: bool,
    pub shadow_heading_max_error_radians: f32,
    pub shadow_footprint_anisotropic: bool,
    pub spatial_depth_contract_passed: bool,
    pub teleport_detected: bool,
}

pub fn run_motion_self_test() -> MotionSelfTest {
    let origin = Pos2::ZERO;
    let screen = Vec2::new(1_920.0, 1_080.0);
    let bottom = screen.y - PET_HEIGHT as f32 - 8.0;

    let mut walking = PetMotion {
        screen_position: Pos2::new(220.0, bottom - 96.0),
        ..PetMotion::default()
    };
    let walking_start = walking.screen_position;
    for _ in 0..120 {
        walking.advance(1.0 / 60.0, Behavior::Walk, origin, screen, false, None);
    }
    let walking_translation_pixels = walking.screen_position.distance(walking_start);

    let mut high_refresh = PetMotion {
        screen_position: Pos2::new(220.0, bottom - 96.0),
        ..PetMotion::default()
    };
    let high_refresh_start = high_refresh.screen_position;
    for _ in 0..480 {
        let integer_native_position = Pos2::new(
            high_refresh.screen_position.x.round(),
            high_refresh.screen_position.y.round(),
        );
        high_refresh.synchronize_native_drag(false, Some(integer_native_position));
        high_refresh.advance(1.0 / 240.0, Behavior::Walk, origin, screen, false, None);
    }
    let high_refresh_native_round_trip_translation_pixels =
        high_refresh.screen_position.distance(high_refresh_start);
    let dragged_position = Pos2::new(777.0, 333.0);
    high_refresh.synchronize_native_drag(true, Some(dragged_position));
    let native_drag_synchronization = high_refresh.screen_position == dragged_position;

    let mut airborne = PetMotion {
        screen_position: Pos2::new(760.0, 610.0),
        ..PetMotion::default()
    };
    let cursor = airborne.screen_position + Vec2::new(-90.0, 200.0);
    let escape_start = airborne.screen_position;
    for _ in 0..12 {
        airborne.advance(
            1.0 / 60.0,
            Behavior::PreEscape,
            origin,
            screen,
            false,
            Some(cursor),
        );
    }
    let escape_translation_pixels = airborne.screen_position.distance(escape_start);

    let flight_start = airborne.screen_position;
    let mut flight_path_pixels = 0.0;
    for _ in 0..240 {
        let before = airborne.screen_position;
        airborne.advance(1.0 / 60.0, Behavior::Flight, origin, screen, false, None);
        flight_path_pixels += airborne.screen_position.distance(before);
    }
    let flight_horizontal_pixels = (airborne.screen_position.x - flight_start.x).abs();
    let flight_vertical_pixels = (airborne.screen_position.y - flight_start.y).abs();
    let flight_altitude_pixels = airborne.altitude_pixels;

    airborne.screen_position.y = bottom - 4.0;
    airborne.altitude_pixels = CRUISE_ALTITUDE_PIXELS;
    let landing_start = airborne.screen_position;
    let landing_start_altitude = airborne.altitude_pixels;
    let mut landing_previous = landing_start;
    let mut landing_first_step_pixels = 0.0_f32;
    let mut landing_max_step_pixels = 0.0_f32;
    let mut landing_completion_step_pixels = 0.0_f32;
    for frame in 1..=30 {
        airborne.advance(1.0 / 60.0, Behavior::Landing, origin, screen, false, None);
        let step = airborne.screen_position.distance(landing_previous);
        if frame == 1 {
            landing_first_step_pixels = step;
        }
        if frame == 30 {
            landing_completion_step_pixels = step;
        }
        landing_max_step_pixels = landing_max_step_pixels.max(step);
        landing_previous = airborne.screen_position;
    }
    let landing_descent_pixels = landing_start_altitude - airborne.altitude_pixels;
    let landing_screen_vertical_displacement_pixels =
        (airborne.screen_position.y - landing_start.y).abs();
    let landing_bottom_clearance_pixels = bottom - airborne.screen_position.y;
    let landing_reached_surface = airborne.altitude_pixels.abs() <= 0.01;
    let landing_target_local = airborne.screen_position.distance(landing_start) <= 140.0;
    let landing_avoids_bottom_edge = landing_bottom_clearance_pixels >= 48.0;
    let touchdown_altitude_pixels = airborne.altitude_pixels;
    let touchdown = airborne.screen_position;
    airborne.advance(1.0 / 60.0, Behavior::Rest, origin, screen, false, None);
    let landing_to_rest_step_pixels = airborne.screen_position.distance(touchdown);

    let mut landing_60_hz = PetMotion {
        screen_position: Pos2::new(760.0, bottom - 4.0),
        altitude_pixels: CRUISE_ALTITUDE_PIXELS,
        ..PetMotion::default()
    };
    let mut landing_120_hz = landing_60_hz.clone();
    for _ in 0..30 {
        landing_60_hz.advance(1.0 / 60.0, Behavior::Landing, origin, screen, false, None);
    }
    for _ in 0..60 {
        landing_120_hz.advance(1.0 / 120.0, Behavior::Landing, origin, screen, false, None);
    }
    let landing_refresh_rate_position_error_pixels = landing_60_hz
        .screen_position
        .distance(landing_120_hz.screen_position);
    let landing_refresh_rate_heading_error_radians =
        wrapped_angle(landing_60_hz.heading_radians - landing_120_hz.heading_radians).abs();
    let landing_refresh_rate_altitude_error_pixels =
        (landing_60_hz.altitude_pixels - landing_120_hz.altitude_pixels).abs();
    let landing_refresh_rate_invariant = landing_refresh_rate_position_error_pixels <= 0.01
        && landing_refresh_rate_heading_error_radians <= 0.001
        && landing_refresh_rate_altitude_error_pixels <= 0.01;

    let landing_position_continuous = landing_first_step_pixels <= 2.5
        && landing_max_step_pixels <= 25.0
        && landing_completion_step_pixels <= 1.0
        && landing_to_rest_step_pixels <= 0.01;

    let ground_shadow = shadow_projection(0.0, 0.0);
    let flight_shadow = shadow_projection(CRUISE_ALTITUDE_PIXELS, 0.0);
    let screen_space_shadow_below_body = ground_shadow.center[1] > CENTER[1] + 40.0
        && flight_shadow.center[1] > ground_shadow.center[1];
    let shadow_altitude_monotonic = flight_shadow.core_alpha < ground_shadow.core_alpha
        && flight_shadow.core_radii[0] < ground_shadow.core_radii[0]
        && flight_shadow.center[1] > ground_shadow.center[1];
    let shadow_footprint_anisotropic = ground_shadow.core_radii[0]
        > ground_shadow.core_radii[1] * 3.0
        && ground_shadow.halo_radii[0] > ground_shadow.halo_radii[1] * 3.0;
    let headings = [0.0, 0.37, PI * 0.5, -1.13, PI - 0.17];
    let reference = shadow_projection(CRUISE_ALTITUDE_PIXELS, headings[0]);
    let mut shadow_center_heading_invariant = true;
    let mut shadow_heading_aligned = true;
    let mut shadow_heading_max_error_radians = 0.0_f32;
    for heading in headings {
        let projection = shadow_projection(CRUISE_ALTITUDE_PIXELS, heading);
        shadow_center_heading_invariant &= projection.center == reference.center
            && projection.core_radii == reference.core_radii
            && projection.halo_radii == reference.halo_radii
            && projection.core_alpha == reference.core_alpha
            && projection.halo_alpha == reference.halo_alpha;

        let mut scene = SceneBuilder::new(heading, 0.0);
        draw_contact_shadow(&mut scene, CRUISE_ALTITUDE_PIXELS, heading);
        let (halo_angle, core_angle) = match (&scene.primitives[0], &scene.primitives[1]) {
            (Primitive::Ellipse { angle: halo, .. }, Primitive::Ellipse { angle: core, .. }) => {
                (*halo, *core)
            }
            _ => panic!("contact shadow must begin with aligned halo and core ellipses"),
        };
        let projection_error = shadow_axis_error_radians(projection.angle, heading);
        let halo_error = shadow_axis_error_radians(halo_angle, heading);
        let core_error = shadow_axis_error_radians(core_angle, heading);
        let pair_error = shadow_axis_error_radians(halo_angle, core_angle);
        shadow_heading_max_error_radians = shadow_heading_max_error_radians
            .max(projection_error)
            .max(halo_error)
            .max(core_error)
            .max(pair_error);
        shadow_heading_aligned &= projection_error <= 0.001
            && halo_error <= 0.001
            && core_error <= 0.001
            && pair_error <= 0.001;
    }
    let spatial_depth_contract_passed = flight_altitude_pixels >= 60.0
        && landing_descent_pixels >= 60.0
        && landing_reached_surface
        && landing_target_local
        && landing_avoids_bottom_edge
        && touchdown_altitude_pixels.abs() <= 0.01
        && screen_space_shadow_below_body
        && shadow_altitude_monotonic
        && shadow_center_heading_invariant
        && shadow_heading_aligned
        && shadow_footprint_anisotropic;
    let teleport_detected = !landing_position_continuous;

    MotionSelfTest {
        passed: walking_translation_pixels > 100.0
            && high_refresh_native_round_trip_translation_pixels > 100.0
            && native_drag_synchronization
            && escape_translation_pixels > 90.0
            && flight_path_pixels > 900.0
            && flight_horizontal_pixels > 150.0
            && flight_vertical_pixels > 100.0
            && spatial_depth_contract_passed
            && landing_position_continuous
            && landing_refresh_rate_invariant
            && !teleport_detected,
        walking_translation_pixels,
        high_refresh_native_round_trip_translation_pixels,
        native_drag_synchronization,
        escape_translation_pixels,
        flight_path_pixels,
        flight_horizontal_pixels,
        flight_vertical_pixels,
        flight_altitude_pixels,
        landing_descent_pixels,
        landing_screen_vertical_displacement_pixels,
        landing_bottom_clearance_pixels,
        landing_reached_surface,
        landing_target_local,
        landing_avoids_bottom_edge,
        touchdown_altitude_pixels,
        landing_first_step_pixels,
        landing_max_step_pixels,
        landing_completion_step_pixels,
        landing_to_rest_step_pixels,
        landing_refresh_rate_position_error_pixels,
        landing_refresh_rate_heading_error_radians,
        landing_refresh_rate_altitude_error_pixels,
        landing_refresh_rate_invariant,
        landing_position_continuous,
        screen_space_shadow_below_body,
        shadow_altitude_monotonic,
        shadow_center_heading_invariant,
        shadow_heading_aligned,
        shadow_heading_max_error_radians,
        shadow_footprint_anisotropic,
        spatial_depth_contract_passed,
        teleport_detected,
    }
}

#[derive(Clone, Copy)]
struct Rgba(u8, u8, u8, u8);

impl Rgba {
    const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self(red, green, blue, 255)
    }

    fn egui(self) -> Color32 {
        Color32::from_rgba_unmultiplied(self.0, self.1, self.2, self.3)
    }
}

enum Primitive {
    Ellipse {
        center: [f32; 2],
        radii: [f32; 2],
        angle: f32,
        fill: Rgba,
        stroke: Option<(Rgba, f32)>,
    },
    Line {
        from: [f32; 2],
        to: [f32; 2],
        width: f32,
        color: Rgba,
    },
    Polygon {
        points: Vec<[f32; 2]>,
        fill: Rgba,
        stroke: Option<(Rgba, f32)>,
    },
}

type CubicSegment = ([f32; 2], [f32; 2], [f32; 2], [f32; 2]);

struct SceneBuilder {
    rotation: f32,
    screen_offset_y: f32,
    primitives: Vec<Primitive>,
}

impl SceneBuilder {
    fn new(heading: f32, screen_offset_y: f32) -> Self {
        Self {
            // The local head is at negative X, hence the half turn.
            rotation: heading + PI,
            screen_offset_y,
            primitives: Vec::with_capacity(220),
        }
    }

    fn point(&self, x: f32, y: f32) -> [f32; 2] {
        let cosine = self.rotation.cos();
        let sine = self.rotation.sin();
        [
            CENTER[0] + x * cosine - y * sine,
            CENTER[1] + x * sine + y * cosine + self.screen_offset_y,
        ]
    }

    fn ellipse(
        &mut self,
        center: [f32; 2],
        radii: [f32; 2],
        angle: f32,
        fill: Rgba,
        stroke: Option<(Rgba, f32)>,
    ) {
        self.primitives.push(Primitive::Ellipse {
            center: self.point(center[0], center[1]),
            radii,
            angle: angle + self.rotation,
            fill,
            stroke,
        });
    }

    fn screen_ellipse(
        &mut self,
        center: [f32; 2],
        radii: [f32; 2],
        angle: f32,
        fill: Rgba,
        stroke: Option<(Rgba, f32)>,
    ) {
        self.primitives.push(Primitive::Ellipse {
            center,
            radii,
            angle,
            fill,
            stroke,
        });
    }

    fn line(&mut self, from: [f32; 2], to: [f32; 2], width: f32, color: Rgba) {
        self.primitives.push(Primitive::Line {
            from: self.point(from[0], from[1]),
            to: self.point(to[0], to[1]),
            width,
            color,
        });
    }

    fn polygon(&mut self, points: &[[f32; 2]], fill: Rgba, stroke: Option<(Rgba, f32)>) {
        let points = points
            .iter()
            .map(|point| self.point(point[0], point[1]))
            .collect();
        self.primitives.push(Primitive::Polygon {
            points,
            fill,
            stroke,
        });
    }
}

#[derive(Clone, Copy)]
struct Palette {
    outline: Rgba,
    highlight: Rgba,
    leg: Rgba,
    wing_edge: Rgba,
    head_core: Rgba,
    head_rim: Rgba,
    eye_core: Rgba,
    eye_rim: Rgba,
    thorax_core: Rgba,
    thorax_edge: Rgba,
}

fn palette(skin: Skin) -> Palette {
    match skin {
        Skin::Firefly => Palette {
            outline: Rgba(26, 67, 45, 245),
            highlight: Rgba(185, 218, 67, 225),
            leg: Rgba(41, 91, 49, 248),
            wing_edge: Rgba(139, 99, 211, 205),
            head_core: Rgba::rgb(23, 74, 56),
            head_rim: Rgba::rgb(163, 177, 56),
            eye_core: Rgba::rgb(202, 75, 31),
            eye_rim: Rgba::rgb(240, 155, 48),
            thorax_core: Rgba::rgb(207, 89, 36),
            thorax_edge: Rgba::rgb(241, 169, 54),
        },
        Skin::Drosophila => Palette {
            outline: Rgba(63, 35, 25, 240),
            highlight: Rgba(239, 178, 93, 210),
            leg: Rgba(73, 45, 34, 240),
            wing_edge: Rgba(114, 157, 169, 178),
            head_core: Rgba::rgb(126, 76, 43),
            head_rim: Rgba::rgb(202, 133, 66),
            eye_core: Rgba::rgb(190, 42, 40),
            eye_rim: Rgba::rgb(247, 138, 70),
            thorax_core: Rgba::rgb(151, 93, 42),
            thorax_edge: Rgba::rgb(217, 148, 64),
        },
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_pet_at_age_with_altitude(
    painter: &Painter,
    rect: Rect,
    skin: Skin,
    behavior: Behavior,
    phase: f32,
    behavior_age_seconds: f32,
    heading: f32,
    altitude_pixels: f32,
    reduced_motion: bool,
) {
    let scene = pet_scene_at_altitude(
        skin,
        behavior,
        phase,
        behavior_age_seconds,
        heading,
        altitude_pixels,
        reduced_motion,
    );
    let scale = (rect.width() / PET_WIDTH as f32).min(rect.height() / PET_HEIGHT as f32);
    let origin = rect.center() - Vec2::new(PET_WIDTH as f32, PET_HEIGHT as f32) * scale * 0.5;
    let position = |point: [f32; 2]| origin + Vec2::new(point[0], point[1]) * scale;

    for primitive in scene {
        match primitive {
            Primitive::Ellipse {
                center,
                radii,
                angle,
                fill,
                stroke,
            } => {
                let points = ellipse_points(center, radii, angle, 48)
                    .into_iter()
                    .map(position)
                    .collect();
                painter.add(Shape::convex_polygon(
                    points,
                    fill.egui(),
                    stroke
                        .map(|(color, width)| Stroke::new(width * scale, color.egui()))
                        .unwrap_or(Stroke::NONE),
                ));
            }
            Primitive::Line {
                from,
                to,
                width,
                color,
            } => {
                painter.line_segment(
                    [position(from), position(to)],
                    Stroke::new(width * scale, color.egui()),
                );
            }
            Primitive::Polygon {
                points,
                fill,
                stroke,
            } => {
                painter.add(Shape::convex_polygon(
                    points.into_iter().map(position).collect(),
                    fill.egui(),
                    stroke
                        .map(|(color, width)| Stroke::new(width * scale, color.egui()))
                        .unwrap_or(Stroke::NONE),
                ));
            }
        }
    }
}

/// Render into the premultiplied BGRA format required by UpdateLayeredWindow.
pub fn render_pet_bgra(
    output: &mut [u8],
    skin: Skin,
    behavior: Behavior,
    phase: f32,
    heading: f32,
    reduced_motion: bool,
) {
    render_pet_bgra_at_age(
        output,
        skin,
        behavior,
        phase,
        phase,
        heading,
        reduced_motion,
    );
}

#[allow(clippy::too_many_arguments)]
pub fn render_pet_bgra_at_age(
    output: &mut [u8],
    skin: Skin,
    behavior: Behavior,
    phase: f32,
    behavior_age_seconds: f32,
    heading: f32,
    reduced_motion: bool,
) {
    render_pet_bgra_at_age_with_altitude(
        output,
        skin,
        behavior,
        phase,
        behavior_age_seconds,
        heading,
        visual_altitude_for_behavior(behavior, behavior_age_seconds),
        reduced_motion,
    );
}

#[allow(clippy::too_many_arguments)]
pub fn render_pet_bgra_at_age_with_altitude(
    output: &mut [u8],
    skin: Skin,
    behavior: Behavior,
    phase: f32,
    behavior_age_seconds: f32,
    heading: f32,
    altitude_pixels: f32,
    reduced_motion: bool,
) {
    assert_eq!(output.len(), PET_WIDTH * PET_HEIGHT * 4);
    let scene = pet_scene_at_altitude(
        skin,
        behavior,
        phase,
        behavior_age_seconds,
        heading,
        altitude_pixels,
        reduced_motion,
    );
    let mut canvas = RasterCanvas::new(PET_WIDTH * RASTER_SCALE, PET_HEIGHT * RASTER_SCALE);
    for primitive in &scene {
        canvas.draw(primitive, RASTER_SCALE as f32);
    }
    canvas.downsample(output, RASTER_SCALE);
}

fn pet_scene(
    skin: Skin,
    behavior: Behavior,
    phase: f32,
    behavior_age_seconds: f32,
    heading: f32,
    reduced_motion: bool,
) -> Vec<Primitive> {
    pet_scene_at_altitude(
        skin,
        behavior,
        phase,
        behavior_age_seconds,
        heading,
        visual_altitude_for_behavior(behavior, behavior_age_seconds),
        reduced_motion,
    )
}

#[allow(clippy::too_many_arguments)]
fn pet_scene_at_altitude(
    skin: Skin,
    behavior: Behavior,
    phase: f32,
    behavior_age_seconds: f32,
    heading: f32,
    altitude_pixels: f32,
    reduced_motion: bool,
) -> Vec<Primitive> {
    let time = if matches!(behavior, Behavior::Rest | Behavior::Quiet) || reduced_motion {
        0.0
    } else {
        phase
    };
    let gait = if matches!(behavior, Behavior::Walk | Behavior::Reverse) {
        time * (0.12 / REFERENCE_TICK_SECONDS) * TAU
    } else {
        0.0
    };
    let screen_offset = match behavior {
        Behavior::Walk | Behavior::Reverse => (time * 8.0).sin() * 0.55,
        Behavior::PreEscape => (time * 15.0).sin() * 1.15,
        Behavior::Flight => (time * 7.0).sin() * 2.4,
        Behavior::Landing => (time * 4.0).sin() * 0.8,
        _ => 0.0,
    };
    let grooming_program = (behavior == Behavior::Groom).then(|| {
        grooming_program_at((behavior_age_seconds / REFERENCE_TICK_SECONDS).floor() as u32)
    });
    let colors = palette(skin);
    let mut scene = SceneBuilder::new(heading, screen_offset);
    draw_behavior_field(&mut scene, behavior, time, skin);
    draw_motion_trails(&mut scene, behavior, time);
    draw_contact_shadow(&mut scene, altitude_pixels, heading);
    draw_wings(&mut scene, behavior, time, grooming_program, colors);
    draw_legs(
        &mut scene,
        skin,
        behavior,
        gait,
        behavior_age_seconds,
        true,
        colors,
    );
    draw_abdomen(&mut scene, skin, behavior, time, colors);
    if skin == Skin::Firefly {
        draw_prism_elytra(&mut scene, behavior, time, grooming_program);
    }
    draw_thorax(&mut scene, skin, time, colors);
    draw_legs(
        &mut scene,
        skin,
        behavior,
        gait,
        behavior_age_seconds,
        false,
        colors,
    );
    draw_head(&mut scene, skin, behavior, time, colors);
    draw_antennae(&mut scene, skin, behavior, time, colors);
    scene.primitives
}

fn behavior_accent(behavior: Behavior, skin: Skin) -> Rgba {
    if skin == Skin::Drosophila {
        match behavior {
            Behavior::PreEscape | Behavior::Flight => Rgba(245, 93, 55, 210),
            Behavior::Groom => Rgba(220, 144, 64, 190),
            Behavior::Landing => Rgba(104, 181, 158, 190),
            _ => Rgba(164, 117, 64, 160),
        }
    } else {
        match behavior {
            Behavior::PreEscape => Rgba(255, 119, 70, 220),
            Behavior::Flight => Rgba(68, 210, 245, 210),
            Behavior::Landing => Rgba(255, 194, 74, 210),
            Behavior::Groom => Rgba(184, 128, 227, 200),
            Behavior::Reverse => Rgba(87, 155, 229, 190),
            Behavior::Alert => Rgba(244, 191, 62, 190),
            _ => Rgba(135, 226, 104, 175),
        }
    }
}

fn draw_behavior_field(scene: &mut SceneBuilder, behavior: Behavior, time: f32, skin: Skin) {
    if !matches!(
        behavior,
        Behavior::Alert | Behavior::PreEscape | Behavior::Landing
    ) {
        return;
    }
    let accent = behavior_accent(behavior, skin);
    let pulse = 0.5 + 0.5 * (time * 3.2).sin();
    scene.ellipse(
        [5.0, 0.0],
        [105.0 + 4.0 + pulse * 6.0, 73.0 + 4.0 + pulse * 6.0],
        0.0,
        Rgba(0, 0, 0, 0),
        Some((Rgba(accent.0, accent.1, accent.2, 42), 1.8)),
    );
    for index in 0..5 {
        let angle = time * (0.56 + index as f32 * 0.06) + index as f32 * 1.21;
        let point = [
            angle.cos() * (83.0 + index as f32 * 3.0),
            angle.sin() * (57.0 + index as f32 * 2.0),
        ];
        let size = 1.6 + (index % 2) as f32 * 0.8;
        scene.ellipse(point, [size, size], 0.0, accent, None);
    }
}

fn draw_motion_trails(scene: &mut SceneBuilder, behavior: Behavior, time: f32) {
    if !matches!(behavior, Behavior::PreEscape | Behavior::Flight) {
        return;
    }
    let drift = (time * 6.0).sin() * 2.2;
    for index in 0..3 {
        let y = -18.0 + index as f32 * 18.0 + drift;
        scene.line(
            [82.0 + index as f32 * 4.0, y],
            [112.0 + index as f32 * 7.0, y - 1.5],
            1.7,
            Rgba(194, 255, 105, 96),
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ShadowProjection {
    center: [f32; 2],
    core_radii: [f32; 2],
    halo_radii: [f32; 2],
    angle: f32,
    core_alpha: u8,
    halo_alpha: u8,
}

fn wrapped_axis_angle(angle: f32) -> f32 {
    angle.rem_euclid(PI)
}

fn shadow_axis_error_radians(first: f32, second: f32) -> f32 {
    let delta = (wrapped_axis_angle(first) - wrapped_axis_angle(second)).abs();
    delta.min(PI - delta)
}

fn shadow_projection(altitude_pixels: f32, heading_radians: f32) -> ShadowProjection {
    let normalized = (altitude_pixels.max(0.0) / CRUISE_ALTITUDE_PIXELS).clamp(0.0, 1.4);
    let unit = normalized.min(1.0);
    let core_scale = 1.0 - 0.36 * unit;
    let halo_scale = 1.0 - 0.24 * unit;
    ShadowProjection {
        // The projection center remains in screen/world space beneath the fly.
        // Only the anisotropic footprint follows the body's in-plane axis.
        center: [CENTER[0], CENTER[1] + 52.0 + normalized * 22.0],
        core_radii: [78.0 * core_scale, 9.0 * (1.0 - 0.22 * unit)],
        halo_radii: [86.0 * halo_scale, 14.0 * (1.0 - 0.12 * unit)],
        angle: wrapped_axis_angle(heading_radians),
        core_alpha: (72.0 - 50.0 * unit).round().clamp(18.0, 72.0) as u8,
        halo_alpha: (22.0 - 14.0 * unit).round().clamp(6.0, 22.0) as u8,
    }
}

fn draw_contact_shadow(scene: &mut SceneBuilder, altitude_pixels: f32, heading_radians: f32) {
    let projection = shadow_projection(altitude_pixels, heading_radians);
    scene.screen_ellipse(
        projection.center,
        projection.halo_radii,
        projection.angle,
        Rgba(3, 10, 9, projection.halo_alpha),
        None,
    );
    scene.screen_ellipse(
        projection.center,
        projection.core_radii,
        projection.angle,
        Rgba(3, 10, 9, projection.core_alpha),
        None,
    );
}

fn draw_wings(
    scene: &mut SceneBuilder,
    behavior: Behavior,
    time: f32,
    grooming: Option<GroomingProgramFrame>,
    colors: Palette,
) {
    if let Some(program) = grooming
        && program.substate == GroomingMotorSubstate::WingClean
    {
        let side = if program.cycle.is_multiple_of(2) {
            -1.0
        } else {
            1.0
        };
        let lift = 0.42 + 0.24 * (program.segment_progress * PI).sin();
        draw_wing(scene, side, lift, colors, program.segment_progress, 0);
        return;
    }
    if !matches!(
        behavior,
        Behavior::PreEscape | Behavior::Flight | Behavior::Landing
    ) {
        return;
    }
    let amplitude = match behavior {
        Behavior::PreEscape => 1.0,
        Behavior::Flight => 0.78,
        Behavior::Landing => 0.42,
        _ => 0.0,
    };
    // Match the recorded 33 ms host increments: escape +1.45 cycles,
    // flight +0.85 cycles, and landing +0.34 cycles per model frame. Use
    // their sampled-equivalent frequencies so elapsed-time rendering remains
    // stable on 30/60/120 Hz displays.
    let cycles_per_second = match behavior {
        Behavior::PreEscape => 0.45 / REFERENCE_TICK_SECONDS,
        Behavior::Flight => -0.15 / REFERENCE_TICK_SECONDS,
        Behavior::Landing => 0.34 / REFERENCE_TICK_SECONDS,
        _ => 0.0,
    };
    let phase = time * cycles_per_second * TAU;
    let left_lift = phase.sin() * amplitude;
    let right_lift = (phase + 0.77).sin() * amplitude;
    draw_wing(scene, -1.0, left_lift, colors, time, 0);
    draw_wing(scene, 1.0, right_lift, colors, time, 1);
}

fn draw_wing(
    scene: &mut SceneBuilder,
    side: f32,
    lift: f32,
    colors: Palette,
    time: f32,
    wing_index: usize,
) {
    let root = [-1.0, side * 8.0];
    let tip = [40.0 + lift * 7.0, side * (83.0 + lift * 8.0)];
    let trailing = [58.0 + lift * 4.0, side * (36.0 + lift * 3.0)];
    let segments = [
        (root, [5.0, side * 34.0], [18.0, side * 72.0], tip),
        (tip, [58.0, side * 77.0], [70.0, side * 51.0], trailing),
        (trailing, [37.0, side * 24.0], [17.0, side * 13.0], root),
    ];
    let outline = cubic_loop(&segments, 10);
    let shimmer = 0.5 + 0.5 * (time * 3.0 + wing_index as f32 * 1.17).sin();
    scene.polygon(
        &outline,
        Rgba(
            113,
            201_u8.saturating_add((shimmer * 26.0) as u8),
            190_u8.saturating_add((shimmer * 28.0) as u8),
            96_u8.saturating_add((shimmer * 26.0) as u8),
        ),
        Some((Rgba(128, 238, 198, 190), 1.15)),
    );
    scene.line(root, tip, 0.72, Rgba(26, 91, 76, 112));
    scene.line(root, trailing, 0.72, Rgba(26, 91, 76, 112));
    for index in 1..=6 {
        let t = index as f32 / 7.0;
        let leading = cubic_point(root, [5.0, side * 34.0], [18.0, side * 72.0], tip, t);
        let trailing_point =
            cubic_point(root, [17.0, side * 13.0], [37.0, side * 24.0], trailing, t);
        scene.line(leading, trailing_point, 0.72, Rgba(26, 91, 76, 112));
    }
    // A subtle highlight along the distal membrane keeps the fast beat
    // readable at the recording's normal desktop scale.
    scene.line(
        mix(tip, trailing, 0.16),
        mix(tip, trailing, 0.76),
        0.58,
        Rgba(
            colors.wing_edge.0,
            colors.wing_edge.1,
            colors.wing_edge.2,
            92,
        ),
    );
}

fn cubic_point(p0: [f32; 2], p1: [f32; 2], p2: [f32; 2], p3: [f32; 2], t: f32) -> [f32; 2] {
    let inverse = 1.0 - t;
    let inverse_squared = inverse * inverse;
    let t_squared = t * t;
    [
        inverse_squared * inverse * p0[0]
            + 3.0 * inverse_squared * t * p1[0]
            + 3.0 * inverse * t_squared * p2[0]
            + t_squared * t * p3[0],
        inverse_squared * inverse * p0[1]
            + 3.0 * inverse_squared * t * p1[1]
            + 3.0 * inverse * t_squared * p2[1]
            + t_squared * t * p3[1],
    ]
}

fn cubic_loop(segments: &[CubicSegment], steps: usize) -> Vec<[f32; 2]> {
    let mut points = Vec::with_capacity(segments.len() * steps);
    for &(p0, p1, p2, p3) in segments {
        for step in 0..steps {
            let t = step as f32 / steps as f32;
            let u = 1.0 - t;
            points.push([
                u.powi(3) * p0[0]
                    + 3.0 * u.powi(2) * t * p1[0]
                    + 3.0 * u * t.powi(2) * p2[0]
                    + t.powi(3) * p3[0],
                u.powi(3) * p0[1]
                    + 3.0 * u.powi(2) * t * p1[1]
                    + 3.0 * u * t.powi(2) * p2[1]
                    + t.powi(3) * p3[1],
            ]);
        }
    }
    points
}

fn mix(a: [f32; 2], b: [f32; 2], t: f32) -> [f32; 2] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

fn draw_abdomen(
    scene: &mut SceneBuilder,
    skin: Skin,
    behavior: Behavior,
    time: f32,
    colors: Palette,
) {
    if skin == Skin::Firefly {
        draw_prism_abdomen(scene, behavior, time, colors);
        return;
    }
    let natural: [(u8, u8, u8); 10] = [
        (178, 113, 45),
        (164, 96, 39),
        (146, 82, 34),
        (130, 72, 31),
        (115, 64, 29),
        (103, 57, 27),
        (92, 51, 26),
        (81, 46, 25),
        (70, 40, 24),
        (60, 35, 23),
    ];
    let pulse = if behavior == Behavior::Rest {
        0.0
    } else {
        0.5 + 0.5 * (time * 3.1).sin()
    };
    for (segment, &(r, g, b)) in natural.iter().enumerate() {
        let x = -5.0 + segment as f32 * 10.7;
        let width = if segment < 2 {
            13.0
        } else {
            12.0 - segment as f32 * 0.45
        };
        let height = (16.0 - segment as f32 * 1.05).max(5.2);
        scene.ellipse(
            [x + width * 0.5, 0.0],
            [width * 0.58, height * 0.5],
            0.0,
            Rgba(r, g.saturating_add((pulse * 18.0) as u8), b, 248),
            Some((colors.outline, 0.9)),
        );
        scene.line(
            [x + 2.0, -height * 0.14],
            [x + width - 2.0, height * 0.22],
            0.62,
            Rgba(
                colors.highlight.0,
                colors.highlight.1,
                colors.highlight.2,
                95,
            ),
        );
    }
    scene.polygon(
        &[[101.0, -3.2], [115.0, 0.0], [101.0, 3.2]],
        Rgba(68, 38, 24, 248),
        Some((colors.highlight, 0.8)),
    );
}

fn draw_prism_abdomen(scene: &mut SceneBuilder, behavior: Behavior, time: f32, _colors: Palette) {
    let glow = if matches!(behavior, Behavior::Rest | Behavior::Quiet) {
        0.64
    } else {
        0.68 + 0.18 * (time * 2.1).sin()
    };
    scene.ellipse(
        [57.0, 0.0],
        [34.0, 30.0],
        0.0,
        Rgba(222, 255, 104, (44.0 + glow * 38.0) as u8),
        None,
    );
    let centers = [8.0_f32, 22.0, 36.0, 50.0, 64.0];
    for (index, center) in centers.into_iter().enumerate() {
        let width = 17.0 - index as f32 * 1.2;
        let height = 25.0 - index as f32 * 2.2;
        let lantern = index >= 3;
        let outer = if lantern {
            Rgba(88, 196_u8.saturating_add((glow * 42.0) as u8), 75, 250)
        } else {
            Rgba(7, 24, 29, 252)
        };
        let inner = if lantern {
            Rgba(216, 245, 83, 214)
        } else {
            Rgba(
                20,
                72_u8.saturating_add(index as u8 * 7),
                48_u8.saturating_add(index as u8 * 5),
                214,
            )
        };
        scene.ellipse(
            [center, 0.0],
            [width * 0.5, height * 0.5],
            0.0,
            outer,
            Some((
                if lantern {
                    Rgba(222, 255, 131, 224)
                } else {
                    Rgba(47, 124, 82, 225)
                },
                0.9,
            )),
        );
        scene.ellipse(
            [center - 1.2, -2.0],
            [width * 0.31, height * 0.31],
            -0.15,
            inner,
            None,
        );
    }
}

fn draw_prism_elytra(
    scene: &mut SceneBuilder,
    behavior: Behavior,
    time: f32,
    grooming: Option<GroomingProgramFrame>,
) {
    let grooming_side = grooming
        .filter(|program| program.substate == GroomingMotorSubstate::WingClean)
        .map(|program| {
            if program.cycle.is_multiple_of(2) {
                -1.0
            } else {
                1.0
            }
        });
    let stable_time = if behavior == Behavior::PreEscape {
        time
    } else {
        0.0
    };
    for (index, side) in [-1.0_f32, 1.0].into_iter().enumerate() {
        let opening_radians = if behavior == Behavior::PreEscape {
            side * 8.0_f32.to_radians()
        } else if grooming_side == Some(side) {
            side * 30.0_f32.to_radians()
        } else {
            0.0
        };
        let pivot = [-1.0, side * 2.5];
        let rotate = |point: [f32; 2]| rotate_point(point, pivot, opening_radians);
        let segments = [
            (
                rotate([-3.0, side * 2.0]),
                rotate([7.0, side * 17.0]),
                rotate([38.0, side * 22.0]),
                rotate([68.0, side * 12.0]),
            ),
            (
                rotate([68.0, side * 12.0]),
                rotate([76.0, side * 8.0]),
                rotate([76.0, side * 3.0]),
                rotate([66.0, side * 1.0]),
            ),
            (
                rotate([66.0, side * 1.0]),
                rotate([35.0, side * 0.3]),
                rotate([10.0, side * 0.4]),
                rotate([-3.0, side * 2.0]),
            ),
        ];
        let outline = cubic_loop(&segments, 10);
        let shimmer = 0.5 + 0.5 * (stable_time * 2.3 + index as f32 * 1.31).sin();
        scene.polygon(
            &outline,
            Rgba(
                37_u8.saturating_add((shimmer * 18.0) as u8),
                121_u8.saturating_add((shimmer * 34.0) as u8),
                82_u8.saturating_add((shimmer * 28.0) as u8),
                255,
            ),
            Some((Rgba(104, 211, 120, 238), 1.25)),
        );
        draw_cubic_line(
            scene,
            rotate([0.0, side * 2.0]),
            rotate([22.0, side * 3.0]),
            rotate([47.0, side * 3.0]),
            rotate([67.0, side * 2.0]),
            0.7,
            Rgba(223, 245, 126, 150),
        );
        for spot in 0..7 {
            let point = rotate([
                9.0 + spot as f32 * 8.0,
                side * (5.0 + (spot % 3) as f32 * 3.0),
            ]);
            scene.ellipse(point, [1.1, 1.1], 0.0, Rgba(180, 252, 144, 78), None);
        }
    }
}

fn rotate_point(point: [f32; 2], pivot: [f32; 2], angle: f32) -> [f32; 2] {
    let x = point[0] - pivot[0];
    let y = point[1] - pivot[1];
    let cosine = angle.cos();
    let sine = angle.sin();
    [
        pivot[0] + x * cosine - y * sine,
        pivot[1] + x * sine + y * cosine,
    ]
}

#[allow(clippy::too_many_arguments)]
fn draw_cubic_line(
    scene: &mut SceneBuilder,
    p0: [f32; 2],
    p1: [f32; 2],
    p2: [f32; 2],
    p3: [f32; 2],
    width: f32,
    color: Rgba,
) {
    let mut previous = p0;
    for step in 1..=12 {
        let point = cubic_point(p0, p1, p2, p3, step as f32 / 12.0);
        scene.line(previous, point, width, color);
        previous = point;
    }
}

fn draw_thorax(scene: &mut SceneBuilder, skin: Skin, time: f32, colors: Palette) {
    if skin == Skin::Firefly {
        let shimmer = 0.5 + 0.5 * (time * 2.0).sin();
        scene.ellipse(
            [-10.5, 0.0],
            [19.5, 18.0],
            0.0,
            Rgba(11, 45, 34, 255),
            Some((Rgba(102, 217, 124, 235), 1.35)),
        );
        scene.ellipse(
            [-13.0, -3.8],
            [12.8, 10.5],
            -0.08,
            Rgba(52_u8.saturating_add((shimmer * 35.0) as u8), 142, 91, 235),
            None,
        );
        let pronotum = cubic_loop(
            &[
                ([-59.0, 0.0], [-56.0, -18.0], [-31.0, -24.0], [-20.0, -11.0]),
                ([-20.0, -11.0], [-15.0, -4.0], [-15.0, 4.0], [-20.0, 11.0]),
                ([-20.0, 11.0], [-31.0, 24.0], [-56.0, 18.0], [-59.0, 0.0]),
            ],
            10,
        );
        scene.polygon(
            &pronotum,
            Rgba(210, 86, 35, 255),
            Some((Rgba(255, 215, 82, 240), 1.25)),
        );
        scene.ellipse([-43.0, 0.0], [6.5, 7.0], 0.0, Rgba(18, 35, 28, 220), None);
        scene.ellipse(
            [-43.0, -7.0],
            [10.5, 5.2],
            -0.12,
            Rgba(246, 151, 45, 105),
            None,
        );
        return;
    }
    let shell = cubic_loop(
        &[
            ([-45.0, 0.0], [-42.0, -20.0], [-15.0, -24.0], [3.0, -12.0]),
            ([3.0, -12.0], [10.0, -5.0], [10.0, 5.0], [3.0, 12.0]),
            ([3.0, 12.0], [-15.0, 24.0], [-42.0, 20.0], [-45.0, 0.0]),
        ],
        9,
    );
    scene.polygon(&shell, colors.thorax_core, Some((colors.thorax_edge, 1.6)));
    scene.ellipse(
        [-21.0, -4.0],
        [22.0, 13.0],
        -0.18,
        if skin == Skin::Firefly {
            Rgba(87, 106, 218, 92)
        } else {
            Rgba(91, 48, 27, 92)
        },
        None,
    );
    for (a, b) in [
        ([-39.0, -6.0], [-7.0, 13.0]),
        ([-35.0, 13.0], [-9.0, -15.0]),
        ([-23.0, -20.0], [-17.0, 20.0]),
    ] {
        scene.line(a, b, 0.85, Rgba(245, 203, 78, 135));
    }
    let glow = if time == 0.0 {
        0
    } else {
        ((time * 3.0).sin().abs() * 18.0) as u8
    };
    for y in [-13.5_f32, 13.5] {
        scene.ellipse(
            [-22.0, y],
            [5.0, 4.0],
            0.0,
            Rgba(237, 184_u8.saturating_add(glow), 69, 238),
            Some((Rgba(221, 248, 255, 210), 0.65)),
        );
    }
}

fn draw_head(scene: &mut SceneBuilder, skin: Skin, behavior: Behavior, time: f32, colors: Palette) {
    if skin == Skin::Firefly {
        scene.ellipse(
            [-65.5, 0.0],
            [12.5, 13.5],
            0.0,
            Rgba(4, 13, 17, 255),
            Some((Rgba(83, 171, 106, 220), 1.0)),
        );
        scene.ellipse(
            [-66.5, -2.8],
            [8.5, 9.0],
            -0.08,
            Rgba(33, 78, 61, 224),
            None,
        );
        for side in [-1.0_f32, 1.0] {
            let pulse = if matches!(behavior, Behavior::Rest | Behavior::Quiet) {
                0.4
            } else {
                0.5 + 0.5 * (time * 3.4 + if side > 0.0 { 0.5 } else { 0.0 }).sin()
            };
            scene.ellipse(
                [-68.0, side * 8.0],
                [5.5, 4.5],
                side * 0.1,
                Rgba(190, 55_u8.saturating_add((pulse * 52.0) as u8), 35, 255),
                Some((Rgba(247, 191, 84, 230), 0.72)),
            );
            scene.ellipse(
                [-70.4, side * 8.0 - 2.2],
                [1.0, 1.0],
                0.0,
                Rgba(255, 250, 205, 220),
                None,
            );
        }
        return;
    }
    scene.ellipse(
        [-60.0, 0.0],
        [18.0, 18.0],
        0.0,
        colors.head_core,
        Some((colors.head_rim, 1.5)),
    );
    scene.ellipse([-63.0, -3.0], [12.5, 12.5], 0.0, Rgba(9, 30, 54, 70), None);
    for side in [-1.0_f32, 1.0] {
        let pulse = if behavior == Behavior::Rest {
            0.0
        } else {
            (time * 4.1 + side).sin().abs() * 24.0
        };
        scene.ellipse(
            [-62.0, side * 9.0],
            [12.5, 9.5],
            side * 0.08,
            Rgba(
                colors.eye_core.0,
                colors.eye_core.1.saturating_add(pulse as u8),
                colors.eye_core.2,
                255,
            ),
            Some((colors.eye_rim, 1.2)),
        );
        for index in -2..=2 {
            let offset = index as f32 * 3.1;
            scene.line(
                [-70.0, side * 9.0 + offset],
                [-54.0, side * 9.0 + offset + side],
                0.48,
                Rgba(252, 220, 157, 92),
            );
        }
    }
    scene.ellipse([-69.0, 0.0], [12.5, 4.0], 0.0, Rgba(27, 89, 103, 210), None);
    for y in [-4.0_f32, 0.0, 4.0] {
        scene.ellipse([-78.0, y], [1.7, 1.7], 0.0, Rgba(244, 205, 78, 245), None);
    }
}

fn draw_legs(
    scene: &mut SceneBuilder,
    skin: Skin,
    behavior: Behavior,
    gait: f32,
    time: f32,
    rear_layer: bool,
    colors: Palette,
) {
    let locomoting = matches!(behavior, Behavior::Walk | Behavior::Reverse);
    let grooming = behavior == Behavior::Groom;
    let grooming_program =
        grooming.then(|| grooming_program_at((time / REFERENCE_TICK_SECONDS).floor() as u32));
    let landing = behavior == Behavior::Landing;
    for side_index in 0..2 {
        let side = if side_index == 0 { -1.0 } else { 1.0 };
        for leg_index in 0..3 {
            if rear_layer != (leg_index == 2) {
                continue;
            }
            let phase_offset = leg_index as f32 * 2.07 + if side_index == 0 { 0.0 } else { PI };
            let swing = if locomoting {
                (gait + phase_offset).sin()
            } else {
                0.0
            };
            if skin == Skin::Firefly {
                let hip_x = -26.0 + leg_index as f32 * 18.0;
                let hip_y = side * (8.0 + leg_index as f32 * 2.0);
                let hip = [hip_x, hip_y];
                let (knee, ankle, toe) = if landing {
                    let reach_x = match leg_index {
                        0 => -78.0,
                        1 => -26.0,
                        _ => 44.0,
                    };
                    let reach_y = side * (42.0 + leg_index as f32 * 5.0);
                    (
                        [
                            (hip_x + reach_x) * 0.5,
                            side * (28.0 + leg_index as f32 * 4.0),
                        ],
                        [reach_x, reach_y],
                        [reach_x + 14.0, reach_y + side * 5.0],
                    )
                } else if let Some(reach) = grooming_program
                    .and_then(|program| grooming_leg_reach(side, leg_index, program))
                {
                    (
                        [
                            (hip_x + reach[0]) * 0.5,
                            (hip_y + reach[1]) * 0.5 + side * 3.0,
                        ],
                        reach,
                        [
                            reach[0] + if leg_index == 2 { 10.0 } else { -8.0 },
                            reach[1] + side * 2.0,
                        ],
                    )
                } else {
                    let reach_x = match leg_index {
                        0 => -61.0 + swing * 4.0,
                        1 => -18.0 + swing * 5.0,
                        _ => 35.0 + swing * 7.0,
                    };
                    let reach_y = side
                        * match leg_index {
                            0 => 31.0,
                            1 => 40.0,
                            _ => 45.0,
                        };
                    (
                        [
                            (hip_x + reach_x) * 0.5 + swing * 2.5,
                            side * (20.0 + leg_index as f32 * 5.0),
                        ],
                        [reach_x, reach_y],
                        [
                            reach_x + if leg_index == 2 { 11.0 } else { -7.0 },
                            reach_y + side * 3.0,
                        ],
                    )
                };
                let alpha = if rear_layer { 188 } else { 238 };
                let dark = Rgba(10, 25, 22, alpha);
                scene.line(hip, knee, if rear_layer { 2.0 } else { 2.35 }, dark);
                scene.line(knee, ankle, if rear_layer { 1.85 } else { 2.2 }, dark);
                scene.line(ankle, toe, 1.4, dark);
                scene.line(hip, knee, 0.66, Rgba(116, 207, 99, 128));
                scene.ellipse(knee, [1.45, 1.45], 0.0, Rgba(218, 157, 57, 220), None);
                continue;
            }
            let hip_x = -31.0 + leg_index as f32 * 13.0;
            let hip_y = side * (8.0 + leg_index as f32 * 2.0);
            let mut reach_x = match leg_index {
                0 => -68.0,
                1 => -22.0 + swing * 5.0,
                _ => 32.0 + swing * 7.0,
            };
            let mut reach_y = side
                * match leg_index {
                    0 => 34.0,
                    1 => 44.0,
                    _ => 48.0,
                };
            if let Some(reach) =
                grooming_program.and_then(|program| grooming_leg_reach(side, leg_index, program))
            {
                reach_x = reach[0];
                reach_y = reach[1];
            } else if landing {
                reach_y *= 1.13;
                reach_x += if leg_index == 2 { 8.0 } else { -3.0 };
            }
            let knee = [
                (hip_x + reach_x) * 0.5 + swing * 3.0,
                side * (22.0 + leg_index as f32 * 5.0),
            ];
            let ankle = [reach_x, reach_y];
            let toe = [
                reach_x + if leg_index == 2 { 12.0 } else { -7.0 },
                reach_y + side * 3.0,
            ];
            let alpha = if rear_layer { 178 } else { 238 };
            let dark = Rgba(colors.leg.0, colors.leg.1, colors.leg.2, alpha);
            scene.line(
                [hip_x, hip_y],
                knee,
                if rear_layer { 2.0 } else { 2.4 },
                dark,
            );
            scene.line(knee, ankle, if rear_layer { 1.8 } else { 2.2 }, dark);
            scene.line(ankle, toe, 1.45, dark);
            scene.line([hip_x, hip_y], knee, 0.65, Rgba(87, 237, 218, 105));
            scene.ellipse(knee, [1.5, 1.5], 0.0, Rgba(235, 184, 65, 220), None);
        }
    }
}

fn grooming_leg_reach(
    side: f32,
    leg_index: usize,
    program: GroomingProgramFrame,
) -> Option<[f32; 2]> {
    let progress = program.segment_progress;
    let stroke = (progress * TAU * 2.0 + if side < 0.0 { 0.0 } else { PI }).sin();
    match (program.substate, leg_index) {
        (GroomingMotorSubstate::Prepare, 0) => Some([-64.0, side * (28.0 - progress * 12.0)]),
        (GroomingMotorSubstate::HeadSweep, 0) => {
            Some([-76.0 + stroke * 10.0, side * (7.0 + stroke * 7.0)])
        }
        (GroomingMotorSubstate::ForelegRub, 0) => {
            Some([-58.0 + stroke * 5.0, side * (2.5 + stroke.abs() * 2.5)])
        }
        (GroomingMotorSubstate::AbdomenBrush, 2) => {
            Some([56.0 + stroke * 13.0, side * (14.0 + stroke * 8.0)])
        }
        (GroomingMotorSubstate::WingClean, 2)
            if (program.cycle.is_multiple_of(2) && side < 0.0)
                || (!program.cycle.is_multiple_of(2) && side > 0.0) =>
        {
            Some([42.0 + stroke * 12.0, side * (7.0 + stroke * 5.0)])
        }
        (GroomingMotorSubstate::Reset, 0) => Some([-62.0, side * (16.0 + progress * 15.0)]),
        _ => None,
    }
}

fn draw_antennae(
    scene: &mut SceneBuilder,
    skin: Skin,
    behavior: Behavior,
    time: f32,
    colors: Palette,
) {
    if skin == Skin::Firefly {
        let sway = if matches!(behavior, Behavior::Rest | Behavior::Quiet) {
            0.0
        } else {
            (time * 2.9).sin() * 2.0
        };
        for side in [-1.0_f32, 1.0] {
            let end = [-103.0 + sway, side * 20.0];
            draw_cubic_line(
                scene,
                [-75.0, side * 6.0],
                [-86.0, side * 10.0],
                [-96.0 + sway, side * 18.0],
                end,
                0.95,
                Rgba(42, 77, 56, 230),
            );
            scene.ellipse(end, [1.5, 1.5], 0.0, Rgba(172, 225, 92, 225), None);
        }
        return;
    }
    let alert = matches!(
        behavior,
        Behavior::Alert | Behavior::PreEscape | Behavior::Flight
    );
    let sway = if alert { (time * 5.2).sin() * 5.0 } else { 0.0 };
    for side in [-1.0_f32, 1.0] {
        let root = [-72.0, side * 8.0];
        let joint = [-87.0, side * (18.0 + sway)];
        let tip = [-101.0, side * (25.0 + sway * 1.3)];
        scene.line(root, joint, 1.45, colors.leg);
        scene.line(
            joint,
            tip,
            1.05,
            Rgba(
                colors.highlight.0,
                colors.highlight.1,
                colors.highlight.2,
                205,
            ),
        );
        scene.ellipse(tip, [1.8, 1.8], 0.0, Rgba(242, 183, 63, 235), None);
    }
}

fn ellipse_points(center: [f32; 2], radii: [f32; 2], angle: f32, count: usize) -> Vec<[f32; 2]> {
    let cosine = angle.cos();
    let sine = angle.sin();
    (0..count)
        .map(|index| {
            let theta = index as f32 / count as f32 * std::f32::consts::TAU;
            let x = theta.cos() * radii[0];
            let y = theta.sin() * radii[1];
            [
                center[0] + x * cosine - y * sine,
                center[1] + x * sine + y * cosine,
            ]
        })
        .collect()
}

struct RasterCanvas {
    width: usize,
    height: usize,
    premultiplied_bgra: Vec<u8>,
}

impl RasterCanvas {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            premultiplied_bgra: vec![0; width * height * 4],
        }
    }

    fn draw(&mut self, primitive: &Primitive, scale: f32) {
        match primitive {
            Primitive::Ellipse {
                center,
                radii,
                angle,
                fill,
                stroke,
            } => self.ellipse(
                [center[0] * scale, center[1] * scale],
                [radii[0] * scale, radii[1] * scale],
                *angle,
                *fill,
                stroke.map(|(color, width)| (color, width * scale)),
            ),
            Primitive::Line {
                from,
                to,
                width,
                color,
            } => self.line(
                [from[0] * scale, from[1] * scale],
                [to[0] * scale, to[1] * scale],
                width * scale,
                *color,
            ),
            Primitive::Polygon {
                points,
                fill,
                stroke,
            } => {
                let points: Vec<_> = points
                    .iter()
                    .map(|p| [p[0] * scale, p[1] * scale])
                    .collect();
                self.polygon(&points, *fill);
                if let Some((color, width)) = stroke {
                    for index in 0..points.len() {
                        self.line(
                            points[index],
                            points[(index + 1) % points.len()],
                            width * scale,
                            *color,
                        );
                    }
                }
            }
        }
    }

    fn ellipse(
        &mut self,
        center: [f32; 2],
        radii: [f32; 2],
        angle: f32,
        fill: Rgba,
        stroke: Option<(Rgba, f32)>,
    ) {
        let extent = radii[0].max(radii[1]) + stroke.map_or(0.0, |(_, width)| width);
        let bounds = self.bounds(
            center[0] - extent,
            center[1] - extent,
            center[0] + extent,
            center[1] + extent,
        );
        let cosine = angle.cos();
        let sine = angle.sin();
        let inner =
            stroke.map(|(_, width)| (1.0 - width / radii[0].min(radii[1]).max(1.0)).max(0.0));
        for y in bounds.1..=bounds.3 {
            for x in bounds.0..=bounds.2 {
                let dx = x as f32 + 0.5 - center[0];
                let dy = y as f32 + 0.5 - center[1];
                let local_x = dx * cosine + dy * sine;
                let local_y = -dx * sine + dy * cosine;
                let distance = (local_x / radii[0]).powi(2) + (local_y / radii[1]).powi(2);
                if distance <= 1.0 {
                    let color = match (stroke, inner) {
                        (Some((stroke_color, _)), Some(inner_radius))
                            if distance.sqrt() >= inner_radius =>
                        {
                            stroke_color
                        }
                        _ => fill,
                    };
                    self.blend(x, y, color);
                }
            }
        }
    }

    fn line(&mut self, from: [f32; 2], to: [f32; 2], width: f32, color: Rgba) {
        let radius = width * 0.5;
        let bounds = self.bounds(
            from[0].min(to[0]) - radius,
            from[1].min(to[1]) - radius,
            from[0].max(to[0]) + radius,
            from[1].max(to[1]) + radius,
        );
        let dx = to[0] - from[0];
        let dy = to[1] - from[1];
        let length_squared = (dx * dx + dy * dy).max(0.0001);
        for y in bounds.1..=bounds.3 {
            for x in bounds.0..=bounds.2 {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                let t =
                    (((px - from[0]) * dx + (py - from[1]) * dy) / length_squared).clamp(0.0, 1.0);
                let nearest_x = from[0] + t * dx;
                let nearest_y = from[1] + t * dy;
                if (px - nearest_x).powi(2) + (py - nearest_y).powi(2) <= radius * radius {
                    self.blend(x, y, color);
                }
            }
        }
    }

    fn polygon(&mut self, points: &[[f32; 2]], color: Rgba) {
        if points.len() < 3 {
            return;
        }
        let min_x = points.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
        let min_y = points.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
        let max_x = points
            .iter()
            .map(|p| p[0])
            .fold(f32::NEG_INFINITY, f32::max);
        let max_y = points
            .iter()
            .map(|p| p[1])
            .fold(f32::NEG_INFINITY, f32::max);
        let bounds = self.bounds(min_x, min_y, max_x, max_y);
        for y in bounds.1..=bounds.3 {
            for x in bounds.0..=bounds.2 {
                if point_in_polygon([x as f32 + 0.5, y as f32 + 0.5], points) {
                    self.blend(x, y, color);
                }
            }
        }
    }

    fn bounds(&self, left: f32, top: f32, right: f32, bottom: f32) -> (usize, usize, usize, usize) {
        let x0 = left.floor().max(0.0) as usize;
        let y0 = top.floor().max(0.0) as usize;
        let x1 = right.ceil().min((self.width - 1) as f32).max(0.0) as usize;
        let y1 = bottom.ceil().min((self.height - 1) as f32).max(0.0) as usize;
        (x0.min(x1), y0.min(y1), x1, y1)
    }

    fn blend(&mut self, x: usize, y: usize, color: Rgba) {
        let index = (y * self.width + x) * 4;
        let source_alpha = color.3 as u32;
        let inverse = 255 - source_alpha;
        let source = [color.2, color.1, color.0];
        for (channel, value) in source.into_iter().enumerate() {
            let source_premultiplied = (value as u32 * source_alpha + 127) / 255;
            self.premultiplied_bgra[index + channel] = (source_premultiplied
                + self.premultiplied_bgra[index + channel] as u32 * inverse / 255)
                .min(255) as u8;
        }
        self.premultiplied_bgra[index + 3] = (source_alpha
            + self.premultiplied_bgra[index + 3] as u32 * inverse / 255)
            .min(255) as u8;
    }

    fn downsample(&self, output: &mut [u8], scale: usize) {
        for y in 0..PET_HEIGHT {
            for x in 0..PET_WIDTH {
                let output_index = (y * PET_WIDTH + x) * 4;
                for channel in 0..4 {
                    let mut total = 0_u32;
                    for sample_y in 0..scale {
                        for sample_x in 0..scale {
                            let source_x = x * scale + sample_x;
                            let source_y = y * scale + sample_y;
                            total += self.premultiplied_bgra
                                [(source_y * self.width + source_x) * 4 + channel]
                                as u32;
                        }
                    }
                    output[output_index + channel] =
                        (total / (scale * scale) as u32).min(255) as u8;
                }
            }
        }
    }
}

fn point_in_polygon(point: [f32; 2], polygon: &[[f32; 2]]) -> bool {
    let mut inside = false;
    let mut previous = polygon.len() - 1;
    for current in 0..polygon.len() {
        let a = polygon[current];
        let b = polygon[previous];
        let denominator = b[1] - a[1];
        let crosses = (a[1] > point[1]) != (b[1] > point[1])
            && point[0]
                < (b[0] - a[0]) * (point[1] - a[1])
                    / if denominator.abs() < f32::EPSILON {
                        f32::EPSILON
                    } else {
                        denominator
                    }
                    + a[0];
        if crosses {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

#[derive(Clone, Debug)]
pub struct FireflyVisualSelfTest {
    pub passed: bool,
    pub opaque_pixels: usize,
    pub translucent_pixels: usize,
    pub red_eye_pixels: usize,
    pub lantern_pixels: usize,
    pub elytron_pixels: usize,
    pub rest_pixel_differences: usize,
    pub escape_wing_pixel_differences: usize,
    pub rest_escape_pixel_differences: usize,
    pub grooming_pixel_differences: usize,
    pub flight_pixel_differences: usize,
    pub landing_pixel_differences: usize,
    pub walking_pixel_differences: usize,
    pub wing_state_contract_passed: bool,
}

impl FireflyVisualSelfTest {
    pub fn rest_temporal_invariant(&self) -> bool {
        self.rest_pixel_differences == 0
    }
}

pub fn run_firefly_visual_self_test() -> FireflyVisualSelfTest {
    let render = |behavior, phase| {
        let mut pixels = vec![0; PET_WIDTH * PET_HEIGHT * 4];
        render_pet_bgra(&mut pixels, Skin::Firefly, behavior, phase, 0.0, false);
        pixels
    };
    let rest = render(Behavior::Rest, 0.0);
    let rest_later = render(Behavior::Rest, 2.0);
    let escape_first = render(Behavior::PreEscape, 0.04);
    let escape_later = render(Behavior::PreEscape, 0.11);
    let flight_first = render(Behavior::Flight, 0.08);
    let flight_later = render(Behavior::Flight, 0.17);
    let landing = render(Behavior::Landing, 0.35);
    let groom = render(Behavior::Groom, 0.35);
    let walk = render(Behavior::Walk, 0.35);
    let ground_wings_stowed = [
        Behavior::Rest,
        Behavior::Walk,
        Behavior::Reverse,
        Behavior::Groom,
    ]
    .into_iter()
    .all(|behavior| {
        wing_panel_count(&pet_scene(Skin::Firefly, behavior, 0.2, 0.2, 0.0, false)) == 0
    });
    let air_wings_deployed = [Behavior::PreEscape, Behavior::Flight, Behavior::Landing]
        .into_iter()
        .all(|behavior| {
            wing_panel_count(&pet_scene(Skin::Firefly, behavior, 0.2, 0.2, 0.0, false)) == 2
        });

    let mut result = FireflyVisualSelfTest {
        passed: false,
        opaque_pixels: 0,
        translucent_pixels: 0,
        red_eye_pixels: 0,
        lantern_pixels: 0,
        elytron_pixels: 0,
        rest_pixel_differences: pixel_difference(&rest, &rest_later),
        escape_wing_pixel_differences: pixel_difference(&escape_first, &escape_later),
        rest_escape_pixel_differences: pixel_difference(&rest, &escape_first),
        grooming_pixel_differences: pixel_difference(&rest, &groom),
        flight_pixel_differences: pixel_difference(&flight_first, &flight_later),
        landing_pixel_differences: pixel_difference(&rest, &landing),
        walking_pixel_differences: pixel_difference(&rest, &walk),
        wing_state_contract_passed: ground_wings_stowed && air_wings_deployed,
    };
    for pixel in rest.as_chunks::<4>().0 {
        let alpha = pixel[3] as u32;
        if alpha == 0 {
            continue;
        }
        if alpha >= 220 {
            result.opaque_pixels += 1;
        } else {
            result.translucent_pixels += 1;
        }
        let unpremultiply = |channel: u8| (channel as u32 * 255 / alpha).min(255) as u8;
        let red = unpremultiply(pixel[2]);
        let green = unpremultiply(pixel[1]);
        let blue = unpremultiply(pixel[0]);
        if red > 145 && red > green.saturating_add(35) && red > blue.saturating_add(20) {
            result.red_eye_pixels += 1;
        }
        if green > 130 && green > red.saturating_add(35) && green > blue.saturating_add(25) {
            result.lantern_pixels += 1;
        }
        if alpha >= 190 && (green > 105 || blue > 130) {
            result.elytron_pixels += 1;
        }
    }
    result.passed = result.opaque_pixels > 1_000
        && result.translucent_pixels > 650
        && result.red_eye_pixels > 70
        && result.lantern_pixels > 45
        && result.elytron_pixels > 450
        && result.rest_temporal_invariant()
        && result.escape_wing_pixel_differences > 180
        && result.flight_pixel_differences > 180
        && result.rest_escape_pixel_differences > 1_000
        && result.grooming_pixel_differences > 100
        && result.landing_pixel_differences > 500
        && result.walking_pixel_differences > 100
        && result.wing_state_contract_passed;
    result
}

fn wing_panel_count(scene: &[Primitive]) -> usize {
    scene
        .iter()
        .filter(|primitive| {
            matches!(
                primitive,
                Primitive::Polygon { fill, .. }
                    if fill.0 == 113 && (96..=122).contains(&fill.3)
            )
        })
        .count()
}

fn pixel_difference(first: &[u8], second: &[u8]) -> usize {
    first
        .as_chunks::<4>()
        .0
        .iter()
        .zip(second.as_chunks::<4>().0)
        .filter(|(first, second)| *first != *second)
        .count()
}

pub fn transparent_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::TRANSPARENT)
        .inner_margin(egui::Margin::same(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct TrajectorySample {
        time_seconds: f32,
        behavior: String,
        x: f32,
        y: f32,
        heading_radians: f32,
        phase: f32,
        behavior_age_seconds: f32,
        altitude_pixels: f32,
    }

    #[test]
    fn heading_points_the_head_along_the_velocity_vector() {
        let right = pet_scene(Skin::Firefly, Behavior::Rest, 0.0, 0.0, 0.0, true);
        let left = pet_scene(Skin::Firefly, Behavior::Rest, 0.0, 0.0, PI, true);
        let head_x = |scene: &[Primitive]| {
            scene
                .iter()
                .filter_map(|primitive| match primitive {
                    Primitive::Ellipse { center, fill, .. }
                        if fill.0 == 4 && fill.1 == 13 && fill.2 == 17 && fill.3 == 255 =>
                    {
                        Some(center[0])
                    }
                    _ => None,
                })
                .next()
                .unwrap()
        };
        assert!(head_x(&right) > CENTER[0]);
        assert!(head_x(&left) < CENTER[0]);
    }

    #[test]
    fn motion_keeps_pet_inside_virtual_screen() {
        let mut motion = PetMotion {
            screen_position: Pos2::new(9_999.0, 9_999.0),
            ..PetMotion::default()
        };
        motion.advance(
            0.1,
            Behavior::Walk,
            Pos2::new(-1_920.0, 0.0),
            Vec2::new(3_840.0, 1_080.0),
            false,
            None,
        );
        assert!(motion.screen_position.x <= 1_920.0 - PET_WIDTH as f32 - 8.0);
        assert!(motion.screen_position.y <= 1_080.0 - PET_HEIGHT as f32 - 8.0);
    }

    #[test]
    fn walking_motion_is_time_based_across_refresh_rates() {
        let mut at_30_hz = PetMotion::default();
        let mut at_120_hz = PetMotion::default();
        let origin = Pos2::ZERO;
        let screen = Vec2::new(3_840.0, 2_160.0);
        for _ in 0..60 {
            at_30_hz.advance(1.0 / 30.0, Behavior::Walk, origin, screen, false, None);
        }
        for _ in 0..240 {
            at_120_hz.advance(1.0 / 120.0, Behavior::Walk, origin, screen, false, None);
        }
        assert!((at_30_hz.screen_position.x - at_120_hz.screen_position.x).abs() < 0.8);
        assert!((at_30_hz.animation_seconds - at_120_hz.animation_seconds).abs() < 0.001);
    }

    #[test]
    fn walking_speed_varies_between_and_within_deterministic_bouts() {
        let origin = Pos2::ZERO;
        let screen = Vec2::new(8_000.0, 4_000.0);
        let mut motion = PetMotion {
            screen_position: Pos2::new(3_000.0, 2_000.0),
            ..PetMotion::default()
        };
        let mut bout_means = Vec::new();
        let mut first_bout_samples = Vec::new();
        for bout in 0..16 {
            motion.advance(1.0 / 60.0, Behavior::Quiet, origin, screen, false, None);
            let mut sum = 0.0;
            let mut count = 0;
            for frame in 0..120 {
                motion.advance(1.0 / 60.0, Behavior::Walk, origin, screen, false, None);
                if frame >= 15 {
                    sum += motion.speed_pixels_per_second;
                    count += 1;
                    if bout == 0 {
                        first_bout_samples.push(motion.speed_pixels_per_second);
                    }
                }
            }
            bout_means.push(sum / count as f32);
        }
        let minimum = bout_means.iter().copied().fold(f32::INFINITY, f32::min);
        let maximum = bout_means.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let rounded_levels = bout_means
            .iter()
            .map(|value| value.round() as i32)
            .collect::<std::collections::BTreeSet<_>>();
        let within_minimum = first_bout_samples
            .iter()
            .copied()
            .fold(f32::INFINITY, f32::min);
        let within_maximum = first_bout_samples
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(maximum / minimum > 2.2, "means={bout_means:?}");
        assert!(rounded_levels.len() >= 10, "means={bout_means:?}");
        assert!(within_maximum - within_minimum > 7.0);

        let mut repeat = PetMotion {
            screen_position: Pos2::new(3_000.0, 2_000.0),
            ..PetMotion::default()
        };
        repeat.advance(1.0 / 60.0, Behavior::Quiet, origin, screen, false, None);
        repeat.advance(1.0 / 60.0, Behavior::Walk, origin, screen, false, None);
        let first = GroundSpeedProfile::for_bout(1);
        assert_eq!(repeat.ground_speed_profile.multiplier, first.multiplier);
        assert_eq!(repeat.ground_speed_profile.stride_phase, first.stride_phase);
    }

    #[test]
    fn natural_flight_uses_deterministic_speed_profiles_and_rapid_saccades() {
        let origin = Pos2::ZERO;
        let screen = Vec2::new(100_000.0, 100_000.0);
        let mut motion = PetMotion {
            screen_position: Pos2::new(50_000.0, 50_000.0),
            natural_flight_motion: true,
            ..PetMotion::default()
        };
        let mut mean_speeds = Vec::new();
        let mut largest_frame_turn = 0.0_f32;
        for _ in 0..16 {
            motion.advance(1.0 / 120.0, Behavior::Quiet, origin, screen, false, None);
            let mut speed_sum = 0.0;
            let mut speed_count = 0_u32;
            for frame in 0..480 {
                let heading_before = motion.heading_radians;
                motion.advance(1.0 / 120.0, Behavior::Flight, origin, screen, false, None);
                largest_frame_turn = largest_frame_turn
                    .max(wrapped_angle(motion.heading_radians - heading_before).abs());
                if frame >= 36 {
                    speed_sum += motion.speed_pixels_per_second;
                    speed_count += 1;
                }
            }
            mean_speeds.push(speed_sum / speed_count as f32);
        }
        let minimum = mean_speeds.iter().copied().fold(f32::INFINITY, f32::min);
        let maximum = mean_speeds
            .iter()
            .copied()
            .fold(f32::NEG_INFINITY, f32::max);
        let rounded_levels = mean_speeds
            .iter()
            .map(|value| value.round() as i32)
            .collect::<std::collections::BTreeSet<_>>();
        assert!(maximum / minimum > 1.75, "means={mean_speeds:?}");
        assert!(rounded_levels.len() >= 10, "means={mean_speeds:?}");
        assert!(largest_frame_turn > 0.10, "turn={largest_frame_turn}");

        for sequence in 0..4_096 {
            let profile = FlightPathProfile::for_bout(sequence);
            assert!(
                profile.saccades[0].start_seconds + profile.saccades[0].duration_seconds < 0.55
            );
            assert_ne!(profile.saccades[0].angle_radians, 0.0);
        }

        let repeat = FlightPathProfile::for_bout(1);
        let same = FlightPathProfile::for_bout(1);
        assert_eq!(repeat.speed_multiplier, same.speed_multiplier);
        assert_eq!(
            repeat.saccades[0].start_seconds,
            same.saccades[0].start_seconds
        );
        assert_eq!(
            repeat.saccades[0].angle_radians,
            same.saccades[0].angle_radians
        );
    }

    #[test]
    fn uncued_course_selection_is_deterministic_and_position_independent() {
        let headings = (1..=64).map(uncued_exploratory_heading).collect::<Vec<_>>();
        assert_eq!(headings[0], uncued_exploratory_heading(1));
        assert!(headings.iter().any(|heading| heading.cos() < -0.70));
        assert!(headings.iter().any(|heading| heading.cos() > 0.70));
        assert!(headings.iter().any(|heading| heading.sin() < -0.70));
        assert!(headings.iter().any(|heading| heading.sin() > 0.70));

        let fallback = 0.75;
        let from_same_heading = uncued_course_selection_saccade(7, PI, fallback);
        let repeated = uncued_course_selection_saccade(7, PI, fallback);
        assert_eq!(from_same_heading, repeated);
        assert!(from_same_heading.abs() <= FLIGHT_COURSE_SELECTION_MAX_RADIANS);
    }

    #[test]
    fn edge_avoidance_is_local_and_turns_an_outward_course_inward() {
        let profile = FlightPathProfile::for_bout(1);
        let left = 8.0;
        let top = 8.0;
        let right = 2_132.0;
        let bottom = 1_152.0;
        let center = Pos2::new((left + right) * 0.5, (top + bottom) * 0.5);
        let movement_bounds = Rect::from_min_max(Pos2::new(left, top), Pos2::new(right, bottom));
        assert_eq!(
            flight_edge_avoidance_turn_rate(center, PI, movement_bounds, 0.0, profile),
            0.0
        );
        assert_eq!(
            flight_edge_avoidance_turn_rate(
                Pos2::new(left, center.y),
                0.0,
                movement_bounds,
                0.0,
                profile,
            ),
            0.0
        );
        assert!(
            flight_edge_avoidance_turn_rate(
                Pos2::new(left, center.y),
                PI,
                movement_bounds,
                0.0,
                profile,
            )
            .abs()
                > 1.0
        );
    }

    #[test]
    fn repeated_cursor_escape_heading_does_not_create_a_one_side_motor_lock() {
        let origin = Pos2::ZERO;
        let screen = Vec2::new(2_560.0, 1_440.0);
        let left = origin.x + 8.0;
        let right = origin.x + screen.x - PET_WIDTH as f32 - 8.0;
        let horizontal_span = right - left;
        let durations: [f32; 12] = [
            1.716, 2.574, 0.594, 1.485, 4.422, 6.171, 0.990, 0.594, 4.224, 1.980, 1.023, 4.224,
        ];

        let mut motion = PetMotion {
            screen_position: Pos2::new(96.0, 640.0),
            natural_flight_motion: true,
            ..PetMotion::default()
        };
        let mut tertiles = [false; 3];
        for duration in durations {
            motion.advance(1.0 / 60.0, Behavior::Quiet, origin, screen, false, None);
            motion.heading_radians = PI;
            let frames = (duration * 60.0).ceil() as usize;
            for _ in 0..frames {
                motion.advance(1.0 / 60.0, Behavior::Flight, origin, screen, false, None);
                let fraction =
                    ((motion.screen_position.x - left) / horizontal_span).clamp(0.0, 1.0);
                tertiles[(fraction * 3.0).floor().min(2.0) as usize] = true;
            }
            for _ in 0..30 {
                motion.advance(1.0 / 60.0, Behavior::Landing, origin, screen, false, None);
            }
        }
        let visited_tertiles = tertiles.iter().filter(|visited| **visited).count();
        assert!(visited_tertiles >= 2, "tertiles={tertiles:?}");
    }

    #[test]
    fn natural_landing_preserves_incoming_velocity_and_finishes_at_rest() {
        let origin = Pos2::ZERO;
        let screen = Vec2::new(100_000.0, 100_000.0);
        let mut motion = PetMotion {
            screen_position: Pos2::new(50_000.0, 50_000.0),
            natural_flight_motion: true,
            ..PetMotion::default()
        };
        motion.advance(1.0 / 60.0, Behavior::PreEscape, origin, screen, false, None);
        for _ in 0..120 {
            motion.advance(1.0 / 60.0, Behavior::Flight, origin, screen, false, None);
        }
        let before_last_flight = motion.screen_position;
        motion.advance(1.0 / 60.0, Behavior::Flight, origin, screen, false, None);
        let flight_step = motion.screen_position - before_last_flight;
        let landing_start = motion.screen_position;
        motion.advance(1.0 / 60.0, Behavior::Landing, origin, screen, false, None);
        let landing_first_step = motion.screen_position - landing_start;
        assert!(
            (landing_first_step.length() - flight_step.length()).abs() <= 1.5,
            "flight={flight_step:?} landing={landing_first_step:?}"
        );
        assert!(
            landing_first_step.dot(flight_step) > 0.0,
            "flight={flight_step:?} landing={landing_first_step:?}"
        );
        let mut maximum_step = landing_first_step.length();
        let mut previous = motion.screen_position;
        for _ in 1..30 {
            motion.advance(1.0 / 60.0, Behavior::Landing, origin, screen, false, None);
            maximum_step = maximum_step.max(motion.screen_position.distance(previous));
            previous = motion.screen_position;
        }
        assert!(maximum_step <= 7.0, "maximum_step={maximum_step}");
        assert!(motion.altitude_pixels.abs() <= 0.01);
        let touchdown = motion.screen_position;
        motion.advance(1.0 / 60.0, Behavior::Rest, origin, screen, false, None);
        assert!(motion.screen_position.distance(touchdown) <= 0.01);
    }

    #[test]
    fn walking_turns_never_rotate_at_a_stationary_onset() {
        let origin = Pos2::ZERO;
        let screen = Vec2::new(8_000.0, 4_000.0);
        let sharp_sequence = (1..64)
            .find(|sequence| {
                matches!(
                    GroundPathMotif::for_bout(*sequence),
                    GroundPathMotif::SharpTurnLeft | GroundPathMotif::SharpTurnRight
                )
            })
            .expect("a sharp motif must be reachable");
        let mut motion = PetMotion {
            screen_position: Pos2::new(3_000.0, 2_000.0),
            ground_bout_sequence: sharp_sequence - 1,
            ..PetMotion::default()
        };
        for _ in 0..30 {
            let before_position = motion.screen_position;
            let before_heading = motion.heading_radians;
            motion.advance(1.0 / 240.0, Behavior::Walk, origin, screen, false, None);
            let translation = motion.screen_position.distance(before_position);
            let turn = wrapped_angle(motion.heading_radians - before_heading).abs();
            if translation < 0.01 {
                assert!(turn < 0.0001, "translation={translation} turn={turn}");
            }
        }
    }

    #[test]
    fn natural_bout_duration_no_longer_predicts_path_length() {
        use mechofly_core::behavior_parameters::{
            BehaviorParameterProfile, duration_draw_for, parameters_for_profile,
        };

        let origin = Pos2::ZERO;
        let screen = Vec2::new(100_000.0, 100_000.0);
        let parameters = parameters_for_profile(BehaviorParameterProfile::N41BNatural);
        let mut motion = PetMotion {
            screen_position: Pos2::new(50_000.0, 50_000.0),
            ..PetMotion::default()
        };
        let mut durations = Vec::new();
        let mut distances = Vec::new();
        let mut speeds = Vec::new();
        for sequence in 1..=128_u64 {
            motion.advance(1.0 / 60.0, Behavior::Quiet, origin, screen, false, None);
            let frames = duration_draw_for(
                parameters,
                0x4D45_4348_4F46_4C59,
                sequence,
                Behavior::Walk,
                0,
            )
            .1;
            let duration = frames as f32 * REFERENCE_TICK_SECONDS;
            let render_steps = (duration * 60.0).ceil() as usize;
            let mut path = 0.0;
            let mut speed_sum = 0.0;
            for _ in 0..render_steps {
                let before = motion.screen_position;
                motion.advance(1.0 / 60.0, Behavior::Walk, origin, screen, false, None);
                path += motion.screen_position.distance(before);
                speed_sum += motion.speed_pixels_per_second;
            }
            durations.push(duration as f64);
            distances.push(path as f64);
            speeds.push((speed_sum / render_steps as f32) as f64);
        }

        let stats = |values: &[f64]| {
            let mean = values.iter().sum::<f64>() / values.len() as f64;
            let variance = values
                .iter()
                .map(|value| (value - mean).powi(2))
                .sum::<f64>()
                / values.len() as f64;
            (mean, variance.sqrt() / mean)
        };
        let squared_correlation = |x: &[f64], y: &[f64]| {
            let x_mean = stats(x).0;
            let y_mean = stats(y).0;
            let covariance = x
                .iter()
                .zip(y)
                .map(|(x, y)| (x - x_mean) * (y - y_mean))
                .sum::<f64>();
            let x_variance = x.iter().map(|x| (x - x_mean).powi(2)).sum::<f64>();
            let y_variance = y.iter().map(|y| (y - y_mean).powi(2)).sum::<f64>();
            (covariance / (x_variance * y_variance).sqrt()).powi(2)
        };
        let speed_cv = stats(&speeds).1;
        let speed_minimum = speeds.iter().copied().fold(f64::INFINITY, f64::min);
        let speed_maximum = speeds.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let duration_distance_r_squared = squared_correlation(&durations, &distances);

        assert!(speed_cv >= 0.18, "speed_cv={speed_cv}");
        assert!(speed_maximum - speed_minimum >= 20.0);
        assert!(
            duration_distance_r_squared <= 0.98,
            "duration_distance_r_squared={duration_distance_r_squared}"
        );
    }

    #[test]
    fn flight_moves_in_two_dimensions_with_recorded_free_flight_curvature() {
        let mut motion = PetMotion {
            screen_position: Pos2::new(800.0, 500.0),
            ..PetMotion::default()
        };
        let start = motion.screen_position;
        for _ in 0..60 {
            motion.advance(
                1.0 / 60.0,
                Behavior::Flight,
                Pos2::ZERO,
                Vec2::new(1_920.0, 1_080.0),
                false,
                None,
            );
        }
        assert!((motion.screen_position.x - start.x).abs() > 20.0);
        assert!((motion.screen_position.y - start.y).abs() > 15.0);
        assert!(motion.heading_radians.abs() > 0.2);
    }

    #[test]
    fn recorded_motion_contract_covers_run_escape_flight_and_touchdown() {
        let result = run_motion_self_test();
        assert!(result.passed, "{result:#?}");
    }

    #[test]
    fn landing_is_continuous_from_flight_through_touchdown_and_rest() {
        let result = run_motion_self_test();
        assert!(result.landing_position_continuous, "{result:#?}");
        assert!(!result.teleport_detected, "{result:#?}");
        assert!(result.landing_first_step_pixels <= 2.5, "{result:#?}");
        assert!(result.landing_max_step_pixels <= 25.0, "{result:#?}");
        assert!(result.landing_completion_step_pixels <= 1.0, "{result:#?}");
        assert!(result.landing_to_rest_step_pixels <= 0.01, "{result:#?}");
    }

    #[test]
    fn landing_interpolation_is_refresh_rate_invariant() {
        let result = run_motion_self_test();
        assert!(result.landing_refresh_rate_invariant, "{result:#?}");
        assert!(
            result.landing_refresh_rate_position_error_pixels <= 0.01,
            "{result:#?}"
        );
        assert!(
            result.landing_refresh_rate_heading_error_radians <= 0.001,
            "{result:#?}"
        );
        assert!(
            result.landing_refresh_rate_altitude_error_pixels <= 0.01,
            "{result:#?}"
        );
    }

    #[test]
    fn shadow_center_is_screen_stable_and_footprint_tracks_heading() {
        let result = run_motion_self_test();
        assert!(result.screen_space_shadow_below_body, "{result:#?}");
        assert!(result.shadow_altitude_monotonic, "{result:#?}");
        assert!(result.shadow_center_heading_invariant, "{result:#?}");
        assert!(result.shadow_heading_aligned, "{result:#?}");
        assert!(
            result.shadow_heading_max_error_radians <= 0.001,
            "{result:#?}"
        );
        assert!(result.shadow_footprint_anisotropic, "{result:#?}");
    }

    #[test]
    fn landing_uses_a_local_surface_instead_of_screen_bottom() {
        let result = run_motion_self_test();
        assert!(result.flight_altitude_pixels >= 60.0, "{result:#?}");
        assert!(result.landing_target_local, "{result:#?}");
        assert!(result.landing_avoids_bottom_edge, "{result:#?}");
        assert!(
            result.landing_bottom_clearance_pixels >= 48.0,
            "{result:#?}"
        );
        assert!(
            result.touchdown_altitude_pixels.abs() <= 0.01,
            "{result:#?}"
        );
    }

    #[test]
    fn prism_uses_the_reference_desktop_scale() {
        let mut pixels = vec![0; PET_WIDTH * PET_HEIGHT * 4];
        render_pet_bgra(&mut pixels, Skin::Firefly, Behavior::Rest, 0.0, 0.0, false);
        let mut minimum_x = PET_WIDTH;
        let mut maximum_x = 0;
        let mut minimum_y = PET_HEIGHT;
        let mut maximum_y = 0;
        for y in 0..PET_HEIGHT {
            for x in 0..PET_WIDTH {
                if pixels[(y * PET_WIDTH + x) * 4 + 3] > 12 {
                    minimum_x = minimum_x.min(x);
                    maximum_x = maximum_x.max(x);
                    minimum_y = minimum_y.min(y);
                    maximum_y = maximum_y.max(y);
                }
            }
        }
        let visible_width = maximum_x.saturating_sub(minimum_x);
        let visible_height = maximum_y.saturating_sub(minimum_y);
        assert!((150..=230).contains(&visible_width));
        assert!((70..=175).contains(&visible_height));
    }

    #[test]
    fn layered_bitmap_has_real_holes_and_premultiplied_color() {
        let mut pixels = vec![0; PET_WIDTH * PET_HEIGHT * 4];
        render_pet_bgra(
            &mut pixels,
            Skin::Firefly,
            Behavior::Flight,
            0.25,
            0.7,
            false,
        );
        let mut transparent = 0;
        let mut visible = 0;
        for pixel in pixels.as_chunks::<4>().0 {
            let alpha = pixel[3];
            if alpha == 0 {
                transparent += 1;
            } else {
                visible += 1;
                assert!(pixel[0] <= alpha && pixel[1] <= alpha && pixel[2] <= alpha);
            }
        }
        assert!(transparent > PET_WIDTH * PET_HEIGHT / 2);
        assert!(visible > PET_WIDTH * PET_HEIGHT / 25);
    }

    #[test]
    fn prism_visual_contract_is_articulated_and_behavior_responsive() {
        let result = run_firefly_visual_self_test();
        assert!(result.passed, "{result:#?}");
        if let Some(path) = std::env::var_os("MECHOFLY_VISUAL_FIXTURE") {
            let mut rest = vec![0; PET_WIDTH * PET_HEIGHT * 4];
            render_pet_bgra(&mut rest, Skin::Firefly, Behavior::Rest, 0.0, 0.0, false);
            write_pam(std::path::Path::new(&path), &rest);
        }
        if let Some(directory) = std::env::var_os("MECHOFLY_VISUAL_FIXTURE_DIR") {
            let directory = std::path::PathBuf::from(directory);
            std::fs::create_dir_all(&directory).expect("visual fixture directory must be writable");
            for (name, behavior, phase, heading) in [
                ("rest", Behavior::Rest, 0.0, 0.0),
                ("walk-a", Behavior::Walk, 0.10, 0.0),
                ("walk-b", Behavior::Walk, 0.42, 0.0),
                ("groom-a", Behavior::Groom, 0.10, 0.0),
                ("groom-b", Behavior::Groom, 0.42, 0.0),
                ("flight-a", Behavior::Flight, 0.08, 0.55),
                ("flight-b", Behavior::Flight, 0.30, 0.55),
                ("landing", Behavior::Landing, 0.34, 0.18),
            ] {
                let mut pixels = vec![0; PET_WIDTH * PET_HEIGHT * 4];
                render_pet_bgra(&mut pixels, Skin::Firefly, behavior, phase, heading, false);
                write_pam(&directory.join(format!("prism-{name}.pam")), &pixels);
            }
            write_trajectory_fixtures(&directory);
        }
    }

    #[test]
    fn grooming_actions_are_pairwise_visible_and_wing_clean_is_asymmetric() {
        let render = |age_frames: u32| {
            let seconds = age_frames as f32 * REFERENCE_TICK_SECONDS;
            let mut pixels = vec![0; PET_WIDTH * PET_HEIGHT * 4];
            render_pet_bgra_at_age(
                &mut pixels,
                Skin::Firefly,
                Behavior::Groom,
                seconds,
                seconds,
                0.0,
                false,
            );
            pixels
        };
        let actions = [render(23), render(45), render(57), render(65)];
        for first in 0..actions.len() {
            for second in first + 1..actions.len() {
                assert!(
                    pixel_difference(&actions[first], &actions[second]) > 100,
                    "grooming actions {first} and {second} are not visually distinct"
                );
            }
        }
        assert_eq!(
            wing_panel_count(&pet_scene(
                Skin::Firefly,
                Behavior::Groom,
                65.0 * REFERENCE_TICK_SECONDS,
                65.0 * REFERENCE_TICK_SECONDS,
                0.0,
                false,
            )),
            1,
            "wing cleaning must expose exactly one wing"
        );
    }

    fn write_trajectory_fixtures(directory: &std::path::Path) {
        let width = 1_920_usize;
        let height = 1_080_usize;
        let origin = Pos2::ZERO;
        let screen = Vec2::new(width as f32, height as f32);
        let bottom = height as f32 - PET_HEIGHT as f32 - 8.0;
        let mut motion = PetMotion {
            screen_position: Pos2::new(240.0, bottom),
            ..PetMotion::default()
        };
        let mut elapsed = 0.0_f32;
        let mut samples = Vec::new();
        let mut record = |motion: &PetMotion, behavior: Behavior, elapsed: f32| {
            samples.push(TrajectorySample {
                time_seconds: elapsed,
                behavior: format!("{behavior:?}"),
                x: motion.screen_position.x,
                y: motion.screen_position.y,
                heading_radians: motion.heading_radians,
                phase: motion.animation_seconds,
                behavior_age_seconds: motion.behavior_age_seconds,
                altitude_pixels: motion.altitude_pixels,
            });
        };

        record(&motion, Behavior::Walk, elapsed);
        for frame in 1_u32..=120 {
            motion.advance(1.0 / 60.0, Behavior::Walk, origin, screen, false, None);
            elapsed += 1.0 / 60.0;
            if frame.is_multiple_of(60) {
                record(&motion, Behavior::Walk, elapsed);
            }
        }
        let cursor = motion.screen_position + Vec2::new(-90.0, 200.0);
        for _ in 0..12 {
            motion.advance(
                1.0 / 60.0,
                Behavior::PreEscape,
                origin,
                screen,
                false,
                Some(cursor),
            );
            elapsed += 1.0 / 60.0;
        }
        record(&motion, Behavior::PreEscape, elapsed);
        for frame in 1_u32..=240 {
            motion.advance(1.0 / 60.0, Behavior::Flight, origin, screen, false, None);
            elapsed += 1.0 / 60.0;
            if frame.is_multiple_of(48) {
                record(&motion, Behavior::Flight, elapsed);
            }
        }
        for frame in 1_u32..=30 {
            motion.advance(1.0 / 60.0, Behavior::Landing, origin, screen, false, None);
            elapsed += 1.0 / 60.0;
            if frame.is_multiple_of(5) {
                record(&motion, Behavior::Landing, elapsed);
            }
        }
        for _ in 0..90 {
            motion.advance(1.0 / 60.0, Behavior::Groom, origin, screen, false, None);
            elapsed += 1.0 / 60.0;
        }
        record(&motion, Behavior::Groom, elapsed);

        for frame in 1_u32..=60 {
            motion.advance(1.0 / 60.0, Behavior::Walk, origin, screen, false, None);
            elapsed += 1.0 / 60.0;
            if frame == 60 {
                record(&motion, Behavior::Walk, elapsed);
            }
        }
        let cursor = motion.screen_position + Vec2::new(-90.0, 200.0);
        for _ in 0..12 {
            motion.advance(
                1.0 / 60.0,
                Behavior::PreEscape,
                origin,
                screen,
                false,
                Some(cursor),
            );
            elapsed += 1.0 / 60.0;
        }
        record(&motion, Behavior::PreEscape, elapsed);
        for frame in 1_u32..=240 {
            motion.advance(1.0 / 60.0, Behavior::Flight, origin, screen, false, None);
            elapsed += 1.0 / 60.0;
            if frame.is_multiple_of(48) {
                record(&motion, Behavior::Flight, elapsed);
            }
        }
        for frame in 1_u32..=30 {
            motion.advance(1.0 / 60.0, Behavior::Landing, origin, screen, false, None);
            elapsed += 1.0 / 60.0;
            if frame.is_multiple_of(5) {
                record(&motion, Behavior::Landing, elapsed);
            }
        }
        for _ in 0..90 {
            motion.advance(1.0 / 60.0, Behavior::Groom, origin, screen, false, None);
            elapsed += 1.0 / 60.0;
        }
        record(&motion, Behavior::Groom, elapsed);

        for (name, skin) in [("drosophila", Skin::Drosophila), ("prism", Skin::Firefly)] {
            let mut canvas = desktop_canvas(width, height);
            for (index, sample) in samples.iter().enumerate() {
                let behavior = match sample.behavior.as_str() {
                    "Walk" => Behavior::Walk,
                    "PreEscape" => Behavior::PreEscape,
                    "Flight" => Behavior::Flight,
                    "Landing" => Behavior::Landing,
                    "Groom" => Behavior::Groom,
                    _ => Behavior::Rest,
                };
                let mut pet = vec![0; PET_WIDTH * PET_HEIGHT * 4];
                render_pet_bgra_at_age_with_altitude(
                    &mut pet,
                    skin,
                    behavior,
                    sample.phase,
                    sample.behavior_age_seconds,
                    sample.heading_radians,
                    sample.altitude_pixels,
                    false,
                );
                stamp_bgra(
                    &mut canvas,
                    width,
                    height,
                    &pet,
                    sample.x.round() as i32,
                    sample.y.round() as i32,
                    if index + 1 == samples.len() { 255 } else { 176 },
                );
            }
            write_bmp(
                &directory.join(format!("behavior-trajectory-{name}.bmp")),
                width,
                height,
                &canvas,
            );
        }

        let json = serde_json::to_string_pretty(&samples).expect("trajectory must serialize");
        std::fs::write(
            directory.join("behavior-trajectory.json"),
            format!("{json}\n"),
        )
        .expect("trajectory receipt must be writable");
    }

    fn desktop_canvas(width: usize, height: usize) -> Vec<u8> {
        let mut canvas = vec![0_u8; width * height * 4];
        for y in 0..height {
            for x in 0..width {
                let offset = (y * width + x) * 4;
                let glow = ((x as f32 / width as f32) * 22.0) as u8;
                canvas[offset] = 26_u8.saturating_add(glow / 3);
                canvas[offset + 1] = 12_u8.saturating_add(glow / 2);
                canvas[offset + 2] = 13_u8.saturating_add(glow);
                canvas[offset + 3] = 255;
                if y + 10 >= height && y + 8 < height {
                    canvas[offset] = 54;
                    canvas[offset + 1] = 70;
                    canvas[offset + 2] = 92;
                }
            }
        }
        canvas
    }

    #[allow(clippy::too_many_arguments)]
    fn stamp_bgra(
        canvas: &mut [u8],
        canvas_width: usize,
        canvas_height: usize,
        pet: &[u8],
        left: i32,
        top: i32,
        opacity: u8,
    ) {
        for pet_y in 0..PET_HEIGHT {
            let canvas_y = top + pet_y as i32;
            if !(0..canvas_height as i32).contains(&canvas_y) {
                continue;
            }
            for pet_x in 0..PET_WIDTH {
                let canvas_x = left + pet_x as i32;
                if !(0..canvas_width as i32).contains(&canvas_x) {
                    continue;
                }
                let source = (pet_y * PET_WIDTH + pet_x) * 4;
                let source_alpha = pet[source + 3] as u32 * opacity as u32 / 255;
                if source_alpha == 0 {
                    continue;
                }
                let target = (canvas_y as usize * canvas_width + canvas_x as usize) * 4;
                let inverse = 255 - source_alpha;
                for channel in 0..3 {
                    let source_channel = pet[source + channel] as u32 * opacity as u32 / 255;
                    canvas[target + channel] = (source_channel
                        + canvas[target + channel] as u32 * inverse / 255)
                        .min(255) as u8;
                }
            }
        }
    }

    fn write_bmp(path: &std::path::Path, width: usize, height: usize, pixels: &[u8]) {
        let pixel_bytes = width * height * 4;
        let file_size = 14 + 40 + pixel_bytes;
        let mut bytes = Vec::with_capacity(file_size);
        bytes.extend_from_slice(b"BM");
        bytes.extend_from_slice(&(file_size as u32).to_le_bytes());
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&54_u32.to_le_bytes());
        bytes.extend_from_slice(&40_u32.to_le_bytes());
        bytes.extend_from_slice(&(width as i32).to_le_bytes());
        bytes.extend_from_slice(&(height as i32).to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&32_u16.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
        bytes.extend_from_slice(&2_835_i32.to_le_bytes());
        bytes.extend_from_slice(&2_835_i32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        for row in (0..height).rev() {
            let start = row * width * 4;
            bytes.extend_from_slice(&pixels[start..start + width * 4]);
        }
        std::fs::write(path, bytes).expect("trajectory bitmap must be writable");
    }

    fn write_pam(path: &std::path::Path, pixels: &[u8]) {
        let mut bytes = format!(
            "P7\nWIDTH {PET_WIDTH}\nHEIGHT {PET_HEIGHT}\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n"
        )
        .into_bytes();
        for pixel in pixels.as_chunks::<4>().0 {
            let alpha = pixel[3] as u32;
            if alpha == 0 {
                bytes.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                let unpremultiply = |channel: u8| (channel as u32 * 255 / alpha).min(255) as u8;
                bytes.extend_from_slice(&[
                    unpremultiply(pixel[2]),
                    unpremultiply(pixel[1]),
                    unpremultiply(pixel[0]),
                    pixel[3],
                ]);
            }
        }
        std::fs::write(path, bytes).expect("visual fixture must be writable");
    }
}
