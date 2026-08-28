use std::{sync::Arc, time::Instant};

use mechofly_core::{
    BoundedReplay, FrameSummary, ModelCheckpoint, ModelEngine, ModelGraph, ModelTier,
    model::{FUNCTIONAL_POPULATION_COUNT, LOOM_POPULATION_OFFSET},
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
        }
    }

    pub fn set_cursor_loom_strength(&mut self, strength: f32) {
        self.sensory_stimulus.fill(0);
        let drive_q15 = (strength.clamp(0.0, 1.0) * 8_192.0).round() as i32;
        for value in self
            .sensory_stimulus
            .iter_mut()
            .skip(LOOM_POPULATION_OFFSET)
            .step_by(FUNCTIONAL_POPULATION_COUNT)
        {
            *value = drive_q15;
        }
    }

    pub fn step(&mut self) {
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
