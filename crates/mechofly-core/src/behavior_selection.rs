//! Compatibility selector for N3. It consumes intent, never neural arrays.
//!
//! The legacy holds and periodic fallback remain intentionally unchanged.
//! Replacing this policy is N4, not an implicit part of this refactor.

use crate::{
    behavior_intent::{
        AUTHORED_BEHAVIOR_ACTIVATION_Q15, BehaviorIntentSnapshot, LOOM_ESCAPE_ACTIVATION_Q15,
        SPIKE_ALERT_THRESHOLD_PER_10K,
    },
    behavior_telemetry::BehaviorTransitionReason,
    model::{
        Behavior, ESCAPE_HOLD_FRAMES, FLIGHT_HOLD_FRAMES, GROOM_HOLD_FRAMES, LANDING_HOLD_FRAMES,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BehaviorDecision {
    pub behavior: Behavior,
    /// None means the current behavior persists; no transition is recorded.
    pub transition_reason: Option<BehaviorTransitionReason>,
}

pub struct LegacyBehaviorSelector;

impl LegacyBehaviorSelector {
    pub fn select(intent: &BehaviorIntentSnapshot) -> BehaviorDecision {
        let behavior = select_behavior(intent);
        BehaviorDecision {
            behavior,
            transition_reason: (behavior != intent.current_behavior)
                .then(|| classify_transition_reason(intent.current_behavior, behavior, intent)),
        }
    }
}

fn select_behavior(intent: &BehaviorIntentSnapshot) -> Behavior {
    match intent.current_behavior {
        Behavior::PreEscape if intent.current_behavior_age_frames < ESCAPE_HOLD_FRAMES => {
            return Behavior::PreEscape;
        }
        Behavior::PreEscape => return Behavior::Flight,
        Behavior::Flight if intent.current_behavior_age_frames < FLIGHT_HOLD_FRAMES => {
            return Behavior::Flight;
        }
        Behavior::Flight => return Behavior::Landing,
        Behavior::Landing if intent.current_behavior_age_frames < LANDING_HOLD_FRAMES => {
            return Behavior::Landing;
        }
        Behavior::Landing => return Behavior::Rest,
        _ => {}
    }

    if intent.loom_activation_q15 >= LOOM_ESCAPE_ACTIVATION_Q15 {
        return Behavior::PreEscape;
    }
    if intent.current_behavior == Behavior::Groom
        && intent.current_behavior_age_frames < GROOM_HOLD_FRAMES
    {
        return Behavior::Groom;
    }
    if intent.groom_activation_q15 >= AUTHORED_BEHAVIOR_ACTIVATION_Q15 {
        return Behavior::Groom;
    }
    if intent.alert_activation_q15 >= AUTHORED_BEHAVIOR_ACTIVATION_Q15 {
        return Behavior::Alert;
    }
    if intent.reverse_activation_q15 >= AUTHORED_BEHAVIOR_ACTIVATION_Q15 {
        return Behavior::Reverse;
    }
    if intent.walk_activation_q15 >= AUTHORED_BEHAVIOR_ACTIVATION_Q15 {
        return Behavior::Walk;
    }

    // The u32 representation saturates only above u32::MAX, far above 1,200;
    // therefore the legacy usize > 1,200 decision is preserved for every input.
    if intent.spike_rate_per_10k > SPIKE_ALERT_THRESHOLD_PER_10K {
        Behavior::Alert
    } else {
        match intent.autonomous_schedule_slot {
            0 => Behavior::Rest,
            1..=3 => Behavior::Walk,
            4 => Behavior::Groom,
            5 | 6 => Behavior::Walk,
            7 => Behavior::Quiet,
            _ => Behavior::Reverse,
        }
    }
}

fn classify_transition_reason(
    from_behavior: Behavior,
    to_behavior: Behavior,
    intent: &BehaviorIntentSnapshot,
) -> BehaviorTransitionReason {
    if from_behavior == Behavior::PreEscape && to_behavior == Behavior::Flight {
        BehaviorTransitionReason::PreEscapeCompleted
    } else if from_behavior == Behavior::Flight && to_behavior == Behavior::Landing {
        BehaviorTransitionReason::FlightCompleted
    } else if from_behavior == Behavior::Landing && to_behavior == Behavior::Rest {
        BehaviorTransitionReason::LandingCompleted
    } else if to_behavior == Behavior::PreEscape
        && intent.loom_activation_q15 >= LOOM_ESCAPE_ACTIVATION_Q15
    {
        BehaviorTransitionReason::LoomPopulationThreshold
    } else if to_behavior == Behavior::Groom
        && intent.groom_activation_q15 >= AUTHORED_BEHAVIOR_ACTIVATION_Q15
    {
        BehaviorTransitionReason::GroomPopulationThreshold
    } else if to_behavior == Behavior::Alert
        && intent.alert_activation_q15 >= AUTHORED_BEHAVIOR_ACTIVATION_Q15
    {
        BehaviorTransitionReason::AlertPopulationThreshold
    } else if to_behavior == Behavior::Reverse
        && intent.reverse_activation_q15 >= AUTHORED_BEHAVIOR_ACTIVATION_Q15
    {
        BehaviorTransitionReason::ReversePopulationThreshold
    } else if to_behavior == Behavior::Walk
        && intent.walk_activation_q15 >= AUTHORED_BEHAVIOR_ACTIVATION_Q15
    {
        BehaviorTransitionReason::WalkPopulationThreshold
    } else if to_behavior == Behavior::Alert
        && intent.spike_rate_per_10k > SPIKE_ALERT_THRESHOLD_PER_10K
    {
        BehaviorTransitionReason::SpikeRateThreshold
    } else {
        BehaviorTransitionReason::LegacyAutonomousSchedule
    }
}
