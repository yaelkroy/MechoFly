use std::{collections::BTreeMap, env, fs, path::Path, sync::Arc};

use mechofly_core::{
    Behavior, ModelEngine, ModelGraph, ModelTier, StepInput,
    behavior_parameters::{BehaviorParameterProfile, parameter_sha256_for},
    provenance::sha256_hex,
};
use serde::Serialize;

const APP_MODEL_SEED: u64 = 0x4D45_4348_4F46_4C59;
const APP_GRAPH_SEED_XOR: u64 = 0x47A9_2D31;

#[derive(Serialize)]
struct TransitionSample {
    sequence: u64,
    frame: u64,
    modeled_seconds: f64,
    from: String,
    to: String,
    reason: String,
    elapsed_ms: u64,
}

#[derive(Serialize)]
struct Report {
    schema_version: u32,
    status: String,
    classification: String,
    profile: String,
    parameter_sha256: String,
    tier: String,
    graph_seed: u64,
    model_seed: u64,
    graph_sha256: String,
    modeled_seconds: u32,
    modeled_frames: u64,
    occupancy_frames: BTreeMap<String, u64>,
    transition_reasons: BTreeMap<String, u64>,
    transitions: Vec<TransitionSample>,
    grooming_bouts: u64,
    grooming_frames: u64,
    first_grooming_seconds: Option<f64>,
    longest_grooming_frames: u64,
    longest_walking_frames: u64,
    final_state_digest: String,
    deterministic_signature_sha256: String,
    claim_boundary: String,
}

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    let output = option(&args, "--output").ok_or("--output is required")?;
    let profile =
        BehaviorParameterProfile::parse(option(&args, "--profile").unwrap_or("n41-b-natural"))?;
    let tier = parse_tier(option(&args, "--tier").unwrap_or("extended-65536"))?;
    let seconds = option(&args, "--seconds")
        .unwrap_or("600")
        .parse::<u32>()
        .map_err(|error| format!("invalid --seconds: {error}"))?;
    if !(30..=1_800).contains(&seconds) {
        return Err("--seconds must be between 30 and 1800".into());
    }

    let graph_seed = APP_MODEL_SEED ^ APP_GRAPH_SEED_XOR;
    let graph = Arc::new(ModelGraph::synthetic(tier, graph_seed));
    let mut engine =
        ModelEngine::new_duration_aware_with_profile(Arc::clone(&graph), APP_MODEL_SEED, profile);
    let stimulus = engine.empty_stimulus();
    let frames = u64::from(seconds)
        .saturating_mul(1_000)
        .div_ceil(u64::from(mechofly_core::MODEL_STEP_MS));
    let mut occupancy = BTreeMap::new();
    let mut reasons = BTreeMap::new();
    let mut transitions = Vec::new();
    let mut observed_sequence = None;
    let mut current_behavior = engine.state.behavior;
    let mut current_run = 0_u64;
    let mut longest_groom = 0_u64;
    let mut longest_walk = 0_u64;
    let mut grooming_bouts = 0_u64;
    let mut first_grooming_frame = None;

    for _ in 0..frames {
        let summary = engine.step_cpu(StepInput {
            stimulus_q15: &stimulus,
        });
        *occupancy
            .entry(format!("{:?}", summary.behavior))
            .or_insert(0) += 1;
        if summary.behavior == current_behavior {
            current_run += 1;
        } else {
            if current_behavior == Behavior::Groom {
                longest_groom = longest_groom.max(current_run);
            }
            if current_behavior == Behavior::Walk {
                longest_walk = longest_walk.max(current_run);
            }
            current_behavior = summary.behavior;
            current_run = 1;
        }
        if let Some(event) = engine.latest_behavior_transition()
            && observed_sequence != Some(event.sequence)
        {
            if event.to_behavior == Behavior::Groom {
                grooming_bouts += 1;
                first_grooming_frame.get_or_insert(event.frame);
            }
            *reasons.entry(event.reason.as_str().to_owned()).or_insert(0) += 1;
            transitions.push(TransitionSample {
                sequence: event.sequence,
                frame: event.frame,
                modeled_seconds: event.frame as f64 * f64::from(mechofly_core::MODEL_STEP_MS)
                    / 1_000.0,
                from: format!("{:?}", event.from_behavior),
                to: format!("{:?}", event.to_behavior),
                reason: event.reason.as_str().to_owned(),
                elapsed_ms: event.elapsed_ms,
            });
            observed_sequence = Some(event.sequence);
        }
    }
    if current_behavior == Behavior::Groom {
        longest_groom = longest_groom.max(current_run);
    }
    if current_behavior == Behavior::Walk {
        longest_walk = longest_walk.max(current_run);
    }
    let grooming_frames = occupancy.get("Groom").copied().unwrap_or(0);
    let deterministic_bytes =
        serde_json::to_vec(&(&occupancy, &reasons, &transitions, engine.state.digest()))
            .map_err(|error| format!("cannot serialize deterministic signature: {error}"))?;
    let report = Report {
        schema_version: 1,
        status: "PASS".into(),
        classification: "exact_application_seed_empty_input_runtime_tier_probe".into(),
        profile: profile.cli().into(),
        parameter_sha256: parameter_sha256_for(profile).into(),
        tier: tier.label().into(),
        graph_seed,
        model_seed: APP_MODEL_SEED,
        graph_sha256: graph.identity.sha256.clone(),
        modeled_seconds: seconds,
        modeled_frames: frames,
        occupancy_frames: occupancy,
        transition_reasons: reasons,
        transitions,
        grooming_bouts,
        grooming_frames,
        first_grooming_seconds: first_grooming_frame.map(|frame| {
            frame as f64 * f64::from(mechofly_core::MODEL_STEP_MS) / 1_000.0
        }),
        longest_grooming_frames: longest_groom,
        longest_walking_frames: longest_walk,
        final_state_digest: engine.state.digest(),
        deterministic_signature_sha256: sha256_hex([deterministic_bytes]),
        claim_boundary: "engineering probe of the exact application seed and graph tier; not a biological calibration or deployment authorization".into(),
    };
    write_atomic_json(Path::new(output), &report)?;
    println!(
        "N41_LIVE_PROFILE=PASS tier={} groom_bouts={} groom_seconds={:.3}",
        report.tier,
        report.grooming_bouts,
        report.grooming_frames as f64 * f64::from(mechofly_core::MODEL_STEP_MS) / 1_000.0
    );
    Ok(())
}

fn parse_tier(value: &str) -> Result<ModelTier, String> {
    match value {
        "demo-4096" => Ok(ModelTier::Demo4096),
        "standard-12615" => Ok(ModelTier::Standard12615),
        "extended-65536" => Ok(ModelTier::Extended65536),
        _ => Err(format!(
            "unknown --tier {value:?}; expected demo-4096, standard-12615, or extended-65536"
        )),
    }
}

fn option<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    args.iter()
        .position(|argument| argument == key)
        .and_then(|index| args.get(index + 1))
        .map(String::as_str)
}

fn write_atomic_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("cannot serialize {}: {error}", path.display()))?;
    let temporary = path.with_extension("json.partial");
    fs::write(&temporary, bytes)
        .map_err(|error| format!("cannot write {}: {error}", temporary.display()))?;
    fs::rename(&temporary, path).map_err(|error| format!("cannot seal {}: {error}", path.display()))
}
