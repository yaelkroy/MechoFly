//! Controller-owned persistence and modeled internal context, in fixed 33 ms steps.
//! Macro actions never read wall-clock time or the legacy autonomous schedule slot.

use serde::{Deserialize, Serialize};

use crate::{
    behavior_intent::BehaviorIntentSnapshot,
    behavior_parameters::{
        BehaviorParameterProfile, BehaviorParameters, FatiguePolicy, duration_draw_for,
        fatigue_response_draw_q15, parameter_sha256_for, parameters, parameters_for_profile,
        parameters_for_sha256,
    },
    behavior_telemetry::BehaviorTransitionReason as Reason,
    model::Behavior,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MotorSubstate {
    #[default]
    None,
    Settling,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InternalContext {
    pub arousal_q15: i32,
    pub fatigue_q15: i32,
    pub contamination_q15: i32,
    pub exploration_q15: i32,
}

impl Default for InternalContext {
    fn default() -> Self {
        Self::new(parameters())
    }
}

impl InternalContext {
    fn new(parameters: &BehaviorParameters) -> Self {
        Self {
            arousal_q15: 0,
            fatigue_q15: 0,
            contamination_q15: 0,
            exploration_q15: parameters.exploration_initial_q15,
        }
    }

    pub fn bucket(self) -> u16 {
        let q = |v: i32| (v.clamp(0, 32_767) / 8_192) as u16;
        q(self.arousal_q15)
            | (q(self.fatigue_q15) << 2)
            | (q(self.contamination_q15) << 4)
            | (q(self.exploration_q15) << 6)
    }
    fn valid(self) -> bool {
        [
            self.arousal_q15,
            self.fatigue_q15,
            self.contamination_q15,
            self.exploration_q15,
        ]
        .iter()
        .all(|x| (0..=32_767).contains(x))
    }
    fn advance(
        &mut self,
        p: &BehaviorParameters,
        behavior: Behavior,
        intent: &BehaviorIntentSnapshot,
    ) {
        let index = behavior as usize;
        let add = |value: i32, delta: i32| (value + delta).clamp(0, p.context_max_q15);
        self.fatigue_q15 = add(self.fatigue_q15, p.fatigue_delta_per_frame[index]);
        self.exploration_q15 = add(self.exploration_q15, p.exploration_delta_per_frame[index]);
        self.contamination_q15 = add(
            self.contamination_q15,
            p.contamination_delta_per_frame[index],
        );
        let target = intent
            .alert_activation_q15
            .max(intent.loom_activation_q15)
            .clamp(0, 8_191)
            * 4;
        self.arousal_q15 += (target - self.arousal_q15) / p.arousal_divisor;
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BehaviorDynamicsState {
    pub schema_version: u32,
    pub parameter_sha256: String,
    pub current_macro_state: Behavior,
    pub current_substate: MotorSubstate,
    pub entered_at_frame: u64,
    pub last_frame: u64,
    pub elapsed_frames: u32,
    pub minimum_dwell_frames: u32,
    pub target_duration_frames: u32,
    pub transition_sequence: u64,
    pub interruptible: bool,
    pub refractory_until_frame: [u64; 9],
    pub last_transition_reason: Reason,
    pub last_transition_evidence: BehaviorIntentSnapshot,
    pub duration_context_bucket: u16,
    pub deterministic_duration_draw: u64,
    /// Walk, reverse, groom, alert. These latches are part of checkpoint state.
    pub active_intents: [bool; 4],
    pub spike_alert_latched: bool,
    pub context: InternalContext,
    pub fault_latched: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicsTransition {
    pub parameter_sha256: String,
    pub controller_sequence: u64,
    pub from_substate: MotorSubstate,
    pub to_substate: MotorSubstate,
    pub minimum_dwell_frames: u32,
    pub target_duration_frames: u32,
    pub exited_duration_draw: u64,
    pub next_duration_draw: u64,
    pub next_target_duration_frames: u32,
    pub context: InternalContext,
}

#[derive(Clone, Debug)]
pub struct DynamicsDecision {
    pub behavior: Behavior,
    pub reason: Option<Reason>,
    pub transition: Option<DynamicsTransition>,
}

impl BehaviorDynamicsState {
    pub fn new(seed: u64, intent: BehaviorIntentSnapshot) -> Self {
        Self::new_with_profile(seed, intent, BehaviorParameterProfile::N4)
    }

    pub fn new_with_profile(
        seed: u64,
        intent: BehaviorIntentSnapshot,
        profile: BehaviorParameterProfile,
    ) -> Self {
        let p = parameters_for_profile(profile);
        let context = InternalContext::new(p);
        let bucket = context.bucket();
        let (draw, target) = duration_draw_for(p, seed, 0, Behavior::Quiet, bucket);
        Self {
            schema_version: 1,
            parameter_sha256: parameter_sha256_for(profile).to_owned(),
            current_macro_state: Behavior::Quiet,
            current_substate: MotorSubstate::None,
            entered_at_frame: intent.frame,
            last_frame: intent.frame,
            elapsed_frames: 0,
            minimum_dwell_frames: p.for_behavior(Behavior::Quiet).minimum_frames,
            target_duration_frames: target,
            transition_sequence: 0,
            interruptible: true,
            refractory_until_frame: [0; 9],
            last_transition_reason: Reason::QuietFallback,
            last_transition_evidence: intent,
            duration_context_bucket: bucket,
            deterministic_duration_draw: draw,
            active_intents: [false; 4],
            spike_alert_latched: false,
            context,
            fault_latched: false,
        }
    }

    pub fn validate(
        &self,
        seed: u64,
        frame: u64,
        behavior: Behavior,
        age: u32,
    ) -> Result<(), String> {
        let p = parameters_for_sha256(&self.parameter_sha256)
            .ok_or("unknown N4/N4.1 parameter identity")?;
        if self.schema_version != 1
            || self.current_macro_state != behavior
            || self.last_frame != frame
            || self.elapsed_frames != age
            || self.entered_at_frame > frame
            || frame - self.entered_at_frame != u64::from(age)
            || !self.context.valid()
            || self.duration_context_bucket > 255
            || self.interruptible == airborne(behavior)
            || (self.current_substate == MotorSubstate::Settling && behavior != Behavior::Rest)
        {
            return Err("invalid, missing or inconsistent N4/N4.1 causal state".into());
        }
        let d = p.for_behavior(behavior);
        let (key, target) = duration_draw_for(
            p,
            seed,
            self.transition_sequence,
            behavior,
            self.duration_context_bucket,
        );
        let expected_min = if self.current_substate == MotorSubstate::Settling {
            p.settle_frames
        } else {
            d.minimum_frames
        };
        let expected_target = if self.current_substate == MotorSubstate::Settling {
            p.settle_frames
        } else {
            target
        };
        if self.minimum_dwell_frames != expected_min
            || self.target_duration_frames != expected_target
            || self.deterministic_duration_draw != key
        {
            return Err(
                "N4/N4.1 duration draw or declared dwell does not match the parameter artifact"
                    .into(),
            );
        }
        Ok(())
    }

    /// One authoritative model step. Invalid state latches a quiet safety state.
    pub fn advance(&mut self, seed: u64, intent: BehaviorIntentSnapshot) -> DynamicsDecision {
        let expected_previous = intent.frame.checked_sub(1);
        if expected_previous.is_none()
            || self
                .validate(
                    seed,
                    expected_previous.unwrap_or(0),
                    intent.current_behavior,
                    intent.current_behavior_age_frames,
                )
                .is_err()
            || self.elapsed_frames == u32::MAX
            || self.transition_sequence == u64::MAX
        {
            return self.fail_quiet(seed, intent);
        }
        self.last_frame = intent.frame;
        self.elapsed_frames += 1;
        if self.fault_latched {
            return self.hold();
        }
        let Some(p) = parameters_for_sha256(&self.parameter_sha256) else {
            return self.fail_quiet(seed, intent);
        };
        self.context.advance(p, self.current_macro_state, &intent);
        self.update_latches(p, &intent);
        // Validated neural looming always wins over ordinary dwell and cooldown.
        // An already running escape sequence completes; it is never restarted per tick.
        if intent.loom_activation_q15 >= p.loom_on_q15 && !airborne(self.current_macro_state) {
            return self.enter(
                seed,
                intent,
                Behavior::PreEscape,
                MotorSubstate::None,
                Reason::LoomPopulationThreshold,
            );
        }
        let elapsed = self.elapsed_frames;
        if airborne(self.current_macro_state) {
            if elapsed < self.target_duration_frames {
                return self.hold();
            }
            let (next, sub, reason) = match self.current_macro_state {
                Behavior::PreEscape => (
                    Behavior::Flight,
                    MotorSubstate::None,
                    Reason::PreEscapeCompleted,
                ),
                Behavior::Flight => (
                    Behavior::Landing,
                    MotorSubstate::None,
                    Reason::FlightCompleted,
                ),
                _ => (
                    Behavior::Rest,
                    MotorSubstate::Settling,
                    Reason::LandingCompleted,
                ),
            };
            return self.enter(seed, intent, next, sub, reason);
        }
        if elapsed < self.minimum_dwell_frames {
            return self.hold();
        }
        if self.current_substate == MotorSubstate::Settling {
            return self.enter(
                seed,
                intent,
                Behavior::Quiet,
                MotorSubstate::None,
                Reason::SettlingCompleted,
            );
        }
        let expired = elapsed >= self.target_duration_frames;
        let exclude = expired.then_some(self.current_macro_state);
        if let Some((next, reason)) = self.external_candidate(seed, p, exclude) {
            if next != self.current_macro_state {
                return self.enter(seed, intent, next, MotorSubstate::None, reason);
            }
            if !expired {
                return self.hold();
            }
        }
        // Once entered, an ordinary bout persists to its sampled review time.
        // A competing external intent can replace it only after minimum dwell.
        if !expired {
            return self.hold();
        }
        let (next, reason) = self.context_candidate(p, exclude);
        self.enter(seed, intent, next, MotorSubstate::None, reason)
    }

    fn hold(&self) -> DynamicsDecision {
        DynamicsDecision {
            behavior: self.current_macro_state,
            reason: None,
            transition: None,
        }
    }

    fn update_latches(&mut self, p: &BehaviorParameters, intent: &BehaviorIntentSnapshot) {
        for (i, value) in [
            intent.walk_activation_q15,
            intent.reverse_activation_q15,
            intent.groom_activation_q15,
            intent.alert_activation_q15,
        ]
        .into_iter()
        .enumerate()
        {
            if value >= p.ordinary_on_q15 {
                self.active_intents[i] = true;
            } else if value <= p.ordinary_off_q15 {
                self.active_intents[i] = false;
            }
        }
        if intent.spike_rate_per_10k > p.spike_on_per_10k {
            self.spike_alert_latched = true;
        } else if intent.spike_rate_per_10k <= p.spike_off_per_10k {
            self.spike_alert_latched = false;
        }
    }

    fn eligible(&self, behavior: Behavior, exclude: Option<Behavior>) -> bool {
        exclude != Some(behavior)
            && self.last_frame >= self.refractory_until_frame[behavior as usize]
    }

    fn external_candidate(
        &self,
        seed: u64,
        p: &BehaviorParameters,
        exclude: Option<Behavior>,
    ) -> Option<(Behavior, Reason)> {
        if p.fatigue_policy == FatiguePolicy::HardGate
            && self.context.fatigue_q15 >= p.fatigue_critical_q15
        {
            return None;
        }
        for (index, behavior, reason) in [
            (2, Behavior::Groom, Reason::GroomPopulationThreshold),
            (3, Behavior::Alert, Reason::AlertPopulationThreshold),
            (1, Behavior::Reverse, Reason::ReversePopulationThreshold),
            (0, Behavior::Walk, Reason::WalkPopulationThreshold),
        ] {
            if (self.active_intents[index] || (index == 3 && self.spike_alert_latched))
                && self.eligible(behavior, exclude)
                && self.fatigue_response_allows(p, seed, behavior)
            {
                let reason = if index == 3 && !self.active_intents[3] {
                    Reason::SpikeRateThreshold
                } else {
                    reason
                };
                return Some((behavior, reason));
            }
        }
        None
    }

    fn context_candidate(
        &self,
        p: &BehaviorParameters,
        exclude: Option<Behavior>,
    ) -> (Behavior, Reason) {
        let c = self.context;
        if c.fatigue_q15 >= p.fatigue_rest_q15 && self.eligible(Behavior::Rest, exclude) {
            return (Behavior::Rest, Reason::FatigueRecovery);
        }
        if c.contamination_q15 >= p.contamination_on_q15 && self.eligible(Behavior::Groom, exclude)
        {
            return (Behavior::Groom, Reason::ModeledContamination);
        }
        if c.exploration_q15 >= p.exploration_on_q15
            && c.fatigue_q15 < p.fatigue_rest_q15
            && self.eligible(Behavior::Walk, exclude)
        {
            return (Behavior::Walk, Reason::ModeledExploration);
        }
        if c.arousal_q15 >= p.arousal_on_q15 && self.eligible(Behavior::Alert, exclude) {
            return (Behavior::Alert, Reason::ModeledArousal);
        }
        if self.current_macro_state == Behavior::Quiet {
            (Behavior::Rest, Reason::DurationCompleted)
        } else {
            (Behavior::Quiet, Reason::QuietFallback)
        }
    }

    fn enter(
        &mut self,
        seed: u64,
        intent: BehaviorIntentSnapshot,
        next: Behavior,
        substate: MotorSubstate,
        reason: Reason,
    ) -> DynamicsDecision {
        let p = parameters_for_sha256(&self.parameter_sha256)
            .expect("validated controller parameter identity");
        let bucket = self.context.bucket();
        let sequence = self.transition_sequence + 1;
        let (draw, target) = duration_draw_for(p, seed, sequence, next, bucket);
        let target = if substate == MotorSubstate::Settling {
            p.settle_frames
        } else {
            target
        };
        let transition = DynamicsTransition {
            parameter_sha256: self.parameter_sha256.clone(),
            controller_sequence: sequence,
            from_substate: self.current_substate,
            to_substate: substate,
            minimum_dwell_frames: self.minimum_dwell_frames,
            target_duration_frames: self.target_duration_frames,
            exited_duration_draw: self.deterministic_duration_draw,
            next_duration_draw: draw,
            next_target_duration_frames: target,
            context: self.context,
        };
        let cooldown = p.for_behavior(self.current_macro_state).refractory_frames;
        self.refractory_until_frame[self.current_macro_state as usize] =
            self.last_frame.saturating_add(u64::from(cooldown));
        self.current_macro_state = next;
        self.current_substate = substate;
        self.entered_at_frame = intent.frame;
        self.last_frame = intent.frame;
        self.elapsed_frames = 0;
        self.minimum_dwell_frames = if substate == MotorSubstate::Settling {
            p.settle_frames
        } else {
            p.for_behavior(next).minimum_frames
        };
        self.target_duration_frames = target;
        self.transition_sequence = sequence;
        self.interruptible = !airborne(next);
        self.last_transition_reason = reason;
        self.last_transition_evidence = intent;
        self.duration_context_bucket = bucket;
        self.deterministic_duration_draw = draw;
        DynamicsDecision {
            behavior: next,
            reason: Some(reason),
            transition: Some(transition),
        }
    }

    fn fail_quiet(&mut self, seed: u64, intent: BehaviorIntentSnapshot) -> DynamicsDecision {
        let before_min = self.minimum_dwell_frames;
        let before_target = self.target_duration_frames;
        let old_draw = self.deterministic_duration_draw;
        let old_sub = self.current_substate;
        let profile =
            crate::behavior_parameters::profile_for_parameter_sha256(&self.parameter_sha256)
                .unwrap_or(BehaviorParameterProfile::N4);
        *self = Self::new_with_profile(seed, intent, profile);
        self.fault_latched = true;
        self.last_transition_reason = Reason::InvalidControllerState;
        DynamicsDecision {
            behavior: Behavior::Quiet,
            reason: Some(Reason::InvalidControllerState),
            transition: Some(DynamicsTransition {
                parameter_sha256: self.parameter_sha256.clone(),
                controller_sequence: 0,
                from_substate: old_sub,
                to_substate: MotorSubstate::None,
                minimum_dwell_frames: before_min,
                target_duration_frames: before_target,
                exited_duration_draw: old_draw,
                next_duration_draw: self.deterministic_duration_draw,
                next_target_duration_frames: self.target_duration_frames,
                context: self.context,
            }),
        }
    }

    pub fn fatigue_response_gain_q15(&self) -> Result<i32, String> {
        let p = parameters_for_sha256(&self.parameter_sha256)
            .ok_or("unknown N4/N4.1 parameter identity")?;
        Ok(fatigue_response_gain_q15(p, self.context.fatigue_q15))
    }

    pub fn fatigue_response_draw_q15(&self, seed: u64, behavior: Behavior) -> Result<i32, String> {
        let p = parameters_for_sha256(&self.parameter_sha256)
            .ok_or("unknown N4/N4.1 parameter identity")?;
        Ok(fatigue_response_draw_q15(
            p,
            seed,
            self.transition_sequence,
            behavior,
            self.context.bucket(),
        ))
    }

    fn fatigue_response_allows(
        &self,
        p: &BehaviorParameters,
        seed: u64,
        behavior: Behavior,
    ) -> bool {
        if p.fatigue_policy == FatiguePolicy::HardGate {
            return true;
        }
        let gain = fatigue_response_gain_q15(p, self.context.fatigue_q15);
        let draw = fatigue_response_draw_q15(
            p,
            seed,
            self.transition_sequence,
            behavior,
            self.context.bucket(),
        );
        draw <= gain
    }
}

pub fn fatigue_response_gain_q15(p: &BehaviorParameters, fatigue_q15: i32) -> i32 {
    if p.fatigue_policy == FatiguePolicy::HardGate || fatigue_q15 <= p.fatigue_suppression_onset_q15
    {
        return p.context_max_q15;
    }
    if fatigue_q15 >= p.fatigue_suppression_full_q15 {
        return p.fatigue_min_response_q15;
    }
    let elapsed = i64::from(fatigue_q15 - p.fatigue_suppression_onset_q15);
    let span = i64::from(p.fatigue_suppression_full_q15 - p.fatigue_suppression_onset_q15);
    let range = i64::from(p.context_max_q15 - p.fatigue_min_response_q15);
    (i64::from(p.context_max_q15) - range * elapsed / span) as i32
}

pub const fn airborne(behavior: Behavior) -> bool {
    matches!(
        behavior,
        Behavior::PreEscape | Behavior::Flight | Behavior::Landing
    )
}
