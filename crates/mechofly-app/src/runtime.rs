use std::{sync::Arc, time::Instant};

#[cfg(feature = "n41-visual-review-b")]
use mechofly_core::behavior_parameters::BehaviorParameterProfile;
use mechofly_core::{
    Action, Behavior, BoundedReplay, FrameSummary, ModelCheckpoint, ModelEngine, ModelGraph,
    ModelTier,
    model::{
        ALERT_POPULATION_OFFSET, FUNCTIONAL_POPULATION_COUNT, GROOM_POPULATION_OFFSET,
        LOOM_POPULATION_OFFSET, REVERSE_POPULATION_OFFSET, WALK_POPULATION_OFFSET,
    },
    provenance::sha256_hex,
};
#[cfg(feature = "n6-product-checkpoint")]
use mechofly_core::{BehaviorIntentSnapshot, GraphIdentity, ModelState, StepInput};
#[cfg(feature = "n6-product-checkpoint")]
use serde::{Deserialize, Serialize};

use crate::compute::{
    ActiveBackend, CapacityAssessment, ComputePreference, RuntimeBackend, assess_capacity,
    backend_for_graph,
};

pub struct SimulationSession {
    pub assessment: CapacityAssessment,
    pub graph: Arc<ModelGraph>,
    pub engine: ModelEngine,
    pub replay: BoundedReplay,
    pub backend: RuntimeBackend,
    pub last_summary: FrameSummary,
    pub session_id: String,
    pub started_unix_millis: u64,
    pub last_step_ms: f64,
    pub runtime_warning: Option<String>,
    sensory_stimulus: Vec<i32>,
    cursor_loom_q15: i32,
    authored_drive: Option<AuthoredDrive>,
}

#[derive(Clone, Copy, Debug)]
struct AuthoredDrive {
    population_offset: usize,
    expires_after_frame: u64,
}

#[cfg(feature = "n6-product-checkpoint")]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoredDriveCheckpoint {
    population_offset: usize,
    expires_after_frame: u64,
}

#[cfg(feature = "n6-product-checkpoint")]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SimulationRuntimeCheckpoint {
    pub(crate) schema_version: u32,
    pub(crate) graph: GraphIdentity,
    pub(crate) model_identity: String,
    pub(crate) state: ModelState,
    pub(crate) summary: FrameSummary,
    pub(crate) last_behavior_intent: BehaviorIntentSnapshot,
    sensory_stimulus: Vec<i32>,
    cursor_loom_q15: i32,
    authored_drive: Option<AuthoredDriveCheckpoint>,
    live_digest: String,
}

#[cfg(feature = "n6-product-checkpoint")]
impl SimulationRuntimeCheckpoint {
    pub(crate) fn validate(&self, graph: &ModelGraph) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err("unsupported simulation-runtime checkpoint schema".to_owned());
        }
        if self.graph != graph.identity {
            return Err("simulation-runtime checkpoint graph identity mismatch".to_owned());
        }
        if self.state.activation.len() != graph.neuron_ids.len()
            || self.state.spikes.len() != graph.neuron_ids.len()
            || self.sensory_stimulus.len() != graph.neuron_ids.len()
        {
            return Err("simulation-runtime checkpoint dimensions do not match graph".to_owned());
        }
        if !(0..=8_192).contains(&self.cursor_loom_q15) {
            return Err("simulation-runtime checkpoint cursor loom is out of range".to_owned());
        }
        if let Some(drive) = self.authored_drive
            && drive.population_offset >= FUNCTIONAL_POPULATION_COUNT
        {
            return Err("simulation-runtime checkpoint authored drive is invalid".to_owned());
        }
        let digest = self.state.digest();
        if digest != self.live_digest || self.summary.state_digest != digest {
            return Err("simulation-runtime checkpoint state digest mismatch".to_owned());
        }
        if self.summary.frame != self.state.frame
            || self.summary.behavior != self.state.behavior
            || self.last_behavior_intent.frame != self.state.frame
        {
            return Err("simulation-runtime checkpoint frame or behavior mismatch".to_owned());
        }
        let restored = ModelEngine::from_state(Arc::new(graph.clone()), self.state.clone())?;
        if restored.model_identity() != self.model_identity {
            return Err("simulation-runtime checkpoint model identity mismatch".to_owned());
        }
        Ok(())
    }

    pub(crate) fn digest(&self, graph: &ModelGraph) -> Result<String, String> {
        self.validate(graph)?;
        let encoded = serde_json::to_vec(self)
            .map_err(|error| format!("cannot encode simulation-runtime checkpoint: {error}"))?;
        Ok(sha256_hex([encoded]))
    }
}

#[cfg(feature = "n6-product-checkpoint")]
pub(crate) struct DiscardedSimulationBranch {
    graph: Arc<ModelGraph>,
    engine: ModelEngine,
    last_summary: FrameSummary,
    sensory_stimulus: Vec<i32>,
    cursor_loom_q15: i32,
    authored_drive: Option<AuthoredDrive>,
}

#[cfg(feature = "n6-product-checkpoint")]
impl DiscardedSimulationBranch {
    fn from_checkpoint(
        graph: Arc<ModelGraph>,
        checkpoint: &SimulationRuntimeCheckpoint,
    ) -> Result<Self, String> {
        checkpoint.validate(&graph)?;
        let mut engine = ModelEngine::from_state(Arc::clone(&graph), checkpoint.state.clone())?;
        engine.last_behavior_intent = checkpoint.last_behavior_intent;
        Ok(Self {
            graph,
            engine,
            last_summary: checkpoint.summary.clone(),
            sensory_stimulus: checkpoint.sensory_stimulus.clone(),
            cursor_loom_q15: checkpoint.cursor_loom_q15,
            authored_drive: checkpoint.authored_drive.map(|drive| AuthoredDrive {
                population_offset: drive.population_offset,
                expires_after_frame: drive.expires_after_frame,
            }),
        })
    }

    pub(crate) fn set_cursor_loom_q15(&mut self, value: i32) {
        self.cursor_loom_q15 = value.clamp(0, 8_192);
    }

    pub(crate) fn step(&mut self) -> FrameSummary {
        self.sensory_stimulus.fill(0);
        apply_population_drive(
            &mut self.sensory_stimulus,
            LOOM_POPULATION_OFFSET,
            self.cursor_loom_q15,
        );
        if self
            .authored_drive
            .is_some_and(|drive| self.engine.state.frame >= drive.expires_after_frame)
        {
            self.authored_drive = None;
        }
        if let Some(drive) = self.authored_drive {
            apply_population_drive(&mut self.sensory_stimulus, drive.population_offset, 8_192);
        }
        self.last_summary = self.engine.step_cpu(StepInput {
            stimulus_q15: &self.sensory_stimulus,
        });
        self.last_summary.clone()
    }

    pub(crate) fn checkpoint(&self) -> SimulationRuntimeCheckpoint {
        SimulationRuntimeCheckpoint {
            schema_version: 1,
            graph: self.graph.identity.clone(),
            model_identity: self.engine.model_identity(),
            state: self.engine.state.clone(),
            summary: self.last_summary.clone(),
            last_behavior_intent: self.engine.last_behavior_intent,
            sensory_stimulus: self.sensory_stimulus.clone(),
            cursor_loom_q15: self.cursor_loom_q15,
            authored_drive: self.authored_drive.map(|drive| AuthoredDriveCheckpoint {
                population_offset: drive.population_offset,
                expires_after_frame: drive.expires_after_frame,
            }),
            live_digest: self.engine.state.digest(),
        }
    }
}

impl SimulationSession {
    #[cfg(feature = "n6-product-checkpoint")]
    pub(crate) fn product_runtime_checkpoint(&self) -> SimulationRuntimeCheckpoint {
        SimulationRuntimeCheckpoint {
            schema_version: 1,
            graph: self.graph.identity.clone(),
            model_identity: self.engine.model_identity(),
            state: self.engine.state.clone(),
            summary: self.last_summary.clone(),
            last_behavior_intent: self.engine.last_behavior_intent,
            sensory_stimulus: self.sensory_stimulus.clone(),
            cursor_loom_q15: self.cursor_loom_q15,
            authored_drive: self.authored_drive.map(|drive| AuthoredDriveCheckpoint {
                population_offset: drive.population_offset,
                expires_after_frame: drive.expires_after_frame,
            }),
            live_digest: self.live_digest(),
        }
    }

    #[cfg(feature = "n6-product-checkpoint")]
    pub(crate) fn discarded_branch(
        &self,
        checkpoint: &SimulationRuntimeCheckpoint,
    ) -> Result<DiscardedSimulationBranch, String> {
        DiscardedSimulationBranch::from_checkpoint(Arc::clone(&self.graph), checkpoint)
    }

    pub fn calibrated(
        render_state: Option<&eframe::egui_wgpu::RenderState>,
        preference: ComputePreference,
        seed: u64,
        started_unix_millis: u64,
    ) -> Self {
        let assessment = assess_capacity(render_state, preference);
        let graph = Arc::new(ModelGraph::synthetic(assessment.tier, seed ^ 0x47A9_2D31));
        Self::with_graph(render_state, assessment, graph, seed, started_unix_millis)
    }

    #[cfg(feature = "n6-product-checkpoint")]
    pub(crate) fn product_checkpoint_fixture(seed: u64) -> Self {
        let graph = Arc::new(ModelGraph::synthetic(
            ModelTier::Demo4096,
            seed ^ 0x47A9_2D31,
        ));
        let assessment = CapacityAssessment {
            schema_version: 1,
            requested: ComputePreference::Cpu,
            selected: ActiveBackend::Cpu,
            tier: ModelTier::Demo4096,
            logical_cpu_count: 1,
            cpu_calibration_ms: 0.0,
            gpu_available: false,
            gpu_calibration_ms: None,
            gpu_adapter: None,
            gpu_backend: None,
            gpu_device_type: None,
            gpu_exact_match: None,
            reason: "fixed CPU-only N6 product-checkpoint fixture".to_owned(),
            started_new_session: true,
        };
        Self::with_graph(None, assessment, graph, seed, 0)
    }

    #[cfg(feature = "n41-visual-review-b")]
    pub fn calibrated_n41_visual_review_b(
        render_state: Option<&eframe::egui_wgpu::RenderState>,
        preference: ComputePreference,
        seed: u64,
        started_unix_millis: u64,
    ) -> Self {
        let assessment = assess_capacity(render_state, preference);
        let graph = Arc::new(ModelGraph::synthetic(assessment.tier, seed ^ 0x47A9_2D31));
        let engine = ModelEngine::new_duration_aware_with_profile(
            Arc::clone(&graph),
            seed,
            BehaviorParameterProfile::N41BNaturalFlight,
        );
        Self::with_graph_and_engine(render_state, assessment, graph, engine, started_unix_millis)
    }

    pub fn with_imported_graph(
        render_state: Option<&eframe::egui_wgpu::RenderState>,
        preference: ComputePreference,
        graph: Arc<ModelGraph>,
        seed: u64,
        started_unix_millis: u64,
    ) -> Self {
        let mut assessment = assess_capacity(render_state, preference);
        assessment.tier = ModelTier::FafbV783Full;
        assessment.reason.push_str(
            "; imported full graph was explicitly pinned and bypassed automatic tier sizing",
        );
        Self::with_graph(render_state, assessment, graph, seed, started_unix_millis)
    }

    fn with_graph(
        render_state: Option<&eframe::egui_wgpu::RenderState>,
        assessment: CapacityAssessment,
        graph: Arc<ModelGraph>,
        seed: u64,
        started_unix_millis: u64,
    ) -> Self {
        let engine = ModelEngine::new_duration_aware(Arc::clone(&graph), seed);
        Self::with_graph_and_engine(render_state, assessment, graph, engine, started_unix_millis)
    }

    fn with_graph_and_engine(
        render_state: Option<&eframe::egui_wgpu::RenderState>,
        mut assessment: CapacityAssessment,
        graph: Arc<ModelGraph>,
        engine: ModelEngine,
        started_unix_millis: u64,
    ) -> Self {
        let last_summary = FrameSummary {
            frame: 0,
            spike_count: 0,
            mean_activation_q15: 0,
            behavior: engine.state.behavior,
            state_digest: engine.state.digest(),
        };
        let backend = backend_for_graph(render_state, &mut assessment, Arc::clone(&graph));
        let session_id = sha256_hex([
            graph.identity.sha256.as_bytes(),
            engine.model_identity().as_bytes(),
            started_unix_millis.to_le_bytes().as_slice(),
            assessment.selected.label().as_bytes(),
        ]);
        let sensory_stimulus = engine.empty_stimulus();
        Self {
            assessment,
            graph,
            engine,
            replay: BoundedReplay::default(),
            backend,
            last_summary,
            session_id,
            started_unix_millis,
            last_step_ms: 0.0,
            runtime_warning: None,
            sensory_stimulus,
            cursor_loom_q15: 0,
            authored_drive: None,
        }
    }

    pub fn set_cursor_loom_strength(&mut self, strength: f32) {
        self.cursor_loom_q15 = (strength.clamp(0.0, 1.0) * 8_192.0).round() as i32;
    }

    pub fn stimulate_behavior(&mut self, behavior: Behavior, duration_ms: u32) -> bool {
        let population_offset = match behavior {
            Behavior::PreEscape => LOOM_POPULATION_OFFSET,
            Behavior::Groom => GROOM_POPULATION_OFFSET,
            Behavior::Alert => ALERT_POPULATION_OFFSET,
            Behavior::Reverse => REVERSE_POPULATION_OFFSET,
            Behavior::Walk => WALK_POPULATION_OFFSET,
            _ => return false,
        };
        let frames = duration_ms.div_ceil(mechofly_core::MODEL_STEP_MS).max(1);
        self.authored_drive = Some(AuthoredDrive {
            population_offset,
            expires_after_frame: self.engine.state.frame.saturating_add(u64::from(frames)),
        });
        true
    }

    pub fn stimulate_action(&mut self, action: Action) -> bool {
        let Some((behavior, duration_ms)) = neural_drive_for_action(action) else {
            self.authored_drive = None;
            return true;
        };
        self.stimulate_behavior(behavior, duration_ms)
    }

    fn prepare_sensory_stimulus(&mut self) {
        self.sensory_stimulus.fill(0);
        apply_population_drive(
            &mut self.sensory_stimulus,
            LOOM_POPULATION_OFFSET,
            self.cursor_loom_q15,
        );
        if self
            .authored_drive
            .is_some_and(|drive| self.engine.state.frame >= drive.expires_after_frame)
        {
            self.authored_drive = None;
        }
        if let Some(drive) = self.authored_drive {
            apply_population_drive(&mut self.sensory_stimulus, drive.population_offset, 8_192);
        }
    }

    pub fn step(&mut self) {
        self.prepare_sensory_stimulus();
        let started = Instant::now();
        let result = self.backend.step(&mut self.engine, &self.sensory_stimulus);
        self.last_summary = match result {
            Ok(summary) => summary,
            Err(error) => {
                self.runtime_warning = Some(format!(
                    "GPU runtime step failed; the session continued on CPU: {error}"
                ));
                self.backend = RuntimeBackend::Cpu;
                self.assessment.selected = ActiveBackend::Cpu;
                self.assessment.reason = self.runtime_warning.clone().unwrap_or_default();
                self.backend
                    .step(&mut self.engine, &self.sensory_stimulus)
                    .expect("CPU backend is infallible")
            }
        };
        self.last_step_ms = started.elapsed().as_secs_f64() * 1_000.0;
        self.replay.push(ModelCheckpoint {
            graph: self.graph.identity.clone(),
            model_identity: self.engine.model_identity(),
            state: self.engine.state.clone(),
            summary: self.last_summary.clone(),
        });
    }

    pub fn live_digest(&self) -> String {
        self.engine.state.digest()
    }

    pub fn short_session_id(&self) -> &str {
        &self.session_id[..12]
    }
}

pub(crate) const fn neural_drive_for_action(action: Action) -> Option<(Behavior, u32)> {
    match action {
        Action::Pause => None,
        Action::Explore => Some((Behavior::Walk, 594)),
        Action::Inspect => Some((Behavior::Alert, 330)),
        Action::Groom => Some((Behavior::Groom, 594)),
    }
}

fn apply_population_drive(stimulus: &mut [i32], offset: usize, drive_q15: i32) {
    for value in stimulus
        .iter_mut()
        .skip(offset)
        .step_by(FUNCTIONAL_POPULATION_COUNT)
    {
        *value = (*value).max(drive_q15);
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_population_drive, neural_drive_for_action};
    use mechofly_core::model::{
        FUNCTIONAL_POPULATION_COUNT, GROOM_POPULATION_OFFSET, LOOM_POPULATION_OFFSET,
    };
    use mechofly_core::{Action, Behavior};

    #[test]
    fn independent_sensory_drives_are_combined_without_erasing_each_other() {
        let mut stimulus = vec![0; FUNCTIONAL_POPULATION_COUNT * 4];
        apply_population_drive(&mut stimulus, LOOM_POPULATION_OFFSET, 4_096);
        apply_population_drive(&mut stimulus, GROOM_POPULATION_OFFSET, 8_192);
        for (index, value) in stimulus.iter().enumerate() {
            let expected = match index % FUNCTIONAL_POPULATION_COUNT {
                LOOM_POPULATION_OFFSET => 4_096,
                GROOM_POPULATION_OFFSET => 8_192,
                _ => 0,
            };
            assert_eq!(*value, expected);
        }
    }

    #[test]
    fn autonomous_policy_actions_enter_through_neural_population_drives() {
        assert_eq!(neural_drive_for_action(Action::Pause), None);
        assert_eq!(
            neural_drive_for_action(Action::Explore),
            Some((Behavior::Walk, 594))
        );
        assert_eq!(
            neural_drive_for_action(Action::Inspect),
            Some((Behavior::Alert, 330))
        );
        assert_eq!(
            neural_drive_for_action(Action::Groom),
            Some((Behavior::Groom, 594))
        );
    }
}
