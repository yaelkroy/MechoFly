use std::sync::Arc;

use mechofly_core::{
    Behavior, ModelEngine, ModelGraph, ModelState, ModelTier, StepInput,
    behavior_dynamics::airborne,
    behavior_parameters::{PARAMETERS_JSON, duration_draw, parameter_sha256, parameters},
    behavior_validation::{evidence_for_state, fresh_state, validate_dynamics},
};

#[test]
fn complete_duration_context_dwell_escape_and_restore_invariants() {
    let r = validate_dynamics().unwrap();
    assert!(r.passed);
    assert_eq!(r.controller_steps, 192_000);
    assert_eq!(r.duration_draw_cases, 4_608);
    assert_eq!(r.minimum_dwell_violations, 0);
    assert_eq!(r.grooming_floor_violations, 0);
    println!(
        "N4_CONTROLLER_STEPS={} N4_RESTORE_CHECKS={}",
        r.controller_steps, r.serialization_next_transition_checks
    );
}

#[test]
fn parameter_artifact_rejects_unsafe_guards_and_has_real_variability() {
    let mut p = parameters().clone();
    p.durations[Behavior::Groom as usize].minimum_frames = 1;
    assert!(p.validate().is_err());
    p = parameters().clone();
    p.ordinary_off_q15 = p.ordinary_on_q15;
    assert!(p.validate().is_err());
    p = parameters().clone();
    p.durations[Behavior::Flight as usize].high_frames = 122;
    assert!(p.validate().is_err());
    let values: std::collections::BTreeSet<_> = (0..64)
        .map(|seed| duration_draw(seed, 1, Behavior::Walk, 0).1)
        .collect();
    assert!(values.len() > 20);
    assert_eq!(parameter_sha256().len(), 64);
    let round: mechofly_core::behavior_parameters::BehaviorParameters =
        serde_json::from_str(PARAMETERS_JSON).unwrap();
    assert_eq!(&round, parameters());
}

#[test]
fn ordinary_hysteresis_is_not_threshold_chatter_and_cooldown_is_enforced() {
    let seed = 15;
    let mut state = fresh_state(seed);
    let mut walk = [0; 9];
    walk[5] = 4_600;
    while state.current_macro_state != Behavior::Walk {
        let input = evidence_for_state(&state, &walk, 0);
        state.advance(seed, input);
    }
    for frame in 0..parameters().for_behavior(Behavior::Walk).minimum_frames - 1 {
        walk[5] = if frame % 2 == 0 { 4_001 } else { 4_599 };
        let input = evidence_for_state(&state, &walk, 0);
        assert_eq!(state.advance(seed, input).behavior, Behavior::Walk);
        assert!(state.active_intents[0]);
    }
    let departure;
    loop {
        let input = evidence_for_state(&state, &walk, 0);
        let out = state.advance(seed, input);
        if out.behavior != Behavior::Walk {
            departure = state.last_frame;
            break;
        }
    }
    assert!(state.refractory_until_frame[Behavior::Walk as usize] >= departure + 20);
    for _ in 0..19 {
        let input = evidence_for_state(&state, &walk, 0);
        assert_ne!(state.advance(seed, input).behavior, Behavior::Walk);
    }
}

#[test]
fn spike_latch_has_separate_exit_threshold_and_emergency_bypasses_dwell() {
    let seed = 33;
    let mut state = fresh_state(seed);
    let input = evidence_for_state(&state, &[0; 10_000], 1_201);
    state.advance(seed, input);
    assert!(state.spike_alert_latched);
    let input = evidence_for_state(&state, &[0; 10_000], 1_100);
    state.advance(seed, input);
    assert!(state.spike_alert_latched);
    let input = evidence_for_state(&state, &[0; 10_000], 1_000);
    state.advance(seed, input);
    assert!(!state.spike_alert_latched);
    let mut loom = [0; 9];
    loom[0] = 5_200;
    let input = evidence_for_state(&state, &loom, 0);
    assert_eq!(state.advance(seed, input).behavior, Behavior::PreEscape);
    let entered = state.last_frame;
    for _ in 0..5 {
        let input = evidence_for_state(&state, &loom, 0);
        assert_eq!(state.advance(seed, input).behavior, Behavior::PreEscape);
    }
    let input = evidence_for_state(&state, &loom, 0);
    assert_eq!(state.advance(seed, input).behavior, Behavior::Flight);
    assert_eq!(state.entered_at_frame - entered, 6);
}

#[test]
fn core_context_not_absolute_periodic_clock_controls_autonomy() {
    let seed = 19;
    let mut a = fresh_state(seed);
    let mut b = a.clone();
    // Shift the absolute time origin without changing elapsed state or context.
    b.last_frame = 90_000;
    b.entered_at_frame = 90_000;
    for _ in 0..2_000 {
        let ia = evidence_for_state(&a, &[0; 9], 0);
        let ib = evidence_for_state(&b, &[0; 9], 0);
        let oa = a.advance(seed, ia);
        let ob = b.advance(seed, ib);
        assert_eq!(oa.behavior, ob.behavior);
        assert_eq!(oa.reason, ob.reason);
        assert_eq!(a.context, b.context);
        assert_eq!(a.deterministic_duration_draw, b.deterministic_duration_draw);
        assert!(!airborne(oa.behavior));
    }
}

#[test]
fn neural_arrays_are_identical_to_n3_for_identical_inputs_and_checkpoint_restores() {
    let graph = Arc::new(ModelGraph::synthetic(ModelTier::Demo4096, 71));
    let mut old = ModelEngine::new(Arc::clone(&graph), 91);
    let mut new = ModelEngine::new_duration_aware(Arc::clone(&graph), 91);
    assert_ne!(old.model_identity(), new.model_identity());
    for frame in 0..900 {
        let mut input = vec![0; graph.neuron_ids.len()];
        let offset = match (frame / 90) % 6 {
            1 => 5,
            2 => 6,
            3 => 3,
            4 => 4,
            _ => 0,
        };
        if (frame / 90) % 6 != 0 {
            for x in input.iter_mut().skip(offset).step_by(9) {
                *x = 8_192;
            }
        }
        old.step_cpu(StepInput {
            stimulus_q15: &input,
        });
        new.step_cpu(StepInput {
            stimulus_q15: &input,
        });
        assert_eq!(old.state.activation, new.state.activation);
        assert_eq!(old.state.spikes, new.state.spikes);
        if frame % 37 == 0 {
            let bytes = serde_json::to_vec(&new.state).unwrap();
            let restored: ModelState = serde_json::from_slice(&bytes).unwrap();
            let mut fork = ModelEngine::from_state(Arc::clone(&graph), restored).unwrap();
            let mut clone = new.clone();
            assert_eq!(
                fork.step_cpu(StepInput {
                    stimulus_q15: &input
                }),
                clone.step_cpu(StepInput {
                    stimulus_q15: &input
                })
            );
            assert_eq!(fork.state, clone.state);
        }
    }
    assert!(
        new.behavior_telemetry_snapshot()
            .controller_semantics_changed
    );
    assert!(
        new.behavior_transition_events()
            .all(|e| e.schema_version == 2 && e.dynamics.is_some())
    );
}

#[test]
fn malformed_n4_checkpoints_rejected_and_legacy_json_remains_unchanged() {
    let graph = Arc::new(ModelGraph::synthetic(ModelTier::Demo4096, 5));
    let mut new = ModelEngine::new_duration_aware(Arc::clone(&graph), 7);
    new.state
        .behavior_dynamics
        .as_mut()
        .unwrap()
        .parameter_sha256 = "wrong".into();
    assert!(ModelEngine::from_state(Arc::clone(&graph), new.state).is_err());
    let old = ModelEngine::new(graph, 7);
    let text = serde_json::to_string(&old.state).unwrap();
    assert!(!text.contains("behavior_dynamics"));
    let decoded: ModelState = serde_json::from_str(&text).unwrap();
    assert_eq!(decoded.digest(), old.state.digest());
}
