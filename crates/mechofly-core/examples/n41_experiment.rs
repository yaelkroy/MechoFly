//! Offline, matched N3/N4/N4.1 experiment. This binary never deploys a pet.
#[allow(dead_code)]
#[path = "../../mechofly-app/src/behavior_baseline.rs"]
mod behavior_baseline;

use std::{collections::BTreeMap, fs, path::Path, sync::Arc};

use behavior_baseline::{SCENARIOS, Scenario, prepare_stimulus};
use mechofly_core::{
    Behavior, ModelEngine, ModelGraph, ModelTier, StepComponentTimings, StepInput,
    behavior_dynamics::airborne,
    behavior_parameters::{
        BehaviorParameterProfile, DYNAMICS_VERSION, N41_DYNAMICS_VERSION,
        N41_NATURAL_BOUT_DYNAMICS_VERSION, N41_NATURAL_FLIGHT_DYNAMICS_VERSION,
        parameter_sha256_for, parameters_for_profile,
    },
    behavior_telemetry::{BEHAVIOR_TELEMETRY_CONTROLLER, BehaviorTransitionReason},
    model::{FUNCTIONAL_POPULATION_COUNT, WALK_POPULATION_OFFSET},
    provenance::sha256_hex,
};
use serde::Serialize;

const MAX_SEEDS: usize = 20;
const MAX_REPEATS: usize = 4;
const MAX_SECONDS: u32 = 1_800;
const TIMING_BUCKET_NS: u64 = 100;
const TIMING_BUCKET_COUNT: usize = 20_001;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExperimentProfile {
    N3,
    Duration(BehaviorParameterProfile),
}

impl ExperimentProfile {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "n3" => Ok(Self::N3),
            _ => BehaviorParameterProfile::parse(value).map(Self::Duration),
        }
    }

    const fn cli(self) -> &'static str {
        match self {
            Self::N3 => "n3",
            Self::Duration(profile) => profile.cli(),
        }
    }

    const fn controller(self) -> &'static str {
        match self {
            Self::N3 => BEHAVIOR_TELEMETRY_CONTROLLER,
            Self::Duration(BehaviorParameterProfile::N4) => DYNAMICS_VERSION,
            Self::Duration(BehaviorParameterProfile::N41BNatural) => {
                N41_NATURAL_BOUT_DYNAMICS_VERSION
            }
            Self::Duration(BehaviorParameterProfile::N41BNaturalFlight) => {
                N41_NATURAL_FLIGHT_DYNAMICS_VERSION
            }
            Self::Duration(_) => N41_DYNAMICS_VERSION,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Config {
    profile: ExperimentProfile,
    seeds: usize,
    repeats: usize,
    seconds: u32,
}

impl Config {
    fn parse(args: &[String]) -> Result<(Self, String, String), String> {
        let output = option(args, "--output").ok_or("--output is required")?;
        let authority = option(args, "--authority").ok_or("--authority is required")?;
        if authority.len() != 64 || !authority.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("--authority must be the experiment executable SHA-256".into());
        }
        let profile =
            ExperimentProfile::parse(option(args, "--profile").ok_or("--profile is required")?)?;
        Ok((
            Self {
                profile,
                seeds: bounded_usize(args, "--seeds", 2, 1, MAX_SEEDS)?,
                repeats: bounded_usize(args, "--repeats", 2, 2, MAX_REPEATS)?,
                seconds: bounded_u32(args, "--seconds", 600, 600, MAX_SECONDS)?,
            },
            output.to_owned(),
            authority.to_ascii_lowercase(),
        ))
    }

    fn frames(self) -> u64 {
        u64::from(self.seconds)
            .saturating_mul(1_000)
            .div_ceil(u64::from(mechofly_core::MODEL_STEP_MS))
    }
}

#[derive(Clone, Debug, Serialize)]
struct DeterministicRun {
    scenario: String,
    seed_index: usize,
    model_seed: u64,
    graph_sha256: String,
    modeled_frames: u64,
    initial_state_digest: String,
    final_state_digest: String,
    final_neural_sha256: String,
    occupancy_frames: BTreeMap<String, u64>,
    early_30s_occupancy_frames: BTreeMap<String, u64>,
    late_5m_occupancy_frames: BTreeMap<String, u64>,
    transition_count: u64,
    transition_reasons: BTreeMap<String, u64>,
    maximum_walk_drive_unresponsive_frames: u64,
    walk_drive_frames: u64,
    walk_during_drive_frames: u64,
    missed_loom_preemptions: u64,
    minimum_dwell_violations: u64,
    grooming_floor_violations: u64,
    controller_faults: u64,
    zero_input_escape_frames: u64,
    periodic_schedule_events: u64,
    final_fatigue_q15: Option<i32>,
    final_fatigue_response_gain_q15: Option<i32>,
}

#[derive(Clone, Debug, Serialize)]
struct RunRecord {
    repeat: usize,
    deterministic_signature_sha256: String,
    deterministic: DeterministicRun,
}

#[derive(Clone, Debug, Serialize)]
struct TimingStats {
    count: u64,
    mean_ns: u64,
    p50_upper_bound_ns: u64,
    p95_upper_bound_ns: u64,
    p99_upper_bound_ns: u64,
    max_ns: u64,
    histogram_bucket_ns: u64,
}

#[derive(Clone, Debug, Serialize)]
struct ComponentTimingReport {
    neural_compute: TimingStats,
    intent_build: TimingStats,
    controller: TimingStats,
    telemetry: TimingStats,
    summary_hash: TimingStats,
    total: TimingStats,
}

#[derive(Debug)]
struct TimingAccumulator {
    count: u64,
    sum: u128,
    max: u64,
    buckets: Vec<u64>,
}

impl Default for TimingAccumulator {
    fn default() -> Self {
        Self {
            count: 0,
            sum: 0,
            max: 0,
            buckets: vec![0; TIMING_BUCKET_COUNT],
        }
    }
}

impl TimingAccumulator {
    fn record(&mut self, value: u64) {
        self.count += 1;
        self.sum += u128::from(value);
        self.max = self.max.max(value);
        let bucket = (value / TIMING_BUCKET_NS).min((TIMING_BUCKET_COUNT - 1) as u64) as usize;
        self.buckets[bucket] += 1;
    }

    fn percentile_upper_bound(&self, numerator: u64, denominator: u64) -> u64 {
        let target = self.count.saturating_mul(numerator).div_ceil(denominator);
        let mut seen = 0_u64;
        for (index, count) in self.buckets.iter().enumerate() {
            seen += count;
            if seen >= target.max(1) {
                if index == TIMING_BUCKET_COUNT - 1 {
                    return self.max;
                }
                return ((index as u64) + 1).saturating_mul(TIMING_BUCKET_NS);
            }
        }
        self.max
    }

    fn report(&self) -> TimingStats {
        TimingStats {
            count: self.count,
            mean_ns: if self.count == 0 {
                0
            } else {
                (self.sum / u128::from(self.count)) as u64
            },
            p50_upper_bound_ns: self.percentile_upper_bound(50, 100),
            p95_upper_bound_ns: self.percentile_upper_bound(95, 100),
            p99_upper_bound_ns: self.percentile_upper_bound(99, 100),
            max_ns: self.max,
            histogram_bucket_ns: TIMING_BUCKET_NS,
        }
    }
}

#[derive(Debug, Default)]
struct ComponentAccumulator {
    neural_compute: TimingAccumulator,
    intent_build: TimingAccumulator,
    controller: TimingAccumulator,
    telemetry: TimingAccumulator,
    summary_hash: TimingAccumulator,
    total: TimingAccumulator,
}

impl ComponentAccumulator {
    fn record(&mut self, timing: StepComponentTimings) {
        self.neural_compute.record(timing.neural_compute_ns);
        self.intent_build.record(timing.intent_build_ns);
        self.controller.record(timing.controller_ns);
        self.telemetry.record(timing.telemetry_ns);
        self.summary_hash.record(timing.summary_hash_ns);
        self.total.record(timing.total_ns);
    }

    fn report(&self) -> ComponentTimingReport {
        ComponentTimingReport {
            neural_compute: self.neural_compute.report(),
            intent_build: self.intent_build.report(),
            controller: self.controller.report(),
            telemetry: self.telemetry.report(),
            summary_hash: self.summary_hash.report(),
            total: self.total.report(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ExperimentReport {
    schema_version: u32,
    status: String,
    classification: String,
    profile: String,
    controller: String,
    parameter_set_id: Option<String>,
    parameter_sha256: Option<String>,
    executable_authority_sha256: String,
    model_step_ms: u32,
    scenario_count: usize,
    seed_count: usize,
    repeats_per_seed: usize,
    modeled_seconds_per_run: u32,
    modeled_frames_per_run: u64,
    run_count: usize,
    exact_repeat_groups: usize,
    all_repeat_groups_equal: bool,
    invariant_violations: u64,
    timings: ComponentTimingReport,
    runs: Vec<RunRecord>,
    claim_boundary: String,
}

fn main() {
    if let Err(error) = execute() {
        eprintln!("N41_EXPERIMENT_ERROR={error}");
        std::process::exit(1);
    }
}

fn execute() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let (config, output, authority) = Config::parse(&args)?;
    let frames = config.frames();
    let early_frames = (30_000_u64).div_ceil(u64::from(mechofly_core::MODEL_STEP_MS));
    let late_frames = (300_000_u64).div_ceil(u64::from(mechofly_core::MODEL_STEP_MS));
    let mut runs = Vec::with_capacity(SCENARIOS.len() * config.seeds * config.repeats);
    let mut timing = ComponentAccumulator::default();
    let total_runs = SCENARIOS.len() * config.seeds * config.repeats;

    for scenario in SCENARIOS {
        for seed_index in 0..config.seeds {
            let graph = Arc::new(ModelGraph::synthetic(
                ModelTier::Demo4096,
                0xB453_1A00_u64 ^ seed_index as u64,
            ));
            graph.validate()?;
            for repeat in 0..config.repeats {
                let deterministic = run_one(
                    config.profile,
                    scenario,
                    seed_index,
                    frames,
                    early_frames,
                    late_frames,
                    Arc::clone(&graph),
                    &mut timing,
                )?;
                let signature = sha256_hex([
                    serde_json::to_vec(&deterministic).map_err(|error| error.to_string())?
                ]);
                runs.push(RunRecord {
                    repeat,
                    deterministic_signature_sha256: signature,
                    deterministic,
                });
                println!(
                    "N41_PROGRESS profile={} completed_runs={} total_runs={} scenario={} seed={} repeat={}",
                    config.profile.cli(),
                    runs.len(),
                    total_runs,
                    scenario.label(),
                    seed_index,
                    repeat
                );
            }
        }
    }

    let mut exact_repeat_groups = 0;
    let mut all_repeat_groups_equal = true;
    for scenario in SCENARIOS {
        for seed_index in 0..config.seeds {
            let group: Vec<_> = runs
                .iter()
                .filter(|run| {
                    run.deterministic.scenario == scenario.label()
                        && run.deterministic.seed_index == seed_index
                })
                .collect();
            let equal = group.len() == config.repeats
                && group.first().is_some_and(|first| {
                    group.iter().all(|run| {
                        run.deterministic_signature_sha256 == first.deterministic_signature_sha256
                    })
                });
            all_repeat_groups_equal &= equal;
            exact_repeat_groups += usize::from(equal);
        }
    }

    let invariant_violations = runs
        .iter()
        .map(|run| {
            run.deterministic.missed_loom_preemptions
                + run.deterministic.minimum_dwell_violations
                + run.deterministic.grooming_floor_violations
                + run.deterministic.controller_faults
                + run.deterministic.zero_input_escape_frames
                + run.deterministic.periodic_schedule_events
        })
        .sum();
    let (parameter_set_id, parameter_sha256) = match config.profile {
        ExperimentProfile::N3 => (None, None),
        ExperimentProfile::Duration(profile) => {
            let p = parameters_for_profile(profile);
            (
                Some(p.parameter_set_id.clone()),
                Some(parameter_sha256_for(profile).to_owned()),
            )
        }
    };
    let passed = all_repeat_groups_equal && invariant_violations == 0;
    let report = ExperimentReport {
        schema_version: 1,
        status: if passed { "PASS" } else { "FAIL" }.to_owned(),
        classification: "offline_matched_controller_experiment_not_deployment".to_owned(),
        profile: config.profile.cli().to_owned(),
        controller: config.profile.controller().to_owned(),
        parameter_set_id,
        parameter_sha256,
        executable_authority_sha256: authority,
        model_step_ms: mechofly_core::MODEL_STEP_MS,
        scenario_count: SCENARIOS.len(),
        seed_count: config.seeds,
        repeats_per_seed: config.repeats,
        modeled_seconds_per_run: config.seconds,
        modeled_frames_per_run: frames,
        run_count: runs.len(),
        exact_repeat_groups,
        all_repeat_groups_equal,
        invariant_violations,
        timings: timing.report(),
        runs,
        claim_boundary: "N4.1 candidates are modeled product-engineering experiments; no biological fit, runtime promotion, shortcut change, or deployment is authorized by this report".to_owned(),
    };
    write_atomic_json(Path::new(&output), &report)?;
    println!(
        "N41_EXPERIMENT={} profile={} runs={} exact_groups={}",
        report.status, report.profile, report.run_count, report.exact_repeat_groups
    );
    if passed {
        Ok(())
    } else {
        Err("experiment invariant or exact-repeat gate failed".into())
    }
}

#[allow(clippy::too_many_arguments)]
fn run_one(
    profile: ExperimentProfile,
    scenario: Scenario,
    seed_index: usize,
    frames: u64,
    early_frames: u64,
    late_frames: u64,
    graph: Arc<ModelGraph>,
    timing: &mut ComponentAccumulator,
) -> Result<DeterministicRun, String> {
    let model_seed = 0x7E1E_0000_u64 ^ seed_index as u64;
    let mut engine = match profile {
        ExperimentProfile::N3 => ModelEngine::new(Arc::clone(&graph), model_seed),
        ExperimentProfile::Duration(profile) => {
            ModelEngine::new_duration_aware_with_profile(Arc::clone(&graph), model_seed, profile)
        }
    };
    let initial_state_digest = engine.state.digest();
    let mut stimulus = engine.empty_stimulus();
    let mut occupancy = BTreeMap::new();
    let mut early = BTreeMap::new();
    let mut late = BTreeMap::new();
    let mut reasons = BTreeMap::new();
    let mut observed_sequence = None;
    let mut missed_loom = 0_u64;
    let mut premature = 0_u64;
    let mut groom_short = 0_u64;
    let mut faults = 0_u64;
    let mut zero_input_escapes = 0_u64;
    let mut periodic_schedule_events = 0_u64;
    let mut walk_drive_frames = 0_u64;
    let mut walk_during_drive_frames = 0_u64;
    let mut unresponsive = 0_u64;
    let mut maximum_unresponsive = 0_u64;

    for frame in 0..frames {
        prepare_stimulus(scenario, frame, &mut stimulus);
        let walk_drive = stimulus
            .iter()
            .skip(WALK_POPULATION_OFFSET)
            .step_by(FUNCTIONAL_POPULATION_COUNT)
            .any(|value| *value > 0);
        let before = engine.state.behavior;
        let (summary, step_timing) = engine.step_cpu_profiled(StepInput {
            stimulus_q15: &stimulus,
        });
        timing.record(step_timing);

        record_behavior(&mut occupancy, summary.behavior);
        if frame < early_frames {
            record_behavior(&mut early, summary.behavior);
        }
        if frame >= frames.saturating_sub(late_frames) {
            record_behavior(&mut late, summary.behavior);
        }
        if walk_drive {
            walk_drive_frames += 1;
            if summary.behavior == Behavior::Walk {
                walk_during_drive_frames += 1;
                unresponsive = 0;
            } else {
                unresponsive += 1;
                maximum_unresponsive = maximum_unresponsive.max(unresponsive);
            }
        } else {
            unresponsive = 0;
        }
        if scenario == Scenario::QuietRest && airborne(summary.behavior) {
            zero_input_escapes += 1;
        }

        if let Some(state) = &engine.state.behavior_dynamics {
            if state.fault_latched {
                faults += 1;
            }
            let p = parameters_for_profile(
                mechofly_core::behavior_parameters::profile_for_parameter_sha256(
                    &state.parameter_sha256,
                )
                .ok_or("unknown controller profile during experiment")?,
            );
            if !airborne(before)
                && engine.last_behavior_intent.loom_activation_q15 >= p.loom_on_q15
                && summary.behavior != Behavior::PreEscape
            {
                missed_loom += 1;
            }
        }

        if let Some(event) = engine.latest_behavior_transition()
            && observed_sequence != Some(event.sequence)
        {
            if let Some(details) = &event.dynamics {
                if !event.emergency_override && event.elapsed_frames < details.minimum_dwell_frames
                {
                    premature += 1;
                }
                if event.from_behavior == Behavior::Groom
                    && !event.emergency_override
                    && event.elapsed_ms < 1_500
                {
                    groom_short += 1;
                }
            }
            if matches!(profile, ExperimentProfile::Duration(_))
                && event.reason == BehaviorTransitionReason::LegacyAutonomousSchedule
            {
                periodic_schedule_events += 1;
            }
            *reasons.entry(event.reason.as_str().to_owned()).or_insert(0) += 1;
            observed_sequence = Some(event.sequence);
        }
    }

    if matches!(profile, ExperimentProfile::Duration(_)) {
        engine
            .state
            .behavior_dynamics
            .as_ref()
            .ok_or("duration controller disappeared")?
            .validate(
                model_seed,
                engine.state.frame,
                engine.state.behavior,
                engine.state.behavior_age_frames,
            )?;
    }
    let activation: Vec<u8> = engine
        .state
        .activation
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    let (final_fatigue_q15, final_gain) = match &engine.state.behavior_dynamics {
        Some(state) => (
            Some(state.context.fatigue_q15),
            Some(state.fatigue_response_gain_q15()?),
        ),
        None => (None, None),
    };
    Ok(DeterministicRun {
        scenario: scenario.label().to_owned(),
        seed_index,
        model_seed,
        graph_sha256: graph.identity.sha256.clone(),
        modeled_frames: frames,
        initial_state_digest,
        final_state_digest: engine.state.digest(),
        final_neural_sha256: sha256_hex([activation.as_slice(), engine.state.spikes.as_slice()]),
        occupancy_frames: occupancy,
        early_30s_occupancy_frames: early,
        late_5m_occupancy_frames: late,
        transition_count: engine.behavior_telemetry_total_event_count(),
        transition_reasons: reasons,
        maximum_walk_drive_unresponsive_frames: maximum_unresponsive,
        walk_drive_frames,
        walk_during_drive_frames,
        missed_loom_preemptions: missed_loom,
        minimum_dwell_violations: premature,
        grooming_floor_violations: groom_short,
        controller_faults: faults,
        zero_input_escape_frames: zero_input_escapes,
        periodic_schedule_events,
        final_fatigue_q15,
        final_fatigue_response_gain_q15: final_gain,
    })
}

fn record_behavior(map: &mut BTreeMap<String, u64>, behavior: Behavior) {
    *map.entry(format!("{behavior:?}")).or_insert(0) += 1;
}

fn write_atomic_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    fs::write(&temporary, [bytes.as_slice(), b"\n"].concat()).map_err(|error| error.to_string())?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    fs::rename(temporary, path).map_err(|error| error.to_string())
}

fn option<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter()
        .position(|argument| argument == key)
        .and_then(|index| args.get(index + 1))
        .map(String::as_str)
}

fn bounded_usize(
    args: &[String],
    key: &str,
    default: usize,
    minimum: usize,
    maximum: usize,
) -> Result<usize, String> {
    let value = option(args, key)
        .map(str::parse::<usize>)
        .transpose()
        .map_err(|error| format!("invalid {key}: {error}"))?
        .unwrap_or(default);
    if !(minimum..=maximum).contains(&value) {
        return Err(format!("{key} must be between {minimum} and {maximum}"));
    }
    Ok(value)
}

fn bounded_u32(
    args: &[String],
    key: &str,
    default: u32,
    minimum: u32,
    maximum: u32,
) -> Result<u32, String> {
    let value = option(args, key)
        .map(str::parse::<u32>)
        .transpose()
        .map_err(|error| format!("invalid {key}: {error}"))?
        .unwrap_or(default);
    if !(minimum..=maximum).contains(&value) {
        return Err(format!("{key} must be between {minimum} and {maximum}"));
    }
    Ok(value)
}
