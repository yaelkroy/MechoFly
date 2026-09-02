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
}
