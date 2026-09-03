//! Versioned initialization priors. These are not fitted ethological constants.
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::model::Behavior;

pub const PARAMETERS_JSON: &str = include_str!("../parameters/n4-engineering-v1.json");
pub const N41_A_PARAMETERS_JSON: &str =
    include_str!("../parameters/n4.1-soft-fatigue-a-responsive-v1.json");
pub const N41_B_PARAMETERS_JSON: &str =
    include_str!("../parameters/n4.1-soft-fatigue-b-balanced-v1.json");
pub const N41_B_NATURAL_PARAMETERS_JSON: &str =
    include_str!("../parameters/n4.1-soft-fatigue-b-natural-bouts-v2.json");
pub const N41_B_NATURAL_FLIGHT_PARAMETERS_JSON: &str =
    include_str!("../parameters/n4.1-soft-fatigue-b-natural-flight-v3.json");
pub const N41_C_PARAMETERS_JSON: &str =
    include_str!("../parameters/n4.1-soft-fatigue-c-conservative-v1.json");
pub const DYNAMICS_VERSION: &str = "n4-explicit-duration-engineering-v1";
pub const N41_DYNAMICS_VERSION: &str = "n4.1-graded-fatigue-engineering-v1";
pub const N41_NATURAL_BOUT_DYNAMICS_VERSION: &str =
    "n4.1-literature-shaped-walk-bouts-product-prior-v1";
pub const N41_NATURAL_FLIGHT_DYNAMICS_VERSION: &str =
    "n4.1-literature-shaped-walk-flight-bouts-uncued-exploration-prior-v2";
pub const DYNAMICS_CLAIM: &str =
    "MODELED / ENGINEERING PRIOR; context and duration rules are authored, not biologically fitted";
pub const N41_NATURAL_BOUT_DYNAMICS_CLAIM: &str = "MODELED / LITERATURE-SHAPED PRODUCT PRIOR; walk-bout quantiles are authored from reported qualitative time scales, not fitted biological constants";
pub const N41_NATURAL_FLIGHT_DYNAMICS_CLAIM: &str = "MODELED / LITERATURE-SHAPED PRODUCT PRIOR; walk and flight quantiles, uncued course selection, and local edge avoidance are authored from qualitative evidence and product constraints, not fitted biological constants; this is not a food-search or territory-coverage model";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FatiguePolicy {
    #[default]
    HardGate,
    GradedResponse,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BehaviorParameterProfile {
    #[default]
    N4,
    N41A,
    N41B,
    N41BNatural,
    N41BNaturalFlight,
    N41C,
}

impl BehaviorParameterProfile {
    pub const ALL: [Self; 6] = [
        Self::N4,
        Self::N41A,
        Self::N41B,
        Self::N41BNatural,
        Self::N41BNaturalFlight,
        Self::N41C,
    ];

    pub const fn cli(self) -> &'static str {
        match self {
            Self::N4 => "n4",
            Self::N41A => "n41-a",
            Self::N41B => "n41-b",
            Self::N41BNatural => "n41-b-natural",
            Self::N41BNaturalFlight => "n41-b-natural-flight",
            Self::N41C => "n41-c",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "n4" => Ok(Self::N4),
            "n41-a" => Ok(Self::N41A),
            "n41-b" => Ok(Self::N41B),
            "n41-b-natural" => Ok(Self::N41BNatural),
            "n41-b-natural-flight" => Ok(Self::N41BNaturalFlight),
            "n41-c" => Ok(Self::N41C),
            _ => Err(format!(
                "unknown behavior parameter profile {value:?}; expected n4, n41-a, n41-b, n41-b-natural, n41-b-natural-flight, or n41-c"
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DurationParameters {
    pub minimum_frames: u32,
    pub low_frames: u32,
    pub high_frames: u32,
    pub refractory_frames: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BehaviorParameters {
    pub schema_version: u32,
    pub parameter_set_id: String,
    pub claim: String,
    pub behavior_order: [String; 9],
    pub duration_key_salt: u64,
    pub durations: [DurationParameters; 9],
    /// Optional event-keyed quantile table for Walk bouts. Empty preserves the
    /// frozen bounded-uniform N4/N4.1-A/B/C behavior exactly.
    #[serde(default)]
    pub walk_duration_quantiles_frames: Vec<u32>,
    /// Optional event-keyed quantile table for Flight bouts. It is populated
    /// only by the additive natural-flight profile; every frozen profile keeps
    /// the previously validated 121-frame envelope.
    #[serde(default)]
    pub flight_duration_quantiles_frames: Vec<u32>,
    pub settle_frames: u32,
    pub ordinary_on_q15: i32,
    pub ordinary_off_q15: i32,
    pub loom_on_q15: i32,
    pub spike_on_per_10k: u32,
    pub spike_off_per_10k: u32,
    pub context_max_q15: i32,
    pub exploration_initial_q15: i32,
    pub exploration_on_q15: i32,
    pub contamination_on_q15: i32,
    pub fatigue_rest_q15: i32,
    pub fatigue_critical_q15: i32,
    #[serde(default)]
    pub fatigue_policy: FatiguePolicy,
    #[serde(default)]
    pub fatigue_suppression_onset_q15: i32,
    #[serde(default)]
    pub fatigue_suppression_full_q15: i32,
    #[serde(default)]
    pub fatigue_min_response_q15: i32,
    pub arousal_on_q15: i32,
    pub arousal_divisor: i32,
    pub fatigue_delta_per_frame: [i32; 9],
    pub exploration_delta_per_frame: [i32; 9],
    pub contamination_delta_per_frame: [i32; 9],
}

impl BehaviorParameters {
    pub fn validate(&self) -> Result<(), String> {
        let order = [
            "rest",
            "quiet",
            "walk",
            "reverse",
            "groom",
            "alert",
            "pre_escape",
            "flight",
            "landing",
        ];
        let identity_valid = match self.fatigue_policy {
            FatiguePolicy::HardGate => {
                self.schema_version == 1
                    && self.parameter_set_id == "n4-engineering-v1"
                    && self.fatigue_suppression_onset_q15 == 0
                    && self.fatigue_suppression_full_q15 == 0
                    && self.fatigue_min_response_q15 == 0
            }
            FatiguePolicy::GradedResponse => {
                ((self.schema_version == 2
                    && matches!(
                        self.parameter_set_id.as_str(),
                        "n4.1-soft-fatigue-a-responsive-v1"
                            | "n4.1-soft-fatigue-b-balanced-v1"
                            | "n4.1-soft-fatigue-c-conservative-v1"
                    ))
                    || (self.schema_version == 3
                        && self.parameter_set_id == "n4.1-soft-fatigue-b-natural-bouts-v2")
                    || (self.schema_version == 4
                        && self.parameter_set_id == "n4.1-soft-fatigue-b-natural-flight-v3"))
                    && self.fatigue_suppression_onset_q15 > 0
                    && self.fatigue_suppression_onset_q15 < self.fatigue_suppression_full_q15
                    && self.fatigue_suppression_full_q15 <= self.context_max_q15
                    && self.fatigue_min_response_q15 > 0
                    && self.fatigue_min_response_q15 < self.context_max_q15
                    && self.fatigue_critical_q15 == self.context_max_q15
            }
        };
        if !identity_valid
            || self.behavior_order.iter().map(String::as_str).ne(order)
            || self.context_max_q15 != 32_767
            || self.arousal_divisor < 1
            || self.ordinary_off_q15 < 0
            || self.ordinary_off_q15 >= self.ordinary_on_q15
            || self.ordinary_on_q15 > 32_767
            || !(1..=32_767).contains(&self.loom_on_q15)
            || self.spike_off_per_10k >= self.spike_on_per_10k
            || self.spike_on_per_10k > 10_000
            || self.settle_frames != 15
        {
            return Err("invalid N4/N4.1 parameter schema, ordering, or thresholds".into());
        }
        for d in self.durations {
            if d.minimum_frames == 0
                || d.minimum_frames > d.low_frames
                || d.low_frames > d.high_frames
                || d.high_frames > 18_182
                || d.refractory_frames > 18_182
            {
                return Err("invalid N4 duration bounds".into());
            }
        }
        let walk = self.for_behavior(Behavior::Walk);
        let natural_walk_bouts = matches!(
            self.parameter_set_id.as_str(),
            "n4.1-soft-fatigue-b-natural-bouts-v2" | "n4.1-soft-fatigue-b-natural-flight-v3"
        );
        if natural_walk_bouts {
            if self.walk_duration_quantiles_frames.len() != 128
                || self.walk_duration_quantiles_frames.first() != Some(&walk.low_frames)
                || self.walk_duration_quantiles_frames.last() != Some(&walk.high_frames)
                || self
                    .walk_duration_quantiles_frames
                    .windows(2)
                    .any(|pair| pair[0] > pair[1])
                || self
                    .walk_duration_quantiles_frames
                    .iter()
                    .any(|value| !(walk.low_frames..=walk.high_frames).contains(value))
            {
                return Err("invalid natural walk-bout quantile table".into());
            }
        } else if !self.walk_duration_quantiles_frames.is_empty() {
            return Err("frozen N4/N4.1-A/B/C profiles cannot carry walk quantiles".into());
        }
        let flight = self.for_behavior(Behavior::Flight);
        let natural_flight = self.parameter_set_id == "n4.1-soft-fatigue-b-natural-flight-v3";
        if natural_flight {
            if self.flight_duration_quantiles_frames.len() != 128
                || self.flight_duration_quantiles_frames.first() != Some(&flight.low_frames)
                || self.flight_duration_quantiles_frames.last() != Some(&flight.high_frames)
                || self
                    .flight_duration_quantiles_frames
                    .windows(2)
                    .any(|pair| pair[0] > pair[1])
                || self
                    .flight_duration_quantiles_frames
                    .iter()
                    .any(|value| !(flight.low_frames..=flight.high_frames).contains(value))
                || flight.minimum_frames != 18
                || flight.low_frames != 18
                || flight.high_frames != 242
            {
                return Err("invalid natural flight-bout quantile table".into());
            }
        } else if !self.flight_duration_quantiles_frames.is_empty() {
            return Err("frozen N4/N4.1 profiles cannot carry flight quantiles".into());
        }
        if u64::from(self.for_behavior(Behavior::Groom).minimum_frames)
            * u64::from(crate::MODEL_STEP_MS)
            < 1_500
        {
            return Err("grooming engineering floor must be at least 1500 ms".into());
        }
        for (b, n) in [(Behavior::PreEscape, 6), (Behavior::Landing, 15)] {
            let d = self.for_behavior(b);
            if d.minimum_frames != n || d.low_frames != n || d.high_frames != n {
                return Err("escape envelopes must retain the validated motion timings".into());
            }
        }
        if !natural_flight {
            let d = self.for_behavior(Behavior::Flight);
            if d.minimum_frames != 121 || d.low_frames != 121 || d.high_frames != 121 {
                return Err("frozen flight envelope must remain 121 frames".into());
            }
        }
        for v in [
            self.exploration_initial_q15,
            self.exploration_on_q15,
            self.contamination_on_q15,
            self.fatigue_rest_q15,
            self.fatigue_critical_q15,
            self.arousal_on_q15,
        ] {
            if !(0..=self.context_max_q15).contains(&v) {
                return Err("context threshold out of range".into());
            }
        }
        if self.fatigue_rest_q15 >= self.fatigue_critical_q15
            || (self.fatigue_policy == FatiguePolicy::GradedResponse
                && self.fatigue_rest_q15 >= self.fatigue_suppression_full_q15)
        {
            return Err("invalid fatigue ordering".into());
        }
        for v in self
            .fatigue_delta_per_frame
            .iter()
            .chain(&self.exploration_delta_per_frame)
            .chain(&self.contamination_delta_per_frame)
        {
            if !(-1024..=1024).contains(v) {
                return Err("context rate outside declared bound".into());
            }
        }
        Ok(())
    }

    pub fn for_behavior(&self, behavior: Behavior) -> DurationParameters {
        self.durations[behavior as usize]
    }
}

pub fn parameters() -> &'static BehaviorParameters {
    parameters_for_profile(BehaviorParameterProfile::N4)
}

pub fn parameters_for_profile(profile: BehaviorParameterProfile) -> &'static BehaviorParameters {
    static N4: OnceLock<BehaviorParameters> = OnceLock::new();
    static N41_A: OnceLock<BehaviorParameters> = OnceLock::new();
    static N41_B: OnceLock<BehaviorParameters> = OnceLock::new();
    static N41_B_NATURAL: OnceLock<BehaviorParameters> = OnceLock::new();
    static N41_B_NATURAL_FLIGHT: OnceLock<BehaviorParameters> = OnceLock::new();
    static N41_C: OnceLock<BehaviorParameters> = OnceLock::new();
    let (cell, json) = match profile {
        BehaviorParameterProfile::N4 => (&N4, PARAMETERS_JSON),
        BehaviorParameterProfile::N41A => (&N41_A, N41_A_PARAMETERS_JSON),
        BehaviorParameterProfile::N41B => (&N41_B, N41_B_PARAMETERS_JSON),
        BehaviorParameterProfile::N41BNatural => (&N41_B_NATURAL, N41_B_NATURAL_PARAMETERS_JSON),
        BehaviorParameterProfile::N41BNaturalFlight => {
            (&N41_B_NATURAL_FLIGHT, N41_B_NATURAL_FLIGHT_PARAMETERS_JSON)
        }
        BehaviorParameterProfile::N41C => (&N41_C, N41_C_PARAMETERS_JSON),
    };
    cell.get_or_init(|| {
        let value: BehaviorParameters =
            serde_json::from_str(json).expect("embedded behavior parameter JSON");
        value
            .validate()
            .expect("embedded N4/N4.1 parameter constraints");
        value
    })
}

pub fn profile_for_parameter_sha256(sha256: &str) -> Option<BehaviorParameterProfile> {
    BehaviorParameterProfile::ALL
        .into_iter()
        .find(|profile| parameter_sha256_for(*profile) == sha256)
}

pub fn parameters_for_sha256(sha256: &str) -> Option<&'static BehaviorParameters> {
    profile_for_parameter_sha256(sha256).map(parameters_for_profile)
}

pub fn dynamics_version_for_sha256(sha256: &str) -> Option<&'static str> {
    profile_for_parameter_sha256(sha256).map(|profile| match profile {
        BehaviorParameterProfile::N4 => DYNAMICS_VERSION,
        BehaviorParameterProfile::N41A
        | BehaviorParameterProfile::N41B
        | BehaviorParameterProfile::N41C => N41_DYNAMICS_VERSION,
        BehaviorParameterProfile::N41BNatural => N41_NATURAL_BOUT_DYNAMICS_VERSION,
        BehaviorParameterProfile::N41BNaturalFlight => N41_NATURAL_FLIGHT_DYNAMICS_VERSION,
    })
}

/// Ordinary SHA-256 of the exact parameter artifact bytes (no custom framing).
pub fn parameter_sha256() -> &'static str {
    parameter_sha256_for(BehaviorParameterProfile::N4)
}

pub fn parameter_sha256_for(profile: BehaviorParameterProfile) -> &'static str {
    static N4: OnceLock<String> = OnceLock::new();
    static N41_A: OnceLock<String> = OnceLock::new();
    static N41_B: OnceLock<String> = OnceLock::new();
    static N41_B_NATURAL: OnceLock<String> = OnceLock::new();
    static N41_B_NATURAL_FLIGHT: OnceLock<String> = OnceLock::new();
    static N41_C: OnceLock<String> = OnceLock::new();
    let (cell, json) = match profile {
        BehaviorParameterProfile::N4 => (&N4, PARAMETERS_JSON),
        BehaviorParameterProfile::N41A => (&N41_A, N41_A_PARAMETERS_JSON),
        BehaviorParameterProfile::N41B => (&N41_B, N41_B_PARAMETERS_JSON),
        BehaviorParameterProfile::N41BNatural => (&N41_B_NATURAL, N41_B_NATURAL_PARAMETERS_JSON),
        BehaviorParameterProfile::N41BNaturalFlight => {
            (&N41_B_NATURAL_FLIGHT, N41_B_NATURAL_FLIGHT_PARAMETERS_JSON)
        }
        BehaviorParameterProfile::N41C => (&N41_C, N41_C_PARAMETERS_JSON),
    };
    cell.get_or_init(|| format!("{:x}", Sha256::digest(json.as_bytes())))
}

/// Stable, non-cryptographic event-keyed draw. No mutable or wall-clock RNG.
pub fn duration_draw(seed: u64, sequence: u64, behavior: Behavior, bucket: u16) -> (u64, u32) {
    duration_draw_for(parameters(), seed, sequence, behavior, bucket)
}

pub fn duration_draw_for(
    p: &BehaviorParameters,
    seed: u64,
    sequence: u64,
    behavior: Behavior,
    bucket: u16,
) -> (u64, u32) {
    let key = mix(seed
        ^ sequence.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (behavior as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93)
        ^ u64::from(bucket).rotate_left(33)
        ^ p.duration_key_salt);
    let d = p.for_behavior(behavior);
    if behavior == Behavior::Walk && !p.walk_duration_quantiles_frames.is_empty() {
        let count = p.walk_duration_quantiles_frames.len() as u64;
        let index = ((u128::from(key) * u128::from(count)) >> 64) as usize;
        return (key, p.walk_duration_quantiles_frames[index]);
    }
    if behavior == Behavior::Flight && !p.flight_duration_quantiles_frames.is_empty() {
        let count = p.flight_duration_quantiles_frames.len() as u64;
        let index = ((u128::from(key) * u128::from(count)) >> 64) as usize;
        return (key, p.flight_duration_quantiles_frames[index]);
    }
    let span = u64::from(d.high_frames - d.low_frames) + 1;
    // Multiply-high mapping: bounded integer rounding, never floating point.
    let offset = ((u128::from(key) * u128::from(span)) >> 64) as u32;
    (key, d.low_frames + offset)
}

pub fn fatigue_response_draw_q15(
    p: &BehaviorParameters,
    seed: u64,
    sequence: u64,
    behavior: Behavior,
    bucket: u16,
) -> i32 {
    let key = mix(seed
        ^ sequence.wrapping_mul(0xA24B_AED4_963E_E407)
        ^ (behavior as u64).wrapping_mul(0x9FB2_1C65_1E98_DF25)
        ^ u64::from(bucket).rotate_left(19)
        ^ p.duration_key_salt
        ^ 0x4E34_312D_4641_5449);
    ((key >> 49) & 0x7fff) as i32
}

fn mix(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

/// Plain file-byte SHA-256 for scientific artifact identities.
pub fn artifact_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
