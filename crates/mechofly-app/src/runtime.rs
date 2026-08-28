use std::{sync::Arc, time::Instant};

use mechofly_core::{
    Behavior, BoundedReplay, FrameSummary, ModelCheckpoint, ModelEngine, ModelGraph, ModelTier,
    model::{
        FUNCTIONAL_POPULATION_COUNT, GROOM_POPULATION_OFFSET, LOOM_POPULATION_OFFSET,
        REVERSE_POPULATION_OFFSET, WALK_POPULATION_OFFSET,
    },
    provenance::sha256_hex,
};

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

impl SimulationSession {
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
        mut assessment: CapacityAssessment,
        graph: Arc<ModelGraph>,
        seed: u64,
        started_unix_millis: u64,
    ) -> Self {
        let engine = ModelEngine::new(Arc::clone(&graph), seed);
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
    use super::apply_population_drive;
    use mechofly_core::model::{
        FUNCTIONAL_POPULATION_COUNT, GROOM_POPULATION_OFFSET, LOOM_POPULATION_OFFSET,
    };

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
}
