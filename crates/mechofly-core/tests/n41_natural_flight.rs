use std::collections::BTreeSet;

use mechofly_core::{
    Behavior, MODEL_STEP_MS,
    behavior_parameters::{
        BehaviorParameterProfile, N41_NATURAL_FLIGHT_DYNAMICS_CLAIM,
        N41_NATURAL_FLIGHT_DYNAMICS_VERSION, duration_draw_for, dynamics_version_for_sha256,
        parameter_sha256_for, parameters_for_profile,
    },
};

const NATURAL_FLIGHT_SHA256: &str =
    "cb3cd2654dcd4fa9def34fb0145645f5d61b59c96c407669cf1e9dd4f12628ef";

#[test]
fn natural_flight_profile_is_additive_and_preserves_every_frozen_identity() {
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
    assert_eq!(
        parameter_sha256_for(BehaviorParameterProfile::N41BNaturalFlight),
        NATURAL_FLIGHT_SHA256
    );
    assert_eq!(
        dynamics_version_for_sha256(NATURAL_FLIGHT_SHA256),
        Some(N41_NATURAL_FLIGHT_DYNAMICS_VERSION)
    );
    assert!(N41_NATURAL_FLIGHT_DYNAMICS_VERSION.contains("uncued-exploration-prior-v2"));
    assert!(N41_NATURAL_FLIGHT_DYNAMICS_CLAIM.contains("not fitted biological constants"));
    assert!(
        N41_NATURAL_FLIGHT_DYNAMICS_CLAIM.contains("not a food-search or territory-coverage model")
    );
}

#[test]
fn flight_quantiles_are_broad_sorted_and_do_not_change_takeoff_or_touchdown() {
    let frozen = parameters_for_profile(BehaviorParameterProfile::N41BNatural);
    let natural = parameters_for_profile(BehaviorParameterProfile::N41BNaturalFlight);
    let flight = natural.for_behavior(Behavior::Flight);

    assert_eq!(
        natural.walk_duration_quantiles_frames,
        frozen.walk_duration_quantiles_frames
    );
    assert_eq!(natural.flight_duration_quantiles_frames.len(), 128);
    assert_eq!(
        natural.flight_duration_quantiles_frames.first(),
        Some(&flight.low_frames)
    );
    assert_eq!(
        natural.flight_duration_quantiles_frames.last(),
        Some(&flight.high_frames)
    );
    assert!(
        natural
            .flight_duration_quantiles_frames
            .windows(2)
            .all(|pair| pair[0] <= pair[1])
    );
    assert!(
        natural
            .flight_duration_quantiles_frames
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            >= 100
    );
    assert_eq!(flight.low_frames * MODEL_STEP_MS, 594);
    assert_eq!(flight.high_frames * MODEL_STEP_MS, 7_986);
    for behavior in [Behavior::PreEscape, Behavior::Landing] {
        assert_eq!(
            natural.for_behavior(behavior),
            frozen.for_behavior(behavior)
        );
    }
}

#[test]
fn event_keyed_flight_draw_is_repeatable_and_not_a_fixed_clock() {
    let natural = parameters_for_profile(BehaviorParameterProfile::N41BNaturalFlight);
    let frozen = parameters_for_profile(BehaviorParameterProfile::N41BNatural);
    let seed = 0x4D45_4348_4F46_4C59;
    let samples: Vec<u32> = (0..65_536_u64)
        .map(|sequence| {
            let first = duration_draw_for(natural, seed, sequence, Behavior::Flight, 0);
            let repeat = duration_draw_for(natural, seed, sequence, Behavior::Flight, 0);
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

    assert!(unique >= 100, "only {unique} distinct flight durations");
    assert!((73.0..=82.0).contains(&mean), "mean={mean}");
    assert!(
        (0.68..=0.76).contains(&coefficient_of_variation),
        "cv={coefficient_of_variation}"
    );
    assert!((0.16..=0.20).contains(&short_fraction));
    assert!((0.09..=0.13).contains(&long_fraction));

    for sequence in 0..4_096_u64 {
        assert_eq!(
            duration_draw_for(frozen, seed, sequence, Behavior::Flight, 0).1,
            121
        );
    }
}
