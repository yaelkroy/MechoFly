//! N6 foundation: an exact, serializable checkpoint for state that determines
//! the next modeled and visible frame.
//!
//! This module is compile-time gated. It does not replace the canonical replay
//! buffer, alter behavior selection, or restore into the live application. A
//! checkpoint may only create discarded CPU branches until later N6 review.

use std::{fs, path::Path};

use eframe::egui::{Pos2, Vec2};
use mechofly_core::{
    Action, Behavior, FrameSummary, PetPolicy, PolicyContext,
    grooming_program::{GroomingProgramFrame, grooming_program_at},
    sha256_hex,
};
use serde::{Deserialize, Serialize};

use crate::{
    pet::{PetMotion, PetMotionCheckpoint, Skin},
    runtime::{DiscardedSimulationBranch, SimulationRuntimeCheckpoint, SimulationSession},
};

const PRODUCT_CHECKPOINT_SCHEMA_VERSION: u32 = 1;
const PRODUCT_CHECKPOINT_CLAIM: &str =
    "MODELED_N6_PRODUCT_CHECKPOINT_FOUNDATION_NOT_FULL_N6_COMPLETION";
const PRODUCT_CHECKPOINT_FEATURE: &str = "n6-product-checkpoint";
const ZERO_INTERVENTION_FRAMES: usize = 64;
#[cfg(feature = "n6-counterfactual-replay")]
const COUNTERFACTUAL_REPLAY_SCHEMA_VERSION: u32 = 1;
#[cfg(feature = "n6-counterfactual-replay")]
const COUNTERFACTUAL_REPLAY_CLAIM: &str =
    "MODELED_N6_PAIRED_COUNTERFACTUAL_REPLAY_NOT_LIVE_RESTORE";
#[cfg(feature = "n6-counterfactual-replay")]
const COUNTERFACTUAL_REPLAY_FEATURE: &str = "n6-counterfactual-replay";
#[cfg(feature = "n6-counterfactual-replay")]
const COUNTERFACTUAL_REPLAY_FRAMES: usize = 96;
#[cfg(feature = "n6-counterfactual-replay")]
const COUNTERFACTUAL_REPLAY_HISTORY_LIMIT: usize = 128;
#[cfg(feature = "n6-counterfactual-replay")]
const COUNTERFACTUAL_INTERVENTION_START: usize = 24;
#[cfg(feature = "n6-counterfactual-replay")]
const COUNTERFACTUAL_INTERVENTION_FRAMES: usize = 12;
#[cfg(feature = "n6-counterfactual-replay")]
const COUNTERFACTUAL_INTERVENTION_LOOM_Q15: i32 = 8_192;
#[cfg(feature = "n6-counterfactual-replay")]
const COUNTERFACTUAL_STEP_SECONDS: f32 = 0.033;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductCheckpoint {
    schema_version: u32,
    claim: String,
    runtime: SimulationRuntimeCheckpoint,
    motor: PetMotionCheckpoint,
    grooming_program: Option<GroomingProgramFrame>,
    policy: PetPolicy,
    current_action: Action,
    last_policy_context: PolicyContext,
    seed: u64,
    skin: Skin,
    event_keyed_rng_state_included: bool,
    wall_clock_excluded: bool,
    live_restore_authorized: bool,
}

impl ProductCheckpoint {
    fn capture(
        session: &SimulationSession,
        motor: &PetMotion,
        policy: &PetPolicy,
        current_action: Action,
        last_policy_context: PolicyContext,
        seed: u64,
        skin: Skin,
    ) -> Self {
        let runtime = session.product_runtime_checkpoint();
        Self::from_parts(
            runtime,
            motor.checkpoint(),
            policy.clone(),
            current_action,
            last_policy_context,
            seed,
            skin,
        )
    }

    fn from_parts(
        runtime: SimulationRuntimeCheckpoint,
        motor: PetMotionCheckpoint,
        policy: PetPolicy,
        current_action: Action,
        last_policy_context: PolicyContext,
        seed: u64,
        skin: Skin,
    ) -> Self {
        let grooming_program =
            derived_grooming_program(runtime.state.behavior, runtime.state.behavior_age_frames);
        Self {
            schema_version: PRODUCT_CHECKPOINT_SCHEMA_VERSION,
            claim: PRODUCT_CHECKPOINT_CLAIM.to_owned(),
            runtime,
            motor,
            grooming_program,
            policy,
            current_action,
            last_policy_context,
            seed,
            skin,
            event_keyed_rng_state_included: true,
            wall_clock_excluded: true,
            live_restore_authorized: false,
        }
    }

    fn validate(&self, session: &SimulationSession) -> Result<(), String> {
        if self.schema_version != PRODUCT_CHECKPOINT_SCHEMA_VERSION {
            return Err("unsupported product-checkpoint schema".to_owned());
        }
        if self.claim != PRODUCT_CHECKPOINT_CLAIM {
            return Err("product-checkpoint claim boundary mismatch".to_owned());
        }
        if !self.event_keyed_rng_state_included
            || !self.wall_clock_excluded
            || self.live_restore_authorized
        {
            return Err("product-checkpoint safety boundary mismatch".to_owned());
        }
        self.runtime.validate(&session.graph)?;
        self.motor.validate()?;
        if self.seed != self.runtime.state.seed {
            return Err("product-checkpoint seed mismatch".to_owned());
        }
        let expected_grooming = derived_grooming_program(
            self.runtime.state.behavior,
            self.runtime.state.behavior_age_frames,
        );
        if self.grooming_program != expected_grooming {
            return Err("product-checkpoint grooming state mismatch".to_owned());
        }
        serde_json::to_vec(&self.policy)
            .map_err(|error| format!("cannot encode product policy: {error}"))?;
        Ok(())
    }

    fn encoded_digest(&self) -> Result<String, String> {
        let encoded = serde_json::to_vec(self)
            .map_err(|error| format!("cannot encode product checkpoint: {error}"))?;
        Ok(sha256_hex([encoded]))
    }

    fn digest(&self, session: &SimulationSession) -> Result<String, String> {
        self.validate(session)?;
        self.encoded_digest()
    }
}

fn derived_grooming_program(
    behavior: Behavior,
    behavior_age_frames: u32,
) -> Option<GroomingProgramFrame> {
    (behavior == Behavior::Groom).then(|| grooming_program_at(behavior_age_frames))
}

struct ProductBranch {
    runtime: DiscardedSimulationBranch,
    motor: PetMotion,
    policy: PetPolicy,
    current_action: Action,
    last_policy_context: PolicyContext,
    seed: u64,
    skin: Skin,
}

impl ProductBranch {
    fn from_checkpoint(
        session: &SimulationSession,
        checkpoint: &ProductCheckpoint,
    ) -> Result<Self, String> {
        checkpoint.validate(session)?;
        Ok(Self {
            runtime: session.discarded_branch(&checkpoint.runtime)?,
            motor: PetMotion::from_checkpoint(&checkpoint.motor)?,
            policy: checkpoint.policy.clone(),
            current_action: checkpoint.current_action,
            last_policy_context: checkpoint.last_policy_context,
            seed: checkpoint.seed,
            skin: checkpoint.skin,
        })
    }

    fn step(
        &mut self,
        cursor_loom_q15: i32,
        dt_seconds: f32,
        screen_origin: Pos2,
        screen_size: Vec2,
    ) -> Result<ProductBranchFrame, String> {
        self.runtime.set_cursor_loom_q15(cursor_loom_q15);
        let summary = self.runtime.step();
        self.motor.advance(
            dt_seconds,
            summary.behavior,
            screen_origin,
            screen_size,
            false,
            None,
        );
        let checkpoint = self.checkpoint();
        Ok(ProductBranchFrame {
            summary,
            motor_digest: checkpoint.motor.digest()?,
            product_digest: checkpoint.encoded_digest()?,
        })
    }

    fn checkpoint(&self) -> ProductCheckpoint {
        ProductCheckpoint::from_parts(
            self.runtime.checkpoint(),
            self.motor.checkpoint(),
            self.policy.clone(),
            self.current_action,
            self.last_policy_context,
            self.seed,
            self.skin,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProductBranchFrame {
    summary: FrameSummary,
    motor_digest: String,
    product_digest: String,
}

#[cfg(feature = "n6-counterfactual-replay")]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CounterfactualIntervention {
    kind: String,
    start_offset: usize,
    duration_frames: usize,
    cursor_loom_q15: i32,
    claim: String,
}

#[cfg(feature = "n6-counterfactual-replay")]
impl CounterfactualIntervention {
    fn fixed_loom() -> Self {
        Self {
            kind: "cursor_loom_q15".to_owned(),
            start_offset: COUNTERFACTUAL_INTERVENTION_START,
            duration_frames: COUNTERFACTUAL_INTERVENTION_FRAMES,
            cursor_loom_q15: COUNTERFACTUAL_INTERVENTION_LOOM_Q15,
            claim: "MODELED_CURSOR_LOOM_INPUT_ONLY".to_owned(),
        }
    }

    fn validate(&self, frame_count: usize) -> Result<(), String> {
        if self.kind != "cursor_loom_q15"
            || self.claim != "MODELED_CURSOR_LOOM_INPUT_ONLY"
            || self.duration_frames == 0
            || !(1..=8_192).contains(&self.cursor_loom_q15)
        {
            return Err("counterfactual intervention contract mismatch".to_owned());
        }
        let end = self
            .start_offset
            .checked_add(self.duration_frames)
            .ok_or_else(|| "counterfactual intervention range overflow".to_owned())?;
        if end > frame_count {
            return Err("counterfactual intervention exceeds replay history".to_owned());
        }
        Ok(())
    }

    fn input_at(&self, offset: usize) -> i32 {
        let end = self.start_offset.saturating_add(self.duration_frames);
        if (self.start_offset..end).contains(&offset) {
            self.cursor_loom_q15
        } else {
            0
        }
    }
}

#[cfg(feature = "n6-counterfactual-replay")]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CounterfactualReplayFrame {
    offset: usize,
    model_frame: u64,
    input_cursor_loom_q15: i32,
    behavior: Behavior,
    state_digest: String,
    motor_digest: String,
    product_digest: String,
}

#[cfg(feature = "n6-counterfactual-replay")]
impl CounterfactualReplayFrame {
    fn modeled_output_eq(&self, other: &Self) -> bool {
        self.model_frame == other.model_frame
            && self.behavior == other.behavior
            && self.state_digest == other.state_digest
            && self.motor_digest == other.motor_digest
            && self.product_digest == other.product_digest
    }
}

#[cfg(feature = "n6-counterfactual-replay")]
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CounterfactualReplayLane {
    label: String,
    frames: Vec<CounterfactualReplayFrame>,
    final_product_digest: String,
}

#[cfg(feature = "n6-counterfactual-replay")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CounterfactualReplayCapsule {
    schema_version: u32,
    claim: String,
    feature: String,
    graph: mechofly_core::GraphIdentity,
    seed: u64,
    source_frame: u64,
    source_checkpoint_sha256: String,
    source_checkpoint: ProductCheckpoint,
    common_event_keyed_randomness: bool,
    wall_clock_excluded: bool,
    history_limit_frames: usize,
    frame_count: usize,
    intervention: CounterfactualIntervention,
    actual: CounterfactualReplayLane,
    alternative: CounterfactualReplayLane,
    first_divergence_offset: usize,
    first_divergence_model_frame: u64,
    first_divergence_reason: String,
    live_restore_authorized: bool,
}

#[cfg(feature = "n6-counterfactual-replay")]
impl CounterfactualReplayCapsule {
    fn build(
        session: &SimulationSession,
        checkpoint: &ProductCheckpoint,
        screen_origin: Pos2,
        screen_size: Vec2,
    ) -> Result<Self, String> {
        checkpoint.validate(session)?;
        let intervention = CounterfactualIntervention::fixed_loom();
        intervention.validate(COUNTERFACTUAL_REPLAY_FRAMES)?;
        let actual = record_counterfactual_lane(
            session,
            checkpoint,
            "actual",
            None,
            COUNTERFACTUAL_REPLAY_FRAMES,
            screen_origin,
            screen_size,
        )?;
        let alternative = record_counterfactual_lane(
            session,
            checkpoint,
            "changed_input",
            Some(&intervention),
            COUNTERFACTUAL_REPLAY_FRAMES,
            screen_origin,
            screen_size,
        )?;
        let first_divergence_offset = first_output_divergence(&actual, &alternative)
            .ok_or_else(|| "changed-input replay did not diverge".to_owned())?;
        let first_divergence_model_frame = actual.frames[first_divergence_offset].model_frame;
        let capsule = Self {
            schema_version: COUNTERFACTUAL_REPLAY_SCHEMA_VERSION,
            claim: COUNTERFACTUAL_REPLAY_CLAIM.to_owned(),
            feature: COUNTERFACTUAL_REPLAY_FEATURE.to_owned(),
            graph: session.graph.identity.clone(),
            seed: checkpoint.seed,
            source_frame: checkpoint.runtime.state.frame,
            source_checkpoint_sha256: checkpoint.digest(session)?,
            source_checkpoint: checkpoint.clone(),
            common_event_keyed_randomness: true,
            wall_clock_excluded: true,
            history_limit_frames: COUNTERFACTUAL_REPLAY_HISTORY_LIMIT,
            frame_count: COUNTERFACTUAL_REPLAY_FRAMES,
            intervention,
            actual,
            alternative,
            first_divergence_offset,
            first_divergence_model_frame,
            first_divergence_reason: "changed_cursor_loom_q15_input".to_owned(),
            live_restore_authorized: false,
        };
        capsule.validate(session, screen_origin, screen_size)?;
        Ok(capsule)
    }

    fn validate(
        &self,
        session: &SimulationSession,
        screen_origin: Pos2,
        screen_size: Vec2,
    ) -> Result<(), String> {
        if self.schema_version != COUNTERFACTUAL_REPLAY_SCHEMA_VERSION
            || self.claim != COUNTERFACTUAL_REPLAY_CLAIM
            || self.feature != COUNTERFACTUAL_REPLAY_FEATURE
            || !self.common_event_keyed_randomness
            || !self.wall_clock_excluded
            || self.live_restore_authorized
        {
            return Err("counterfactual replay capsule boundary mismatch".to_owned());
        }
        if self.history_limit_frames != COUNTERFACTUAL_REPLAY_HISTORY_LIMIT
            || self.frame_count != COUNTERFACTUAL_REPLAY_FRAMES
            || self.frame_count > self.history_limit_frames
        {
            return Err("counterfactual replay history is not bounded".to_owned());
        }
        self.intervention.validate(self.frame_count)?;
        self.source_checkpoint.validate(session)?;
        if self.graph != session.graph.identity
            || self.graph != self.source_checkpoint.runtime.graph
            || self.seed != self.source_checkpoint.seed
            || self.source_frame != self.source_checkpoint.runtime.state.frame
            || self.source_checkpoint_sha256 != self.source_checkpoint.digest(session)?
        {
            return Err("counterfactual replay source identity mismatch".to_owned());
        }
        validate_counterfactual_lane(self, &self.actual, "actual", false)?;
        validate_counterfactual_lane(self, &self.alternative, "changed_input", true)?;
        for offset in 0..self.intervention.start_offset {
            if !self.actual.frames[offset].modeled_output_eq(&self.alternative.frames[offset]) {
                return Err("counterfactual replay diverged before intervention".to_owned());
            }
        }
        let observed_first = first_output_divergence(&self.actual, &self.alternative)
            .ok_or_else(|| "counterfactual replay has no output divergence".to_owned())?;
        let expected_model_frame = self
            .source_frame
            .saturating_add(observed_first as u64)
            .saturating_add(1);
        if observed_first != self.intervention.start_offset
            || self.first_divergence_offset != observed_first
            || self.first_divergence_model_frame != expected_model_frame
            || self.first_divergence_reason != "changed_cursor_loom_q15_input"
            || self.actual.frames[observed_first].state_digest
                == self.alternative.frames[observed_first].state_digest
        {
            return Err("counterfactual replay divergence proof mismatch".to_owned());
        }
        let replayed_actual = record_counterfactual_lane(
            session,
            &self.source_checkpoint,
            "actual",
            None,
            self.frame_count,
            screen_origin,
            screen_size,
        )?;
        let replayed_alternative = record_counterfactual_lane(
            session,
            &self.source_checkpoint,
            "changed_input",
            Some(&self.intervention),
            self.frame_count,
            screen_origin,
            screen_size,
        )?;
        if replayed_actual != self.actual || replayed_alternative != self.alternative {
            return Err("counterfactual replay did not re-verify exactly".to_owned());
        }
        Ok(())
    }

    fn encoded_digest(&self) -> Result<String, String> {
        let encoded = serde_json::to_vec(self)
            .map_err(|error| format!("cannot encode counterfactual replay capsule: {error}"))?;
        Ok(sha256_hex([encoded]))
    }
}

#[cfg(feature = "n6-counterfactual-replay")]
fn record_counterfactual_lane(
    session: &SimulationSession,
    checkpoint: &ProductCheckpoint,
    label: &str,
    intervention: Option<&CounterfactualIntervention>,
    frame_count: usize,
    screen_origin: Pos2,
    screen_size: Vec2,
) -> Result<CounterfactualReplayLane, String> {
    let mut branch = ProductBranch::from_checkpoint(session, checkpoint)?;
    let mut frames = Vec::with_capacity(frame_count);
    for offset in 0..frame_count {
        let input_cursor_loom_q15 = intervention.map_or(0, |value| value.input_at(offset));
        let output = branch.step(
            input_cursor_loom_q15,
            COUNTERFACTUAL_STEP_SECONDS,
            screen_origin,
            screen_size,
        )?;
        frames.push(CounterfactualReplayFrame {
            offset,
            model_frame: output.summary.frame,
            input_cursor_loom_q15,
            behavior: output.summary.behavior,
            state_digest: output.summary.state_digest,
            motor_digest: output.motor_digest,
            product_digest: output.product_digest,
        });
    }
    let final_product_digest = branch.checkpoint().digest(session)?;
    Ok(CounterfactualReplayLane {
        label: label.to_owned(),
        frames,
        final_product_digest,
    })
}

#[cfg(feature = "n6-counterfactual-replay")]
fn validate_counterfactual_lane(
    capsule: &CounterfactualReplayCapsule,
    lane: &CounterfactualReplayLane,
    expected_label: &str,
    changed_input: bool,
) -> Result<(), String> {
    if lane.label != expected_label || lane.frames.len() != capsule.frame_count {
        return Err("counterfactual replay lane shape mismatch".to_owned());
    }
    for (offset, frame) in lane.frames.iter().enumerate() {
        let expected_input = if changed_input {
            capsule.intervention.input_at(offset)
        } else {
            0
        };
        let expected_model_frame = capsule
            .source_frame
            .saturating_add(offset as u64)
            .saturating_add(1);
        if frame.offset != offset
            || frame.model_frame != expected_model_frame
            || frame.input_cursor_loom_q15 != expected_input
            || frame.state_digest.len() != 64
            || frame.motor_digest.len() != 64
            || frame.product_digest.len() != 64
        {
            return Err("counterfactual replay lane frame mismatch".to_owned());
        }
    }
    if lane
        .frames
        .last()
        .is_none_or(|frame| frame.product_digest != lane.final_product_digest)
    {
        return Err("counterfactual replay final digest mismatch".to_owned());
    }
    Ok(())
}

#[cfg(feature = "n6-counterfactual-replay")]
fn first_output_divergence(
    actual: &CounterfactualReplayLane,
    alternative: &CounterfactualReplayLane,
) -> Option<usize> {
    actual
        .frames
        .iter()
        .zip(&alternative.frames)
        .position(|(actual, alternative)| !actual.modeled_output_eq(alternative))
}

#[cfg(feature = "n6-counterfactual-replay")]
#[derive(Debug, Serialize)]
struct CounterfactualReplayReceipt {
    schema_version: u32,
    status: &'static str,
    classification: &'static str,
    feature: &'static str,
    source_checkpoint_sha256: String,
    replay_capsule_sha256: String,
    graph_sha256: String,
    seed: u64,
    source_frame: u64,
    frame_count: usize,
    history_limit_frames: usize,
    first_divergence_offset: usize,
    first_divergence_model_frame: u64,
    first_divergence_reason: String,
    actual_final_product_digest: String,
    alternative_final_product_digest: String,
    source_checkpoint_embedded: bool,
    paired_lanes_serialized: bool,
    serialization_round_trip_exact: bool,
    unknown_fields_rejected: bool,
    intervention_tamper_rejected: bool,
    frame_tamper_rejected: bool,
    deterministic_reverification_exact: bool,
    pre_intervention_identity_exact: bool,
    changed_input_only: bool,
    common_event_keyed_randomness: bool,
    history_bounded: bool,
    live_state_digest_unchanged: bool,
    live_policy_digest_unchanged: bool,
    live_motor_digest_unchanged: bool,
    live_restore_authorized: bool,
    controller_or_motor_semantics_changed: bool,
    parameter_json_changed: bool,
    screen_ecology_added: bool,
    food_search_added: bool,
    appdata_write_authorized: bool,
    promotion_authorized: bool,
    deployment_authorized: bool,
    replay_capsule: CounterfactualReplayCapsule,
}

#[derive(Debug, Serialize)]
struct ProductCheckpointReceipt {
    schema_version: u32,
    status: &'static str,
    classification: &'static str,
    feature: &'static str,
    canonical_default_profile: &'static str,
    checkpoint_schema_version: u32,
    checkpoint_sha256: String,
    runtime_checkpoint_sha256: String,
    motor_checkpoint_sha256: String,
    serialization_round_trip_exact: bool,
    unknown_fields_rejected: bool,
    graph_tamper_rejected: bool,
    state_tamper_rejected: bool,
    motor_round_trip_exact: bool,
    grooming_program_state_derived: bool,
    policy_state_serialized: bool,
    sensory_and_authored_drive_state_serialized: bool,
    event_keyed_rng_state_included: bool,
    wall_clock_excluded: bool,
    zero_intervention_frames: usize,
    zero_intervention_branches_identical: bool,
    changed_input_branch_diverged: bool,
    live_state_digest_unchanged: bool,
    live_policy_digest_unchanged: bool,
    live_motor_digest_unchanged: bool,
    live_restore_authorized: bool,
    controller_or_motor_semantics_changed: bool,
    parameter_json_changed: bool,
    screen_ecology_added: bool,
    food_search_added: bool,
    appdata_write_authorized: bool,
    promotion_authorized: bool,
    deployment_authorized: bool,
}

pub(crate) fn run(receipt_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let receipt = evaluate().map_err(std::io::Error::other)?;
    if let Some(parent) = receipt_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(receipt_path, serde_json::to_vec_pretty(&receipt)?)?;
    println!(
        "N6_PRODUCT_CHECKPOINT=PASS schema={} zero_intervention_frames={} live_unchanged=true",
        receipt.checkpoint_schema_version, receipt.zero_intervention_frames
    );
    Ok(())
}

#[cfg(feature = "n6-counterfactual-replay")]
pub(crate) fn run_counterfactual_replay(
    receipt_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let receipt = evaluate_counterfactual_replay().map_err(std::io::Error::other)?;
    if let Some(parent) = receipt_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(receipt_path, serde_json::to_vec_pretty(&receipt)?)?;
    println!(
        "N6_COUNTERFACTUAL_REPLAY=PASS frames={} first_divergence={} live_unchanged=true",
        receipt.frame_count, receipt.first_divergence_offset
    );
    Ok(())
}

#[cfg(feature = "n6-counterfactual-replay")]
fn evaluate_counterfactual_replay() -> Result<CounterfactualReplayReceipt, String> {
    let seed = 0x4D45_4348_4F46_4C59;
    let mut session = SimulationSession::product_checkpoint_fixture(seed);
    let mut motor = PetMotion::default();
    let screen_origin = Pos2::ZERO;
    let screen_size = Vec2::new(1_920.0, 1_080.0);
    for _ in 0..12 {
        session.step();
        motor.advance(
            COUNTERFACTUAL_STEP_SECONDS,
            session.last_summary.behavior,
            screen_origin,
            screen_size,
            false,
            None,
        );
    }
    let policy = PetPolicy::default();
    let current_action = Action::Explore;
    let last_policy_context = PolicyContext {
        behavior: session.last_summary.behavior,
        recent_interaction: false,
    };
    let checkpoint = ProductCheckpoint::capture(
        &session,
        &motor,
        &policy,
        current_action,
        last_policy_context,
        seed,
        Skin::Drosophila,
    );
    checkpoint.validate(&session)?;

    let live_state_before = session.live_digest();
    let live_policy_before = policy.digest();
    let live_motor_before = motor.checkpoint().digest()?;
    let capsule =
        CounterfactualReplayCapsule::build(&session, &checkpoint, screen_origin, screen_size)?;
    let repeated =
        CounterfactualReplayCapsule::build(&session, &checkpoint, screen_origin, screen_size)?;
    let deterministic_reverification_exact = capsule == repeated;
    if !deterministic_reverification_exact {
        return Err("counterfactual replay capsule changed across repeats".to_owned());
    }

    let encoded = serde_json::to_vec(&capsule)
        .map_err(|error| format!("cannot encode counterfactual replay capsule: {error}"))?;
    let decoded: CounterfactualReplayCapsule = serde_json::from_slice(&encoded)
        .map_err(|error| format!("cannot decode counterfactual replay capsule: {error}"))?;
    decoded.validate(&session, screen_origin, screen_size)?;
    let serialization_round_trip_exact = capsule == decoded;
    if !serialization_round_trip_exact {
        return Err("counterfactual replay serialization changed the capsule".to_owned());
    }

    let mut unknown_value: serde_json::Value = serde_json::from_slice(&encoded)
        .map_err(|error| format!("cannot decode replay capsule JSON value: {error}"))?;
    unknown_value
        .as_object_mut()
        .ok_or_else(|| "counterfactual replay capsule did not encode as an object".to_owned())?
        .insert("unexpected_field".to_owned(), serde_json::Value::Bool(true));
    let unknown_fields_rejected =
        serde_json::from_value::<CounterfactualReplayCapsule>(unknown_value).is_err();
    if !unknown_fields_rejected {
        return Err("counterfactual replay accepted an unknown field".to_owned());
    }

    let mut intervention_tamper = capsule.clone();
    intervention_tamper.intervention.cursor_loom_q15 = 0;
    let intervention_tamper_rejected = intervention_tamper
        .validate(&session, screen_origin, screen_size)
        .is_err();
    if !intervention_tamper_rejected {
        return Err("counterfactual replay accepted intervention tampering".to_owned());
    }

    let mut frame_tamper = capsule.clone();
    let frame_tamper_offset = frame_tamper.first_divergence_offset;
    frame_tamper.alternative.frames[frame_tamper_offset]
        .state_digest
        .push('0');
    let frame_tamper_rejected = frame_tamper
        .validate(&session, screen_origin, screen_size)
        .is_err();
    if !frame_tamper_rejected {
        return Err("counterfactual replay accepted frame tampering".to_owned());
    }

    let pre_intervention_identity_exact = (0..capsule.intervention.start_offset).all(|offset| {
        capsule.actual.frames[offset].modeled_output_eq(&capsule.alternative.frames[offset])
    });
    if !pre_intervention_identity_exact {
        return Err("counterfactual replay lost pre-intervention identity".to_owned());
    }
    let changed_input_only = capsule
        .actual
        .frames
        .iter()
        .enumerate()
        .all(|(offset, frame)| {
            frame.input_cursor_loom_q15 == 0
                && capsule.alternative.frames[offset].input_cursor_loom_q15
                    == capsule.intervention.input_at(offset)
        });
    if !changed_input_only {
        return Err(
            "counterfactual replay changed more than the declared input schedule".to_owned(),
        );
    }
    let history_bounded = capsule.frame_count <= capsule.history_limit_frames
        && capsule.actual.frames.len() == capsule.frame_count
        && capsule.alternative.frames.len() == capsule.frame_count;
    if !history_bounded {
        return Err("counterfactual replay history exceeded its bound".to_owned());
    }

    let live_state_digest_unchanged = session.live_digest() == live_state_before;
    let live_policy_digest_unchanged = policy.digest() == live_policy_before;
    let live_motor_digest_unchanged = motor.checkpoint().digest()? == live_motor_before;
    if !(live_state_digest_unchanged && live_policy_digest_unchanged && live_motor_digest_unchanged)
    {
        return Err("counterfactual replay mutated live product state".to_owned());
    }

    let source_checkpoint_sha256 = checkpoint.digest(&session)?;
    let replay_capsule_sha256 = capsule.encoded_digest()?;
    Ok(CounterfactualReplayReceipt {
        schema_version: 1,
        status: "PASS",
        classification: "n6_behavior_neutral_paired_counterfactual_replay",
        feature: COUNTERFACTUAL_REPLAY_FEATURE,
        source_checkpoint_sha256,
        replay_capsule_sha256,
        graph_sha256: capsule.graph.sha256.clone(),
        seed: capsule.seed,
        source_frame: capsule.source_frame,
        frame_count: capsule.frame_count,
        history_limit_frames: capsule.history_limit_frames,
        first_divergence_offset: capsule.first_divergence_offset,
        first_divergence_model_frame: capsule.first_divergence_model_frame,
        first_divergence_reason: capsule.first_divergence_reason.clone(),
        actual_final_product_digest: capsule.actual.final_product_digest.clone(),
        alternative_final_product_digest: capsule.alternative.final_product_digest.clone(),
        source_checkpoint_embedded: true,
        paired_lanes_serialized: true,
        serialization_round_trip_exact,
        unknown_fields_rejected,
        intervention_tamper_rejected,
        frame_tamper_rejected,
        deterministic_reverification_exact,
        pre_intervention_identity_exact,
        changed_input_only,
        common_event_keyed_randomness: capsule.common_event_keyed_randomness,
        history_bounded,
        live_state_digest_unchanged,
        live_policy_digest_unchanged,
        live_motor_digest_unchanged,
        live_restore_authorized: false,
        controller_or_motor_semantics_changed: false,
        parameter_json_changed: false,
        screen_ecology_added: false,
        food_search_added: false,
        appdata_write_authorized: false,
        promotion_authorized: false,
        deployment_authorized: false,
        replay_capsule: capsule,
    })
}

fn evaluate() -> Result<ProductCheckpointReceipt, String> {
    let seed = 0x4D45_4348_4F46_4C59;
    let mut session = SimulationSession::product_checkpoint_fixture(seed);
    let mut motor = PetMotion::default();
    let screen_origin = Pos2::ZERO;
    let screen_size = Vec2::new(1_920.0, 1_080.0);
    for _ in 0..12 {
        session.step();
        motor.advance(
            0.033,
            session.last_summary.behavior,
            screen_origin,
            screen_size,
            false,
            None,
        );
    }
    let policy = PetPolicy::default();
    let current_action = Action::Explore;
    let last_policy_context = PolicyContext {
        behavior: session.last_summary.behavior,
        recent_interaction: false,
    };
    let checkpoint = ProductCheckpoint::capture(
        &session,
        &motor,
        &policy,
        current_action,
        last_policy_context,
        seed,
        Skin::Drosophila,
    );
    checkpoint.validate(&session)?;

    let checkpoint_sha256 = checkpoint.digest(&session)?;
    let runtime_checkpoint_sha256 = checkpoint.runtime.digest(&session.graph)?;
    let motor_checkpoint_sha256 = checkpoint.motor.digest()?;
    let encoded = serde_json::to_vec(&checkpoint)
        .map_err(|error| format!("cannot encode product checkpoint: {error}"))?;
    let decoded: ProductCheckpoint = serde_json::from_slice(&encoded)
        .map_err(|error| format!("cannot decode product checkpoint: {error}"))?;
    let serialization_round_trip_exact = checkpoint == decoded;
    if !serialization_round_trip_exact {
        return Err("product-checkpoint serialization round trip changed state".to_owned());
    }

    let mut unknown_value: serde_json::Value = serde_json::from_slice(&encoded)
        .map_err(|error| format!("cannot decode checkpoint JSON value: {error}"))?;
    unknown_value
        .as_object_mut()
        .ok_or_else(|| "product checkpoint did not encode as an object".to_owned())?
        .insert("unexpected_field".to_owned(), serde_json::Value::Bool(true));
    let unknown_fields_rejected =
        serde_json::from_value::<ProductCheckpoint>(unknown_value).is_err();
    if !unknown_fields_rejected {
        return Err("product checkpoint accepted an unknown field".to_owned());
    }

    let mut graph_tamper = checkpoint.clone();
    graph_tamper.runtime.graph.sha256.push('0');
    let graph_tamper_rejected = graph_tamper.validate(&session).is_err();
    if !graph_tamper_rejected {
        return Err("product checkpoint accepted graph-identity tampering".to_owned());
    }

    let mut state_tamper = checkpoint.clone();
    state_tamper.runtime.state.activation[0] =
        state_tamper.runtime.state.activation[0].saturating_add(1);
    let state_tamper_rejected = state_tamper.validate(&session).is_err();
    if !state_tamper_rejected {
        return Err("product checkpoint accepted state tampering".to_owned());
    }

    let restored_motor = PetMotion::from_checkpoint(&checkpoint.motor)?;
    let motor_round_trip_exact = restored_motor.checkpoint() == checkpoint.motor;
    if !motor_round_trip_exact {
        return Err("pet motor checkpoint did not restore exactly".to_owned());
    }
    let grooming_program_state_derived =
        derived_grooming_program(Behavior::Groom, 53) == Some(grooming_program_at(53));
    if !grooming_program_state_derived {
        return Err("grooming program state was not deterministically derived".to_owned());
    }

    let live_state_before = session.live_digest();
    let live_policy_before = policy.digest();
    let live_motor_before = motor.checkpoint().digest()?;
    let mut branch_a = ProductBranch::from_checkpoint(&session, &checkpoint)?;
    let mut branch_b = ProductBranch::from_checkpoint(&session, &checkpoint)?;
    for _ in 0..ZERO_INTERVENTION_FRAMES {
        let frame_a = branch_a.step(0, 0.033, screen_origin, screen_size)?;
        let frame_b = branch_b.step(0, 0.033, screen_origin, screen_size)?;
        if frame_a != frame_b {
            return Err("zero-intervention product branches diverged".to_owned());
        }
    }
    let zero_intervention_branches_identical = branch_a.checkpoint() == branch_b.checkpoint();
    if !zero_intervention_branches_identical {
        return Err("zero-intervention product checkpoints diverged".to_owned());
    }

    let mut control = ProductBranch::from_checkpoint(&session, &checkpoint)?;
    let mut changed = ProductBranch::from_checkpoint(&session, &checkpoint)?;
    let control_frame = control.step(0, 0.033, screen_origin, screen_size)?;
    let changed_frame = changed.step(8_192, 0.033, screen_origin, screen_size)?;
    let changed_input_branch_diverged =
        control_frame.summary.state_digest != changed_frame.summary.state_digest;
    if !changed_input_branch_diverged {
        return Err("changed-input product branch did not diverge".to_owned());
    }

    let live_state_digest_unchanged = session.live_digest() == live_state_before;
    let live_policy_digest_unchanged = policy.digest() == live_policy_before;
    let live_motor_digest_unchanged = motor.checkpoint().digest()? == live_motor_before;
    if !(live_state_digest_unchanged && live_policy_digest_unchanged && live_motor_digest_unchanged)
    {
        return Err("discarded product branches mutated live state".to_owned());
    }

    Ok(ProductCheckpointReceipt {
        schema_version: 1,
        status: "PASS",
        classification: "n6_behavior_neutral_product_checkpoint_foundation",
        feature: PRODUCT_CHECKPOINT_FEATURE,
        canonical_default_profile: "n4",
        checkpoint_schema_version: PRODUCT_CHECKPOINT_SCHEMA_VERSION,
        checkpoint_sha256,
        runtime_checkpoint_sha256,
        motor_checkpoint_sha256,
        serialization_round_trip_exact,
        unknown_fields_rejected,
        graph_tamper_rejected,
        state_tamper_rejected,
        motor_round_trip_exact,
        grooming_program_state_derived,
        policy_state_serialized: true,
        sensory_and_authored_drive_state_serialized: true,
        event_keyed_rng_state_included: checkpoint.event_keyed_rng_state_included,
        wall_clock_excluded: checkpoint.wall_clock_excluded,
        zero_intervention_frames: ZERO_INTERVENTION_FRAMES,
        zero_intervention_branches_identical,
        changed_input_branch_diverged,
        live_state_digest_unchanged,
        live_policy_digest_unchanged,
        live_motor_digest_unchanged,
        live_restore_authorized: false,
        controller_or_motor_semantics_changed: false,
        parameter_json_changed: false,
        screen_ecology_added: false,
        food_search_added: false,
        appdata_write_authorized: false,
        promotion_authorized: false,
        deployment_authorized: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_checkpoint_round_trip_and_discarded_branch_contract_passes() {
        let receipt = evaluate().expect("N6 product-checkpoint fixture must pass");
        assert_eq!(receipt.status, "PASS");
        assert!(receipt.zero_intervention_branches_identical);
        assert!(receipt.changed_input_branch_diverged);
        assert!(receipt.live_state_digest_unchanged);
        assert!(!receipt.live_restore_authorized);
    }

    #[cfg(feature = "n6-counterfactual-replay")]
    #[test]
    fn paired_counterfactual_replay_is_exact_bounded_and_isolated() {
        let receipt =
            evaluate_counterfactual_replay().expect("N6 counterfactual replay fixture must pass");
        assert_eq!(receipt.status, "PASS");
        assert_eq!(
            receipt.first_divergence_offset,
            COUNTERFACTUAL_INTERVENTION_START
        );
        assert!(receipt.serialization_round_trip_exact);
        assert!(receipt.deterministic_reverification_exact);
        assert!(receipt.pre_intervention_identity_exact);
        assert!(receipt.changed_input_only);
        assert!(receipt.history_bounded);
        assert!(receipt.live_state_digest_unchanged);
        assert!(!receipt.live_restore_authorized);
    }
}
