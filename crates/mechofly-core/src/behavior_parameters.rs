//! Versioned initialization priors. These are not fitted ethological constants.
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::model::Behavior;

pub const PARAMETERS_JSON: &str = include_str!("../parameters/n4-engineering-v1.json");
pub const DYNAMICS_VERSION: &str = "n4-explicit-duration-engineering-v1";
pub const DYNAMICS_CLAIM: &str =
    "MODELED / ENGINEERING PRIOR; context and duration rules are authored, not biologically fitted";

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
        if self.schema_version != 1
            || self.parameter_set_id != "n4-engineering-v1"
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
            return Err("invalid N4 parameter schema, ordering, or thresholds".into());
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
        if self.fatigue_rest_q15 >= self.fatigue_critical_q15 {
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
    static VALUE: OnceLock<BehaviorParameters> = OnceLock::new();
    VALUE.get_or_init(|| {
        let value: BehaviorParameters =
            serde_json::from_str(PARAMETERS_JSON).expect("embedded parameter JSON");
        value.validate().expect("embedded N4 parameter constraints");
        value
    })
}

/// Ordinary SHA-256 of the exact parameter artifact bytes (no custom framing).
pub fn parameter_sha256() -> &'static str {
    static VALUE: OnceLock<String> = OnceLock::new();
    VALUE.get_or_init(|| format!("{:x}", Sha256::digest(PARAMETERS_JSON.as_bytes())))
}

/// Stable, non-cryptographic event-keyed draw. No mutable or wall-clock RNG.
pub fn duration_draw(seed: u64, sequence: u64, behavior: Behavior, bucket: u16) -> (u64, u32) {
    let p = parameters();
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

fn mix(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

/// Plain file-byte SHA-256 for scientific artifact identities.
pub fn artifact_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
