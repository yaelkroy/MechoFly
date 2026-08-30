use super::*;

#[test]
fn exhaustive_threshold_hold_priority_and_schedule_boundary_parity() {
    let mut cases = 0_u64;
    let mut state = ModelState {
        frame: 0,
        seed: 0,
        activation: vec![ACTIVATION_MIN; 9],
        spikes: vec![0; 9],
        behavior: Behavior::Rest,
        behavior_age_frames: 0,
    };
    // 3^5 combinations include all simultaneous-priority ties at both entry
    // thresholds. Ages straddle every hold boundary. Frames straddle schedule
    // boundaries and include the largest representable frame.
    for encoded in 0..243_usize {
        let mut code = encoded;
        for (offset, threshold) in [
            (LOOM_POPULATION_OFFSET, 5_200),
            (GROOM_POPULATION_OFFSET, 4_600),
            (ALERT_POPULATION_OFFSET, 4_600),
            (REVERSE_POPULATION_OFFSET, 4_600),
            (WALK_POPULATION_OFFSET, 4_600),
        ] {
            state.activation[offset] = threshold + (code % 3) as i32 - 1;
            code /= 3;
        }
        for behavior in BEHAVIORS {
            state.behavior = behavior;
            for age in [0, 4, 5, 13, 14, 44, 45, 119, 120, u32::MAX] {
                state.behavior_age_frames = age;
                for frame in [0, 89, 90, 359, 360, 629, 630, 719, 720, 809, 810, u64::MAX] {
                    state.frame = frame;
                    for spikes in [0, 1, 2] {
                        assert_selector_parity(&state, spikes);
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 787_320);
    println!("N3_EXHAUSTIVE_SELECTOR_CASES={cases}");
}

#[test]
fn spike_rate_strict_boundary_and_saturating_representation_preserved() {
    let mut state = ModelState {
        frame: 90,
        seed: 0,
        activation: vec![-1; 10_000],
        spikes: vec![0; 10_000],
        behavior: Behavior::Rest,
        behavior_age_frames: 0,
    };
    for spikes in [
        0,
        1_199,
        1_200,
        1_201,
        10_000,
        u32::MAX as usize,
        usize::MAX,
    ] {
        assert_selector_parity(&state, spikes);
    }
    assert_eq!(
        LegacyBehaviorSelector::select(&new_intent(&state, 1_200)).behavior,
        Behavior::Walk
    );
    assert_eq!(
        LegacyBehaviorSelector::select(&new_intent(&state, 1_201)).behavior,
        Behavior::Alert
    );
    state.activation.clear();
    state.spikes.clear();
    for spikes in [0, 1, usize::MAX] {
        assert_selector_parity(&state, spikes);
    }
}

#[test]
fn sparse_population_extrema_and_signed_mean_match_frozen_rules() {
    let mut rng = 0x81cb_a972_ef43_10d5_u64;
    for index in 0..20_000_u64 {
        let count = [0, 1, 2, 5, 8, 9, 10, 17, 36, 127][index as usize % 10];
        let mut activation = Vec::with_capacity(count);
        for _ in 0..count {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            activation.push((rng as u16 as i32) - 32_768);
        }
        let state = ModelState {
            frame: index.saturating_mul(137),
            seed: rng,
            activation,
            spikes: vec![0; count],
            behavior: BEHAVIORS[index as usize % BEHAVIORS.len()],
            behavior_age_frames: (rng % 125) as u32,
        };
        let spikes = (rng % (count as u64 + 1)) as usize;
        assert_selector_parity(&state, spikes);
        let evidence = NeuralEvidence::collect(state.frame, &state.activation, spikes);
        let expected_mean = if count == 0 {
            0
        } else {
            (state
                .activation
                .iter()
                .map(|value| i64::from(*value))
                .sum::<i64>()
                / count as i64) as i32
        };
        assert_eq!(evidence.mean_activation_q15, expected_mean);
        assert_eq!(evidence.neuron_count, count);
    }
    println!("N3_SPARSE_RANDOMIZED_CASES=20000");
}

#[test]
fn intent_serialization_is_byte_exact_and_round_trips() {
    let state = ModelState {
        frame: 91,
        seed: 17,
        activation: vec![5_200, -1, 0, 4_599, 4_600, 4_601, 4_602, 9, -9],
        spikes: vec![0; 9],
        behavior: Behavior::Groom,
        behavior_age_frames: 44,
    };
    let intent = new_intent(&state, 2);
    let encoded = serde_json::to_vec(&intent).unwrap();
    let expected = concat!(
        "{\"schema_version\":1,\"frame\":91,\"current_behavior\":\"groom\",",
        "\"current_behavior_age_frames\":44,\"spike_count\":2,\"spike_rate_per_10k\":2222,",
        "\"spike_alert_threshold_per_10k\":1200,\"autonomous_schedule_slot\":1,",
        "\"loom_activation_q15\":5200,\"groom_activation_q15\":4602,",
        "\"alert_activation_q15\":4599,\"reverse_activation_q15\":4600,",
        "\"walk_activation_q15\":4601,\"loom_entry_threshold_q15\":5200,",
        "\"authored_behavior_entry_threshold_q15\":4600}"
    );
    assert_eq!(encoded, expected.as_bytes());
    assert_eq!(
        encoded,
        serde_json::to_vec(&frozen::intent(&state, 2)).unwrap()
    );
    let decoded: BehaviorIntentSnapshot = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(decoded, intent);
    assert_eq!(
        LegacyBehaviorSelector::select(&decoded),
        LegacyBehaviorSelector::select(&intent)
    );
    let before = state.clone();
    let _ = new_intent(&state, 2);
    assert_eq!(
        state, before,
        "intent construction may not mutate source state"
    );
}
