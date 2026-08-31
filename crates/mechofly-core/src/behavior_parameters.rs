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
pub const N41_C_PARAMETERS_JSON: &str =
    include_str!("../parameters/n4.1-soft-fatigue-c-conservative-v1.json");
pub const DYNAMICS_VERSION: &str = "n4-explicit-duration-engineering-v1";
pub const N41_DYNAMICS_VERSION: &str = "n4.1-graded-fatigue-engineering-v1";
pub const DYNAMICS_CLAIM: &str =
    "MODELED / ENGINEERING PRIOR; context and duration rules are authored, not biologically fitted";

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
    N41C,
}

impl BehaviorParameterProfile {
    pub const ALL: [Self; 4] = [Self::N4, Self::N41A, Self::N41B, Self::N41C];

    pub const fn cli(self) -> &'static str {
        match self {
            Self::N4 => "n4",
            Self::N41A => "n41-a",
            Self::N41B => "n41-b",
            Self::N41C => "n41-c",
        }
    }

    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "n4" => Ok(Self::N4),
            "n41-a" => Ok(Self::N41A),
            "n41-b" => Ok(Self::N41B),
            "n41-c" => Ok(Self::N41C),
            _ => Err(format!(
                "unknown behavior parameter profile {value:?}; expected n4, n41-a, n41-b, or n41-c"
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
                self.schema_version == 2
                    && matches!(
                        self.parameter_set_id.as_str(),
                        "n4.1-soft-fatigue-a-responsive-v1"
                            | "n4.1-soft-fatigue-b-balanced-v1"
                            | "n4.1-soft-fatigue-c-conservative-v1"
                    )
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
        if u64::from(self.for_behavior(Behavior::Groom).minimum_frames)
            * u64::from(crate::MODEL_STEP_MS)
            < 1_500
        {
            return Err("grooming engineering floor must be at least 1500 ms".into());
        }
        for (b, n) in [
            (Behavior::PreEscape, 6),
            (Behavior::Flight, 121),
            (Behavior::Landing, 15),
        ] {
            let d = self.for_behavior(b);
            if d.minimum_frames != n || d.low_frames != n || d.high_frames != n {
                return Err("escape envelopes must retain the validated motion timings".into());
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
    static N41_C: OnceLock<BehaviorParameters> = OnceLock::new();
    let (cell, json) = match profile {
        BehaviorParameterProfile::N4 => (&N4, PARAMETERS_JSON),
        BehaviorParameterProfile::N41A => (&N41_A, N41_A_PARAMETERS_JSON),
        BehaviorParameterProfile::N41B => (&N41_B, N41_B_PARAMETERS_JSON),
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
    static N41_C: OnceLock<String> = OnceLock::new();
    let (cell, json) = match profile {
        BehaviorParameterProfile::N4 => (&N4, PARAMETERS_JSON),
        BehaviorParameterProfile::N41A => (&N41_A, N41_A_PARAMETERS_JSON),
        BehaviorParameterProfile::N41B => (&N41_B, N41_B_PARAMETERS_JSON),
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
