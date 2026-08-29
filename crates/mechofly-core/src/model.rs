use std::sync::Arc;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{MODEL_VERSION, graph::ModelGraph, graph::mix64, provenance::sha256_hex};

pub const ACTIVATION_MIN: i32 = -32_768;
pub const ACTIVATION_MAX: i32 = 32_767;
pub const SPIKE_THRESHOLD: i32 = 8_000;
pub const RESET_DELTA: i32 = 10_000;
pub const FUNCTIONAL_POPULATION_COUNT: usize = 9;
pub const LOOM_POPULATION_OFFSET: usize = 0;
pub const ALERT_POPULATION_OFFSET: usize = 3;
pub const REVERSE_POPULATION_OFFSET: usize = 4;
pub const WALK_POPULATION_OFFSET: usize = 5;
pub const GROOM_POPULATION_OFFSET: usize = 6;
pub const ESCAPE_HOLD_FRAMES: u32 = 5;
pub const FLIGHT_HOLD_FRAMES: u32 = 120;
pub const LANDING_HOLD_FRAMES: u32 = 14;
pub const GROOM_HOLD_FRAMES: u32 = 45;
const LOOM_ESCAPE_ACTIVATION_Q15: i32 = 5_200;
const AUTHORED_BEHAVIOR_ACTIVATION_Q15: i32 = 4_600;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Behavior {
    #[default]
    Rest,
    Quiet,
    Walk,
    Reverse,
    Groom,
    Alert,
    PreEscape,
    Flight,
    Landing,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModelState {
    pub frame: u64,
    pub seed: u64,
    pub activation: Vec<i32>,
    pub spikes: Vec<u8>,
    pub behavior: Behavior,
    pub behavior_age_frames: u32,
}

impl ModelState {
    pub fn digest(&self) -> String {
        let frame = self.frame.to_le_bytes();
        let seed = self.seed.to_le_bytes();
        let behavior = [self.behavior as u8];
        let age = self.behavior_age_frames.to_le_bytes();
        let activation: Vec<u8> = self
            .activation
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        sha256_hex([
            frame.as_slice(),
            seed.as_slice(),
            &behavior,
            age.as_slice(),
            activation.as_slice(),
            self.spikes.as_slice(),
        ])
    }
}

#[derive(Clone, Debug)]
pub struct StepInput<'a> {
    pub stimulus_q15: &'a [i32],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FrameSummary {
    pub frame: u64,
    pub spike_count: usize,
    pub mean_activation_q15: i32,
    pub behavior: Behavior,
    pub state_digest: String,
}

#[derive(Clone)]
pub struct ModelEngine {
    pub graph: Arc<ModelGraph>,
    pub state: ModelState,
}

impl ModelEngine {
    pub fn new(graph: Arc<ModelGraph>, seed: u64) -> Self {
        let count = graph.neuron_ids.len();
        let activation = (0..count)
            .map(|index| initial_activation(seed, index))
            .collect();
        Self {
            graph,
            state: ModelState {
                frame: 0,
                seed,
                activation,
                spikes: vec![0; count],
                behavior: Behavior::Rest,
                behavior_age_frames: 0,
            },
        }
    }

    pub fn from_state(graph: Arc<ModelGraph>, state: ModelState) -> Result<Self, String> {
        if state.activation.len() != graph.neuron_ids.len()
            || state.spikes.len() != graph.neuron_ids.len()
        {
            return Err("checkpoint dimensions do not match graph".to_owned());
        }
        Ok(Self { graph, state })
    }

    pub fn empty_stimulus(&self) -> Vec<i32> {
        vec![0; self.state.activation.len()]
    }

    pub fn step_cpu(&mut self, input: StepInput<'_>) -> FrameSummary {
        assert_eq!(input.stimulus_q15.len(), self.state.activation.len());
        let next_frame = self.state.frame + 1;
        let previous = &self.state.activation;
        let offsets = &self.graph.incoming_offsets;
        let sources = &self.graph.incoming_sources;
        let weights = &self.graph.modeled_weights;
        let seed = self.state.seed;

        let computed: Vec<(i32, u8)> = (0..previous.len())
            .into_par_iter()
            .map(|target| {
                update_neuron(
                    target,
                    next_frame,
                    seed,
                    previous,
                    offsets,
                    sources,
                    weights,
                    input.stimulus_q15[target],
                )
            })
            .collect();

        let (activation, spikes): (Vec<i32>, Vec<u8>) = computed.into_iter().unzip();
        self.accept_backend_step(activation, spikes)
    }

    pub fn accept_backend_step(&mut self, activation: Vec<i32>, spikes: Vec<u8>) -> FrameSummary {
        assert_eq!(activation.len(), self.state.activation.len());
        assert_eq!(spikes.len(), self.state.spikes.len());
        self.state.activation = activation;
        self.state.spikes = spikes;
        self.state.frame += 1;
        let spike_count = self.state.spikes.iter().map(|value| *value as usize).sum();
        let mean_activation_q15 = if self.state.activation.is_empty() {
            0
        } else {
            (self.state.activation.iter().map(|v| *v as i64).sum::<i64>()
                / self.state.activation.len() as i64) as i32
        };
        let next_behavior = modeled_behavior(&self.state, spike_count);
        if next_behavior == self.state.behavior {
            self.state.behavior_age_frames += 1;
        } else {
            self.state.behavior = next_behavior;
            self.state.behavior_age_frames = 0;
        }
        self.summary(spike_count, mean_activation_q15)
    }

    pub fn summary(&self, spike_count: usize, mean_activation_q15: i32) -> FrameSummary {
        FrameSummary {
            frame: self.state.frame,
            spike_count,
            mean_activation_q15,
            behavior: self.state.behavior,
            state_digest: self.state.digest(),
        }
    }

    pub fn model_identity(&self) -> String {
        sha256_hex([
            MODEL_VERSION.as_bytes(),
            self.graph.identity.sha256.as_bytes(),
            self.state.seed.to_le_bytes().as_slice(),
        ])
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update_neuron(
    target: usize,
    frame: u64,
    seed: u64,
    previous: &[i32],
    offsets: &[u32],
    sources: &[u32],
    weights: &[i32],
    stimulus_q15: i32,
) -> (i32, u8) {
    let mut drive = stimulus_q15.clamp(-8_192, 8_192);
    let start = offsets[target] as usize;
    let end = offsets[target + 1] as usize;
    for edge in start..end {
        let source = sources[edge] as usize;
        let contribution = (previous[source] * weights[edge] / 4_096).clamp(-512, 512);
        drive = (drive + contribution).clamp(-65_536, 65_536);
    }
    let noise_hash = model_noise(seed, frame, target as u32);
    let noise = ((noise_hash & 0x1ff) as i32) - 256;
    drive = (drive + noise).clamp(-65_536, 65_536);

    let candidate = ((previous[target] * 13) + (drive * 3)) / 16;
    if candidate > SPIKE_THRESHOLD {
        (
            (candidate - RESET_DELTA).clamp(ACTIVATION_MIN, ACTIVATION_MAX),
            1,
        )
    } else {
        (candidate.clamp(ACTIVATION_MIN, ACTIVATION_MAX), 0)
    }
}

pub fn model_noise(seed: u64, frame: u64, target: u32) -> u32 {
    let mut value = (seed as u32)
        ^ ((seed >> 32) as u32)
        ^ (frame as u32).rotate_left(17)
        ^ ((frame >> 32) as u32).rotate_left(7)
        ^ target.wrapping_mul(0x9E37_79B9);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^ (value >> 16)
}

fn initial_activation(seed: u64, index: usize) -> i32 {
    let h = mix64(seed ^ (index as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93));
    ((h & 0x0fff) as i32) - 2_048
}

fn modeled_behavior(state: &ModelState, spike_count: usize) -> Behavior {
    match state.behavior {
        Behavior::PreEscape if state.behavior_age_frames < ESCAPE_HOLD_FRAMES => {
            return Behavior::PreEscape;
        }
        Behavior::PreEscape => return Behavior::Flight,
        Behavior::Flight if state.behavior_age_frames < FLIGHT_HOLD_FRAMES => {
            return Behavior::Flight;
        }
        Behavior::Flight => return Behavior::Landing,
        Behavior::Landing if state.behavior_age_frames < LANDING_HOLD_FRAMES => {
            return Behavior::Landing;
        }
        Behavior::Landing => return Behavior::Rest,
        _ => {}
    }

    let loom_activation = functional_population_activation(state, LOOM_POPULATION_OFFSET);
    if loom_activation >= LOOM_ESCAPE_ACTIVATION_Q15 {
        return Behavior::PreEscape;
    }

    if state.behavior == Behavior::Groom && state.behavior_age_frames < GROOM_HOLD_FRAMES {
        return Behavior::Groom;
    }

    // These are bounded authored inputs to modeled functional populations,
    // not presentation-only pose switches. The model must cross the same
    // activation boundary before the corresponding motor program is exposed.
    if functional_population_activation(state, GROOM_POPULATION_OFFSET)
        >= AUTHORED_BEHAVIOR_ACTIVATION_Q15
    {
        return Behavior::Groom;
    }
    if functional_population_activation(state, ALERT_POPULATION_OFFSET)
        >= AUTHORED_BEHAVIOR_ACTIVATION_Q15
    {
        return Behavior::Alert;
    }
    if functional_population_activation(state, REVERSE_POPULATION_OFFSET)
        >= AUTHORED_BEHAVIOR_ACTIVATION_Q15
    {
        return Behavior::Reverse;
    }
    if functional_population_activation(state, WALK_POPULATION_OFFSET)
        >= AUTHORED_BEHAVIOR_ACTIVATION_Q15
    {
        return Behavior::Walk;
    }

    let rate_per_10k = spike_count.saturating_mul(10_000) / state.activation.len().max(1);
    if rate_per_10k > 1_200 {
        Behavior::Alert
    } else if rate_per_10k > 320 {
        Behavior::Quiet
    } else {
        Behavior::Rest
    }
}

pub fn run_autonomy_schedule_self_test() -> bool {
    let mut state = ModelState {
        frame: 0,
        seed: 11,
        activation: vec![0; FUNCTIONAL_POPULATION_COUNT * 4],
        spikes: vec![0; FUNCTIONAL_POPULATION_COUNT * 4],
        behavior: Behavior::Rest,
        behavior_age_frames: 0,
    };
    (0..900).all(|frame| {
        state.frame = frame;
        matches!(
            modeled_behavior(&state, 0),
            Behavior::Rest | Behavior::Quiet
        )
    })
}

fn functional_population_activation(state: &ModelState, offset: usize) -> i32 {
    state
        .activation
        .iter()
        .skip(offset)
        .step_by(FUNCTIONAL_POPULATION_COUNT)
        .copied()
        .max()
        .unwrap_or(ACTIVATION_MIN)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::ModelTier;

    #[test]
    fn identical_engines_remain_identical() {
        let graph = Arc::new(ModelGraph::synthetic(ModelTier::Demo4096, 5));
        let mut a = ModelEngine::new(Arc::clone(&graph), 11);
        let mut b = ModelEngine::new(graph, 11);
        let stimulus = a.empty_stimulus();
        for _ in 0..20 {
            assert_eq!(
                a.step_cpu(StepInput {
                    stimulus_q15: &stimulus
                }),
                b.step_cpu(StepInput {
                    stimulus_q15: &stimulus
                })
            );
        }
        assert_eq!(a.state, b.state);
    }

    #[test]
    fn loom_population_crosses_neural_threshold_before_escape() {
        let graph = Arc::new(ModelGraph::synthetic(ModelTier::Demo4096, 7));
        let mut engine = ModelEngine::new(graph, 11);
        let mut stimulus = engine.empty_stimulus();
        for value in stimulus
            .iter_mut()
            .skip(LOOM_POPULATION_OFFSET)
            .step_by(FUNCTIONAL_POPULATION_COUNT)
        {
            *value = 8_192;
        }

        let mut escaped = false;
        for _ in 0..24 {
            let summary = engine.step_cpu(StepInput {
                stimulus_q15: &stimulus,
            });
            if summary.behavior == Behavior::PreEscape {
                escaped = true;
                break;
            }
        }
        assert!(
            escaped,
            "loom neural population never crossed the controller threshold"
        );
    }

    #[test]
    fn frame_counter_no_longer_schedules_motor_actions() {
        assert!(run_autonomy_schedule_self_test());
    }

    #[test]
    fn zero_sensory_input_cannot_trigger_escape_sequence() {
        let graph = Arc::new(ModelGraph::synthetic(ModelTier::Demo4096, 7));
        let mut engine = ModelEngine::new(graph, 11);
        let stimulus = engine.empty_stimulus();
        for _ in 0..300 {
            let behavior = engine
                .step_cpu(StepInput {
                    stimulus_q15: &stimulus,
                })
                .behavior;
            assert!(!matches!(
                behavior,
                Behavior::PreEscape | Behavior::Flight | Behavior::Landing
            ));
        }
    }

    #[test]
    fn recorded_escape_flight_landing_envelopes_are_pinned() {
        let graph = Arc::new(ModelGraph::synthetic(ModelTier::Demo4096, 7));
        let mut state = ModelEngine::new(graph, 11).state;

        state.behavior = Behavior::PreEscape;
        state.behavior_age_frames = ESCAPE_HOLD_FRAMES - 1;
        assert_eq!(modeled_behavior(&state, 0), Behavior::PreEscape);
        state.behavior_age_frames = ESCAPE_HOLD_FRAMES;
        assert_eq!(modeled_behavior(&state, 0), Behavior::Flight);

        state.behavior = Behavior::Flight;
        state.behavior_age_frames = FLIGHT_HOLD_FRAMES - 1;
        assert_eq!(modeled_behavior(&state, 0), Behavior::Flight);
        state.behavior_age_frames = FLIGHT_HOLD_FRAMES;
        assert_eq!(modeled_behavior(&state, 0), Behavior::Landing);

        state.behavior = Behavior::Landing;
        state.behavior_age_frames = LANDING_HOLD_FRAMES - 1;
        assert_eq!(modeled_behavior(&state, 0), Behavior::Landing);
        state.behavior_age_frames = LANDING_HOLD_FRAMES;
        assert_eq!(modeled_behavior(&state, 0), Behavior::Rest);

        assert_eq!((ESCAPE_HOLD_FRAMES + 1) * crate::MODEL_STEP_MS, 198);
        assert_eq!((FLIGHT_HOLD_FRAMES + 1) * crate::MODEL_STEP_MS, 3_993);
        assert_eq!((LANDING_HOLD_FRAMES + 1) * crate::MODEL_STEP_MS, 495);
    }

    #[test]
    fn authored_functional_populations_select_their_motor_programs() {
        let graph = Arc::new(ModelGraph::synthetic(ModelTier::Demo4096, 9));
        for (offset, expected) in [
            (GROOM_POPULATION_OFFSET, Behavior::Groom),
            (ALERT_POPULATION_OFFSET, Behavior::Alert),
            (REVERSE_POPULATION_OFFSET, Behavior::Reverse),
            (WALK_POPULATION_OFFSET, Behavior::Walk),
        ] {
            let mut engine = ModelEngine::new(Arc::clone(&graph), 11);
            let mut stimulus = engine.empty_stimulus();
            for value in stimulus
                .iter_mut()
                .skip(offset)
                .step_by(FUNCTIONAL_POPULATION_COUNT)
            {
                *value = 8_192;
            }
            let reached = (0..24).any(|_| {
                engine
                    .step_cpu(StepInput {
                        stimulus_q15: &stimulus,
                    })
                    .behavior
                    == expected
            });
            assert!(
                reached,
                "{expected:?} population never reached its motor program"
            );
        }
    }

    #[test]
    fn grooming_has_a_minimum_dwell_but_loom_still_preempts_it() {
        let graph = Arc::new(ModelGraph::synthetic(ModelTier::Demo4096, 12));
        let mut state = ModelEngine::new(graph, 11).state;
        state.behavior = Behavior::Groom;
        state.behavior_age_frames = GROOM_HOLD_FRAMES - 1;
        assert_eq!(modeled_behavior(&state, 0), Behavior::Groom);

        for value in state
            .activation
            .iter_mut()
            .skip(LOOM_POPULATION_OFFSET)
            .step_by(FUNCTIONAL_POPULATION_COUNT)
        {
            *value = LOOM_ESCAPE_ACTIVATION_Q15;
        }
        assert_eq!(modeled_behavior(&state, 0), Behavior::PreEscape);
        let recorded_dwell_frames = std::hint::black_box(GROOM_HOLD_FRAMES);
        assert!((recorded_dwell_frames + 1) * crate::MODEL_STEP_MS >= 1_500);
    }
}
