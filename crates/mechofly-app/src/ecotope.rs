use eframe::egui::{Pos2, Vec2};
use mechofly_core::Behavior;
use serde::{Deserialize, Serialize};

const BELIEF_WIDTH: usize = 32;
const BELIEF_HEIGHT: usize = 18;
const BELIEF_CELL_COUNT: usize = BELIEF_WIDTH * BELIEF_HEIGHT;
const WORLD_TICK_MS: u32 = 100;
const SAMPLE_INTERVAL_MS: u32 = 1_000;
const Q16: f32 = 65_536.0;
const RESOURCE_SEMANTICS: &str = "generated_or_explicit_only";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EcologicalMode {
    #[default]
    QuietWake,
    Rest,
    Explore,
    ApproachResource,
    Feed,
    LocalSearch,
    RefugeSeek,
    Alert,
    Escape,
}

impl EcologicalMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::QuietWake => "QUIET WAKE",
            Self::Rest => "REST",
            Self::Explore => "EXPLORE",
            Self::ApproachResource => "APPROACH RESOURCE",
            Self::Feed => "FEED / SAMPLE",
            Self::LocalSearch => "LOCAL SEARCH",
            Self::RefugeSeek => "REFUGE SEEK",
            Self::Alert => "ALERT",
            Self::Escape => "ESCAPE",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CursorThreatEvidence {
    pub strength: f32,
    pub distance_pixels: f32,
    pub closing_speed_pixels_per_second: f32,
    pub time_to_contact_seconds: f32,
    pub contact: bool,
    pub claim: &'static str,
}

impl Default for CursorThreatEvidence {
    fn default() -> Self {
        Self {
            strength: 0.0,
            distance_pixels: f32::INFINITY,
            closing_speed_pixels_per_second: 0.0,
            time_to_contact_seconds: f32::INFINITY,
            contact: false,
            claim: "VIRTUAL_CURSOR_APPROACH_PROXY_NOT_MEASURED_OPTICAL_LOOM",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct CursorThreatEstimator {
    previous_distance_pixels: Option<f32>,
    filtered_strength: f32,
}

impl CursorThreatEstimator {
    pub fn update(
        &mut self,
        delta_ms: u32,
        cursor_position: Option<Pos2>,
        pet_center: Pos2,
        contact: bool,
    ) -> CursorThreatEvidence {
        let Some(cursor) = cursor_position else {
            self.previous_distance_pixels = None;
            self.filtered_strength = 0.0;
            return CursorThreatEvidence::default();
        };
        let delta_seconds = (delta_ms.max(1) as f32 / 1_000.0).clamp(0.001, 0.25);
        let distance = pet_center.distance(cursor);
        let closing_speed = self
            .previous_distance_pixels
            .map(|previous| ((previous - distance) / delta_seconds).max(0.0))
            .unwrap_or(0.0);
        self.previous_distance_pixels = Some(distance);

        let time_to_contact = if closing_speed > 1.0 {
            distance / closing_speed
        } else {
            f32::INFINITY
        };
        let closing_score = ((closing_speed - 160.0) / 640.0).clamp(0.0, 1.0);
        let contact_time_score = ((1.25 - time_to_contact) / 1.05).clamp(0.0, 1.0);
        let distance_gate = ((560.0 - distance) / 440.0).clamp(0.0, 1.0);
        let raw = if contact {
            1.0
        } else {
            closing_score * (0.25 + 0.75 * contact_time_score) * distance_gate
        };
        let decay = (1.0 - delta_seconds / 0.35).clamp(0.0, 1.0);
        self.filtered_strength = raw.max(self.filtered_strength * decay).clamp(0.0, 1.0);

        CursorThreatEvidence {
            strength: self.filtered_strength,
            distance_pixels: distance,
            closing_speed_pixels_per_second: closing_speed,
            time_to_contact_seconds: time_to_contact,
            contact,
            ..CursorThreatEvidence::default()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
struct BeliefCell {
    expected_reward: f32,
    reward_variance: f32,
    visit_count: u32,
    last_visit_tick: u64,
    last_reward_tick: Option<u64>,
}

impl Default for BeliefCell {
    fn default() -> Self {
        Self {
            expected_reward: 0.0,
            reward_variance: 1.0,
            visit_count: 0,
            last_visit_tick: 0,
            last_reward_tick: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EcotopeSnapshot {
    pub mode: EcologicalMode,
    pub source_epoch: u64,
    pub hunger: f32,
    pub fatigue: f32,
    pub cue_strength: f32,
    pub best_expected_reward: f32,
    pub best_uncertainty: f32,
    pub source_normalized: [f32; 2],
    pub plume_normalized: [f32; 2],
    pub target_normalized: Option<[f32; 2]>,
    pub resource_semantics: &'static str,
    pub generated_habitat_visible: bool,
    pub learning_claim: &'static str,
    pub transition_reason: &'static str,
}

impl Default for EcotopeSnapshot {
    fn default() -> Self {
        Self {
            mode: EcologicalMode::QuietWake,
            source_epoch: 1,
            hunger: 0.62,
            fatigue: 0.24,
            cue_strength: 0.0,
            best_expected_reward: 0.0,
            best_uncertainty: 1.0,
            source_normalized: [0.72, 0.68],
            plume_normalized: [0.72, 0.68],
            target_normalized: None,
            resource_semantics: RESOURCE_SEMANTICS,
            generated_habitat_visible: false,
            learning_claim: "MODELED_SOFTWARE_SPATIAL_LEARNING_FROM_GENERATED_REWARD",
            transition_reason: "initial quiet wake",
        }
    }
}

#[derive(Clone, Debug)]
pub struct EcotopeDirective {
    pub snapshot: EcotopeSnapshot,
    pub target_screen: Option<Pos2>,
    pub requested_behavior: Option<Behavior>,
    pub drive_duration_ms: u32,
}

#[derive(Clone, Debug)]
pub struct ScreenEcotope {
    seed: u64,
    world_tick: u64,
    tick_accumulator_ms: u32,
    sample_accumulator_ms: u32,
    source_epoch: u64,
    source_normalized: [f32; 2],
    refuge_normalized: [f32; 2],
    plume_offset_q16: [i32; 2],
    plume_velocity_q16: [i32; 2],
    hunger: f32,
    fatigue: f32,
    recent_reward_ms: Option<u64>,
    remembered_source: Option<[f32; 2]>,
    belief: Vec<BeliefCell>,
    mode: EcologicalMode,
    transition_reason: &'static str,
}

impl ScreenEcotope {
    pub fn new(seed: u64) -> Self {
        let source_x = 0.18 + unit_interval(mix64(seed ^ 0xF001)) * 0.64;
        let source_y = 0.28 + unit_interval(mix64(seed ^ 0xF002)) * 0.48;
        let source_normalized = [source_x, source_y];
        let refuge_normalized = if source_x < 0.5 {
            [0.90, 0.84]
        } else {
            [0.10, 0.84]
        };
        Self {
            seed,
            world_tick: 0,
            tick_accumulator_ms: 0,
            sample_accumulator_ms: 0,
            source_epoch: 1,
            source_normalized,
            refuge_normalized,
            plume_offset_q16: [0, 0],
            plume_velocity_q16: [0, 0],
            hunger: 0.62,
            fatigue: 0.24,
            recent_reward_ms: None,
            remembered_source: None,
            belief: vec![BeliefCell::default(); BELIEF_CELL_COUNT],
            mode: EcologicalMode::QuietWake,
            transition_reason: "initial quiet wake",
        }
    }

    pub fn step(
        &mut self,
        delta_ms: u32,
        fly_center: Pos2,
        screen_origin: Pos2,
        screen_size: Vec2,
        behavior: Behavior,
        cursor_threat: f32,
    ) -> EcotopeDirective {
        let delta_ms = delta_ms.min(250);
        self.advance_world(delta_ms);
        self.advance_motivation(delta_ms, behavior);

        let fly_normalized = normalize_position(fly_center, screen_origin, screen_size);
        let plume_normalized = self.plume_normalized();
        let cue_distance_sq = squared_distance(fly_normalized, plume_normalized);
        let sigma = 0.16_f32;
        let intermittent = if mix64(self.seed ^ (self.world_tick / 10)) & 0xff < 214 {
            1.0
        } else {
            0.06
        };
        let cue_strength = (-cue_distance_sq / (2.0 * sigma * sigma)).exp() * intermittent;
        let at_source = squared_distance(fly_normalized, self.source_normalized) < 0.045_f32.powi(2);
        let reward = at_source && self.hunger > 0.10;

        self.sample_accumulator_ms = self.sample_accumulator_ms.saturating_add(delta_ms);
        while self.sample_accumulator_ms >= SAMPLE_INTERVAL_MS {
            self.sample_accumulator_ms -= SAMPLE_INTERVAL_MS;
            self.update_belief(fly_normalized, reward);
            if reward {
                self.hunger = (self.hunger - 0.08).max(0.0);
                self.recent_reward_ms = Some(0);
                self.remembered_source = Some(self.source_normalized);
            } else if let Some(age) = &mut self.recent_reward_ms {
                *age = age.saturating_add(u64::from(SAMPLE_INTERVAL_MS));
            }
        }

        let previous_mode = self.mode;
        let (mode, target_normalized, reason) = self.select_mode(
            behavior,
            cursor_threat,
            cue_strength,
            reward,
            fly_normalized,
        );
        self.mode = mode;
        self.transition_reason = if previous_mode == mode {
            self.transition_reason
        } else {
            reason
        };

        let (best_expected_reward, best_uncertainty) = self.best_belief();
        let target_screen = target_normalized
            .map(|target| denormalize_position(target, screen_origin, screen_size));
        let requested_behavior = match mode {
            EcologicalMode::Explore
            | EcologicalMode::ApproachResource
            | EcologicalMode::LocalSearch
            | EcologicalMode::RefugeSeek
                if matches!(behavior, Behavior::Rest | Behavior::Quiet | Behavior::Alert) =>
            {
                Some(Behavior::Walk)
            }
            _ => None,
        };

        EcotopeDirective {
            snapshot: EcotopeSnapshot {
                mode,
                source_epoch: self.source_epoch,
                hunger: self.hunger,
                fatigue: self.fatigue,
                cue_strength,
                best_expected_reward,
                best_uncertainty,
                source_normalized: self.source_normalized,
                plume_normalized,
                target_normalized,
                resource_semantics: RESOURCE_SEMANTICS,
                generated_habitat_visible: false,
                learning_claim: "MODELED_SOFTWARE_SPATIAL_LEARNING_FROM_GENERATED_REWARD",
                transition_reason: self.transition_reason,
            },
            target_screen,
            requested_behavior,
            drive_duration_ms: 594,
        }
    }

    fn advance_world(&mut self, delta_ms: u32) {
        self.tick_accumulator_ms = self.tick_accumulator_ms.saturating_add(delta_ms);
        while self.tick_accumulator_ms >= WORLD_TICK_MS {
            self.tick_accumulator_ms -= WORLD_TICK_MS;
            self.world_tick = self.world_tick.saturating_add(1);
            for axis in 0..2 {
                let hash = mix64(
                    self.seed
                        ^ self.world_tick.rotate_left(17)
                        ^ (axis as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15),
                );
                let noise = ((hash & 0x1ff) as i32) - 256;
                self.plume_velocity_q16[axis] =
                    (self.plume_velocity_q16[axis] * 15 / 16 + noise * 2).clamp(-420, 420);
                self.plume_offset_q16[axis] = (self.plume_offset_q16[axis]
                    + self.plume_velocity_q16[axis])
                    .clamp(-5_242, 5_242);
            }
        }
    }

    fn advance_motivation(&mut self, delta_ms: u32, behavior: Behavior) {
        let delta = delta_ms as f32;
        self.hunger = (self.hunger + delta * 0.000_000_05).clamp(0.0, 1.0);
        let fatigue_delta = if matches!(
            behavior,
            Behavior::Walk | Behavior::Reverse | Behavior::PreEscape | Behavior::Flight
        ) {
            delta * 0.000_000_14
        } else if matches!(behavior, Behavior::Rest | Behavior::Quiet) {
            -delta * 0.000_000_18
        } else {
            0.0
        };
        self.fatigue = (self.fatigue + fatigue_delta).clamp(0.0, 1.0);
    }

    fn update_belief(&mut self, position: [f32; 2], reward: bool) {
        for cell in &mut self.belief {
            cell.reward_variance = (cell.reward_variance + 0.002).min(1.5);
        }
        let index = belief_index(position);
        let cell = &mut self.belief[index];
        let measurement = if reward { 1.0 } else { 0.0 };
        let measurement_variance = 0.20;
        let gain = cell.reward_variance / (cell.reward_variance + measurement_variance);
        cell.expected_reward += gain * (measurement - cell.expected_reward);
        cell.reward_variance = ((1.0 - gain) * cell.reward_variance).max(0.01);
        cell.visit_count = cell.visit_count.saturating_add(1);
        cell.last_visit_tick = self.world_tick;
        if reward {
            cell.last_reward_tick = Some(self.world_tick);
        }
    }

    fn select_mode(
        &self,
        behavior: Behavior,
        cursor_threat: f32,
        cue_strength: f32,
        reward: bool,
        fly_normalized: [f32; 2],
    ) -> (EcologicalMode, Option<[f32; 2]>, &'static str) {
        if cursor_threat >= 0.55 {
            return (EcologicalMode::Escape, None, "cursor approach threat crossed escape gate");
        }
        if cursor_threat >= 0.18 {
            return (EcologicalMode::Alert, None, "cursor approach raised vigilance");
        }
        if self.fatigue >= 0.82 {
            return (
                EcologicalMode::RefugeSeek,
                Some(self.refuge_normalized),
                "fatigue selected a stable peripheral refuge",
            );
        }
        if reward {
            return (
                EcologicalMode::Feed,
                Some(self.source_normalized),
                "generated fermentation source delivered reward",
            );
        }
        if self.hunger >= 0.32 && cue_strength >= 0.12 {
            return (
                EcologicalMode::ApproachResource,
                Some(self.source_normalized),
                "virtual fermentation cue supports directed approach",
            );
        }
        if self.hunger >= 0.20
            && self
                .recent_reward_ms
                .is_some_and(|age| age <= 45_000)
        {
            let center = self.remembered_source.unwrap_or_else(|| self.best_belief_position());
            let age = self.recent_reward_ms.unwrap_or_default() as f32 / 45_000.0;
            let radius = 0.025 + age * 0.12;
            let phase_hash = mix64(self.seed ^ self.world_tick / 6);
            let phase = unit_interval(phase_hash) * std::f32::consts::TAU;
            let target = [
                (center[0] + phase.cos() * radius).clamp(0.04, 0.96),
                (center[1] + phase.sin() * radius).clamp(0.04, 0.96),
            ];
            return (
                EcologicalMode::LocalSearch,
                Some(target),
                "recent reward memory supports bounded local search",
            );
        }
        if self.fatigue >= 0.62 {
            return (EcologicalMode::Rest, None, "fatigue supports an immobile rest bout");
        }
        if matches!(behavior, Behavior::Rest | Behavior::Quiet) && self.hunger < 0.28 {
            return (EcologicalMode::QuietWake, None, "no salient cue; preserve quiet wake");
        }

        let epoch = self.world_tick / 450;
        let hash_x = mix64(self.seed ^ epoch ^ 0xE101);
        let hash_y = mix64(self.seed ^ epoch ^ 0xE102);
        let exploration_target = [
            0.08 + unit_interval(hash_x) * 0.84,
            0.10 + unit_interval(hash_y) * 0.76,
        ];
        let target = if squared_distance(fly_normalized, exploration_target) < 0.02_f32.powi(2) {
            [1.0 - exploration_target[0], exploration_target[1]]
        } else {
            exploration_target
        };
        (
            EcologicalMode::Explore,
            Some(target),
            "uncued exploration selected a persistent low-frequency target",
        )
    }

    fn plume_normalized(&self) -> [f32; 2] {
        [
            (self.source_normalized[0] + self.plume_offset_q16[0] as f32 / Q16)
                .clamp(0.02, 0.98),
            (self.source_normalized[1] + self.plume_offset_q16[1] as f32 / Q16)
                .clamp(0.02, 0.98),
        ]
    }

    fn best_belief(&self) -> (f32, f32) {
        self.belief
            .iter()
            .max_by(|left, right| {
                left.expected_reward
                    .total_cmp(&right.expected_reward)
                    .then_with(|| right.reward_variance.total_cmp(&left.reward_variance))
            })
            .map(|cell| (cell.expected_reward, cell.reward_variance.sqrt()))
            .unwrap_or((0.0, 1.0))
    }

    fn best_belief_position(&self) -> [f32; 2] {
        let index = self
            .belief
            .iter()
            .enumerate()
            .max_by(|(_, left), (_, right)| {
                left.expected_reward
                    .total_cmp(&right.expected_reward)
                    .then_with(|| right.reward_variance.total_cmp(&left.reward_variance))
            })
            .map(|(index, _)| index)
            .unwrap_or(BELIEF_CELL_COUNT / 2);
        belief_center(index)
    }
}

#[derive(Clone, Debug)]
pub struct EcotopeSelfTest {
    pub passed: bool,
    pub deterministic_replay: bool,
    pub stationary_source: bool,
    pub plume_changes_without_source_motion: bool,
    pub reward_updates_belief: bool,
    pub generated_only_resource_semantics: bool,
    pub habitat_hidden_in_work_mode: bool,
    pub stationary_cursor_not_loom: bool,
    pub approaching_cursor_loom: bool,
}

pub fn run_ecotope_self_test() -> EcotopeSelfTest {
    let origin = Pos2::ZERO;
    let screen = Vec2::new(1_920.0, 1_080.0);
    let mut first = ScreenEcotope::new(0xEC07_0FE1);
    let mut second = ScreenEcotope::new(0xEC07_0FE1);
    let source_before = first.source_normalized;
    let plume_before = first.plume_normalized();
    let fly = denormalize_position(source_before, origin, screen);
    let mut first_snapshot = EcotopeSnapshot::default();
    let mut second_snapshot = EcotopeSnapshot::default();
    for _ in 0..60 {
        first_snapshot = first
            .step(100, fly, origin, screen, Behavior::Quiet, 0.0)
            .snapshot;
        second_snapshot = second
            .step(100, fly, origin, screen, Behavior::Quiet, 0.0)
            .snapshot;
    }
    let deterministic_replay = first_snapshot == second_snapshot;
    let stationary_source = first_snapshot.source_normalized == source_before;
    let plume_changes_without_source_motion = first_snapshot.plume_normalized != plume_before;
    let reward_updates_belief = first_snapshot.best_expected_reward > 0.5;

    let pet_center = Pos2::new(960.0, 540.0);
    let mut stationary_estimator = CursorThreatEstimator::default();
    let mut stationary = CursorThreatEvidence::default();
    for _ in 0..30 {
        stationary = stationary_estimator.update(
            50,
            Some(Pos2::new(1_240.0, 540.0)),
            pet_center,
            false,
        );
    }
    let stationary_cursor_not_loom = stationary.strength < 0.05;

    let mut approaching_estimator = CursorThreatEstimator::default();
    let mut approaching = CursorThreatEvidence::default();
    for distance in (80..=520).rev().step_by(40) {
        approaching = approaching_estimator.update(
            40,
            Some(Pos2::new(960.0 + distance as f32, 540.0)),
            pet_center,
            false,
        );
    }
    let approaching_cursor_loom = approaching.strength >= 0.55;
    let generated_only_resource_semantics = first_snapshot.resource_semantics == RESOURCE_SEMANTICS;
    let habitat_hidden_in_work_mode = !first_snapshot.generated_habitat_visible;

    EcotopeSelfTest {
        passed: deterministic_replay
            && stationary_source
            && plume_changes_without_source_motion
            && reward_updates_belief
            && generated_only_resource_semantics
            && habitat_hidden_in_work_mode
            && stationary_cursor_not_loom
            && approaching_cursor_loom,
        deterministic_replay,
        stationary_source,
        plume_changes_without_source_motion,
        reward_updates_belief,
        generated_only_resource_semantics,
        habitat_hidden_in_work_mode,
        stationary_cursor_not_loom,
        approaching_cursor_loom,
    }
}

fn normalize_position(position: Pos2, origin: Pos2, size: Vec2) -> [f32; 2] {
    [
        ((position.x - origin.x) / size.x.max(1.0)).clamp(0.0, 1.0),
        ((position.y - origin.y) / size.y.max(1.0)).clamp(0.0, 1.0),
    ]
}

fn denormalize_position(position: [f32; 2], origin: Pos2, size: Vec2) -> Pos2 {
    Pos2::new(
        origin.x + position[0] * size.x.max(1.0),
        origin.y + position[1] * size.y.max(1.0),
    )
}

fn belief_index(position: [f32; 2]) -> usize {
    let x = (position[0].clamp(0.0, 0.999_999) * BELIEF_WIDTH as f32) as usize;
    let y = (position[1].clamp(0.0, 0.999_999) * BELIEF_HEIGHT as f32) as usize;
    y * BELIEF_WIDTH + x
}

fn belief_center(index: usize) -> [f32; 2] {
    let x = index % BELIEF_WIDTH;
    let y = index / BELIEF_WIDTH;
    [
        (x as f32 + 0.5) / BELIEF_WIDTH as f32,
        (y as f32 + 0.5) / BELIEF_HEIGHT as f32,
    ]
}

fn squared_distance(left: [f32; 2], right: [f32; 2]) -> f32 {
    let x = left[0] - right[0];
    let y = left[1] - right[1];
    x * x + y * y
}

fn unit_interval(value: u64) -> f32 {
    ((value >> 40) as u32 & 0x00ff_ffff) as f32 / 0x0100_0000 as f32
}

fn mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_source_and_moving_plume_are_separate_state() {
        let result = run_ecotope_self_test();
        assert!(result.stationary_source);
        assert!(result.plume_changes_without_source_motion);
    }

    #[test]
    fn identical_histories_replay_identically() {
        assert!(run_ecotope_self_test().deterministic_replay);
    }

    #[test]
    fn stationary_cursor_is_not_a_loom_but_rapid_approach_is() {
        let result = run_ecotope_self_test();
        assert!(result.stationary_cursor_not_loom);
        assert!(result.approaching_cursor_loom);
    }

    #[test]
    fn generated_reward_updates_spatial_belief() {
        assert!(run_ecotope_self_test().reward_updates_belief);
    }
}
