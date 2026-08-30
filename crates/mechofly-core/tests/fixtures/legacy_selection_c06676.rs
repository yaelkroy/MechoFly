const LOOM_ESCAPE_ACTIVATION_Q15: i32 = 5_200;
const AUTHORED_BEHAVIOR_ACTIVATION_Q15: i32 = 4_600;
const SPIKE_ALERT_THRESHOLD_PER_10K: u32 = 1_200;

fn behavior_intent_snapshot(state: &ModelState, spike_count: usize) -> BehaviorIntentSnapshot {
    BehaviorIntentSnapshot {
        schema_version: BEHAVIOR_TELEMETRY_SCHEMA_VERSION,
        frame: state.frame,
        current_behavior: state.behavior,
        current_behavior_age_frames: state.behavior_age_frames,
        spike_count: spike_count.min(u32::MAX as usize) as u32,
        spike_rate_per_10k: (spike_count.saturating_mul(10_000) / state.activation.len().max(1))
            .min(u32::MAX as usize) as u32,
        spike_alert_threshold_per_10k: SPIKE_ALERT_THRESHOLD_PER_10K,
        autonomous_schedule_slot: ((state.frame / 90) % 9) as u8,
        loom_activation_q15: functional_population_activation(state, LOOM_POPULATION_OFFSET),
        groom_activation_q15: functional_population_activation(state, GROOM_POPULATION_OFFSET),
        alert_activation_q15: functional_population_activation(state, ALERT_POPULATION_OFFSET),
        reverse_activation_q15: functional_population_activation(state, REVERSE_POPULATION_OFFSET),
        walk_activation_q15: functional_population_activation(state, WALK_POPULATION_OFFSET),
        loom_entry_threshold_q15: LOOM_ESCAPE_ACTIVATION_Q15,
        authored_behavior_entry_threshold_q15: AUTHORED_BEHAVIOR_ACTIVATION_Q15,
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

fn modeled_behavior(state: &ModelState, spike_count: usize) -> Behavior {
    match state.behavior {
        Behavior::PreEscape if state.behavior_age_frames < ESCAPE_HOLD_FRAMES => {
            return Behavior::PreEscape;
        }
        Behavior::PreEscape => return Behavior::Flight,
        Behavior::Flight if state.behavior_age_frames < FLIGHT_HOLD_FRAMES => {
            return Behavior::Flight;
        }
        Behavior::Flight => return Behavior::Landing,
        Behavior::Landing if state.behavior_age_frames < LANDING_HOLD_FRAMES => {
            return Behavior::Landing;
        }
        Behavior::Landing => return Behavior::Rest,
        _ => {}
    }

    let loom_activation = functional_population_activation(state, LOOM_POPULATION_OFFSET);
    if loom_activation >= LOOM_ESCAPE_ACTIVATION_Q15 {
        return Behavior::PreEscape;
    }

    if state.behavior == Behavior::Groom && state.behavior_age_frames < GROOM_HOLD_FRAMES {
        return Behavior::Groom;
    }

    // These are bounded authored inputs to modeled functional populations,
    // not presentation-only pose switches. The model must cross the same
    // activation boundary before the corresponding motor program is exposed.
    if functional_population_activation(state, GROOM_POPULATION_OFFSET)
        >= AUTHORED_BEHAVIOR_ACTIVATION_Q15
    {
        return Behavior::Groom;
    }
    if functional_population_activation(state, ALERT_POPULATION_OFFSET)
        >= AUTHORED_BEHAVIOR_ACTIVATION_Q15
    {
        return Behavior::Alert;
    }
    if functional_population_activation(state, REVERSE_POPULATION_OFFSET)
        >= AUTHORED_BEHAVIOR_ACTIVATION_Q15
    {
        return Behavior::Reverse;
    }
    if functional_population_activation(state, WALK_POPULATION_OFFSET)
        >= AUTHORED_BEHAVIOR_ACTIVATION_Q15
    {
        return Behavior::Walk;
    }

    let rate_per_10k = spike_count.saturating_mul(10_000) / state.activation.len().max(1);
    if rate_per_10k > 1_200 {
        Behavior::Alert
    } else {
        match (state.frame / 90) % 9 {
            0 => Behavior::Rest,
            1..=3 => Behavior::Walk,
            4 => Behavior::Groom,
            5 | 6 => Behavior::Walk,
            7 => Behavior::Quiet,
            _ => Behavior::Reverse,
        }
    }
}

fn functional_population_activation(state: &ModelState, offset: usize) -> i32 {
    state
        .activation
        .iter()
        .skip(offset)
        .step_by(FUNCTIONAL_POPULATION_COUNT)
        .copied()
        .max()
        .unwrap_or(ACTIVATION_MIN)
}

