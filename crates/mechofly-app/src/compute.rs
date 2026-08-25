use std::{sync::mpsc, sync::Arc, time::Instant};

use bytemuck::{Pod, Zeroable};
use mechofly_core::{
    FrameSummary, ModelEngine, ModelGraph, ModelTier, StepInput,
};
use serde::{Deserialize, Serialize};
use wgpu::util::DeviceExt;

const CALIBRATION_STEPS: usize = 6;
const WORKGROUP_SIZE: u32 = 64;
const STEP_BUDGET_MS: f64 = 22.0;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ComputePreference {
    #[default]
    Auto,
    Cpu,
    Gpu,
}

impl ComputePreference {
    pub const ALL: [Self; 3] = [Self::Auto, Self::Cpu, Self::Gpu];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "Automatic",
            Self::Cpu => "CPU",
            Self::Gpu => "GPU",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActiveBackend {
    Cpu,
    Gpu,
}

impl ActiveBackend {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Cpu => "CPU",
            Self::Gpu => "GPU / WGSL",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapacityAssessment {
    pub schema_version: u32,
    pub requested: ComputePreference,
    pub selected: ActiveBackend,
    pub tier: ModelTier,
    pub logical_cpu_count: usize,
    pub cpu_calibration_ms: f64,
    pub gpu_available: bool,
    pub gpu_calibration_ms: Option<f64>,
    pub gpu_adapter: Option<String>,
    pub gpu_backend: Option<String>,
    pub gpu_device_type: Option<String>,
    pub gpu_exact_match: Option<bool>,
    pub reason: String,
    pub started_new_session: bool,
}

impl CapacityAssessment {
    pub fn short_status(&self) -> String {
        format!(
            "{} · {} · CPU {:.2} ms{}",
            self.selected.label(),
            self.tier.label(),
            self.cpu_calibration_ms,
            self.gpu_calibration_ms
                .map(|value| format!(" · GPU {value:.2} ms"))
                .unwrap_or_default()
        )
    }
}

pub enum RuntimeBackend {
    Cpu,
    Gpu(Box<GpuStepper>),
}

impl RuntimeBackend {
    pub fn step(
        &mut self,
        engine: &mut ModelEngine,
        stimulus: &[i32],
    ) -> Result<FrameSummary, String> {
        match self {
            Self::Cpu => Ok(engine.step_cpu(StepInput {
                    stimulus_q15: stimulus,
                })),
            Self::Gpu(gpu) => gpu.step(engine, stimulus),
        }
    }
}

pub fn assess_capacity(
    render_state: Option<&eframe::egui_wgpu::RenderState>,
    preference: ComputePreference,
) -> CapacityAssessment {
    let logical_cpu_count = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1);
    let graph = Arc::new(ModelGraph::synthetic(ModelTier::Demo4096, 0xA110_0C0A));
    let seed = 0x5EED_CAFE_D15C_A11B;
    let cpu_calibration_ms = benchmark_cpu(Arc::clone(&graph), seed);
    let mut gpu_available = false;
    let mut gpu_calibration_ms = None;
    let mut gpu_adapter = None;
    let mut gpu_backend = None;
    let mut gpu_device_type = None;
    let mut gpu_exact_match = None;
    let mut gpu_reason = String::new();

    if let Some(render_state) = render_state {
        let info = render_state.adapter.get_info();
        gpu_adapter = Some(info.name.clone());
        gpu_backend = Some(format!("{:?}", info.backend));
        gpu_device_type = Some(format!("{:?}", info.device_type));
        match benchmark_gpu(render_state, graph, seed) {
            Ok(result) => {
                gpu_exact_match = Some(result.exact_match);
                gpu_calibration_ms = Some(result.milliseconds);
                gpu_available = result.exact_match;
                if !result.exact_match {
                    gpu_reason = "GPU kernel did not exactly match the CPU fixture".to_owned();
                }
            }
            Err(error) => gpu_reason = error,
        }
    } else {
        gpu_reason = "the rendering backend did not expose a GPU adapter".to_owned();
    }

    let selected = match preference {
        ComputePreference::Cpu => ActiveBackend::Cpu,
        ComputePreference::Gpu if gpu_available => ActiveBackend::Gpu,
        ComputePreference::Gpu => ActiveBackend::Cpu,
        ComputePreference::Auto => match gpu_calibration_ms {
            Some(gpu_ms) if gpu_available && gpu_ms < cpu_calibration_ms * 0.92 => {
                ActiveBackend::Gpu
            }
            _ => ActiveBackend::Cpu,
        },
    };
    let selected_ms = match selected {
        ActiveBackend::Cpu => cpu_calibration_ms,
        ActiveBackend::Gpu => gpu_calibration_ms.unwrap_or(cpu_calibration_ms),
    };
    let tier = tier_for_calibration(selected_ms, logical_cpu_count);
    let reason = match (preference, selected) {
        (ComputePreference::Gpu, ActiveBackend::Cpu) => {
            format!("GPU was requested but unavailable; using CPU because {gpu_reason}")
        }
        (ComputePreference::Auto, ActiveBackend::Gpu) => {
            "automatic calibration selected the exact-matching WGSL backend".to_owned()
        }
        (ComputePreference::Auto, ActiveBackend::Cpu) if !gpu_available => {
            format!("automatic calibration selected CPU; {gpu_reason}")
        }
        (ComputePreference::Auto, ActiveBackend::Cpu) => {
            "automatic calibration measured CPU as the better fit".to_owned()
        }
        (ComputePreference::Cpu, _) => "CPU was explicitly selected".to_owned(),
        (ComputePreference::Gpu, ActiveBackend::Gpu) => {
            "GPU was explicitly selected and passed exactness calibration".to_owned()
        }
    };

    CapacityAssessment {
        schema_version: 1,
        requested: preference,
        selected,
        tier,
        logical_cpu_count,
        cpu_calibration_ms,
        gpu_available,
        gpu_calibration_ms,
        gpu_adapter,
        gpu_backend,
        gpu_device_type,
        gpu_exact_match,
        reason,
        started_new_session: true,
    }
}

pub fn backend_for_graph(
    render_state: Option<&eframe::egui_wgpu::RenderState>,
    assessment: &mut CapacityAssessment,
    graph: Arc<ModelGraph>,
) -> RuntimeBackend {
    if assessment.selected == ActiveBackend::Gpu {
        if let Some(render_state) = render_state {
            match GpuStepper::new(render_state, graph) {
                Ok(gpu) => return RuntimeBackend::Gpu(Box::new(gpu)),
                Err(error) => {
                    assessment.selected = ActiveBackend::Cpu;
                    assessment.reason = format!(
                        "GPU passed calibration but session initialization failed; using CPU: {error}"
                    );
                }
            }
        }
    }
    RuntimeBackend::Cpu
}

fn benchmark_cpu(graph: Arc<ModelGraph>, seed: u64) -> f64 {
    let mut engine = ModelEngine::new(graph, seed);
    let stimulus = engine.empty_stimulus();
    engine.step_cpu(StepInput {
        stimulus_q15: &stimulus,
    });
    let start = Instant::now();
    for _ in 0..CALIBRATION_STEPS {
        engine.step_cpu(StepInput {
            stimulus_q15: &stimulus,
        });
    }
    (start.elapsed().as_secs_f64() * 1_000.0 / CALIBRATION_STEPS as f64).max(0.001)
}

fn tier_for_calibration(demo_ms: f64, logical_cpu_count: usize) -> ModelTier {
    let extended_projection = demo_ms * (65_536.0 / 4_096.0);
    let standard_projection = demo_ms * (12_615.0 / 4_096.0);
    if extended_projection <= STEP_BUDGET_MS && logical_cpu_count >= 4 {
        ModelTier::Extended65536
    } else if standard_projection <= STEP_BUDGET_MS {
        ModelTier::Standard12615
    } else {
        ModelTier::Demo4096
    }
}

struct GpuBenchmark {
    milliseconds: f64,
    exact_match: bool,
}

fn benchmark_gpu(
    render_state: &eframe::egui_wgpu::RenderState,
    graph: Arc<ModelGraph>,
    seed: u64,
) -> Result<GpuBenchmark, String> {
    let capabilities = render_state.adapter.get_downlevel_capabilities();
    if !capabilities
        .flags
        .contains(wgpu::DownlevelFlags::COMPUTE_SHADERS)
    {
        return Err("adapter does not expose compute shaders".to_owned());
    }
    let mut cpu = ModelEngine::new(Arc::clone(&graph), seed);
    let mut gpu_engine = cpu.clone();
    let stimulus = cpu.empty_stimulus();
    let mut gpu = GpuStepper::new(render_state, graph)?;
    gpu.step(&mut gpu_engine, &stimulus)?;
    cpu.step_cpu(StepInput {
        stimulus_q15: &stimulus,
    });
    if cpu.state != gpu_engine.state {
        return Ok(GpuBenchmark {
            milliseconds: f64::INFINITY,
            exact_match: false,
        });
    }
    let start = Instant::now();
    for _ in 0..CALIBRATION_STEPS {
        gpu.step(&mut gpu_engine, &stimulus)?;
        cpu.step_cpu(StepInput {
            stimulus_q15: &stimulus,
        });
    }
    let exact_match = cpu.state == gpu_engine.state;
    Ok(GpuBenchmark {
        milliseconds: start.elapsed().as_secs_f64() * 1_000.0 / CALIBRATION_STEPS as f64,
        exact_match,
    })
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuParams {
    neuron_count: u32,
    frame_low: u32,
    frame_high: u32,
    seed_folded: u32,
}

pub struct GpuStepper {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    bind_group: wgpu::BindGroup,
    params: wgpu::Buffer,
    state: wgpu::Buffer,
    stimulus: wgpu::Buffer,
    output: wgpu::Buffer,
    readback: wgpu::Buffer,
    neuron_count: usize,
}

impl GpuStepper {
    fn new(
        render_state: &eframe::egui_wgpu::RenderState,
        graph: Arc<ModelGraph>,
    ) -> Result<Self, String> {
        graph.validate()?;
        let device = render_state.device.clone();
        let queue = render_state.queue.clone();
        let neuron_count = graph.neuron_ids.len();
        let state_bytes = (neuron_count * std::mem::size_of::<i32>()) as u64;
        let result_bytes = (neuron_count * 8) as u64;
        let limits = render_state.adapter.limits();
        let largest_storage = [
            state_bytes,
            result_bytes,
            (graph.incoming_sources.len() * 4) as u64,
        ]
        .into_iter()
        .max()
        .unwrap_or_default();
        if largest_storage > limits.max_storage_buffer_binding_size as u64 {
            return Err(format!(
                "graph needs a {largest_storage}-byte storage binding; adapter limit is {}",
                limits.max_storage_buffer_binding_size
            ));
        }

        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MechoFly compute parameters"),
            size: std::mem::size_of::<GpuParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let offsets = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("MechoFly incoming offsets"),
            contents: bytemuck::cast_slice(&graph.incoming_offsets),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let sources = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("MechoFly incoming sources"),
            contents: bytemuck::cast_slice(&graph.incoming_sources),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let weights = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("MechoFly modeled weights"),
            contents: bytemuck::cast_slice(&graph.modeled_weights),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let state = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MechoFly current activation"),
            size: state_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let stimulus = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MechoFly authored stimulus"),
            size: state_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let output = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MechoFly computed activation and spikes"),
            size: result_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MechoFly compute readback"),
            size: result_bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("MechoFly compute layout"),
            entries: &[
                binding(0, wgpu::BufferBindingType::Uniform, false),
                binding(1, wgpu::BufferBindingType::Storage { read_only: true }, false),
                binding(2, wgpu::BufferBindingType::Storage { read_only: true }, false),
                binding(3, wgpu::BufferBindingType::Storage { read_only: true }, false),
                binding(4, wgpu::BufferBindingType::Storage { read_only: true }, false),
                binding(5, wgpu::BufferBindingType::Storage { read_only: true }, false),
                binding(6, wgpu::BufferBindingType::Storage { read_only: false }, false),
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MechoFly compute bindings"),
            layout: &layout,
            entries: &[
                entry(0, &params),
                entry(1, &offsets),
                entry(2, &sources),
                entry(3, &weights),
                entry(4, &state),
                entry(5, &stimulus),
                entry(6, &output),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("MechoFly compute pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/neural_step.wgsl"));
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("MechoFly deterministic neural step"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        Ok(Self {
            device,
            queue,
            pipeline,
            bind_group,
            params,
            state,
            stimulus,
            output,
            readback,
            neuron_count,
        })
    }

    fn step(
        &mut self,
        engine: &mut ModelEngine,
        stimulus: &[i32],
    ) -> Result<FrameSummary, String> {
        if engine.state.activation.len() != self.neuron_count || stimulus.len() != self.neuron_count
        {
            return Err("GPU step dimensions differ from the active graph".to_owned());
        }
        let next_frame = engine.state.frame + 1;
        let params = GpuParams {
            neuron_count: self.neuron_count as u32,
            frame_low: next_frame as u32,
            frame_high: (next_frame >> 32) as u32,
            seed_folded: (engine.state.seed as u32) ^ ((engine.state.seed >> 32) as u32),
        };
        self.queue
            .write_buffer(&self.params, 0, bytemuck::bytes_of(&params));
        self.queue
            .write_buffer(&self.state, 0, bytemuck::cast_slice(&engine.state.activation));
        self.queue
            .write_buffer(&self.stimulus, 0, bytemuck::cast_slice(stimulus));

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("MechoFly neural step encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("MechoFly neural step pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.dispatch_workgroups((self.neuron_count as u32).div_ceil(WORKGROUP_SIZE), 1, 1);
        }
        encoder.copy_buffer_to_buffer(&self.output, 0, &self.readback, 0, self.output.size());
        self.queue.submit([encoder.finish()]);

        let slice = self.readback.slice(..);
        let (sender, receiver) = mpsc::sync_channel(1);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|error| format!("GPU wait failed: {error}"))?;
        receiver
            .recv()
            .map_err(|error| format!("GPU map callback failed: {error}"))?
            .map_err(|error| format!("GPU readback failed: {error}"))?;
        let mapped = slice
            .get_mapped_range()
            .map_err(|error| format!("GPU mapped range failed: {error}"))?;
        let mut activation = Vec::with_capacity(self.neuron_count);
        let mut spikes = Vec::with_capacity(self.neuron_count);
        for bytes in mapped.chunks_exact(8) {
            activation.push(i32::from_le_bytes(bytes[0..4].try_into().unwrap()));
            spikes.push(u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as u8);
        }
        drop(mapped);
        self.readback.unmap();
        Ok(engine.accept_backend_step(activation, spikes))
    }
}

fn binding(
    binding: u32,
    ty: wgpu::BufferBindingType,
    has_dynamic_offset: bool,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset,
            min_binding_size: None,
        },
        count: None,
    }
}

fn entry(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}
