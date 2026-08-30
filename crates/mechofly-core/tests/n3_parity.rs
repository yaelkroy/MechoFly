//! Independent compatibility oracle frozen from c06676c93085.
//! This is deliberately test-only; production has one intent/selection path.

use std::sync::Arc;

use mechofly_core::{
    Behavior, BehaviorContext, BehaviorIntentBuilder, BehaviorIntentSnapshot,
    BehaviorTelemetryLedger, BehaviorTransitionEvent, BehaviorTransitionReason, FrameSummary,
    LegacyBehaviorSelector, ModelEngine, ModelGraph, ModelState, ModelTier, NeuralEvidence,
    StepInput,
    model::{
        ACTIVATION_MIN, ALERT_POPULATION_OFFSET, FUNCTIONAL_POPULATION_COUNT,
        GROOM_POPULATION_OFFSET, LOOM_POPULATION_OFFSET, REVERSE_POPULATION_OFFSET,
        WALK_POPULATION_OFFSET, update_neuron,
    },
};

mod frozen {
    use super::*;
    use mechofly_core::{
        BEHAVIOR_TELEMETRY_SCHEMA_VERSION,
        model::{ESCAPE_HOLD_FRAMES, FLIGHT_HOLD_FRAMES, GROOM_HOLD_FRAMES, LANDING_HOLD_FRAMES},
    };

    include!("fixtures/legacy_selection_c06676.rs");

    pub fn intent(state: &ModelState, spikes: usize) -> BehaviorIntentSnapshot {
        behavior_intent_snapshot(state, spikes)
    }

    pub fn behavior(state: &ModelState, spikes: usize) -> Behavior {
        modeled_behavior(state, spikes)
    }

    pub fn reason(
        from: Behavior,
        to: Behavior,
        intent: &BehaviorIntentSnapshot,
    ) -> BehaviorTransitionReason {
        classify_transition_reason(from, to, intent)
    }
}

const BEHAVIORS: [Behavior; 9] = [
    Behavior::Rest,
    Behavior::Quiet,
    Behavior::Walk,
    Behavior::Reverse,
    Behavior::Groom,
    Behavior::Alert,
    Behavior::PreEscape,
    Behavior::Flight,
    Behavior::Landing,
];

fn new_intent(state: &ModelState, spikes: usize) -> BehaviorIntentSnapshot {
    BehaviorIntentBuilder::build(
        &NeuralEvidence::collect(state.frame, &state.activation, spikes),
        BehaviorContext {
            current_behavior: state.behavior,
            current_behavior_age_frames: state.behavior_age_frames,
        },
    )
}

fn assert_selector_parity(state: &ModelState, spikes: usize) {
    let intent = new_intent(state, spikes);
    let old_intent = frozen::intent(state, spikes);
    assert_eq!(intent, old_intent);
    let decision = LegacyBehaviorSelector::select(&intent);
    let old_behavior = frozen::behavior(state, spikes);
    assert_eq!(decision.behavior, old_behavior, "state={state:?}, spikes={spikes}");
    let old_reason = (old_behavior != state.behavior)
        .then(|| frozen::reason(state.behavior, old_behavior, &old_intent));
    assert_eq!(decision.transition_reason, old_reason);
}

#[path = "n3/selection_cases.rs"]
mod selection_cases;
#[path = "n3/engine_cases.rs"]
mod engine_cases;
