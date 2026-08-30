//! Stateless conversion from neural evidence to the existing intent schema.
//!
//! Scores are legacy modeled population activations, not probabilities. N3
//! deliberately adds no smoothing, duration sampling, policy bias, or new inputs.

use crate::{
    behavior_telemetry::BEHAVIOR_TELEMETRY_SCHEMA_VERSION, model::Behavior,
    neural_evidence::NeuralEvidence,
};

pub const BEHAVIOR_PIPELINE_VERSION: &str = "n3-separated-legacy-v1";
pub(crate) const LOOM_ESCAPE_ACTIVATION_Q15: i32 = 5_200;
pub(crate) const AUTHORED_BEHAVIOR_ACTIVATION_Q15: i32 = 4_600;
pub(crate) const SPIKE_ALERT_THRESHOLD_PER_10K: u32 = 1_200;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BehaviorContext {
    pub current_behavior: Behavior,
    pub current_behavior_age_frames: u32,
}

// Retain the established type and wire format; this layer owns construction.
pub use crate::behavior_telemetry::BehaviorIntentSnapshot;

/// A stateless builder; the returned record is the selector's actual input.
pub struct BehaviorIntentBuilder;

impl BehaviorIntentBuilder {
    pub fn build(evidence: &NeuralEvidence, context: BehaviorContext) -> BehaviorIntentSnapshot {
        BehaviorIntentSnapshot {
            schema_version: BEHAVIOR_TELEMETRY_SCHEMA_VERSION,
            frame: evidence.frame,
            current_behavior: context.current_behavior,
            current_behavior_age_frames: context.current_behavior_age_frames,
            spike_count: evidence.spike_count.min(u32::MAX as usize) as u32,
            spike_rate_per_10k: evidence.spike_rate_per_10k.min(u32::MAX as usize) as u32,
            spike_alert_threshold_per_10k: SPIKE_ALERT_THRESHOLD_PER_10K,
            autonomous_schedule_slot: ((evidence.frame / 90) % 9) as u8,
            loom_activation_q15: evidence.loom_activation_q15,
            groom_activation_q15: evidence.groom_activation_q15,
            alert_activation_q15: evidence.alert_activation_q15,
            reverse_activation_q15: evidence.reverse_activation_q15,
            walk_activation_q15: evidence.walk_activation_q15,
            loom_entry_threshold_q15: LOOM_ESCAPE_ACTIVATION_Q15,
            authored_behavior_entry_threshold_q15: AUTHORED_BEHAVIOR_ACTIVATION_Q15,
        }
    }
}
