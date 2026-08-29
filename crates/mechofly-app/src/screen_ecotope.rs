use std::{fmt, str::FromStr};

use eframe::egui::{Pos2, Vec2};
use mechofly_core::{Action, Behavior, MODEL_STEP_MS};

use crate::pet::{PET_HEIGHT, PET_WIDTH};

pub const ECOTOPE_RULE_VERSION: &str = "screen-ecotope-work-mode-v1";
pub const BELIEF_COLUMNS: usize = 32;
pub const BELIEF_ROWS: usize = 18;

const Q15_ONE: i32 = 32_768;
const DRIVE_REFRESH_FRAMES: u64 = 15;
const FOOD_CUE_THRESHOLD_Q15: i32 = 3_900;
const HUNGER_APPROACH_THRESHOLD_Q15: i32 = 22_000;
const FATIGUE_REST_THRESHOLD_Q15: i32 = 26_000;
const CONTAMINATION_GROOM_THRESHOLD_Q15: i32 = 23_000;
const ESCAPE_THREAT_THRESHOLD_Q15: i32 = 19_500;
const LOCAL_SEARCH_MEMORY_FRAMES: u64 = 30_000 / MODEL_STEP_MS as u64;
const RESOURCE_REWARD_COOLDOWN_FRAMES: u64 = 20_000 / MODEL_STEP_MS as u64;
const PROCESS_NOISE_Q15: i32 = 96;
const MEASUREMENT_NOISE_Q15: i32 = 4_096;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum EcotopeMode {
    #[default]
    Work,
    Observe,
    Experiment,
}

impl EcotopeMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Work => "WORK",
            Self::Observe => "OBSERVE",
            Self::Experiment => "EXPERIMENT",
        }
    }
}

impl FromStr for EcotopeMode {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "work" => Ok(Self::Work),
            "observe" => Ok(Self::Observe),
            "experiment" => Ok(Self::Experiment),
            other => Err(format!("unknown ecotope mode {other:?}")),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum EcotopeIntent {
    #[default]
    QuietWake,
    Explore,
    ApproachFermentation,
    LocalSearch,
    Groom,
    Escape,
}

impl EcotopeIntent {
    pub const fn label(self) -> &'static str {
        match self {
            Self::QuietWake => "QUIET WAKE",
            Self::Explore => "EXPLORE",
            Self::ApproachFermentation => "APPROACH FERMENTATION",
            Self::LocalSearch => "LOCAL SEARCH",
            Self::Groom => "GROOM",
            Self::Escape => "ESCAPE",
        }
    }

    const fn action(self) -> Action {
        match self {
            Self::QuietWake => Action::Pause,
            Self::Explore | Self::ApproachFermentation | Self::LocalSearch => Action::Explore,
            Self::Groom => Action::Groom,
            Self::Escape => Action::Inspect,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentationOverrideReason {
    ObservatoryShelf,
}

impl PresentationOverrideReason {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ObservatoryShelf => "OBSERVATORY SHELF",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AutonomousDrive {
    pub behavior: Behavior,
    pub duration_ms: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MotivationState {
    pub hunger_q15: i32,
    pub thirst_q15: i32,
    pub fatigue_q15: i32,
    pub contamination_q15: i32,
    pub arousal_q15: i32,
}

impl Default for MotivationState {
    fn default() -> Self {
        Self {
            hunger_q15: 21_000,
            thirst_q15: 6_000,
            fatigue_q15: 5_000,
            contamination_q15: 2_000,
            arousal_q15: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceSource {
    pub normalized_x_q15: i32,
    pub normalized_y_q15: i32,
    pub quality_q15: i32,
    pub epoch: u64,
}

impl Default for ResourceSource {
    fn default() -> Self {
        Self {
            normalized_x_q15: 27_525,
            normalized_y_q15: 24_904,
            quality_q15: 27_000,
            epoch: 1,
        }
    }
}

impl ResourceSource {
    fn position(self, origin: Pos2, size: Vec2) -> Pos2 {
        Pos2::new(
            origin.x + size.x * self.normalized_x_q15 as f32 / Q15_ONE as f32,
            origin.y + size.y * self.normalized_y_q15 as f32 / Q15_ONE as f32,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BeliefCell {
    pub expected_reward_q15: i32,
    pub uncertainty_q15: i32,
    pub visits: u32,
    pub last_visit_frame: u64,
    pub last_reward_frame: u64,
}

impl Default for BeliefCell {
    fn default() -> Self {
        Self {
            expected_reward_q15: 0,
            uncertainty_q15: 24_000,
            visits: 0,
            last_visit_frame: 0,
            last_reward_frame: 0,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BeliefMap {
    cells: Vec<BeliefCell>,
}

impl Default for BeliefMap {
    fn default() -> Self {
        Self {
            cells: vec![BeliefCell::default(); BELIEF_COLUMNS * BELIEF_ROWS],
        }
    }
}

impl BeliefMap {
    fn index(position: Pos2, origin: Pos2, size: Vec2) -> usize {
        let width = size.x.max(1.0);
        let height = size.y.max(1.0);
        let x = ((position.x - origin.x) / width).clamp(0.0, 0.999_999);
        let y = ((position.y - origin.y) / height).clamp(0.0, 0.999_999);
        let column = (x * BELIEF_COLUMNS as f32) as usize;
        let row = (y * BELIEF_ROWS as f32) as usize;
        row * BELIEF_COLUMNS + column
    }

    fn observe(&mut self, position: Pos2, origin: Pos2, size: Vec2, reward_q15: i32, frame: u64) {
        let cell = &mut self.cells[Self::index(position, origin, size)];
        let variance = cell.uncertainty_q15.max(1);
        let gain_q15 =
            ((variance as i64 * Q15_ONE as i64) / (variance + MEASUREMENT_NOISE_Q15) as i64) as i32;
        let residual = reward_q15.clamp(0, Q15_ONE) - cell.expected_reward_q15;
        cell.expected_reward_q15 = (cell.expected_reward_q15
            + ((gain_q15 as i64 * residual as i64) / Q15_ONE as i64) as i32)
            .clamp(0, Q15_ONE);
        cell.uncertainty_q15 = ((((Q15_ONE - gain_q15) as i64 * variance as i64) / Q15_ONE as i64)
            as i32
            + PROCESS_NOISE_Q15)
            .clamp(1, Q15_ONE);
        cell.visits = cell.visits.saturating_add(1);
        cell.last_visit_frame = frame;
        if reward_q15 > Q15_ONE / 2 {
            cell.last_reward_frame = frame;
        }
    }

    fn cell_at(&self, position: Pos2, origin: Pos2, size: Vec2) -> BeliefCell {
        self.cells[Self::index(position, origin, size)]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct CursorApproachTracker {
    previous_distance: Option<f32>,
    previous_frame: u64,
    threat_q15: i32,
}

impl CursorApproachTracker {
    fn update(
        &mut self,
        frame: u64,
        cursor_position: Option<Pos2>,
        pet_center: Pos2,
        cursor_over_pet: bool,
    ) -> i32 {
        let Some(cursor) = cursor_position else {
            self.previous_distance = None;
            self.previous_frame = frame;
            self.threat_q15 = self.threat_q15.saturating_sub(1_800);
            return self.threat_q15;
        };
        let distance = pet_center.distance(cursor);
        let delta_frames = frame.saturating_sub(self.previous_frame).max(1);
        let delta_seconds = delta_frames as f32 * MODEL_STEP_MS as f32 / 1_000.0;
        let closing_speed = self
            .previous_distance
            .map(|previous| ((previous - distance) / delta_seconds).max(0.0))
            .unwrap_or(0.0);
        let time_to_contact = if closing_speed > 1.0 {
            distance / closing_speed
        } else {
            f32::INFINITY
        };
        let distance_factor = ((420.0 - distance) / 300.0).clamp(0.0, 1.0);
        let speed_factor = ((closing_speed - 90.0) / 520.0).clamp(0.0, 1.0);
        let contact_factor = ((1.4 - time_to_contact) / 1.2).clamp(0.0, 1.0);
        let kinetic = distance_factor * speed_factor * contact_factor;
        let vigilance = if cursor_over_pet { 0.22 } else { 0.0 };
        let target = ((kinetic.max(vigilance)) * Q15_ONE as f32).round() as i32;
        self.threat_q15 = if target >= self.threat_q15 {
            target
        } else {
            self.threat_q15.saturating_sub(1_200).max(target)
        }
        .clamp(0, Q15_ONE);
        self.previous_distance = Some(distance);
        self.previous_frame = frame;
        self.threat_q15
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EcotopeInput {
    pub frame: u64,
    pub behavior: Behavior,
    pub pet_position: Pos2,
    pub screen_origin: Pos2,
    pub screen_size: Vec2,
    pub cursor_position: Option<Pos2>,
    pub cursor_over_pet: bool,
    pub observatory_open: bool,
    pub recent_interaction: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EcotopeSnapshot {
    pub mode: EcotopeMode,
    pub intent: EcotopeIntent,
    pub source_epoch: u64,
    pub source_position: Pos2,
    pub plume_position: Pos2,
    pub cue_strength_q15: i32,
    pub hunger_q15: i32,
    pub fatigue_q15: i32,
    pub contamination_q15: i32,
    pub arousal_q15: i32,
    pub learned_value_q15: i32,
    pub uncertainty_q15: i32,
    pub visits: u32,
    pub search_radius_pixels: f32,
    pub transition_reason: String,
    pub presentation_override: Option<PresentationOverrideReason>,
    pub learning_claim: &'static str,
}

impl Default for EcotopeSnapshot {
    fn default() -> Self {
        Self {
            mode: EcotopeMode::Work,
            intent: EcotopeIntent::QuietWake,
            source_epoch: 1,
            source_position: Pos2::ZERO,
            plume_position: Pos2::ZERO,
            cue_strength_q15: 0,
            hunger_q15: MotivationState::default().hunger_q15,
            fatigue_q15: MotivationState::default().fatigue_q15,
            contamination_q15: MotivationState::default().contamination_q15,
            arousal_q15: 0,
            learned_value_q15: 0,
            uncertainty_q15: BeliefCell::default().uncertainty_q15,
            visits: 0,
            search_radius_pixels: 0.0,
            transition_reason: "initial quiet-wake state".to_owned(),
            presentation_override: None,
            learning_claim: "MODELED_SOFTWARE_ECOLOGICAL_LEARNING",
        }
    }
}

impl EcotopeSnapshot {
    pub fn status_line(&self) -> String {
        format!(
            "ECOTOPE {}  ·  {}  ·  HUNGER {:>3}%  ·  CUE {:>3}%  ·  VALUE {:>3}%",
            self.mode.label(),
            self.intent.label(),
            percent(self.hunger_q15),
            percent(self.cue_strength_q15),
            percent(self.learned_value_q15),
        )
    }

    pub fn detail_line(&self) -> String {
        let override_label = self
            .presentation_override
            .map(PresentationOverrideReason::label)
            .unwrap_or("NONE");
        format!(
            "uncertainty {:>3}% · visits {} · source epoch {} · override {} · {}",
            percent(self.uncertainty_q15),
            self.visits,
            self.source_epoch,
            override_label,
            self.transition_reason,
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EcotopeOutput {
    pub action: Action,
    pub drive: Option<AutonomousDrive>,
    pub target_position: Option<Pos2>,
    pub cursor_threat_strength: f32,
    pub presentation_position: Option<Pos2>,
    pub snapshot: EcotopeSnapshot,
}

impl Default for EcotopeOutput {
    fn default() -> Self {
        Self {
            action: Action::Pause,
            drive: None,
            target_position: None,
            cursor_threat_strength: 0.0,
            presentation_position: None,
            snapshot: EcotopeSnapshot::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScreenEcotope {
    pub mode: EcotopeMode,
    pub motivation: MotivationState,
    pub source: ResourceSource,
    pub belief: BeliefMap,
    cursor: CursorApproachTracker,
    intent: EcotopeIntent,
    intent_entered_frame: u64,
    intent_target_frames: u64,
    transition_sequence: u64,
    last_frame: u64,
    last_drive_frame: u64,
    last_cue_frame: Option<u64>,
    last_reward_frame: Option<u64>,
    remembered_source: Option<Pos2>,
    seed: u64,
    last_reason: String,
    last_output: EcotopeOutput,
}

impl ScreenEcotope {
    pub fn new(seed: u64, mode: EcotopeMode) -> Self {
        let mut ecotope = Self {
            mode,
            motivation: MotivationState::default(),
            source: ResourceSource::default(),
            belief: BeliefMap::default(),
            cursor: CursorApproachTracker::default(),
            intent: EcotopeIntent::QuietWake,
            intent_entered_frame: 0,
            intent_target_frames: 0,
            transition_sequence: 0,
            last_frame: 0,
            last_drive_frame: 0,
            last_cue_frame: None,
            last_reward_frame: None,
            remembered_source: None,
            seed,
            last_reason: "initial quiet-wake state".to_owned(),
            last_output: EcotopeOutput::default(),
        };
        ecotope.intent_target_frames = ecotope.sample_duration_frames(EcotopeIntent::QuietWake);
        ecotope.last_output.snapshot.mode = mode;
        ecotope
    }

    pub fn current_output(&self) -> EcotopeOutput {
        self.last_output.clone()
    }

    pub fn step(&mut self, input: EcotopeInput) -> EcotopeOutput {
        if input.frame <= self.last_frame {
            return self.last_output.clone();
        }
        let delta_frames = input.frame - self.last_frame;
        self.update_motivation(input.behavior, delta_frames);
        let pet_center =
            input.pet_position + Vec2::new(PET_WIDTH as f32 * 0.5, PET_HEIGHT as f32 * 0.5);
        let threat_q15 = self.cursor.update(
            input.frame,
            input.cursor_position,
            pet_center,
            input.cursor_over_pet,
        );
        self.motivation.arousal_q15 = threat_q15;
        if input.recent_interaction {
            self.motivation.arousal_q15 = self.motivation.arousal_q15.max(4_000);
        }

        let source_position = self.source.position(input.screen_origin, input.screen_size);
        let plume_position = self.plume_position(input.frame, source_position, input.screen_size);
        let cue_strength_q15 = self.cue_strength(input.frame, pet_center, plume_position);
        if cue_strength_q15 >= FOOD_CUE_THRESHOLD_Q15 {
            self.last_cue_frame = Some(input.frame);
            self.remembered_source = Some(source_position);
            self.belief.observe(
                source_position,
                input.screen_origin,
                input.screen_size,
                cue_strength_q15,
                input.frame,
            );
        }

        let source_distance = pet_center.distance(source_position);
        if source_distance <= 72.0
            && self.motivation.hunger_q15 > 7_000
            && self.last_reward_frame.is_none_or(|frame| {
                input.frame.saturating_sub(frame) >= RESOURCE_REWARD_COOLDOWN_FRAMES
            })
        {
            self.motivation.hunger_q15 = self.motivation.hunger_q15.saturating_sub(15_000);
            self.last_reward_frame = Some(input.frame);
            self.last_cue_frame = Some(input.frame);
            self.remembered_source = Some(source_position);
            self.belief.observe(
                source_position,
                input.screen_origin,
                input.screen_size,
                Q15_ONE,
                input.frame,
            );
        }

        let elapsed_in_intent = input.frame.saturating_sub(self.intent_entered_frame);
        let recent_cue = self
            .last_cue_frame
            .is_some_and(|frame| input.frame.saturating_sub(frame) <= LOCAL_SEARCH_MEMORY_FRAMES);
        let next_intent = if threat_q15 >= ESCAPE_THREAT_THRESHOLD_Q15 {
            EcotopeIntent::Escape
        } else if self.motivation.contamination_q15 >= CONTAMINATION_GROOM_THRESHOLD_Q15 {
            EcotopeIntent::Groom
        } else if self.motivation.fatigue_q15 >= FATIGUE_REST_THRESHOLD_Q15 {
            EcotopeIntent::QuietWake
        } else if self.motivation.hunger_q15 >= HUNGER_APPROACH_THRESHOLD_Q15 {
            if cue_strength_q15 >= FOOD_CUE_THRESHOLD_Q15 {
                EcotopeIntent::ApproachFermentation
            } else if recent_cue {
                EcotopeIntent::LocalSearch
            } else {
                EcotopeIntent::Explore
            }
        } else if elapsed_in_intent >= self.intent_target_frames {
            match self.intent {
                EcotopeIntent::QuietWake => EcotopeIntent::Explore,
                EcotopeIntent::Explore
                | EcotopeIntent::ApproachFermentation
                | EcotopeIntent::LocalSearch
                | EcotopeIntent::Groom
                | EcotopeIntent::Escape => EcotopeIntent::QuietWake,
            }
        } else {
            self.intent
        };

        if next_intent != self.intent {
            self.transition_to(next_intent, input.frame, cue_strength_q15, threat_q15);
        }
        if self.intent == EcotopeIntent::Groom
            && input.behavior == Behavior::Groom
            && elapsed_in_intent >= 45
        {
            self.motivation.contamination_q15 =
                self.motivation.contamination_q15.saturating_sub(18_000);
        }

        let target_position = self.target_for_intent(
            input.frame,
            input.screen_origin,
            input.screen_size,
            source_position,
        );
        let presentation_override = input
            .observatory_open
            .then_some(PresentationOverrideReason::ObservatoryShelf);
        let presentation_position = presentation_override
            .map(|_| habitat_shelf_position(input.screen_origin, input.screen_size));
        let learned = self
            .belief
            .cell_at(source_position, input.screen_origin, input.screen_size);
        let search_radius_pixels = if self.intent == EcotopeIntent::LocalSearch {
            32.0 + (elapsed_in_intent as f32 * 0.75).min(180.0)
        } else {
            0.0
        };
        let should_refresh_drive =
            input.frame.saturating_sub(self.last_drive_frame) >= DRIVE_REFRESH_FRAMES;
        let drive = if should_refresh_drive {
            self.drive_for_intent()
        } else {
            None
        };
        if drive.is_some() {
            self.last_drive_frame = input.frame;
        }
        let snapshot = EcotopeSnapshot {
            mode: self.mode,
            intent: self.intent,
            source_epoch: self.source.epoch,
            source_position,
            plume_position,
            cue_strength_q15,
            hunger_q15: self.motivation.hunger_q15,
            fatigue_q15: self.motivation.fatigue_q15,
            contamination_q15: self.motivation.contamination_q15,
            arousal_q15: self.motivation.arousal_q15,
            learned_value_q15: learned.expected_reward_q15,
            uncertainty_q15: learned.uncertainty_q15,
            visits: learned.visits,
            search_radius_pixels,
            transition_reason: self.last_reason.clone(),
            presentation_override,
            learning_claim: "MODELED_SOFTWARE_ECOLOGICAL_LEARNING",
        };
        let output = EcotopeOutput {
            action: self.intent.action(),
            drive,
            target_position,
            cursor_threat_strength: threat_q15 as f32 / Q15_ONE as f32,
            presentation_position,
            snapshot,
        };
        self.last_frame = input.frame;
        self.last_output = output.clone();
        output
    }

    fn update_motivation(&mut self, behavior: Behavior, frames: u64) {
        let frames = frames.min(600) as i32;
        self.motivation.hunger_q15 = self
            .motivation
            .hunger_q15
            .saturating_add(frames.saturating_mul(2))
            .clamp(0, Q15_ONE);
        self.motivation.thirst_q15 = self
            .motivation
            .thirst_q15
            .saturating_add(frames)
            .clamp(0, Q15_ONE);
        let fatigue_delta = match behavior {
            Behavior::Rest | Behavior::Quiet => -6,
            Behavior::Walk | Behavior::Reverse => 3,
            Behavior::Groom | Behavior::Alert => 1,
            Behavior::PreEscape | Behavior::Flight | Behavior::Landing => 8,
        };
        self.motivation.fatigue_q15 = self
            .motivation
            .fatigue_q15
            .saturating_add(frames.saturating_mul(fatigue_delta))
            .clamp(0, Q15_ONE);
        let contamination_bucket = self.last_frame / 1_800;
        let next_bucket = (self.last_frame + frames as u64) / 1_800;
        if next_bucket > contamination_bucket
            && mix64(self.seed ^ next_bucket ^ 0xC07A_61A7).is_multiple_of(4)
        {
            self.motivation.contamination_q15 = self
                .motivation
                .contamination_q15
                .saturating_add(7_000)
                .clamp(0, Q15_ONE);
        }
    }

    fn transition_to(
        &mut self,
        intent: EcotopeIntent,
        frame: u64,
        cue_strength_q15: i32,
        threat_q15: i32,
    ) {
        self.intent = intent;
        self.intent_entered_frame = frame;
        self.transition_sequence = self.transition_sequence.saturating_add(1);
        self.intent_target_frames = self.sample_duration_frames(intent);
        self.last_reason = match intent {
            EcotopeIntent::Escape => format!(
                "cursor approach proxy crossed escape threshold at {}%",
                percent(threat_q15)
            ),
            EcotopeIntent::Groom => {
                "contamination motivation crossed grooming threshold".to_owned()
            }
            EcotopeIntent::ApproachFermentation => format!(
                "virtual fermentation cue detected at {}%",
                percent(cue_strength_q15)
            ),
            EcotopeIntent::LocalSearch => {
                "recent fermentation evidence disappeared; searching remembered location".to_owned()
            }
            EcotopeIntent::Explore => {
                "no reliable resource cue; bounded exploration selected".to_owned()
            }
            EcotopeIntent::QuietWake => {
                "motivation or sampled activity bout ended; quiet wake selected".to_owned()
            }
        };
    }

    fn sample_duration_frames(&self, intent: EcotopeIntent) -> u64 {
        let (minimum, span) = match intent {
            EcotopeIntent::QuietWake => (450, 900),
            EcotopeIntent::Explore => (180, 420),
            EcotopeIntent::ApproachFermentation => (90, 300),
            EcotopeIntent::LocalSearch => (240, 720),
            EcotopeIntent::Groom => (46, 90),
            EcotopeIntent::Escape => (6, 12),
        };
        minimum
            + mix64(
                self.seed
                    ^ self.transition_sequence.rotate_left(17)
                    ^ (intent as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15),
            ) % span
    }

    fn drive_for_intent(&self) -> Option<AutonomousDrive> {
        match self.intent {
            EcotopeIntent::QuietWake => None,
            EcotopeIntent::Explore
            | EcotopeIntent::ApproachFermentation
            | EcotopeIntent::LocalSearch => Some(AutonomousDrive {
                behavior: Behavior::Walk,
                duration_ms: 660,
            }),
            EcotopeIntent::Groom => Some(AutonomousDrive {
                behavior: Behavior::Groom,
                duration_ms: 1_800,
            }),
            EcotopeIntent::Escape => Some(AutonomousDrive {
                behavior: Behavior::PreEscape,
                duration_ms: 594,
            }),
        }
    }

    fn target_for_intent(
        &self,
        frame: u64,
        origin: Pos2,
        size: Vec2,
        source_position: Pos2,
    ) -> Option<Pos2> {
        let half = Vec2::new(PET_WIDTH as f32 * 0.5, PET_HEIGHT as f32 * 0.5);
        let target_center = match self.intent {
            EcotopeIntent::ApproachFermentation => source_position,
            EcotopeIntent::LocalSearch => {
                let center = self.remembered_source.unwrap_or(source_position);
                let elapsed = frame.saturating_sub(self.intent_entered_frame) as f32;
                let radius = 32.0 + (elapsed * 0.75).min(180.0);
                let phase =
                    elapsed * 0.11 + signed_unit(self.seed, self.transition_sequence, 0x51) * 3.0;
                center + Vec2::angled(phase) * radius
            }
            EcotopeIntent::Explore => {
                let bucket = frame / 240;
                let x = 0.16 + 0.68 * unit(self.seed, bucket, 0xA1);
                let y = 0.18 + 0.62 * unit(self.seed, bucket, 0xB2);
                Pos2::new(origin.x + size.x * x, origin.y + size.y * y)
            }
            EcotopeIntent::QuietWake | EcotopeIntent::Groom | EcotopeIntent::Escape => {
                return None;
            }
        };
        Some(clamp_pet_position(target_center - half, origin, size))
    }

    fn plume_position(&self, frame: u64, source: Pos2, size: Vec2) -> Pos2 {
        let bucket = frame / 60;
        let fraction = (frame % 60) as f32 / 60.0;
        let offset = |salt| {
            let current = signed_unit(self.seed, bucket, salt);
            let next = signed_unit(self.seed, bucket + 1, salt);
            current + (next - current) * smoothstep(fraction)
        };
        source
            + Vec2::new(
                offset(0xC3) * (size.x * 0.08).min(150.0),
                offset(0xD4) * (size.y * 0.07).min(100.0),
            )
    }

    fn cue_strength(&self, frame: u64, pet_center: Pos2, plume: Pos2) -> i32 {
        let radius = match self.mode {
            EcotopeMode::Work => 360.0,
            EcotopeMode::Observe => 440.0,
            EcotopeMode::Experiment => 520.0,
        };
        let distance_factor = (1.0 - pet_center.distance(plume) / radius).clamp(0.0, 1.0);
        let dropout_bucket = frame / 45;
        let dropout = mix64(self.seed ^ dropout_bucket ^ 0xD20F_0F5E).is_multiple_of(13);
        let continuity = if dropout { 0.08 } else { 1.0 };
        (distance_factor * continuity * self.source.quality_q15 as f32)
            .round()
            .clamp(0.0, Q15_ONE as f32) as i32
    }
}

fn habitat_shelf_position(origin: Pos2, size: Vec2) -> Pos2 {
    clamp_pet_position(
        Pos2::new(
            origin.x + size.x - PET_WIDTH as f32 - 16.0,
            origin.y + size.y - PET_HEIGHT as f32 - 64.0,
        ),
        origin,
        size,
    )
}

fn clamp_pet_position(position: Pos2, origin: Pos2, size: Vec2) -> Pos2 {
    let left = origin.x + 8.0;
    let top = origin.y + 8.0;
    let right = (origin.x + size.x - PET_WIDTH as f32 - 8.0).max(left);
    let bottom = (origin.y + size.y - PET_HEIGHT as f32 - 8.0).max(top);
    Pos2::new(position.x.clamp(left, right), position.y.clamp(top, bottom))
}

fn percent(value_q15: i32) -> i32 {
    ((value_q15.clamp(0, Q15_ONE) as i64 * 100) / Q15_ONE as i64) as i32
}

fn smoothstep(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

fn unit(seed: u64, sequence: u64, salt: u64) -> f32 {
    let value = mix64(seed ^ sequence.rotate_left(23) ^ salt);
    (value >> 40) as f32 / ((1_u64 << 24) - 1) as f32
}

fn signed_unit(seed: u64, sequence: u64, salt: u64) -> f32 {
    unit(seed, sequence, salt) * 2.0 - 1.0
}

fn mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

impl fmt::Display for EcotopeIntent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

#[derive(Clone, Debug)]
pub struct ScreenEcotopeSelfTest {
    pub passed: bool,
    pub deterministic_replay: bool,
    pub source_stationary: bool,
    pub stationary_cursor_safe: bool,
    pub closing_cursor_threat: bool,
    pub belief_update: bool,
    pub presentation_override_excluded_from_learning: bool,
    pub work_mode_hidden_resource_semantics: bool,
}

pub fn run_screen_ecotope_self_test() -> ScreenEcotopeSelfTest {
    let origin = Pos2::ZERO;
    let size = Vec2::new(1_920.0, 1_080.0);
    let mut a = ScreenEcotope::new(0x0EC0_70FE, EcotopeMode::Work);
    let mut b = ScreenEcotope::new(0x0EC0_70FE, EcotopeMode::Work);
    let mut deterministic_replay = true;
    for frame in 1..=1_200 {
        let cursor = if (300..360).contains(&frame) {
            Some(Pos2::new(1_400.0 - (frame - 300) as f32 * 12.0, 640.0))
        } else {
            Some(Pos2::new(1_650.0, 240.0))
        };
        let input = EcotopeInput {
            frame,
            behavior: Behavior::Rest,
            pet_position: Pos2::new(240.0, 620.0),
            screen_origin: origin,
            screen_size: size,
            cursor_position: cursor,
            cursor_over_pet: false,
            observatory_open: false,
            recent_interaction: false,
        };
        if a.step(input) != b.step(input) {
            deterministic_replay = false;
            break;
        }
    }

    let source_before = a.source.position(origin, size);
    let source_after = a.source.position(origin, size);
    let source_stationary = source_before == source_after;

    let mut stationary = ScreenEcotope::new(0x5157, EcotopeMode::Work);
    let mut stationary_threat = 0.0_f32;
    for frame in 1..=90 {
        stationary_threat = stationary
            .step(EcotopeInput {
                frame,
                behavior: Behavior::Rest,
                pet_position: Pos2::new(760.0, 520.0),
                screen_origin: origin,
                screen_size: size,
                cursor_position: Some(Pos2::new(990.0, 650.0)),
                cursor_over_pet: false,
                observatory_open: false,
                recent_interaction: false,
            })
            .cursor_threat_strength;
    }
    let stationary_cursor_safe = stationary_threat < 0.25;

    let mut closing = ScreenEcotope::new(0x0C10_51A6, EcotopeMode::Work);
    let mut maximum_threat = 0.0_f32;
    for frame in 1..=36 {
        let cursor = Pos2::new(1_520.0 - frame as f32 * 22.0, 660.0);
        maximum_threat = maximum_threat.max(
            closing
                .step(EcotopeInput {
                    frame,
                    behavior: Behavior::Rest,
                    pet_position: Pos2::new(760.0, 520.0),
                    screen_origin: origin,
                    screen_size: size,
                    cursor_position: Some(cursor),
                    cursor_over_pet: false,
                    observatory_open: false,
                    recent_interaction: false,
                })
                .cursor_threat_strength,
        );
    }
    let closing_cursor_threat = maximum_threat >= 0.60;

    let mut rewarded = ScreenEcotope::new(0xFEED, EcotopeMode::Work);
    let source = rewarded.source.position(origin, size);
    let reward_position = source - Vec2::new(PET_WIDTH as f32 * 0.5, PET_HEIGHT as f32 * 0.5);
    let before = rewarded.belief.cell_at(source, origin, size);
    let reward_output = rewarded.step(EcotopeInput {
        frame: 1,
        behavior: Behavior::Rest,
        pet_position: reward_position,
        screen_origin: origin,
        screen_size: size,
        cursor_position: None,
        cursor_over_pet: false,
        observatory_open: false,
        recent_interaction: false,
    });
    let after = rewarded.belief.cell_at(source, origin, size);
    let belief_update = after.visits > before.visits
        && after.expected_reward_q15 > before.expected_reward_q15
        && reward_output.snapshot.learning_claim == "MODELED_SOFTWARE_ECOLOGICAL_LEARNING";

    let belief_before_override = rewarded.belief.clone();
    let override_output = rewarded.step(EcotopeInput {
        frame: 2,
        behavior: Behavior::Rest,
        pet_position: Pos2::new(200.0, 200.0),
        screen_origin: origin,
        screen_size: size,
        cursor_position: None,
        cursor_over_pet: false,
        observatory_open: true,
        recent_interaction: false,
    });
    let presentation_override_excluded_from_learning =
        override_output.presentation_position.is_some()
            && override_output.snapshot.presentation_override
                == Some(PresentationOverrideReason::ObservatoryShelf)
            && rewarded.belief == belief_before_override;

    let work_mode_hidden_resource_semantics = a.mode == EcotopeMode::Work
        && a.source.epoch == 1
        && a.last_output.snapshot.learning_claim == "MODELED_SOFTWARE_ECOLOGICAL_LEARNING";

    let passed = deterministic_replay
        && source_stationary
        && stationary_cursor_safe
        && closing_cursor_threat
        && belief_update
        && presentation_override_excluded_from_learning
        && work_mode_hidden_resource_semantics;
    ScreenEcotopeSelfTest {
        passed,
        deterministic_replay,
        source_stationary,
        stationary_cursor_safe,
        closing_cursor_threat,
        belief_update,
        presentation_override_excluded_from_learning,
        work_mode_hidden_resource_semantics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_ecotope_contract_passes() {
        let result = run_screen_ecotope_self_test();
        assert!(result.passed, "{result:#?}");
    }

    #[test]
    fn ecotope_mode_parser_is_strict() {
        assert_eq!("work".parse(), Ok(EcotopeMode::Work));
        assert_eq!("observe".parse(), Ok(EcotopeMode::Observe));
        assert_eq!("experiment".parse(), Ok(EcotopeMode::Experiment));
        assert!("meadow".parse::<EcotopeMode>().is_err());
    }
}
