use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    MODEL_STEP_MS, MODEL_VERSION,
    model::{FrameSummary, ModelEngine, StepInput},
    replay::ModelCheckpoint,
};

#[derive(Clone, Copy, Debug)]
pub struct StimulationPolicy {
    pub max_unique_targets: usize,
    pub max_amplitude: f32,
    pub min_duration_ms: u32,
    pub max_duration_ms: u32,
    pub max_dosage_neuron_seconds: f32,
    pub max_comparison_frames: u32,
}

impl Default for StimulationPolicy {
    fn default() -> Self {
        Self {
            max_unique_targets: 64,
            max_amplitude: 0.25,
            min_duration_ms: MODEL_STEP_MS,
            max_duration_ms: 990,
            max_dosage_neuron_seconds: 4.0,
            max_comparison_frames: 120,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StimulationRequest {
    pub targets: Vec<usize>,
    pub amplitude: f32,
    pub duration_ms: u32,
    pub comparison_frames: u32,
    pub authored_label: String,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum StimulationValidationError {
    #[error("at least one target is required")]
    NoTargets,
    #[error("target count exceeds the limit of {0}")]
    TooManyTargets(usize),
    #[error("target {target} is outside the graph's {neuron_count} neurons")]
    TargetOutOfRange { target: usize, neuron_count: usize },
    #[error("targets must be unique")]
    DuplicateTarget,
    #[error("amplitude must be finite and in (0, {0}]")]
    InvalidAmplitude(String),
    #[error("duration must be {min_ms}–{max_ms} ms and a multiple of {step_ms} ms")]
    InvalidDuration {
        min_ms: u32,
        max_ms: u32,
        step_ms: u32,
    },
    #[error("comparison length must be 1–{0} frames")]
    InvalidComparisonFrames(u32),
    #[error("dosage exceeds the {0} neuron-second ceiling")]
    DosageExceeded(String),
    #[error("authored intervention label is required")]
    MissingAuthoredLabel,
    #[error("checkpoint graph, causal state, model identity or digest is invalid")]
    InvalidCheckpoint,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NeuronSample {
    pub index: u32,
    pub activation_q15: i16,
    pub spiked: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ComparisonFrame {
    pub offset: u32,
    pub actual: FrameSummary,
    pub alternative: FrameSummary,
    pub differing_neurons: usize,
    pub actual_sample: Vec<NeuronSample>,
    pub alternative_sample: Vec<NeuronSample>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComparisonReceipt {
    pub schema_version: u32,
    pub status: String,
    pub claim: String,
    pub model_version: String,
    pub graph_id: String,
    pub graph_sha256: String,
    pub source_frame: u64,
    pub live_before_sha256: String,
    pub live_after_sha256: String,
    pub actual_final_sha256: String,
    pub alternative_final_sha256: String,
    pub live_state_unchanged: bool,
    pub alternative_differs: bool,
    pub request: StimulationRequest,
    pub safety_limits: SafetyLimitsReceipt,
    pub live_hardware_authority: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SafetyLimitsReceipt {
    pub max_unique_targets: usize,
    pub max_amplitude: f32,
    pub min_duration_ms: u32,
    pub max_duration_ms: u32,
    pub max_dosage_neuron_seconds: f32,
    pub max_comparison_frames: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ComparisonResult {
    pub frames: Vec<ComparisonFrame>,
    pub receipt: ComparisonReceipt,
}

impl StimulationPolicy {
    pub fn validate(
        &self,
        request: &StimulationRequest,
        neuron_count: usize,
    ) -> Result<Vec<usize>, StimulationValidationError> {
        if request.targets.is_empty() {
            return Err(StimulationValidationError::NoTargets);
        }
        if request.targets.len() > self.max_unique_targets {
            return Err(StimulationValidationError::TooManyTargets(
                self.max_unique_targets,
            ));
        }
        let mut targets = request.targets.clone();
        targets.sort_unstable();
        if targets.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(StimulationValidationError::DuplicateTarget);
        }
        if let Some(&target) = targets.iter().find(|target| **target >= neuron_count) {
            return Err(StimulationValidationError::TargetOutOfRange {
                target,
                neuron_count,
            });
        }
        if !request.amplitude.is_finite()
            || request.amplitude <= 0.0
            || request.amplitude > self.max_amplitude
        {
            return Err(StimulationValidationError::InvalidAmplitude(format!(
                "{:.3}",
                self.max_amplitude
            )));
        }
        if request.duration_ms < self.min_duration_ms
            || request.duration_ms > self.max_duration_ms
            || !request.duration_ms.is_multiple_of(MODEL_STEP_MS)
        {
            return Err(StimulationValidationError::InvalidDuration {
                min_ms: self.min_duration_ms,
                max_ms: self.max_duration_ms,
                step_ms: MODEL_STEP_MS,
            });
        }
        if request.comparison_frames == 0 || request.comparison_frames > self.max_comparison_frames
        {
            return Err(StimulationValidationError::InvalidComparisonFrames(
                self.max_comparison_frames,
            ));
        }
        let dosage =
            targets.len() as f32 * request.amplitude * (request.duration_ms as f32 / 1_000.0);
        if dosage > self.max_dosage_neuron_seconds {
            return Err(StimulationValidationError::DosageExceeded(format!(
                "{:.3}",
                self.max_dosage_neuron_seconds
            )));
        }
        if request.authored_label.trim().is_empty() {
            return Err(StimulationValidationError::MissingAuthoredLabel);
        }
        Ok(targets)
    }

    pub fn compare_from_checkpoint(
        &self,
        checkpoint: &ModelCheckpoint,
        live_state_digest: impl Fn() -> String,
        request: StimulationRequest,
        graph: Arc<crate::ModelGraph>,
    ) -> Result<ComparisonResult, StimulationValidationError> {
        let targets = self.validate(&request, graph.neuron_ids.len())?;
        let live_before = live_state_digest();
        if checkpoint.graph != graph.identity
            || checkpoint.summary.state_digest != checkpoint.state.digest()
        {
            return Err(StimulationValidationError::InvalidCheckpoint);
        }
        let mut actual = ModelEngine::from_state(Arc::clone(&graph), checkpoint.state.clone())
            .map_err(|_| StimulationValidationError::InvalidCheckpoint)?;
        if actual.model_identity() != checkpoint.model_identity {
            return Err(StimulationValidationError::InvalidCheckpoint);
        }
        let mut alternative = ModelEngine::from_state(graph, checkpoint.state.clone())
            .map_err(|_| StimulationValidationError::InvalidCheckpoint)?;
        let zero = actual.empty_stimulus();
        let mut authored = alternative.empty_stimulus();
        let authored_q15 = (request.amplitude * 32_768.0).round() as i32;
        for &target in &targets {
            authored[target] = authored_q15;
        }
        let stimulation_frames = request.duration_ms / MODEL_STEP_MS;
        let mut frames = Vec::with_capacity(request.comparison_frames as usize);

        for offset in 0..request.comparison_frames {
            let actual_summary = actual.step_cpu(StepInput {
                stimulus_q15: &zero,
            });
            let applied = if offset < stimulation_frames {
                &authored
            } else {
                &zero
            };
            let alternative_summary = alternative.step_cpu(StepInput {
                stimulus_q15: applied,
            });
            let differing_neurons = actual
                .state
                .activation
                .iter()
                .zip(&alternative.state.activation)
                .filter(|(left, right)| left != right)
                .count();
            let actual_sample = sample_neurons(&actual.state.activation, &actual.state.spikes);
            let alternative_sample =
                sample_neurons(&alternative.state.activation, &alternative.state.spikes);
            frames.push(ComparisonFrame {
                offset,
                actual: actual_summary,
                alternative: alternative_summary,
                differing_neurons,
                actual_sample,
                alternative_sample,
            });
        }

        let live_after = live_state_digest();
        let actual_final = actual.state.digest();
        let alternative_final = alternative.state.digest();
        let receipt = ComparisonReceipt {
            schema_version: 1,
            status: "PASS".to_owned(),
            claim: "AUTHORED_INTERVENTION_ON_DISCARDED_MODELED_BRANCH".to_owned(),
            model_version: MODEL_VERSION.to_owned(),
            graph_id: checkpoint.graph.graph_id.clone(),
            graph_sha256: checkpoint.graph.sha256.clone(),
            source_frame: checkpoint.state.frame,
            live_before_sha256: live_before.clone(),
            live_after_sha256: live_after.clone(),
            actual_final_sha256: actual_final.clone(),
            alternative_final_sha256: alternative_final.clone(),
            live_state_unchanged: live_before == live_after,
            alternative_differs: actual_final != alternative_final,
            request,
            safety_limits: SafetyLimitsReceipt {
                max_unique_targets: self.max_unique_targets,
                max_amplitude: self.max_amplitude,
                min_duration_ms: self.min_duration_ms,
                max_duration_ms: self.max_duration_ms,
                max_dosage_neuron_seconds: self.max_dosage_neuron_seconds,
                max_comparison_frames: self.max_comparison_frames,
            },
            live_hardware_authority: "NONE".to_owned(),
        };
        Ok(ComparisonResult { frames, receipt })
    }
}

fn sample_neurons(activation: &[i32], spikes: &[u8]) -> Vec<NeuronSample> {
    const SAMPLE_LIMIT: usize = 192;
    let stride = activation.len().div_ceil(SAMPLE_LIMIT).max(1);
    (0..activation.len())
        .step_by(stride)
        .take(SAMPLE_LIMIT)
        .map(|index| NeuronSample {
            index: index as u32,
            activation_q15: activation[index].clamp(i16::MIN as i32, i16::MAX as i32) as i16,
            spiked: spikes[index] != 0,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{BoundedReplay, ModelGraph, ModelTier};

    #[test]
    fn preview_changes_only_the_cloned_branch() {
        let graph = Arc::new(ModelGraph::synthetic(ModelTier::Demo4096, 3));
        let mut live = ModelEngine::new(Arc::clone(&graph), 99);
        let zero = live.empty_stimulus();
        let summary = live.step_cpu(StepInput {
            stimulus_q15: &zero,
        });
        let checkpoint = ModelCheckpoint {
            graph: live.graph.identity.clone(),
            model_identity: live.model_identity(),
            state: live.state.clone(),
            summary,
        };
        let mut replay = BoundedReplay::default();
        replay.push(checkpoint.clone());
        let original = live.state.digest();
        let result = StimulationPolicy::default()
            .compare_from_checkpoint(
                &checkpoint,
                || live.state.digest(),
                StimulationRequest {
                    targets: vec![3, 7, 11],
                    amplitude: 0.2,
                    duration_ms: 330,
                    comparison_frames: 30,
                    authored_label: "test preview".to_owned(),
                },
                graph,
            )
            .unwrap();
        assert_eq!(live.state.digest(), original);
        assert!(result.receipt.live_state_unchanged);
        assert!(result.receipt.alternative_differs);
        assert!(
            result
                .frames
                .iter()
                .any(|frame| frame.differing_neurons > 0)
        );
    }
}
