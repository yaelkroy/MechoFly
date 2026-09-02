//! N7 foundation: deterministic scientific explanation and provenance records
//! derived from the accepted N6 paired replay.
//!
//! This module is compile-time gated and emits an offline receipt only. It does
//! not alter controller or motor semantics and it does not promote data into the
//! live product UI.

use std::{fs, path::Path};

use mechofly_core::{
    Behavior, BehaviorIntentSnapshot,
    behavior_dynamics::{InternalContext, MotorSubstate},
    sha256_hex,
};
use serde::{Deserialize, Serialize};

use crate::product_checkpoint::{
    ScientificControllerFrame, ScientificControllerLane, ScientificReplaySource,
    scientific_replay_source,
};

const SCIENTIFIC_EXPLANATION_SCHEMA_VERSION: u32 = 1;
const SCIENTIFIC_EXPLANATION_FEATURE: &str = "n7-scientific-explanation";
const SCIENTIFIC_EXPLANATION_CLAIM: &str =
    "MODELED_N7_SCIENTIFIC_EXPLANATION_FOUNDATION_NOT_UI_PROMOTION";
const ACCEPTED_R10_REPLAY_CAPSULE_SHA256: &str =
    "d93d88d8ab2b2d0da15605e761adcdb9b303345756ed14c2f2f41e6ddc7c7835";
const ACCEPTED_R10_GRAPH_SHA256: &str =
    "f5d5c0d3d28e3fe7cc75de5129d9fe0bba3789b18601b90848f4043456c8b37a";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum EvidenceClass {
    #[serde(rename = "MEASURED")]
    Measured,
    #[serde(rename = "DERIVED")]
    Derived,
    #[serde(rename = "MODELED")]
    Modeled,
    #[serde(rename = "INFERRED")]
    Inferred,
    #[serde(rename = "AUTHORED")]
    Authored,
    #[serde(rename = "ENGINEERING PRIOR")]
    EngineeringPrior,
    #[serde(rename = "UNRESOLVED")]
    Unresolved,
    #[serde(rename = "PRESENTATION SAFETY OVERRIDE")]
    PresentationSafetyOverride,
}

fn evidence_vocabulary() -> Vec<EvidenceClass> {
    vec![
        EvidenceClass::Measured,
        EvidenceClass::Derived,
        EvidenceClass::Modeled,
        EvidenceClass::Inferred,
        EvidenceClass::Authored,
        EvidenceClass::EngineeringPrior,
        EvidenceClass::Unresolved,
        EvidenceClass::PresentationSafetyOverride,
    ]
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Tagged<T> {
    value: T,
    evidence_class: EvidenceClass,
}

impl<T> Tagged<T> {
    const fn new(value: T, evidence_class: EvidenceClass) -> Self {
        Self {
            value,
            evidence_class,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProvenanceEntry {
    id: String,
    evidence_class: EvidenceClass,
    scope: String,
    statement: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScientificIntervention {
    start_offset: Tagged<usize>,
    duration_frames: Tagged<usize>,
    cursor_loom_q15: Tagged<i32>,
    claim: Tagged<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ControllerExplanationFrame {
    offset: Tagged<usize>,
    model_frame: Tagged<u64>,
    input_cursor_loom_q15: Tagged<i32>,
    macro_state: Tagged<Behavior>,
    substate: Tagged<MotorSubstate>,
    behavior_age_frames: Tagged<u32>,
    entered_at_frame: Tagged<u64>,
    minimum_dwell_frames: Tagged<u32>,
    sampled_target_duration_frames: Tagged<u32>,
    transition_sequence: Tagged<u64>,
    deterministic_duration_draw: Tagged<u64>,
    duration_context_bucket: Tagged<u16>,
    hysteresis_active_intents: Tagged<[bool; 4]>,
    spike_alert_latched: Tagged<bool>,
    refractory_until_frame: Tagged<[u64; 9]>,
    interruptible: Tagged<bool>,
    internal_context: Tagged<InternalContext>,
    population_evidence: Tagged<BehaviorIntentSnapshot>,
    spike_count: Tagged<usize>,
    mean_activation_q15: Tagged<i32>,
    why_state_began: Tagged<String>,
    why_state_persisted: Tagged<String>,
    uncertainty: Tagged<String>,
    state_digest: Tagged<String>,
    motor_digest: Tagged<String>,
    product_digest: Tagged<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BehaviorEpisodeExplanation {
    macro_state: Tagged<Behavior>,
    controller_entered_at_frame: Tagged<u64>,
    visible_start_offset: Tagged<usize>,
    visible_end_offset_inclusive: Tagged<usize>,
    visible_start_model_frame: Tagged<u64>,
    visible_end_model_frame: Tagged<u64>,
    entry_visible_in_window: Tagged<bool>,
    exit_visible_in_window: Tagged<bool>,
    why_began: Tagged<String>,
    why_persisted: Tagged<String>,
    why_ended: Tagged<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScientificControllerLaneExplanation {
    label: Tagged<String>,
    frames: Vec<ControllerExplanationFrame>,
    episodes: Vec<BehaviorEpisodeExplanation>,
    final_product_digest: Tagged<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScientificSafetyBoundary {
    offline_fixture_only: bool,
    live_restore_authorized: bool,
    controller_or_motor_semantics_changed: bool,
    parameter_json_changed: bool,
    live_ui_promoted: bool,
    selected_neuron_ui_added: bool,
    screen_ecology_added: bool,
    food_search_added: bool,
    appdata_write_authorized: bool,
    promotion_authorized: bool,
    deployment_authorized: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScientificExplanationBundle {
    schema_version: u32,
    claim: String,
    feature: String,
    source_replay_capsule_sha256: Tagged<String>,
    source_replay_capsule_serialized_bytes: Tagged<usize>,
    source_checkpoint_sha256: Tagged<String>,
    graph_sha256: Tagged<String>,
    seed: Tagged<u64>,
    source_frame: Tagged<u64>,
    frame_count: Tagged<usize>,
    history_limit_frames: Tagged<usize>,
    intervention: ScientificIntervention,
    actual: ScientificControllerLaneExplanation,
    counterfactual: ScientificControllerLaneExplanation,
    first_divergence_offset: Tagged<usize>,
    first_divergence_model_frame: Tagged<u64>,
    first_divergence_reason: Tagged<String>,
    causal_contrast: Tagged<String>,
    bounded_history_uncertainty: Tagged<String>,
    presentation_safety_override_active: Tagged<bool>,
    evidence_vocabulary: Vec<EvidenceClass>,
    provenance_catalog: Vec<ProvenanceEntry>,
    safety: ScientificSafetyBoundary,
}

impl ScientificExplanationBundle {
    fn from_source(source: &ScientificReplaySource) -> Result<Self, String> {
        validate_source(source)?;
        Ok(Self {
            schema_version: SCIENTIFIC_EXPLANATION_SCHEMA_VERSION,
            claim: SCIENTIFIC_EXPLANATION_CLAIM.to_owned(),
            feature: SCIENTIFIC_EXPLANATION_FEATURE.to_owned(),
            source_replay_capsule_sha256: Tagged::new(
                source.replay_capsule_sha256.clone(),
                EvidenceClass::Derived,
            ),
            source_replay_capsule_serialized_bytes: Tagged::new(
                source.replay_capsule_serialized_bytes,
                EvidenceClass::Measured,
            ),
            source_checkpoint_sha256: Tagged::new(
                source.source_checkpoint_sha256.clone(),
                EvidenceClass::Derived,
            ),
            graph_sha256: Tagged::new(source.graph.sha256.clone(), EvidenceClass::Derived),
            seed: Tagged::new(source.seed, EvidenceClass::Authored),
            source_frame: Tagged::new(source.source_frame, EvidenceClass::Modeled),
            frame_count: Tagged::new(source.frame_count, EvidenceClass::EngineeringPrior),
            history_limit_frames: Tagged::new(
                source.history_limit_frames,
                EvidenceClass::EngineeringPrior,
            ),
            intervention: ScientificIntervention {
                start_offset: Tagged::new(
                    source.intervention_start_offset,
                    EvidenceClass::Authored,
                ),
                duration_frames: Tagged::new(
                    source.intervention_duration_frames,
                    EvidenceClass::Authored,
                ),
                cursor_loom_q15: Tagged::new(
                    source.intervention_cursor_loom_q15,
                    EvidenceClass::Authored,
                ),
                claim: Tagged::new(
                    "modeled cursor-loom input schedule only".to_owned(),
                    EvidenceClass::Authored,
                ),
            },
            actual: explain_lane(&source.actual)?,
            counterfactual: explain_lane(&source.alternative)?,
            first_divergence_offset: Tagged::new(
                source.first_divergence_offset,
                EvidenceClass::Derived,
            ),
            first_divergence_model_frame: Tagged::new(
                source.first_divergence_model_frame,
                EvidenceClass::Derived,
            ),
            first_divergence_reason: Tagged::new(
                source.first_divergence_reason.clone(),
                EvidenceClass::Derived,
            ),
            causal_contrast: Tagged::new(
                "With common event-keyed randomness and identical pre-intervention state, the declared cursor-loom input is the only authored lane difference; the later state difference is therefore attributed to that modeled intervention within this replay fixture."
                    .to_owned(),
                EvidenceClass::Inferred,
            ),
            bounded_history_uncertainty: Tagged::new(
                "The 96-frame replay cannot establish biological validity, empirical calibration, or events outside its retained window; open episode endings remain unresolved."
                    .to_owned(),
                EvidenceClass::Unresolved,
            ),
            presentation_safety_override_active: Tagged::new(
                false,
                EvidenceClass::PresentationSafetyOverride,
            ),
            evidence_vocabulary: evidence_vocabulary(),
            provenance_catalog: provenance_catalog(),
            safety: ScientificSafetyBoundary {
                offline_fixture_only: true,
                live_restore_authorized: false,
                controller_or_motor_semantics_changed: false,
                parameter_json_changed: false,
                live_ui_promoted: false,
                selected_neuron_ui_added: false,
                screen_ecology_added: false,
                food_search_added: false,
                appdata_write_authorized: false,
                promotion_authorized: false,
                deployment_authorized: false,
            },
        })
    }

    fn validate_against(&self, source: &ScientificReplaySource) -> Result<(), String> {
        let expected = Self::from_source(source)?;
        if self != &expected {
            return Err("scientific explanation differs from its deterministic source".to_owned());
        }
        if self.evidence_vocabulary != evidence_vocabulary()
            || self.provenance_catalog.len() != evidence_vocabulary().len()
        {
            return Err("scientific evidence vocabulary is incomplete".to_owned());
        }
        if self
            .actual
            .episodes
            .last()
            .is_none_or(|episode| episode.why_ended.evidence_class != EvidenceClass::Unresolved)
            || self
                .counterfactual
                .episodes
                .last()
                .is_none_or(|episode| episode.why_ended.evidence_class != EvidenceClass::Unresolved)
        {
            return Err("bounded replay fabricated an open episode ending".to_owned());
        }
        Ok(())
    }

    fn encoded_digest(&self) -> Result<String, String> {
        let encoded = serde_json::to_vec(self)
            .map_err(|error| format!("cannot encode scientific explanation: {error}"))?;
        Ok(sha256_hex([encoded]))
    }
}

fn validate_source(source: &ScientificReplaySource) -> Result<(), String> {
    if source.replay_capsule_sha256 != ACCEPTED_R10_REPLAY_CAPSULE_SHA256
        || source.graph.sha256 != ACCEPTED_R10_GRAPH_SHA256
        || source.source_frame != 12
        || source.frame_count != 96
        || source.history_limit_frames != 128
        || source.intervention_start_offset != 24
        || source.intervention_duration_frames != 12
        || source.intervention_cursor_loom_q15 != 8_192
        || source.first_divergence_offset != 24
        || source.first_divergence_model_frame != 37
        || source.first_divergence_reason != "changed_cursor_loom_q15_input"
    {
        return Err("scientific explanation source identity or boundary mismatch".to_owned());
    }
    if source.actual.label != "actual"
        || source.alternative.label != "changed_input"
        || source.actual.frames.len() != source.frame_count
        || source.alternative.frames.len() != source.frame_count
        || source.frame_count > source.history_limit_frames
    {
        return Err("scientific explanation source lane shape mismatch".to_owned());
    }
    let observed_divergence = source
        .actual
        .frames
        .iter()
        .zip(&source.alternative.frames)
        .position(|(actual, alternative)| {
            actual.state_digest != alternative.state_digest
                || actual.motor_digest != alternative.motor_digest
                || actual.product_digest != alternative.product_digest
                || actual.behavior != alternative.behavior
        });
    if observed_divergence != Some(source.first_divergence_offset) {
        return Err("scientific explanation source divergence mismatch".to_owned());
    }
    for (lane, changed_input) in [(&source.actual, false), (&source.alternative, true)] {
        for (offset, frame) in lane.frames.iter().enumerate() {
            let intervention_end = source
                .intervention_start_offset
                .saturating_add(source.intervention_duration_frames);
            let expected_input = if changed_input
                && (source.intervention_start_offset..intervention_end).contains(&offset)
            {
                source.intervention_cursor_loom_q15
            } else {
                0
            };
            if frame.offset != offset
                || frame.model_frame
                    != source
                        .source_frame
                        .saturating_add(offset as u64)
                        .saturating_add(1)
                || frame.input_cursor_loom_q15 != expected_input
                || frame.behavior != frame.controller.current_macro_state
                || frame.state_digest.len() != 64
                || frame.motor_digest.len() != 64
                || frame.product_digest.len() != 64
            {
                return Err("scientific explanation controller frame mismatch".to_owned());
            }
            frame.controller.validate(
                source.seed,
                frame.model_frame,
                frame.behavior,
                frame.controller.elapsed_frames,
            )?;
        }
    }
    if !(source.live_state_digest_unchanged
        && source.live_policy_digest_unchanged
        && source.live_motor_digest_unchanged)
    {
        return Err("scientific explanation source mutated live product state".to_owned());
    }
    Ok(())
}

fn explain_lane(
    lane: &ScientificControllerLane,
) -> Result<ScientificControllerLaneExplanation, String> {
    if lane.frames.is_empty() {
        return Err("cannot explain an empty controller lane".to_owned());
    }
    let frames = lane.frames.iter().map(explain_frame).collect();
    let episodes = explain_episodes(&lane.frames)?;
    Ok(ScientificControllerLaneExplanation {
        label: Tagged::new(lane.label.clone(), EvidenceClass::Derived),
        frames,
        episodes,
        final_product_digest: Tagged::new(
            lane.final_product_digest.clone(),
            EvidenceClass::Derived,
        ),
    })
}

fn explain_frame(frame: &ScientificControllerFrame) -> ControllerExplanationFrame {
    let controller = &frame.controller;
    let why_state_began = format!(
        "entered {:?} at model frame {} because {}",
        controller.current_macro_state,
        controller.entered_at_frame,
        controller.last_transition_reason.as_str()
    );
    let why_state_persisted = if controller.elapsed_frames < controller.minimum_dwell_frames {
        format!(
            "minimum dwell remains active: elapsed {} of {} frames",
            controller.elapsed_frames, controller.minimum_dwell_frames
        )
    } else if controller.elapsed_frames < controller.target_duration_frames {
        format!(
            "sampled review time has not been reached: elapsed {} of {} frames",
            controller.elapsed_frames, controller.target_duration_frames
        )
    } else {
        "the modeled controller retained this state after its current review step".to_owned()
    };
    ControllerExplanationFrame {
        offset: Tagged::new(frame.offset, EvidenceClass::Derived),
        model_frame: Tagged::new(frame.model_frame, EvidenceClass::Modeled),
        input_cursor_loom_q15: Tagged::new(frame.input_cursor_loom_q15, EvidenceClass::Authored),
        macro_state: Tagged::new(frame.behavior, EvidenceClass::Modeled),
        substate: Tagged::new(controller.current_substate, EvidenceClass::Modeled),
        behavior_age_frames: Tagged::new(controller.elapsed_frames, EvidenceClass::Modeled),
        entered_at_frame: Tagged::new(controller.entered_at_frame, EvidenceClass::Modeled),
        minimum_dwell_frames: Tagged::new(
            controller.minimum_dwell_frames,
            EvidenceClass::EngineeringPrior,
        ),
        sampled_target_duration_frames: Tagged::new(
            controller.target_duration_frames,
            EvidenceClass::Modeled,
        ),
        transition_sequence: Tagged::new(controller.transition_sequence, EvidenceClass::Modeled),
        deterministic_duration_draw: Tagged::new(
            controller.deterministic_duration_draw,
            EvidenceClass::Modeled,
        ),
        duration_context_bucket: Tagged::new(
            controller.duration_context_bucket,
            EvidenceClass::Derived,
        ),
        hysteresis_active_intents: Tagged::new(
            controller.active_intents,
            EvidenceClass::Modeled,
        ),
        spike_alert_latched: Tagged::new(
            controller.spike_alert_latched,
            EvidenceClass::Modeled,
        ),
        refractory_until_frame: Tagged::new(
            controller.refractory_until_frame,
            EvidenceClass::Modeled,
        ),
        interruptible: Tagged::new(controller.interruptible, EvidenceClass::Modeled),
        internal_context: Tagged::new(controller.context, EvidenceClass::Modeled),
        population_evidence: Tagged::new(frame.intent, EvidenceClass::Modeled),
        spike_count: Tagged::new(frame.spike_count, EvidenceClass::Modeled),
        mean_activation_q15: Tagged::new(frame.mean_activation_q15, EvidenceClass::Modeled),
        why_state_began: Tagged::new(why_state_began, EvidenceClass::Derived),
        why_state_persisted: Tagged::new(why_state_persisted, EvidenceClass::Derived),
        uncertainty: Tagged::new(
            "controller state is exact for this modeled fixture; biological validity is not established"
                .to_owned(),
            EvidenceClass::Unresolved,
        ),
        state_digest: Tagged::new(frame.state_digest.clone(), EvidenceClass::Derived),
        motor_digest: Tagged::new(frame.motor_digest.clone(), EvidenceClass::Derived),
        product_digest: Tagged::new(frame.product_digest.clone(), EvidenceClass::Derived),
    }
}

fn explain_episodes(
    frames: &[ScientificControllerFrame],
) -> Result<Vec<BehaviorEpisodeExplanation>, String> {
    let first = frames
        .first()
        .ok_or_else(|| "cannot explain episodes from empty history".to_owned())?;
    let mut episodes = Vec::new();
    let mut start = 0usize;
    for index in 1..=frames.len() {
        let boundary = index == frames.len()
            || frames[index].behavior != frames[start].behavior
            || frames[index].controller.entered_at_frame
                != frames[start].controller.entered_at_frame;
        if !boundary {
            continue;
        }
        let start_frame = &frames[start];
        let end_frame = &frames[index - 1];
        let entry_visible = start_frame.controller.entered_at_frame >= first.model_frame;
        let (exit_visible, why_ended) = if index < frames.len() {
            let next = &frames[index];
            (
                true,
                Tagged::new(
                    format!(
                        "controller entered {:?} because {}",
                        next.behavior,
                        next.controller.last_transition_reason.as_str()
                    ),
                    EvidenceClass::Derived,
                ),
            )
        } else {
            (
                false,
                Tagged::new(
                    "episode remained open when the bounded replay window ended".to_owned(),
                    EvidenceClass::Unresolved,
                ),
            )
        };
        episodes.push(BehaviorEpisodeExplanation {
            macro_state: Tagged::new(start_frame.behavior, EvidenceClass::Modeled),
            controller_entered_at_frame: Tagged::new(
                start_frame.controller.entered_at_frame,
                EvidenceClass::Modeled,
            ),
            visible_start_offset: Tagged::new(start, EvidenceClass::Derived),
            visible_end_offset_inclusive: Tagged::new(index - 1, EvidenceClass::Derived),
            visible_start_model_frame: Tagged::new(
                start_frame.model_frame,
                EvidenceClass::Modeled,
            ),
            visible_end_model_frame: Tagged::new(end_frame.model_frame, EvidenceClass::Modeled),
            entry_visible_in_window: Tagged::new(entry_visible, EvidenceClass::Derived),
            exit_visible_in_window: Tagged::new(exit_visible, EvidenceClass::Derived),
            why_began: Tagged::new(
                format!(
                    "controller entry reason: {}",
                    start_frame.controller.last_transition_reason.as_str()
                ),
                EvidenceClass::Derived,
            ),
            why_persisted: Tagged::new(
                "per-frame persistence is explained by minimum dwell, sampled duration, hysteresis, refractory, and interruptibility state"
                    .to_owned(),
                EvidenceClass::Derived,
            ),
            why_ended,
        });
        start = index;
    }
    Ok(episodes)
}

fn provenance_catalog() -> Vec<ProvenanceEntry> {
    vec![
        ProvenanceEntry {
            id: "serialized-source-size".to_owned(),
            evidence_class: EvidenceClass::Measured,
            scope: "receipt construction".to_owned(),
            statement: "byte count measured from the accepted serialized R10 replay capsule"
                .to_owned(),
        },
        ProvenanceEntry {
            id: "digest-and-window-index".to_owned(),
            evidence_class: EvidenceClass::Derived,
            scope: "replay identity".to_owned(),
            statement: "digests, offsets, and episode boundaries derived deterministically"
                .to_owned(),
        },
        ProvenanceEntry {
            id: "controller-and-neural-state".to_owned(),
            evidence_class: EvidenceClass::Modeled,
            scope: "offline replay lanes".to_owned(),
            statement: "state, spike, intent, context, and behavior values are model outputs"
                .to_owned(),
        },
        ProvenanceEntry {
            id: "causal-contrast".to_owned(),
            evidence_class: EvidenceClass::Inferred,
            scope: "paired replay comparison".to_owned(),
            statement: "causal attribution is limited to the controlled replay fixture".to_owned(),
        },
        ProvenanceEntry {
            id: "loom-intervention".to_owned(),
            evidence_class: EvidenceClass::Authored,
            scope: "counterfactual input".to_owned(),
            statement: "the cursor-loom schedule and fixed fixture seed are authored inputs"
                .to_owned(),
        },
        ProvenanceEntry {
            id: "controller-priors".to_owned(),
            evidence_class: EvidenceClass::EngineeringPrior,
            scope: "duration controller".to_owned(),
            statement: "dwell thresholds and history bounds are engineering priors".to_owned(),
        },
        ProvenanceEntry {
            id: "bounded-history-and-biology".to_owned(),
            evidence_class: EvidenceClass::Unresolved,
            scope: "scientific limits".to_owned(),
            statement: "biology, calibration, and events outside the window remain unresolved"
                .to_owned(),
        },
        ProvenanceEntry {
            id: "presentation-safety".to_owned(),
            evidence_class: EvidenceClass::PresentationSafetyOverride,
            scope: "future UI only".to_owned(),
            statement: "no presentation relocation override is active in this offline foundation"
                .to_owned(),
        },
    ]
}

#[derive(Debug, Serialize)]
struct ScientificExplanationReceipt {
    schema_version: u32,
    status: &'static str,
    classification: &'static str,
    feature: &'static str,
    claim: &'static str,
    source_replay_capsule_sha256: String,
    scientific_explanation_bundle_sha256: String,
    graph_sha256: String,
    frame_count_per_lane: usize,
    history_limit_frames: usize,
    actual_episode_count: usize,
    counterfactual_episode_count: usize,
    first_divergence_offset: usize,
    first_divergence_model_frame: u64,
    accepted_r10_capsule_exact: bool,
    actual_and_counterfactual_controller_lanes: bool,
    evidence_vocabulary_complete: bool,
    every_displayed_quantity_tagged: bool,
    why_began_present: bool,
    why_persisted_present: bool,
    why_ended_present: bool,
    minimum_dwell_present: bool,
    sampled_duration_present: bool,
    hysteresis_present: bool,
    refractory_present: bool,
    interruptibility_present: bool,
    uncertainty_not_fabricated: bool,
    serialization_round_trip_exact: bool,
    unknown_fields_rejected: bool,
    causal_statement_tamper_rejected: bool,
    provenance_tag_tamper_rejected: bool,
    deterministic_reverification_exact: bool,
    source_reverification_exact: bool,
    history_bounded: bool,
    live_state_digest_unchanged: bool,
    live_policy_digest_unchanged: bool,
    live_motor_digest_unchanged: bool,
    live_restore_authorized: bool,
    controller_or_motor_semantics_changed: bool,
    parameter_json_changed: bool,
    live_ui_promoted: bool,
    screen_ecology_added: bool,
    food_search_added: bool,
    appdata_write_authorized: bool,
    promotion_authorized: bool,
    deployment_authorized: bool,
    scientific_explanation_bundle: ScientificExplanationBundle,
}

pub(crate) fn run(receipt_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let receipt = evaluate().map_err(std::io::Error::other)?;
    if let Some(parent) = receipt_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(receipt_path, serde_json::to_vec_pretty(&receipt)?)?;
    println!(
        "N7_SCIENTIFIC_EXPLANATION=PASS frames={} actual_episodes={} counterfactual_episodes={} live_unchanged=true ui_promoted=false",
        receipt.frame_count_per_lane,
        receipt.actual_episode_count,
        receipt.counterfactual_episode_count
    );
    Ok(())
}

fn evaluate() -> Result<ScientificExplanationReceipt, String> {
    let source = scientific_replay_source()?;
    let repeated_source = scientific_replay_source()?;
    let source_reverification_exact = source == repeated_source;
    if !source_reverification_exact {
        return Err("N7 explanation source changed across repeats".to_owned());
    }

    let bundle = ScientificExplanationBundle::from_source(&source)?;
    bundle.validate_against(&source)?;
    let repeated_bundle = ScientificExplanationBundle::from_source(&source)?;
    let deterministic_reverification_exact = bundle == repeated_bundle;
    if !deterministic_reverification_exact {
        return Err("N7 scientific explanation changed across repeats".to_owned());
    }

    let encoded = serde_json::to_vec(&bundle)
        .map_err(|error| format!("cannot encode scientific explanation bundle: {error}"))?;
    let decoded: ScientificExplanationBundle = serde_json::from_slice(&encoded)
        .map_err(|error| format!("cannot decode scientific explanation bundle: {error}"))?;
    decoded.validate_against(&source)?;
    let serialization_round_trip_exact = bundle == decoded;
    if !serialization_round_trip_exact {
        return Err("N7 explanation serialization changed the bundle".to_owned());
    }

    let mut unknown_value: serde_json::Value = serde_json::from_slice(&encoded)
        .map_err(|error| format!("cannot decode explanation JSON value: {error}"))?;
    unknown_value
        .as_object_mut()
        .ok_or_else(|| "scientific explanation did not encode as an object".to_owned())?
        .insert("unexpected_field".to_owned(), serde_json::Value::Bool(true));
    let unknown_fields_rejected =
        serde_json::from_value::<ScientificExplanationBundle>(unknown_value).is_err();
    if !unknown_fields_rejected {
        return Err("N7 explanation accepted an unknown field".to_owned());
    }

    let mut causal_tamper = bundle.clone();
    causal_tamper.causal_contrast.value = "unbounded biological causation".to_owned();
    let causal_statement_tamper_rejected = causal_tamper.validate_against(&source).is_err();
    if !causal_statement_tamper_rejected {
        return Err("N7 explanation accepted causal-statement tampering".to_owned());
    }

    let mut provenance_tamper = bundle.clone();
    provenance_tamper.actual.frames[0]
        .minimum_dwell_frames
        .evidence_class = EvidenceClass::Measured;
    let provenance_tag_tamper_rejected = provenance_tamper.validate_against(&source).is_err();
    if !provenance_tag_tamper_rejected {
        return Err("N7 explanation accepted provenance-tag tampering".to_owned());
    }

    let evidence_vocabulary_complete = bundle.evidence_vocabulary == evidence_vocabulary();
    let uncertainty_not_fabricated = bundle.bounded_history_uncertainty.evidence_class
        == EvidenceClass::Unresolved
        && bundle
            .actual
            .episodes
            .last()
            .is_some_and(|episode| episode.why_ended.evidence_class == EvidenceClass::Unresolved)
        && bundle
            .counterfactual
            .episodes
            .last()
            .is_some_and(|episode| episode.why_ended.evidence_class == EvidenceClass::Unresolved);
    let episodes = bundle
        .actual
        .episodes
        .iter()
        .chain(&bundle.counterfactual.episodes);
    let why_began_present = episodes
        .clone()
        .all(|episode| !episode.why_began.value.is_empty());
    let why_persisted_present = episodes
        .clone()
        .all(|episode| !episode.why_persisted.value.is_empty());
    let why_ended_present = episodes
        .clone()
        .all(|episode| !episode.why_ended.value.is_empty());
    let history_bounded = source.frame_count <= source.history_limit_frames;
    if !(evidence_vocabulary_complete
        && uncertainty_not_fabricated
        && why_began_present
        && why_persisted_present
        && why_ended_present
        && history_bounded)
    {
        return Err("N7 explanation completeness gate failed".to_owned());
    }

    Ok(ScientificExplanationReceipt {
        schema_version: 1,
        status: "PASS",
        classification: "n7_behavior_neutral_scientific_explanation_foundation",
        feature: SCIENTIFIC_EXPLANATION_FEATURE,
        claim: SCIENTIFIC_EXPLANATION_CLAIM,
        source_replay_capsule_sha256: source.replay_capsule_sha256.clone(),
        scientific_explanation_bundle_sha256: bundle.encoded_digest()?,
        graph_sha256: source.graph.sha256.clone(),
        frame_count_per_lane: source.frame_count,
        history_limit_frames: source.history_limit_frames,
        actual_episode_count: bundle.actual.episodes.len(),
        counterfactual_episode_count: bundle.counterfactual.episodes.len(),
        first_divergence_offset: source.first_divergence_offset,
        first_divergence_model_frame: source.first_divergence_model_frame,
        accepted_r10_capsule_exact: true,
        actual_and_counterfactual_controller_lanes: true,
        evidence_vocabulary_complete,
        every_displayed_quantity_tagged: true,
        why_began_present,
        why_persisted_present,
        why_ended_present,
        minimum_dwell_present: true,
        sampled_duration_present: true,
        hysteresis_present: true,
        refractory_present: true,
        interruptibility_present: true,
        uncertainty_not_fabricated,
        serialization_round_trip_exact,
        unknown_fields_rejected,
        causal_statement_tamper_rejected,
        provenance_tag_tamper_rejected,
        deterministic_reverification_exact,
        source_reverification_exact,
        history_bounded,
        live_state_digest_unchanged: source.live_state_digest_unchanged,
        live_policy_digest_unchanged: source.live_policy_digest_unchanged,
        live_motor_digest_unchanged: source.live_motor_digest_unchanged,
        live_restore_authorized: false,
        controller_or_motor_semantics_changed: false,
        parameter_json_changed: false,
        live_ui_promoted: false,
        screen_ecology_added: false,
        food_search_added: false,
        appdata_write_authorized: false,
        promotion_authorized: false,
        deployment_authorized: false,
        scientific_explanation_bundle: bundle,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scientific_explanation_is_tagged_deterministic_and_behavior_neutral() {
        let receipt = evaluate().expect("N7 scientific-explanation fixture must pass");
        assert_eq!(receipt.status, "PASS");
        assert_eq!(receipt.frame_count_per_lane, 96);
        assert_eq!(receipt.first_divergence_offset, 24);
        assert!(receipt.evidence_vocabulary_complete);
        assert!(receipt.uncertainty_not_fabricated);
        assert!(receipt.deterministic_reverification_exact);
        assert!(receipt.live_state_digest_unchanged);
        assert!(!receipt.live_ui_promoted);
        assert!(!receipt.deployment_authorized);
    }
}
