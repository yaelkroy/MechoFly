use std::{collections::BTreeSet, sync::Arc};

use mechofly_core::behavior_dynamics::{BehaviorDynamicsState, fatigue_response_gain_q15};
use mechofly_core::behavior_parameters::{
    BehaviorParameterProfile, FatiguePolicy, parameter_sha256, parameter_sha256_for,
    parameters_for_profile,
};
use mechofly_core::behavior_validation::{evidence_for_state, fresh_state};
use mechofly_core::{Behavior, ModelEngine, ModelGraph, ModelState, ModelTier, StepInput};

#[test]
fn immutable_profiles_are_valid_unique_and_keep_the_frozen_n4_artifact() {
    assert_eq!(
        parameter_sha256(),
        "1c950fcbe0c4884e238f6279e10309b8810c6949d5610dfc4889d6c35252072b"
    );
    for (profile, expected) in [
        (
            BehaviorParameterProfile::N41A,
            "94350dcaa0755fce9fca2d8c3d429eb54c0b4aa370c7cf56bfc4236bb7339615",
        ),
        (
            BehaviorParameterProfile::N41B,
            "bec74a4ab771b61923ac81d71fd532d88001abd4bf00e90bf799e6e30703c138",
        ),
        (
            BehaviorParameterProfile::N41BNatural,
            "a6c32576e4b869d10b8ff6f58ed3a7c9482ad831c767d697e5fd5e90e888ec6c",
        ),
        (
            BehaviorParameterProfile::N41C,
            "b1296cd9640a39852dfa5d8cba2387798fbe681869dc53b8fd24224225f0a18d",
        ),
    ] {
        assert_eq!(parameter_sha256_for(profile), expected);
    }
    let identities: BTreeSet<_> = BehaviorParameterProfile::ALL
        .into_iter()
        .map(|profile| {
            let p = parameters_for_profile(profile);
            p.validate().unwrap();
            (
                p.parameter_set_id.clone(),
                parameter_sha256_for(profile).to_owned(),
            )
        })
        .collect();
    assert_eq!(identities.len(), BehaviorParameterProfile::ALL.len());
    assert_eq!(
        parameters_for_profile(BehaviorParameterProfile::N4).fatigue_policy,
        FatiguePolicy::HardGate
    );
    for profile in [
        BehaviorParameterProfile::N41A,
        BehaviorParameterProfile::N41B,
        BehaviorParameterProfile::N41BNatural,
        BehaviorParameterProfile::N41C,
    ] {
        assert_eq!(
            parameters_for_profile(profile).fatigue_policy,
            FatiguePolicy::GradedResponse
        );
    }
}

#[test]
fn graded_gain_is_bounded_monotone_and_never_zero() {
    for profile in [
        BehaviorParameterProfile::N41A,
        BehaviorParameterProfile::N41B,
        BehaviorParameterProfile::N41BNatural,
        BehaviorParameterProfile::N41C,
    ] {
        let p = parameters_for_profile(profile);
        let mut previous = p.context_max_q15;
        for fatigue in 0..=p.context_max_q15 {
            let gain = fatigue_response_gain_q15(p, fatigue);
            assert!((p.fatigue_min_response_q15..=p.context_max_q15).contains(&gain));
            assert!(gain <= previous);
            previous = gain;
        }
        assert_eq!(
            fatigue_response_gain_q15(p, p.fatigue_suppression_onset_q15),
            p.context_max_q15
        );
        assert_eq!(
            fatigue_response_gain_q15(p, p.fatigue_suppression_full_q15),
            p.fatigue_min_response_q15
        );
    }
}

#[test]
fn response_draw_is_deterministic_and_floor_is_reachable_across_event_keys() {
    for profile in [
        BehaviorParameterProfile::N41A,
        BehaviorParameterProfile::N41B,
        BehaviorParameterProfile::N41BNatural,
        BehaviorParameterProfile::N41C,
    ] {
        let p = parameters_for_profile(profile);
        let mut accepted_at_floor = 0;
        let mut declined_at_floor = 0;
        for seed in 0..256_u64 {
            let evidence = evidence_for_state(&fresh_state(seed), &[0; 9], 0);
            let mut state = BehaviorDynamicsState::new_with_profile(seed, evidence, profile);
            state.context.fatigue_q15 = p.fatigue_suppression_full_q15;
            let a = state
                .fatigue_response_draw_q15(seed, Behavior::Walk)
                .unwrap();
            let b = state
                .fatigue_response_draw_q15(seed, Behavior::Walk)
                .unwrap();
            assert_eq!(a, b);
            if a <= p.fatigue_min_response_q15 {
                accepted_at_floor += 1;
            } else {
                declined_at_floor += 1;
            }
        }
        assert!(accepted_at_floor > 0);
        assert!(declined_at_floor > 0);
    }
}

#[test]
fn every_profile_preserves_neural_arrays_escape_and_checkpoint_replay() {
    for profile in BehaviorParameterProfile::ALL {
        let seed = 0x51;
        let evidence = evidence_for_state(&fresh_state(seed), &[0; 9], 0);
        let mut state = BehaviorDynamicsState::new_with_profile(seed, evidence, profile);
        let mut loom = [0; 9];
        loom[0] = 5_200;
        let intent = evidence_for_state(&state, &loom, 0);
        assert_eq!(state.advance(seed, intent).behavior, Behavior::PreEscape);
    }

    let graph = Arc::new(ModelGraph::synthetic(ModelTier::Demo4096, 0x41));
    let mut engines = vec![ModelEngine::new(Arc::clone(&graph), 0x51)];
    engines.extend(BehaviorParameterProfile::ALL.map(|profile| {
        ModelEngine::new_duration_aware_with_profile(Arc::clone(&graph), 0x51, profile)
    }));

    for frame in 0..1_200 {
        let mut input = vec![0; graph.neuron_ids.len()];
        let offset = match (frame / 120) % 5 {
            1 => 5,
            2 => 6,
            3 => 3,
            4 => 0,
            _ => usize::MAX,
        };
        if offset != usize::MAX {
            for value in input.iter_mut().skip(offset).step_by(9) {
                *value = 8_192;
            }
        }
        for engine in &mut engines {
            engine.step_cpu(StepInput {
                stimulus_q15: &input,
            });
        }
        for engine in engines.iter().skip(1) {
            assert_eq!(engine.state.activation, engines[0].state.activation);
            assert_eq!(engine.state.spikes, engines[0].state.spikes);
        }
        if frame % 113 == 0 {
            for engine in engines.iter().skip(1) {
                let state_json = serde_json::to_vec(&engine.state).unwrap();
                let restored: ModelState = serde_json::from_slice(&state_json).unwrap();
                let mut fork = ModelEngine::from_state(Arc::clone(&graph), restored).unwrap();
                let mut clone = engine.clone();
                assert_eq!(
                    fork.step_cpu(StepInput {
                        stimulus_q15: &input,
                    }),
                    clone.step_cpu(StepInput {
                        stimulus_q15: &input,
                    })
                );
                assert_eq!(fork.state, clone.state);
            }
        }
    }
}
