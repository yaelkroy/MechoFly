use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::{MODEL_STEP_MS, model::Behavior, provenance::sha256_hex};

pub const BEHAVIOR_TELEMETRY_SCHEMA_VERSION: u32 = 1;
pub const BEHAVIOR_TELEMETRY_CONTROLLER: &str = "legacy-threshold-hold-v1-observed";
pub const BEHAVIOR_TELEMETRY_CLAIM_BOUNDARY: &str = "observational telemetry for the existing controller; no controller threshold, hold, priority, autonomous schedule, neural state, or visible behavior is changed";
pub const MAX_BEHAVIOR_TRANSITION_EVENTS: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BehaviorTransitionReason {
    PreEscapeCompleted,
    FlightCompleted,
    LandingCompleted,
    LoomPopulationThreshold,
    GroomPopulationThreshold,
    AlertPopulationThreshold,
    ReversePopulationThreshold,
    WalkPopulationThreshold,
    SpikeRateThreshold,
    LegacyAutonomousSchedule,
}

impl BehaviorTransitionReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PreEscapeCompleted => "pre_escape_completed",
            Self::FlightCompleted => "flight_completed",
            Self::LandingCompleted => "landing_completed",
            Self::LoomPopulationThreshold => "loom_population_threshold",
            Self::GroomPopulationThreshold => "groom_population_threshold",
            Self::AlertPopulationThreshold => "alert_population_threshold",
            Self::ReversePopulationThreshold => "reverse_population_threshold",
            Self::WalkPopulationThreshold => "walk_population_threshold",
            Self::SpikeRateThreshold => "spike_rate_threshold",
            Self::LegacyAutonomousSchedule => "legacy_autonomous_schedule",
        }
    }

    pub const fn emergency_override(self) -> bool {
        matches!(self, Self::LoomPopulationThreshold)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BehaviorIntentSnapshot {
    pub schema_version: u32,
    pub frame: u64,
    pub current_behavior: Behavior,
    pub current_behavior_age_frames: u32,
    pub spike_count: u32,
    pub spike_rate_per_10k: u32,
    pub spike_alert_threshold_per_10k: u32,
    pub autonomous_schedule_slot: u8,
    pub loom_activation_q15: i32,
    pub groom_activation_q15: i32,
    pub alert_activation_q15: i32,
    pub reverse_activation_q15: i32,
    pub walk_activation_q15: i32,
    pub loom_entry_threshold_q15: i32,
    pub authored_behavior_entry_threshold_q15: i32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BehaviorTransitionEvent {
    pub schema_version: u32,
    pub sequence: u64,
    pub frame: u64,
    pub tick_ms: u64,
    pub from_behavior: Behavior,
    pub to_behavior: Behavior,
    pub elapsed_frames: u32,
    pub elapsed_ms: u64,
    pub reason: BehaviorTransitionReason,
    pub emergency_override: bool,
    pub intent: BehaviorIntentSnapshot,
    pub pre_transition_state_digest: String,
    pub post_transition_state_digest: String,
}

impl BehaviorTransitionEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        frame: u64,
        from_behavior: Behavior,
        to_behavior: Behavior,
        elapsed_frames: u32,
        reason: BehaviorTransitionReason,
        intent: BehaviorIntentSnapshot,
        pre_transition_state_digest: String,
        post_transition_state_digest: String,
    ) -> Self {
        Self {
            schema_version: BEHAVIOR_TELEMETRY_SCHEMA_VERSION,
            sequence: 0,
            frame,
            tick_ms: frame.saturating_mul(u64::from(MODEL_STEP_MS)),
            from_behavior,
            to_behavior,
            elapsed_frames,
            elapsed_ms: u64::from(elapsed_frames).saturating_mul(u64::from(MODEL_STEP_MS)),
            reason,
            emergency_override: reason.emergency_override(),
            intent,
            pre_transition_state_digest,
            post_transition_state_digest,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BehaviorTelemetrySnapshot {
    pub schema_version: u32,
    pub controller: String,
    pub claim_boundary: String,
    pub observational_only: bool,
    pub controller_semantics_changed: bool,
    pub capacity: usize,
    pub complete_from_frame: u64,
    pub total_event_count: u64,
    pub retained_event_count: usize,
    pub dropped_event_count: u64,
    pub first_retained_sequence: Option<u64>,
    pub last_retained_sequence: Option<u64>,
    pub retained_sequence_contiguous: bool,
    pub event_stream_sha256: String,
    pub latest_intent: BehaviorIntentSnapshot,
    pub events: Vec<BehaviorTransitionEvent>,
}

#[derive(Clone, Debug)]
pub struct BehaviorTelemetryLedger {
    capacity: usize,
    complete_from_frame: u64,
    next_sequence: u64,
    dropped_event_count: u64,
    event_stream_sha256: String,
    events: VecDeque<BehaviorTransitionEvent>,
}

impl Default for BehaviorTelemetryLedger {
    fn default() -> Self {
        Self::new(0)
    }
}

impl BehaviorTelemetryLedger {
    pub fn new(complete_from_frame: u64) -> Self {
        Self::with_capacity(complete_from_frame, MAX_BEHAVIOR_TRANSITION_EVENTS)
    }

    pub fn with_capacity(complete_from_frame: u64, capacity: usize) -> Self {
        let capacity = capacity.clamp(1, MAX_BEHAVIOR_TRANSITION_EVENTS);
        Self {
            capacity,
            complete_from_frame,
            next_sequence: 0,
            dropped_event_count: 0,
            event_stream_sha256: sha256_hex([b"mechofly-behavior-transition-stream-v1"]),
            events: VecDeque::with_capacity(capacity),
        }
    }

    pub fn record(&mut self, mut event: BehaviorTransitionEvent) {
        event.sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(1);
        let encoded = serde_json::to_vec(&event)
            .expect("behavior transition telemetry must remain serializable");
        self.event_stream_sha256 =
            sha256_hex([self.event_stream_sha256.as_bytes(), encoded.as_slice()]);
        while self.events.len() >= self.capacity {
            self.events.pop_front();
            self.dropped_event_count = self.dropped_event_count.saturating_add(1);
        }
        self.events.push_back(event);
    }

    pub fn events(&self) -> impl DoubleEndedIterator<Item = &BehaviorTransitionEvent> {
        self.events.iter()
    }

    pub fn latest(&self) -> Option<&BehaviorTransitionEvent> {
        self.events.back()
    }

    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn retained_event_count(&self) -> usize {
        self.events.len()
    }

    pub const fn total_event_count(&self) -> u64 {
        self.next_sequence
    }

    pub const fn dropped_event_count(&self) -> u64 {
        self.dropped_event_count
    }

    pub fn event_stream_sha256(&self) -> &str {
        &self.event_stream_sha256
    }

    pub fn retained_sequence_contiguous(&self) -> bool {
        self.events
            .iter()
            .zip(self.events.iter().skip(1))
            .all(|(left, right)| right.sequence == left.sequence.saturating_add(1))
    }

    pub fn snapshot(&self, latest_intent: BehaviorIntentSnapshot) -> BehaviorTelemetrySnapshot {
        BehaviorTelemetrySnapshot {
            schema_version: BEHAVIOR_TELEMETRY_SCHEMA_VERSION,
            controller: BEHAVIOR_TELEMETRY_CONTROLLER.to_owned(),
            claim_boundary: BEHAVIOR_TELEMETRY_CLAIM_BOUNDARY.to_owned(),
            observational_only: true,
            controller_semantics_changed: false,
            capacity: self.capacity,
            complete_from_frame: self.complete_from_frame,
            total_event_count: self.next_sequence,
            retained_event_count: self.events.len(),
            dropped_event_count: self.dropped_event_count,
            first_retained_sequence: self.events.front().map(|event| event.sequence),
            last_retained_sequence: self.events.back().map(|event| event.sequence),
            retained_sequence_contiguous: self.retained_sequence_contiguous(),
            event_stream_sha256: self.event_stream_sha256.clone(),
            latest_intent,
            events: self.events.iter().cloned().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intent(frame: u64) -> BehaviorIntentSnapshot {
        BehaviorIntentSnapshot {
            schema_version: BEHAVIOR_TELEMETRY_SCHEMA_VERSION,
            frame,
            current_behavior: Behavior::Rest,
            current_behavior_age_frames: frame as u32,
            spike_count: 0,
            spike_rate_per_10k: 0,
            spike_alert_threshold_per_10k: 1_200,
            autonomous_schedule_slot: 0,
            loom_activation_q15: 0,
            groom_activation_q15: 0,
            alert_activation_q15: 0,
            reverse_activation_q15: 0,
            walk_activation_q15: 0,
            loom_entry_threshold_q15: 5_200,
            authored_behavior_entry_threshold_q15: 4_600,
        }
    }

    fn event(frame: u64) -> BehaviorTransitionEvent {
        BehaviorTransitionEvent::new(
            frame,
            Behavior::Rest,
            Behavior::Walk,
            1,
            BehaviorTransitionReason::LegacyAutonomousSchedule,
            intent(frame),
            format!("pre-{frame}"),
            format!("post-{frame}"),
        )
    }

    #[test]
    fn ledger_is_bounded_and_retains_monotonic_sequences() {
        let mut ledger = BehaviorTelemetryLedger::with_capacity(0, 3);
        for frame in 1..=5 {
            ledger.record(event(frame));
        }
        let snapshot = ledger.snapshot(intent(5));
        assert_eq!(snapshot.total_event_count, 5);
        assert_eq!(snapshot.retained_event_count, 3);
        assert_eq!(snapshot.dropped_event_count, 2);
        assert_eq!(snapshot.first_retained_sequence, Some(2));
        assert_eq!(snapshot.last_retained_sequence, Some(4));
        assert!(snapshot.retained_sequence_contiguous);
        assert_eq!(snapshot.events.len(), 3);
    }

    #[test]
    fn identical_event_streams_have_identical_digests() {
        let mut first = BehaviorTelemetryLedger::new(0);
        let mut second = BehaviorTelemetryLedger::new(0);
        for frame in 1..=12 {
            first.record(event(frame));
            second.record(event(frame));
        }
        assert_eq!(first.snapshot(intent(12)), second.snapshot(intent(12)));
    }
}
