use std::{collections::BTreeMap, fs, path::Path, sync::Arc, time::Instant};

use mechofly_core::{
    Behavior, ModelEngine, ModelGraph, ModelTier, StepInput,
    behavior_telemetry::{
        BEHAVIOR_TELEMETRY_CLAIM_BOUNDARY, BEHAVIOR_TELEMETRY_CONTROLLER,
        BEHAVIOR_TELEMETRY_SCHEMA_VERSION, BehaviorTransitionEvent,
    },
    model::{
        ALERT_POPULATION_OFFSET, FUNCTIONAL_POPULATION_COUNT, GROOM_POPULATION_OFFSET,
        LOOM_POPULATION_OFFSET, REVERSE_POPULATION_OFFSET, WALK_POPULATION_OFFSET,
    },
    provenance::sha256_hex,
};
use serde::Serialize;

const BASELINE_SCHEMA_VERSION: u32 = 1;
const FULL_CAMPAIGN_SEEDS: usize = 20;
const FULL_CAMPAIGN_REPEATS: usize = 2;
const FULL_CAMPAIGN_SECONDS: u32 = 30 * 60;
const SCENARIOS: [Scenario; 5] = [
    Scenario::QuietRest,
    Scenario::Walking,
    Scenario::Grooming,
    Scenario::RepeatedLoom,
    Scenario::Mixed,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Scenario {
    QuietRest,
    Walking,
    Grooming,
    RepeatedLoom,
    Mixed,
}

impl Scenario {
    const fn label(self) -> &'static str {
        match self {
            Self::QuietRest => "quiet_rest",
            Self::Walking => "walking",
            Self::Grooming => "grooming",
            Self::RepeatedLoom => "repeated_loom",
            Self::Mixed => "mixed",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct BaselineConfig {
    seeds: usize,
    repeats: usize,
    seconds: u32,
}

impl BaselineConfig {
    fn from_args(args: &[String]) -> Result<Self, String> {
        Ok(Self {
            seeds: parse_bounded_usize(args, "--baseline-seeds", 2, 1, FULL_CAMPAIGN_SEEDS)?,
            repeats: parse_bounded_usize(args, "--baseline-repeats", 2, 1, 4)?,
            seconds: parse_bounded_u32(args, "--baseline-seconds", 30, 1, FULL_CAMPAIGN_SECONDS)?,
        })
    }

    const fn planned_full_matrix(self) -> bool {
        self.seeds == FULL_CAMPAIGN_SEEDS
            && self.repeats == FULL_CAMPAIGN_REPEATS
            && self.seconds == FULL_CAMPAIGN_SECONDS
    }

    fn frame_count(self) -> u64 {
        u64::from(self.seconds)
            .saturating_mul(1_000)
            .div_ceil(u64::from(mechofly_core::MODEL_STEP_MS))
            .max(1)
    }
}

#[derive(Serialize)]
struct BaselineReport {
    schema_version: u32,
    status: String,
    campaign_classification: String,
    planned_full_matrix_complete: bool,
    controller: String,
    telemetry_schema_version: u32,
    telemetry_claim_boundary: String,
    telemetry_observational_only: bool,
    controller_semantics_changed: bool,
    model_version: String,
    model_step_ms: u32,
    scenario_count: usize,
    seed_count: usize,
    repeats_per_seed: usize,
    modeled_seconds_per_run: u32,
    modeled_frames_per_run: u64,
    run_count: usize,
    all_repeat_groups_equal: bool,
    total_transition_count: u64,
    total_dropped_transition_count: u64,
    runs: Vec<RunReport>,
    repeat_groups: Vec<RepeatGroupReport>,
    claim_boundary: String,
}

#[derive(Clone, Serialize)]
struct RunReport {
    scenario: String,
    seed_index: usize,
    repeat: usize,
    graph_sha256: String,
    model_seed: u64,
    modeled_frames: u64,
    modeled_ms: u64,
    wall_time_ms: u128,
    mean_step_micros: f64,
    p99_step_micros: u128,
    max_step_micros: u128,
    initial_state_digest: String,
    final_state_digest: String,
    transition_count: u64,
    retained_transition_count: usize,
    dropped_transition_count: u64,
    transition_stream_sha256: String,
    retained_sequence_contiguous: bool,
    state_occupancy_frames: BTreeMap<String, u64>,
    transition_reasons: BTreeMap<String, u64>,
    bout_duration_bins: BoutDurationBins,
    escape_latency_frames: Vec<u64>,
    deterministic_signature_sha256: String,
}

#[derive(Clone, Default, Serialize)]
struct BoutDurationBins {
    under_100_ms: u64,
    under_250_ms: u64,
    under_500_ms: u64,
    under_1_000_ms: u64,
    under_2_000_ms: u64,
    at_least_2_000_ms: u64,
}

impl BoutDurationBins {
    fn record(&mut self, elapsed_ms: u64) {
        match elapsed_ms {
            0..=99 => self.under_100_ms += 1,
            100..=249 => self.under_250_ms += 1,
            250..=499 => self.under_500_ms += 1,
            500..=999 => self.under_1_000_ms += 1,
            1_000..=1_999 => self.under_2_000_ms += 1,
            _ => self.at_least_2_000_ms += 1,
        }
    }
}

#[derive(Serialize)]
struct RepeatGroupReport {
    scenario: String,
    seed_index: usize,
    repeat_count: usize,
    signatures: Vec<String>,
    byte_equivalent: bool,
}

pub fn run(path: &Path, args: &[String]) -> Result<(), String> {
    let config = BaselineConfig::from_args(args)?;
    let frames = config.frame_count();
    let mut runs = Vec::with_capacity(SCENARIOS.len() * config.seeds * config.repeats);

    for scenario in SCENARIOS {
        for seed_index in 0..config.seeds {
            let graph_seed = 0xB453_1A00_u64 ^ seed_index as u64;
            let graph = Arc::new(ModelGraph::synthetic(ModelTier::Demo4096, graph_seed));
            graph.validate()?;
            for repeat in 0..config.repeats {
                runs.push(run_one(
                    scenario,
                    seed_index,
                    repeat,
                    frames,
                    Arc::clone(&graph),
                )?);
            }
        }
    }

    let mut repeat_groups = Vec::with_capacity(SCENARIOS.len() * config.seeds);
    let mut all_repeat_groups_equal = true;
    for scenario in SCENARIOS {
        for seed_index in 0..config.seeds {
            let matching: Vec<&RunReport> = runs
                .iter()
                .filter(|run| run.scenario == scenario.label() && run.seed_index == seed_index)
                .collect();
            let signatures: Vec<String> = matching
                .iter()
                .map(|run| run.deterministic_signature_sha256.clone())
                .collect();
            let byte_equivalent = signatures
                .first()
                .is_some_and(|first| signatures.iter().all(|value| value == first));
            all_repeat_groups_equal &= byte_equivalent && signatures.len() == config.repeats;
            repeat_groups.push(RepeatGroupReport {
                scenario: scenario.label().to_owned(),
                seed_index,
                repeat_count: signatures.len(),
                signatures,
                byte_equivalent,
            });
        }
    }

    let total_transition_count = runs.iter().map(|run| run.transition_count).sum();
    let total_dropped_transition_count = runs.iter().map(|run| run.dropped_transition_count).sum();
    let every_run_has_transition = runs.iter().all(|run| run.transition_count > 0);
    let status = if all_repeat_groups_equal && every_run_has_transition {
        "PASS"
    } else {
        "FAIL"
    };
    let report = BaselineReport {
        schema_version: BASELINE_SCHEMA_VERSION,
        status: status.to_owned(),
        campaign_classification: if config.planned_full_matrix() {
            "planned_full_matrix"
        } else {
            "local_smoke"
        }
        .to_owned(),
        planned_full_matrix_complete: config.planned_full_matrix(),
        controller: BEHAVIOR_TELEMETRY_CONTROLLER.to_owned(),
        telemetry_schema_version: BEHAVIOR_TELEMETRY_SCHEMA_VERSION,
        telemetry_claim_boundary: BEHAVIOR_TELEMETRY_CLAIM_BOUNDARY.to_owned(),
        telemetry_observational_only: true,
        controller_semantics_changed: false,
        model_version: mechofly_core::MODEL_VERSION.to_owned(),
        model_step_ms: mechofly_core::MODEL_STEP_MS,
        scenario_count: SCENARIOS.len(),
        seed_count: config.seeds,
        repeats_per_seed: config.repeats,
        modeled_seconds_per_run: config.seconds,
        modeled_frames_per_run: frames,
        run_count: runs.len(),
        all_repeat_groups_equal,
        total_transition_count,
        total_dropped_transition_count,
        runs,
        repeat_groups,
        claim_boundary:
            "instrumentation baseline for the existing deterministic controller; this report is not an empirically calibrated ethogram or a biological-fit result"
                .to_owned(),
    };

    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create behavior-baseline directory: {error}"))?;
    }
    let json = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("cannot serialize behavior baseline: {error}"))?;
    fs::write(path, format!("{json}\n"))
        .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    if report.status == "PASS" {
        Ok(())
    } else {
        Err("behavior transition telemetry baseline invariants failed".to_owned())
    }
}

fn run_one(
    scenario: Scenario,
    seed_index: usize,
    repeat: usize,
    frames: u64,
    graph: Arc<ModelGraph>,
) -> Result<RunReport, String> {
    let model_seed = 0x7E1E_0000_u64 ^ seed_index as u64;
    let mut engine = ModelEngine::new(Arc::clone(&graph), model_seed);
    let initial_state_digest = engine.state.digest();
    let mut stimulus = engine.empty_stimulus();
    let mut occupancy = BTreeMap::new();
    let mut reasons = BTreeMap::new();
    let mut bins = BoutDurationBins::default();
    let mut escape_latency_frames = Vec::new();
    let mut pending_loom_frame = None;
    let mut observed_sequence = None;
    let mut step_micros = Vec::with_capacity(frames as usize);
    let wall_started = Instant::now();

    for frame in 0..frames {
        let loom_onset = prepare_stimulus(scenario, frame, &mut stimulus);
        if loom_onset {
            pending_loom_frame = Some(frame);
        }

        let step_started = Instant::now();
        let summary = engine.step_cpu(StepInput {
            stimulus_q15: &stimulus,
        });
        step_micros.push(step_started.elapsed().as_micros());

        *occupancy
            .entry(format!("{:?}", summary.behavior))
            .or_insert(0) += 1;
        if let Some(event) = engine.latest_behavior_transition()
            && observed_sequence != Some(event.sequence)
        {
            observe_event(
                event,
                &mut reasons,
                &mut bins,
                &mut pending_loom_frame,
                &mut escape_latency_frames,
            );
            observed_sequence = Some(event.sequence);
        }
    }

    step_micros.sort_unstable();
    let p99_index = step_micros.len().saturating_sub(1).saturating_mul(99) / 100;
    let p99_step_micros = step_micros.get(p99_index).copied().unwrap_or_default();
    let max_step_micros = step_micros.last().copied().unwrap_or_default();
    let mean_step_micros = if step_micros.is_empty() {
        0.0
    } else {
        step_micros.iter().copied().sum::<u128>() as f64 / step_micros.len() as f64
    };

    let telemetry = engine.behavior_telemetry_snapshot();
    let deterministic_payload = serde_json::to_vec(&(
        scenario.label(),
        seed_index,
        engine.state.digest(),
        &telemetry.event_stream_sha256,
        telemetry.total_event_count,
        &occupancy,
        &reasons,
        &bins,
        &escape_latency_frames,
    ))
    .map_err(|error| format!("cannot serialize deterministic baseline signature: {error}"))?;
    let deterministic_signature_sha256 = sha256_hex([deterministic_payload]);

    Ok(RunReport {
        scenario: scenario.label().to_owned(),
        seed_index,
        repeat,
        graph_sha256: graph.identity.sha256.clone(),
        model_seed,
        modeled_frames: frames,
        modeled_ms: frames.saturating_mul(u64::from(mechofly_core::MODEL_STEP_MS)),
        wall_time_ms: wall_started.elapsed().as_millis(),
        mean_step_micros,
        p99_step_micros,
        max_step_micros,
        initial_state_digest,
        final_state_digest: engine.state.digest(),
        transition_count: telemetry.total_event_count,
        retained_transition_count: telemetry.retained_event_count,
        dropped_transition_count: telemetry.dropped_event_count,
        transition_stream_sha256: telemetry.event_stream_sha256,
        retained_sequence_contiguous: telemetry.retained_sequence_contiguous,
        state_occupancy_frames: occupancy,
        transition_reasons: reasons,
        bout_duration_bins: bins,
        escape_latency_frames,
        deterministic_signature_sha256,
    })
}

fn observe_event(
    event: &BehaviorTransitionEvent,
    reasons: &mut BTreeMap<String, u64>,
    bins: &mut BoutDurationBins,
    pending_loom_frame: &mut Option<u64>,
    escape_latency_frames: &mut Vec<u64>,
) {
    *reasons.entry(event.reason.as_str().to_owned()).or_insert(0) += 1;
    bins.record(event.elapsed_ms);
    if event.to_behavior == Behavior::PreEscape
        && let Some(onset) = pending_loom_frame.take()
    {
        escape_latency_frames.push(event.frame.saturating_sub(onset));
    }
}

fn prepare_stimulus(scenario: Scenario, frame: u64, stimulus: &mut [i32]) -> bool {
    stimulus.fill(0);
    match scenario {
        Scenario::QuietRest => false,
        Scenario::Walking => {
            apply_population_drive(stimulus, WALK_POPULATION_OFFSET, 8_192);
            false
        }
        Scenario::Grooming => {
            apply_population_drive(stimulus, GROOM_POPULATION_OFFSET, 8_192);
            false
        }
        Scenario::RepeatedLoom => {
            let phase = frame % 300;
            if phase < 12 {
                apply_population_drive(stimulus, LOOM_POPULATION_OFFSET, 8_192);
            }
            phase == 0
        }
        Scenario::Mixed => {
            let phase = (frame / 120) % 6;
            match phase {
                1 => apply_population_drive(stimulus, WALK_POPULATION_OFFSET, 8_192),
                2 => apply_population_drive(stimulus, GROOM_POPULATION_OFFSET, 8_192),
                3 => apply_population_drive(stimulus, ALERT_POPULATION_OFFSET, 8_192),
                4 => apply_population_drive(stimulus, REVERSE_POPULATION_OFFSET, 8_192),
                5 if frame % 120 < 12 => {
                    apply_population_drive(stimulus, LOOM_POPULATION_OFFSET, 8_192)
                }
                _ => {}
            }
            phase == 5 && frame.is_multiple_of(120)
        }
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

fn parse_bounded_usize(
    args: &[String],
    option: &str,
    default: usize,
    minimum: usize,
    maximum: usize,
) -> Result<usize, String> {
    let Some(value) = option_value(args, option) else {
        return Ok(default);
    };
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("{option} must be an integer"))?;
    if !(minimum..=maximum).contains(&parsed) {
        return Err(format!("{option} must be between {minimum} and {maximum}"));
    }
    Ok(parsed)
}

fn parse_bounded_u32(
    args: &[String],
    option: &str,
    default: u32,
    minimum: u32,
    maximum: u32,
) -> Result<u32, String> {
    let Some(value) = option_value(args, option) else {
        return Ok(default);
    };
    let parsed = value
        .parse::<u32>()
        .map_err(|_| format!("{option} must be an integer"))?;
    if !(minimum..=maximum).contains(&parsed) {
        return Err(format!("{option} must be between {minimum} and {maximum}"));
    }
    Ok(parsed)
}

fn option_value<'a>(args: &'a [String], option: &str) -> Option<&'a str> {
    args.iter()
        .position(|arg| arg == option)
        .and_then(|index| args.get(index + 1))
        .map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_matrix_is_deterministic_and_contains_all_scenarios() {
        let path = std::env::temp_dir().join(format!(
            "mechofly-behavior-baseline-{}.json",
            std::process::id()
        ));
        let args = vec![
            "--baseline-seeds".to_owned(),
            "1".to_owned(),
            "--baseline-repeats".to_owned(),
            "2".to_owned(),
            "--baseline-seconds".to_owned(),
            "3".to_owned(),
        ];
        run(&path, &args).expect("baseline smoke must pass");
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).expect("baseline report must be readable"))
                .expect("baseline report must be valid JSON");
        assert_eq!(value["status"], "PASS");
        assert_eq!(value["scenario_count"], 5);
        assert_eq!(value["run_count"], 10);
        assert_eq!(value["all_repeat_groups_equal"], true);
        fs::remove_file(path).ok();
    }
}
