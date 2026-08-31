//! Resumable N4 campaign. Uses the original stimulus protocol; persists each run.
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{BufWriter, Write},
    path::Path,
    sync::Arc,
    time::Instant,
};

use mechofly_core::{
    Behavior, ModelEngine, ModelGraph, ModelTier, StepInput,
    behavior_dynamics::{BehaviorDynamicsState, airborne},
    behavior_parameters::{DYNAMICS_VERSION, parameter_sha256},
    behavior_telemetry::BehaviorTransitionReason,
    provenance::sha256_hex,
};
use serde::{Deserialize, Serialize};

use crate::behavior_baseline::{SCENARIOS, Scenario, prepare_stimulus};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunDeterministic {
    pub scenario: String,
    pub seed_index: usize,
    pub model_seed: u64,
    pub graph_sha256: String,
    pub frames: u64,
    pub modeled_ms: u64,
    pub initial_state_digest: String,
    pub final_state_digest: String,
    pub final_neural_sha256: String,
    pub final_controller: BehaviorDynamicsState,
    pub occupancy_frames: BTreeMap<String, u64>,
    pub transition_count: u64,
    pub transition_reasons: BTreeMap<String, u64>,
    pub bout_duration_bins: BTreeMap<String, u64>,
    pub escape_latency_frames: Vec<u64>,
    pub eligible_loom_frames: u64,
    pub missed_loom_preemptions: u64,
    pub minimum_dwell_violations: u64,
    pub grooming_floor_violations: u64,
    pub periodic_schedule_events: u64,
    pub controller_faults: u64,
    pub retained_transition_count: usize,
    pub dropped_transition_count: u64,
    pub transition_stream_sha256: String,
    pub raw_events_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunRecord {
    pub schema_version: u32,
    pub authority: String,
    pub parameter_sha256: String,
    pub repeat: usize,
    pub deterministic: RunDeterministic,
    pub deterministic_signature_sha256: String,
    pub wall_time_ms: u128,
    pub mean_step_ns: u64,
    pub p99_step_ns: u64,
    pub max_step_ns: u64,
}

#[derive(Serialize)]
struct Campaign {
    schema_version: u32,
    controller: String,
    parameter_sha256: String,
    authority: String,
    controller_semantics_changed: bool,
    graph_scope: String,
    seeds: usize,
    repeats: usize,
    seconds: u32,
    runs: Vec<RunRecord>,
}

pub fn run(directory: &Path, args: &[String]) -> Result<(), String> {
    let seeds = argument(args, "--campaign-seeds", 2, 20)? as usize;
    let repeats = argument(args, "--campaign-repeats", 2, 4)? as usize;
    let seconds = argument(args, "--campaign-seconds", 30, 1_800)?;
    let authority =
        option(args, "--campaign-authority").ok_or("--campaign-authority is required")?;
    if authority.len() != 64 || !authority.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("campaign authority must be the exact executable SHA-256".into());
    }
    fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let frames = (u64::from(seconds) * 1_000).div_ceil(u64::from(mechofly_core::MODEL_STEP_MS));
    let mut runs = Vec::with_capacity(5 * seeds * repeats);
    for scenario in SCENARIOS {
        for seed in 0..seeds {
            let graph = Arc::new(ModelGraph::synthetic(
                ModelTier::Demo4096,
                0xB453_1A00 ^ seed as u64,
            ));
            graph.validate()?;
            for repeat in 0..repeats {
                let stem = format!("{}-seed-{seed:02}-repeat-{repeat}", scenario.label());
                let path = directory.join(format!("{stem}.json"));
                let events = directory.join(format!("{stem}.events.jsonl"));
                let reused = path.is_file();
                let record = if reused {
                    let r: RunRecord =
                        serde_json::from_slice(&fs::read(&path).map_err(|e| e.to_string())?)
                            .map_err(|e| e.to_string())?;
                    if r.schema_version != 1
                        || r.authority != authority
                        || r.parameter_sha256 != parameter_sha256()
                        || r.repeat != repeat
                        || r.deterministic.scenario != scenario.label()
                        || r.deterministic.seed_index != seed
                        || r.deterministic.frames != frames
                        || r.deterministic.graph_sha256 != graph.identity.sha256
                        || r.deterministic.raw_events_sha256 != file_sha256(&events)?
                    {
                        return Err(format!(
                            "cached run identity or checksum mismatch: {}",
                            path.display()
                        ));
                    }
                    verify_run(&r)?;
                    r
                } else {
                    let record = run_one(
                        scenario,
                        seed,
                        repeat,
                        frames,
                        Arc::clone(&graph),
                        &events,
                        authority,
                    )?;
                    atomic_json(&path, &record)?;
                    record
                };
                verify_run(&record)?;
                runs.push(record);
                println!(
                    "N4_PROGRESS completed_runs={} total_runs={} scenario={} seed={} repeat={} reused={}",
                    runs.len(),
                    5 * seeds * repeats,
                    scenario.label(),
                    seed,
                    repeat,
                    reused
                );
                std::io::stdout().flush().map_err(|e| e.to_string())?;
            }
        }
    }
    for group in runs.chunks_exact(repeats) {
        for r in group {
            if r.deterministic_signature_sha256 != group[0].deterministic_signature_sha256 {
                return Err("N4 exact repeat mismatch".into());
            }
        }
    }
    let report = Campaign {
        schema_version: 1,
        controller: DYNAMICS_VERSION.into(),
        parameter_sha256: parameter_sha256().into(),
        authority: authority.into(),
        controller_semantics_changed: true,
        graph_scope: "synthetic Demo4096 CPU, shared D0 stimulus protocol".into(),
        seeds,
        repeats,
        seconds,
        runs,
    };
    atomic_json(&directory.join("campaign.json"), &report)?;
    println!("N4_CAMPAIGN=PASS runs={}", report.runs.len());
    Ok(())
}

fn run_one(
    scenario: Scenario,
    seed: usize,
    repeat: usize,
    frames: u64,
    graph: Arc<ModelGraph>,
    events_path: &Path,
    authority: &str,
) -> Result<RunRecord, String> {
    let model_seed = 0x7E1E_0000 ^ seed as u64;
    let mut engine = ModelEngine::new_duration_aware(Arc::clone(&graph), model_seed);
    let initial = engine.state.digest();
    let mut stimulus = engine.empty_stimulus();
    let mut occupancy = BTreeMap::new();
    let mut reasons = BTreeMap::new();
    let mut bins = BTreeMap::new();
    let mut latencies = Vec::new();
    let mut pending_loom = None;
    let mut observed_sequence = None;
    let mut eligible_loom = 0;
    let mut missed = 0;
    let mut premature = 0;
    let mut groom_short = 0;
    let mut schedule = 0;
    let mut faults = 0;
    let mut timings = Vec::with_capacity(frames as usize);
    let tmp = events_path.with_extension("jsonl.tmp");
    let mut stream = BufWriter::new(File::create(&tmp).map_err(|e| e.to_string())?);
    let started = Instant::now();
    for frame in 0..frames {
        if prepare_stimulus(scenario, frame, &mut stimulus) {
            pending_loom = Some(frame);
        }
        let before = engine.state.behavior;
        let clock = Instant::now();
        let summary = engine.step_cpu(StepInput {
            stimulus_q15: &stimulus,
        });
        timings.push(clock.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64);
        *occupancy
            .entry(format!("{:?}", summary.behavior))
            .or_insert(0) += 1;
        let state = engine
            .state
            .behavior_dynamics
            .as_ref()
            .ok_or("N4 campaign unexpectedly entered legacy mode")?;
        if state.fault_latched {
            faults += 1;
        }
        if !airborne(before) && engine.last_behavior_intent.loom_activation_q15 >= 5_200 {
            eligible_loom += 1;
            if summary.behavior != Behavior::PreEscape {
                missed += 1;
            }
        }
        if scenario == Scenario::QuietRest && airborne(summary.behavior) {
            return Err("zero input generated an escape".into());
        }
        if let Some(event) = engine.latest_behavior_transition()
            && observed_sequence != Some(event.sequence)
        {
            let details = event
                .dynamics
                .as_ref()
                .ok_or("N4 transition lacks duration provenance")?;
            if !event.emergency_override && event.elapsed_frames < details.minimum_dwell_frames {
                premature += 1;
            }
            if event.from_behavior == Behavior::Groom
                && !event.emergency_override
                && event.elapsed_ms < 1_500
            {
                groom_short += 1;
            }
            if event.reason == BehaviorTransitionReason::LegacyAutonomousSchedule {
                schedule += 1;
            }
            *reasons.entry(event.reason.as_str().to_owned()).or_insert(0) += 1;
            *bins
                .entry(bout_bin(event.elapsed_ms).to_owned())
                .or_insert(0) += 1;
            if event.to_behavior == Behavior::PreEscape
                && let Some(onset) = pending_loom.take()
            {
                latencies.push(event.frame.saturating_sub(onset));
            }
            serde_json::to_writer(&mut stream, event).map_err(|e| e.to_string())?;
            stream.write_all(b"\n").map_err(|e| e.to_string())?;
            observed_sequence = Some(event.sequence);
        }
        if missed + premature + groom_short + schedule + faults != 0 {
            return Err("N4 runtime invariant violated; do not deploy".into());
        }
    }
    stream.flush().map_err(|e| e.to_string())?;
    stream.get_ref().sync_all().map_err(|e| e.to_string())?;
    drop(stream);
    if events_path.exists() {
        fs::remove_file(events_path).map_err(|e| e.to_string())?;
    }
    fs::rename(&tmp, events_path).map_err(|e| e.to_string())?;
    let wall_time_ms = started.elapsed().as_millis();
    let mean = (timings.iter().map(|v| u128::from(*v)).sum::<u128>() / u128::from(frames)) as u64;
    timings.sort_unstable();
    let p99 = timings[(timings.len() - 1) * 99 / 100];
    let max = *timings.last().ok_or("empty timing sequence")?;
    let snapshot = engine.behavior_telemetry_snapshot();
    let activation: Vec<u8> = engine
        .state
        .activation
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let deterministic = RunDeterministic {
        scenario: scenario.label().into(),
        seed_index: seed,
        model_seed,
        graph_sha256: graph.identity.sha256.clone(),
        frames,
        modeled_ms: frames * 33,
        initial_state_digest: initial,
        final_state_digest: engine.state.digest(),
        final_neural_sha256: sha256_hex([activation.as_slice(), engine.state.spikes.as_slice()]),
        final_controller: engine
            .state
            .behavior_dynamics
            .clone()
            .ok_or("N4 controller missing")?,
        occupancy_frames: occupancy,
        transition_count: snapshot.total_event_count,
        transition_reasons: reasons,
        bout_duration_bins: bins,
        escape_latency_frames: latencies,
        eligible_loom_frames: eligible_loom,
        missed_loom_preemptions: missed,
        minimum_dwell_violations: premature,
        grooming_floor_violations: groom_short,
        periodic_schedule_events: schedule,
        controller_faults: faults,
        retained_transition_count: snapshot.retained_event_count,
        dropped_transition_count: snapshot.dropped_event_count,
        transition_stream_sha256: snapshot.event_stream_sha256,
        raw_events_sha256: file_sha256(events_path)?,
    };
    let signature = sha256_hex([serde_json::to_vec(&deterministic).map_err(|e| e.to_string())?]);
    Ok(RunRecord {
        schema_version: 1,
        authority: authority.into(),
        parameter_sha256: parameter_sha256().into(),
        repeat,
        deterministic,
        deterministic_signature_sha256: signature,
        wall_time_ms,
        mean_step_ns: mean,
        p99_step_ns: p99,
        max_step_ns: max,
    })
}

fn verify_run(record: &RunRecord) -> Result<(), String> {
    let r = &record.deterministic;
    let signature = sha256_hex([serde_json::to_vec(r).map_err(|e| e.to_string())?]);
    if signature != record.deterministic_signature_sha256
        || r.frames == 0
        || r.occupancy_frames.values().sum::<u64>() != r.frames
        || r.transition_reasons.values().sum::<u64>() != r.transition_count
        || r.bout_duration_bins.values().sum::<u64>() != r.transition_count
        || r.missed_loom_preemptions
            + r.minimum_dwell_violations
            + r.grooming_floor_violations
            + r.periodic_schedule_events
            + r.controller_faults
            != 0
        || r.transition_count != r.retained_transition_count as u64 + r.dropped_transition_count
        || r.retained_transition_count > 512
    {
        return Err("N4 report accounting or deterministic signature mismatch".into());
    }
    r.final_controller.validate(
        r.model_seed,
        r.frames,
        r.final_controller.current_macro_state,
        r.final_controller.elapsed_frames,
    )?;
    Ok(())
}

fn bout_bin(ms: u64) -> &'static str {
    match ms {
        0..=99 => "under_100_ms",
        100..=249 => "under_250_ms",
        250..=499 => "under_500_ms",
        500..=999 => "under_1_000_ms",
        1_000..=1_999 => "under_2_000_ms",
        _ => "at_least_2_000_ms",
    }
}

fn file_sha256(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    Ok(mechofly_core::behavior_parameters::artifact_sha256(&bytes))
}

fn atomic_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    let mut file = File::create(&tmp).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(&mut file, value).map_err(|e| e.to_string())?;
    file.write_all(b"\n").map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    fs::rename(tmp, path).map_err(|e| e.to_string())
}

fn option<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}
fn argument(args: &[String], key: &str, default: u32, max: u32) -> Result<u32, String> {
    let value = option(args, key)
        .map(|v| v.parse::<u32>())
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or(default);
    if value == 0 || value > max {
        return Err(format!("{key} must be between 1 and {max}"));
    }
    Ok(value)
}
