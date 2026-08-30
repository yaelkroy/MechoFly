//! Read-only neural reductions. This module neither selects nor advances behavior.
//!
//! N3 preserves the legacy reduction rules: a functional population is the
//! maximum activation in its strided lane, not a mean or a measured firing rate.

use crate::model::{
    ACTIVATION_MIN, ALERT_POPULATION_OFFSET, FUNCTIONAL_POPULATION_COUNT, GROOM_POPULATION_OFFSET,
    LOOM_POPULATION_OFFSET, REVERSE_POPULATION_OFFSET, WALK_POPULATION_OFFSET,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NeuralEvidence {
    pub frame: u64,
    pub neuron_count: usize,
    pub spike_count: usize,
    pub mean_activation_q15: i32,
    pub spike_rate_per_10k: usize,
    pub loom_activation_q15: i32,
    pub groom_activation_q15: i32,
    pub alert_activation_q15: i32,
    pub reverse_activation_q15: i32,
    pub walk_activation_q15: i32,
}

impl NeuralEvidence {
    /// Collect the same integer evidence from CPU or GPU backend output.
    /// No allocation, filtering, clamping of population maxima, or hidden state.
    pub fn collect(frame: u64, activation: &[i32], spike_count: usize) -> Self {
        let mean_activation_q15 = if activation.is_empty() {
            0
        } else {
            (activation.iter().map(|value| *value as i64).sum::<i64>() / activation.len() as i64)
                as i32
        };
        Self {
            frame,
            neuron_count: activation.len(),
            spike_count,
            mean_activation_q15,
            spike_rate_per_10k: spike_count.saturating_mul(10_000) / activation.len().max(1),
            loom_activation_q15: population_maximum(activation, LOOM_POPULATION_OFFSET),
            groom_activation_q15: population_maximum(activation, GROOM_POPULATION_OFFSET),
            alert_activation_q15: population_maximum(activation, ALERT_POPULATION_OFFSET),
            reverse_activation_q15: population_maximum(activation, REVERSE_POPULATION_OFFSET),
            walk_activation_q15: population_maximum(activation, WALK_POPULATION_OFFSET),
        }
    }
}

fn population_maximum(activation: &[i32], offset: usize) -> i32 {
    activation
        .iter()
        .skip(offset)
        .step_by(FUNCTIONAL_POPULATION_COUNT)
        .copied()
        .max()
        .unwrap_or(ACTIVATION_MIN)
}
