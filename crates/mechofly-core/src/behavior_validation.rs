//! Executable controller invariants; shared by self-test and the Windows gates.
use serde::Serialize;

use crate::{
    behavior_dynamics::{BehaviorDynamicsState, airborne},
    behavior_intent::{BehaviorContext, BehaviorIntentBuilder, BehaviorIntentSnapshot},
    behavior_parameters::{duration_draw, parameter_sha256, parameters},
    behavior_telemetry::BehaviorTransitionReason,
    model::Behavior,
    neural_evidence::NeuralEvidence,
};

#[derive(Clone, Debug, Serialize)]
pub struct DynamicsValidation {
    pub passed: bool,
    pub parameter_sha256: String,
    pub controller_steps: u64,
    pub transitions: u64,
    pub minimum_dwell_violations: u64,
    pub grooming_floor_violations: u64,
    pub missed_loom_preemptions: u64,
    pub zero_input_escapes: u64,
    pub periodic_schedule_events: u64,
    pub duration_draw_cases: u64,
    pub serialization_next_transition_checks: u64,
    pub invalid_state_fail_closed: bool,
    pub bounded_context: bool,
    pub state_inline_bytes: usize,
    pub scientific_claim: String,
}

pub fn evidence_for_state(
    state: &BehaviorDynamicsState,
    values: &[i32],
    spikes: usize,
) -> BehaviorIntentSnapshot {
    let evidence = NeuralEvidence::collect(state.last_frame.saturating_add(1), values, spikes);
    BehaviorIntentBuilder::build_duration_aware(
        &evidence,
        BehaviorContext {
            current_behavior: state.current_macro_state,
            current_behavior_age_frames: state.elapsed_frames,
        },
    )
}

pub fn fresh_state(seed: u64) -> BehaviorDynamicsState {
    let evidence = NeuralEvidence::collect(0, &[0; 9], 0);
    let intent = BehaviorIntentBuilder::build_duration_aware(
        &evidence,
        BehaviorContext {
            current_behavior: Behavior::Quiet,
            current_behavior_age_frames: 0,
        },
    );
    BehaviorDynamicsState::new(seed, intent)
}

pub fn validate_dynamics() -> Result<DynamicsValidation, String> {
    parameters().validate()?;
    let mut result = DynamicsValidation {
        passed: false,
        parameter_sha256: parameter_sha256().to_owned(),
        controller_steps: 0,
        transitions: 0,
        minimum_dwell_violations: 0,
        grooming_floor_violations: 0,
        missed_loom_preemptions: 0,
        zero_input_escapes: 0,
        periodic_schedule_events: 0,
        duration_draw_cases: 0,
        serialization_next_transition_checks: 0,
        invalid_state_fail_closed: false,
        bounded_context: true,
        state_inline_bytes: std::mem::size_of::<BehaviorDynamicsState>(),
        scientific_claim: "engineered controller invariants; no biological fit or sleep criterion"
            .to_owned(),
    };
    for seed in 0..16_u64 {
        for behavior in [
            Behavior::Quiet,
            Behavior::Rest,
            Behavior::Walk,
            Behavior::Reverse,
            Behavior::Groom,
            Behavior::Alert,
            Behavior::PreEscape,
            Behavior::Flight,
            Behavior::Landing,
        ] {
            for sequence in 0..32 {
                let a = duration_draw(seed, sequence, behavior, (sequence * 7) as u16);
                let b = duration_draw(seed, sequence, behavior, (sequence * 7) as u16);
                let d = parameters().for_behavior(behavior);
                if a != b || a.1 < d.low_frames || a.1 > d.high_frames {
                    return Err("duration sampler is nondeterministic or outside its bounds".into());
                }
                result.duration_draw_cases += 1;
            }
        }
        for scenario in 0..5 {
            let mut state = fresh_state(seed);
            for frame in 0..2_400_u64 {
                let mut values = [0; 9];
                match scenario {
                    1 => values[5] = if frame % 120 < 60 { 4_601 } else { 4_000 },
                    2 => values[6] = if frame % 240 < 120 { 4_601 } else { 3_799 },
                    3 => {
                        values[5] = 4_601;
                        values[6] = if frame % 2 == 0 { 4_601 } else { 3_799 };
                    }
                    4 if frame % 300 < 12 => values[0] = 5_200,
                    _ => {}
                }
                let before = state.clone();
                let intent = evidence_for_state(&state, &values, 0);
                let out = state.advance(seed, intent);
                result.controller_steps += 1;
                state.validate(seed, intent.frame, out.behavior, state.elapsed_frames)?;
                if values[0] >= parameters().loom_on_q15
                    && !airborne(before.current_macro_state)
                    && out.behavior != Behavior::PreEscape
                {
                    result.missed_loom_preemptions += 1;
                }
                if scenario == 0 && airborne(out.behavior) {
                    result.zero_input_escapes += 1;
                }
                if let Some(reason) = out.reason {
                    result.transitions += 1;
                    let elapsed = before.elapsed_frames + 1;
                    if !reason.emergency_override() && elapsed < before.minimum_dwell_frames {
                        result.minimum_dwell_violations += 1;
                    }
                    if before.current_macro_state == Behavior::Groom
                        && !reason.emergency_override()
                        && u64::from(elapsed) * u64::from(crate::MODEL_STEP_MS) < 1_500
                    {
                        result.grooming_floor_violations += 1;
                    }
                    if reason == BehaviorTransitionReason::LegacyAutonomousSchedule {
                        result.periodic_schedule_events += 1;
                    }
                }
                if frame % 97 == 0 {
                    let encoded = serde_json::to_vec(&before).map_err(|e| e.to_string())?;
                    let mut restored: BehaviorDynamicsState =
                        serde_json::from_slice(&encoded).map_err(|e| e.to_string())?;
                    let restored_out = restored.advance(seed, intent);
                    if restored != state
                        || restored_out.behavior != out.behavior
                        || restored_out.reason != out.reason
                        || restored_out.transition != out.transition
                    {
                        return Err("controller checkpoint changed the next transition".into());
                    }
                    result.serialization_next_transition_checks += 1;
                }
            }
        }
    }
    let mut invalid = fresh_state(7);
    invalid.target_duration_frames = 0;
    let intent = evidence_for_state(&invalid, &[0; 9], 0);
    let out = invalid.advance(7, intent);
    result.invalid_state_fail_closed = out.behavior == Behavior::Quiet
        && invalid.fault_latched
        && out.reason == Some(BehaviorTransitionReason::InvalidControllerState);
    for _ in 0..100 {
        let intent = evidence_for_state(&invalid, &[8_000; 9], 0);
        result.invalid_state_fail_closed &= invalid.advance(7, intent).behavior == Behavior::Quiet;
    }
    result.passed = result.minimum_dwell_violations == 0
        && result.grooming_floor_violations == 0
        && result.missed_loom_preemptions == 0
        && result.zero_input_escapes == 0
        && result.periodic_schedule_events == 0
        && result.invalid_state_fail_closed
        && result.serialization_next_transition_checks > 0
        && result.transitions > 0;
    if !result.passed {
        return Err(format!("N4 invariant failure: {result:?}"));
    }
    Ok(result)
}
