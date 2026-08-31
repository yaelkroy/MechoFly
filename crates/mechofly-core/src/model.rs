use std::sync::Arc;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    MODEL_VERSION,
    behavior_dynamics::BehaviorDynamicsState,
    behavior_intent::{BehaviorContext, BehaviorIntentBuilder, BehaviorIntentSnapshot},
    behavior_parameters::{DYNAMICS_CLAIM, DYNAMICS_VERSION},
    behavior_selection::LegacyBehaviorSelector,
    behavior_telemetry::{
        BehaviorTelemetryLedger, BehaviorTelemetrySnapshot, BehaviorTransitionEvent,
    },
    graph::{ModelGraph, mix64},
    neural_evidence::NeuralEvidence,
    provenance::sha256_hex,
};

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
    /// None is an explicit legacy/N3 checkpoint, never a partially restored N4 state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub behavior_dynamics: Option<BehaviorDynamicsState>,
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
        let legacy = sha256_hex([
            frame.as_slice(),
            seed.as_slice(),
            &behavior,
            age.as_slice(),
            activation.as_slice(),
            self.spikes.as_slice(),
        ]);
        match &self.behavior_dynamics {
            None => legacy,
            Some(dynamics) => {
                let encoded = serde_json::to_vec(dynamics).expect("N4 integer state serialization");
                sha256_hex([
                    b"mechofly-full-state-n4-v1".as_slice(),
                    legacy.as_bytes(),
                    encoded.as_slice(),
                ])
            }
        }
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
    behavior_telemetry: BehaviorTelemetryLedger,
    behavior_telemetry_enabled: bool,
    pub last_behavior_intent: BehaviorIntentSnapshot,
}

impl ModelEngine {
    pub fn new(graph: Arc<ModelGraph>, seed: u64) -> Self {
        let count = graph.neuron_ids.len();
        let activation = (0..count)
            .map(|index| initial_activation(seed, index))
            .collect();
        let state = ModelState {
            frame: 0,
            seed,
            activation,
            spikes: vec![0; count],
            behavior: Behavior::Rest,
            behavior_age_frames: 0,
            behavior_dynamics: None,
        };
        let evidence = NeuralEvidence::collect(state.frame, &state.activation, 0);
        let last_behavior_intent = BehaviorIntentBuilder::build(
            &evidence,
            BehaviorContext {
                current_behavior: state.behavior,
                current_behavior_age_frames: state.behavior_age_frames,
            },
        );
        Self {
            graph,
            state,
            behavior_telemetry: BehaviorTelemetryLedger::new(0),
            behavior_telemetry_enabled: true,
            last_behavior_intent,
        }
    }

    /// Active application constructor. `new` remains the explicit N3 compatibility harness.
    pub fn new_duration_aware(graph: Arc<ModelGraph>, seed: u64) -> Self {
        let mut engine = Self::new(graph, seed);
        engine.state.behavior = Behavior::Quiet;
        let evidence = NeuralEvidence::collect(0, &engine.state.activation, 0);
        let intent = BehaviorIntentBuilder::build_duration_aware(
            &evidence,
            BehaviorContext {
                current_behavior: Behavior::Quiet,
                current_behavior_age_frames: 0,
            },
        );
        engine.state.behavior_dynamics = Some(BehaviorDynamicsState::new(seed, intent));
        engine.last_behavior_intent = intent;
        engine
    }

    pub fn from_state(graph: Arc<ModelGraph>, state: ModelState) -> Result<Self, String> {
        if state.activation.len() != graph.neuron_ids.len()
            || state.spikes.len() != graph.neuron_ids.len()
        {
            return Err("checkpoint dimensions do not match graph".to_owned());
        }
        if let Some(dynamics) = &state.behavior_dynamics {
            dynamics.validate(
                state.seed,
                state.frame,
                state.behavior,
                state.behavior_age_frames,
            )?;
        }
        let complete_from_frame = state.frame;
        let spike_count = state.spikes.iter().map(|value| *value as usize).sum();
        let evidence = NeuralEvidence::collect(state.frame, &state.activation, spike_count);
        let builder = if state.behavior_dynamics.is_some() {
            BehaviorIntentBuilder::build_duration_aware
        } else {
            BehaviorIntentBuilder::build
        };
        let last_behavior_intent = builder(
            &evidence,
            BehaviorContext {
                current_behavior: state.behavior,
                current_behavior_age_frames: state.behavior_age_frames,
            },
        );
        Ok(Self {
            graph,
            state,
            behavior_telemetry: BehaviorTelemetryLedger::new(complete_from_frame),
            behavior_telemetry_enabled: true,
            last_behavior_intent,
        })
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
        let evidence =
            NeuralEvidence::collect(self.state.frame, &self.state.activation, spike_count);
        let mean_activation_q15 = evidence.mean_activation_q15;
        let builder = if self.state.behavior_dynamics.is_some() {
            BehaviorIntentBuilder::build_duration_aware
        } else {
            BehaviorIntentBuilder::build
        };
        let intent = builder(
            &evidence,
            BehaviorContext {
                current_behavior: self.state.behavior,
                current_behavior_age_frames: self.state.behavior_age_frames,
            },
        );
        let mut next_dynamics = self.state.behavior_dynamics.clone();
        let (next_behavior, transition_reason, dynamics_transition) = match &mut next_dynamics {
            Some(dynamics) => {
                let decision = dynamics.advance(self.state.seed, intent);
                (decision.behavior, decision.reason, decision.transition)
            }
            None => {
                let decision = LegacyBehaviorSelector::select(&intent);
                (decision.behavior, decision.transition_reason, None)
            }
        };
        self.last_behavior_intent = intent;
        if next_behavior == self.state.behavior && transition_reason.is_none() {
            self.state.behavior_age_frames += 1;
            self.state.behavior_dynamics = next_dynamics;
        } else {
            let from_behavior = self.state.behavior;
            let elapsed_frames = self.state.behavior_age_frames.saturating_add(1);
            let pre_transition_state_digest = self.state.digest();
            let reason = transition_reason.expect("a transition must carry its reason");
            self.state.behavior = next_behavior;
            self.state.behavior_age_frames = 0;
            self.state.behavior_dynamics = next_dynamics;
            if self.behavior_telemetry_enabled {
                let post_transition_state_digest = self.state.digest();
                let mut event = BehaviorTransitionEvent::new(
                    self.state.frame,
                    from_behavior,
                    next_behavior,
                    elapsed_frames,
                    reason,
                    intent,
                    pre_transition_state_digest,
                    post_transition_state_digest,
                );
                if dynamics_transition.is_some() {
                    event.schema_version = 2;
                    event.dynamics = dynamics_transition;
                }
                self.behavior_telemetry.record(event);
            }
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

    pub fn set_behavior_telemetry_enabled(&mut self, enabled: bool) {
        self.behavior_telemetry_enabled = enabled;
    }

    pub const fn behavior_telemetry_enabled(&self) -> bool {
        self.behavior_telemetry_enabled
    }

    pub fn behavior_transition_events(
        &self,
    ) -> impl DoubleEndedIterator<Item = &BehaviorTransitionEvent> {
        self.behavior_telemetry.events()
    }

    pub fn latest_behavior_transition(&self) -> Option<&BehaviorTransitionEvent> {
        self.behavior_telemetry.latest()
    }

    pub fn behavior_telemetry_snapshot(&self) -> BehaviorTelemetrySnapshot {
        let mut snapshot = self.behavior_telemetry.snapshot(self.last_behavior_intent);
        if self.state.behavior_dynamics.is_some() {
            snapshot.schema_version = 2;
            snapshot.controller = DYNAMICS_VERSION.to_owned();
            snapshot.claim_boundary = DYNAMICS_CLAIM.to_owned();
            snapshot.controller_semantics_changed = true;
        }
        snapshot
    }

    pub const fn behavior_telemetry_total_event_count(&self) -> u64 {
        self.behavior_telemetry.total_event_count()
    }

    pub fn behavior_telemetry_stream_sha256(&self) -> &str {
        self.behavior_telemetry.event_stream_sha256()
    }

    pub fn model_identity(&self) -> String {
        let legacy = sha256_hex([
            MODEL_VERSION.as_bytes(),
            self.graph.identity.sha256.as_bytes(),
            self.state.seed.to_le_bytes().as_slice(),
        ]);
        match &self.state.behavior_dynamics {
            None => legacy,
            Some(dynamics) => sha256_hex([
                legacy.as_bytes(),
                DYNAMICS_VERSION.as_bytes(),
                dynamics.parameter_sha256.as_bytes(),
            ]),
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{behavior_intent::LOOM_ESCAPE_ACTIVATION_Q15, graph::ModelTier};

    fn select_state(state: &ModelState, spike_count: usize) -> Behavior {
        let evidence = NeuralEvidence::collect(state.frame, &state.activation, spike_count);
        let intent = BehaviorIntentBuilder::build(
            &evidence,
            BehaviorContext {
                current_behavior: state.behavior,
                current_behavior_age_frames: state.behavior_age_frames,
            },
        );
        LegacyBehaviorSelector::select(&intent).behavior
    }

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
        assert_eq!(select_state(&state, 0), Behavior::PreEscape);
        state.behavior_age_frames = ESCAPE_HOLD_FRAMES;
        assert_eq!(select_state(&state, 0), Behavior::Flight);

        state.behavior = Behavior::Flight;
        state.behavior_age_frames = FLIGHT_HOLD_FRAMES - 1;
        assert_eq!(select_state(&state, 0), Behavior::Flight);
        state.behavior_age_frames = FLIGHT_HOLD_FRAMES;
        assert_eq!(select_state(&state, 0), Behavior::Landing);

        state.behavior = Behavior::Landing;
        state.behavior_age_frames = LANDING_HOLD_FRAMES - 1;
        assert_eq!(select_state(&state, 0), Behavior::Landing);
        state.behavior_age_frames = LANDING_HOLD_FRAMES;
        assert_eq!(select_state(&state, 0), Behavior::Rest);

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
    fn transition_telemetry_is_observational_and_deterministic() {
        let graph = Arc::new(ModelGraph::synthetic(ModelTier::Demo4096, 14));
        let mut first = ModelEngine::new(Arc::clone(&graph), 91);
        let mut second = ModelEngine::new(Arc::clone(&graph), 91);
        let mut disabled = ModelEngine::new(graph, 91);
        disabled.set_behavior_telemetry_enabled(false);

        for frame in 0..720_u64 {
            let mut stimulus = first.empty_stimulus();
            let phase = (frame / 90) % 8;
            let offset = match phase {
                1 => Some(WALK_POPULATION_OFFSET),
                2 => Some(GROOM_POPULATION_OFFSET),
                3 => Some(ALERT_POPULATION_OFFSET),
                4 => Some(REVERSE_POPULATION_OFFSET),
                5 if frame % 90 < 12 => Some(LOOM_POPULATION_OFFSET),
                _ => None,
            };
            if let Some(offset) = offset {
                for value in stimulus
                    .iter_mut()
                    .skip(offset)
                    .step_by(FUNCTIONAL_POPULATION_COUNT)
                {
                    *value = 8_192;
                }
            }

            let first_summary = first.step_cpu(StepInput {
                stimulus_q15: &stimulus,
            });
            let second_summary = second.step_cpu(StepInput {
                stimulus_q15: &stimulus,
            });
            let disabled_summary = disabled.step_cpu(StepInput {
                stimulus_q15: &stimulus,
            });
            assert_eq!(first_summary, second_summary);
            assert_eq!(first_summary, disabled_summary);
            assert_eq!(first.state, second.state);
            assert_eq!(first.state, disabled.state);
        }

        let first_snapshot = first.behavior_telemetry_snapshot();
        let second_snapshot = second.behavior_telemetry_snapshot();
        let disabled_snapshot = disabled.behavior_telemetry_snapshot();
        assert_eq!(first_snapshot, second_snapshot);
        assert!(first_snapshot.observational_only);
        assert!(!first_snapshot.controller_semantics_changed);
        assert!(first_snapshot.total_event_count > 0);
        assert!(first_snapshot.retained_sequence_contiguous);
        assert_eq!(disabled_snapshot.total_event_count, 0);
        assert!(!disabled.behavior_telemetry_enabled());
    }

    #[test]
    fn grooming_has_a_minimum_dwell_but_loom_still_preempts_it() {
        let graph = Arc::new(ModelGraph::synthetic(ModelTier::Demo4096, 12));
        let mut state = ModelEngine::new(graph, 11).state;
        state.behavior = Behavior::Groom;
        state.behavior_age_frames = GROOM_HOLD_FRAMES - 1;
        assert_eq!(select_state(&state, 0), Behavior::Groom);

        for value in state
            .activation
            .iter_mut()
            .skip(LOOM_POPULATION_OFFSET)
            .step_by(FUNCTIONAL_POPULATION_COUNT)
        {
            *value = LOOM_ESCAPE_ACTIVATION_Q15;
        }
        assert_eq!(select_state(&state, 0), Behavior::PreEscape);
        let recorded_dwell_frames = std::hint::black_box(GROOM_HOLD_FRAMES);
        assert!((recorded_dwell_frames + 1) * crate::MODEL_STEP_MS >= 1_500);
    }
}
