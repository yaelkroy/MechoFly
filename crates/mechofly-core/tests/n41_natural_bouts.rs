use std::collections::BTreeSet;

use mechofly_core::{
    Behavior, MODEL_STEP_MS,
    behavior_parameters::{
        BehaviorParameterProfile, N41_NATURAL_BOUT_DYNAMICS_CLAIM,
        N41_NATURAL_BOUT_DYNAMICS_VERSION, duration_draw_for, dynamics_version_for_sha256,
        parameter_sha256_for, parameters_for_profile,
    },
};

const NATURAL_SHA256: &str = "a6c32576e4b869d10b8ff6f58ed3a7c9482ad831c767d697e5fd5e90e888ec6c";

#[test]
fn natural_profile_is_additive_and_keeps_frozen_profile_artifacts() {
    for (profile, expected) in [
        (
            BehaviorParameterProfile::N4,
            "1c950fcbe0c4884e238f6279e10309b8810c6949d5610dfc4889d6c35252072b",
        ),
        (
            BehaviorParameterProfile::N41A,
            "94350dcaa0755fce9fca2d8c3d429eb54c0b4aa370c7cf56bfc4236bb7339615",
        ),
        (
            BehaviorParameterProfile::N41B,
            "bec74a4ab771b61923ac81d71fd532d88001abd4bf00e90bf799e6e30703c138",
        ),
        (
            BehaviorParameterProfile::N41C,
            "b1296cd9640a39852dfa5d8cba2387798fbe681869dc53b8fd24224225f0a18d",
        ),
    ] {
        assert_eq!(parameter_sha256_for(profile), expected);
    }
    assert_eq!(
        parameter_sha256_for(BehaviorParameterProfile::N41BNatural),
        NATURAL_SHA256
    );
    assert_eq!(
        dynamics_version_for_sha256(NATURAL_SHA256),
        Some(N41_NATURAL_BOUT_DYNAMICS_VERSION)
    );
    assert!(N41_NATURAL_BOUT_DYNAMICS_CLAIM.contains("not fitted biological constants"));
}

#[test]
fn authored_walk_quantiles_are_sorted_broad_and_explicitly_bounded() {
    let p = parameters_for_profile(BehaviorParameterProfile::N41BNatural);
    let table = &p.walk_duration_quantiles_frames;
    let walk = p.for_behavior(Behavior::Walk);
    assert_eq!(table.len(), 128);
    assert_eq!(table.first(), Some(&walk.low_frames));
    assert_eq!(table.last(), Some(&walk.high_frames));
    assert!(table.windows(2).all(|pair| pair[0] <= pair[1]));
    assert_eq!(table.iter().copied().collect::<BTreeSet<_>>().len(), 96);
    assert_eq!(walk.minimum_frames, 3);
    assert_eq!(walk.high_frames * MODEL_STEP_MS, 10_329);
}

#[test]
fn event_keyed_walk_draw_is_deterministic_and_not_uniform_clockwork() {
    let natural = parameters_for_profile(BehaviorParameterProfile::N41BNatural);
    let frozen_b = parameters_for_profile(BehaviorParameterProfile::N41B);
    let seed = 0x4D45_4348_4F46_4C59;
    let samples: Vec<u32> = (0..65_536_u64)
        .map(|sequence| {
            let first = duration_draw_for(natural, seed, sequence, Behavior::Walk, 0);
            let repeat = duration_draw_for(natural, seed, sequence, Behavior::Walk, 0);
            assert_eq!(first, repeat);
            first.1
        })
        .collect();
    let unique = samples.iter().copied().collect::<BTreeSet<_>>().len();
    let mean = samples.iter().map(|value| f64::from(*value)).sum::<f64>() / samples.len() as f64;
    let variance = samples
        .iter()
        .map(|value| (f64::from(*value) - mean).powi(2))
        .sum::<f64>()
        / samples.len() as f64;
    let coefficient_of_variation = variance.sqrt() / mean;
    let short_fraction =
        samples.iter().filter(|value| **value <= 30).count() as f64 / samples.len() as f64;
    let long_fraction =
        samples.iter().filter(|value| **value >= 152).count() as f64 / samples.len() as f64;

    assert!(unique >= 90, "only {unique} distinct durations");
    assert!((48.0..=57.0).contains(&mean), "mean={mean}");
    assert!(
        (0.90..=1.15).contains(&coefficient_of_variation),
        "cv={coefficient_of_variation}"
    );
    assert!((0.40..=0.50).contains(&short_fraction));
    assert!((0.04..=0.09).contains(&long_fraction));

    for sequence in 0..4_096_u64 {
        let duration = duration_draw_for(frozen_b, seed, sequence, Behavior::Walk, 0).1;
        assert!((60..=150).contains(&duration));
    }
}
