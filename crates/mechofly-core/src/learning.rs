use serde::{Deserialize, Serialize};

use crate::{Behavior, graph::mix64, provenance::sha256_hex};

pub const LEARNING_RULE_VERSION: &str = "bounded-contextual-bandit-v1";
const CONTEXT_COUNT: usize = 12;
const ACTION_COUNT: usize = 4;
const VALUE_LIMIT: i16 = 2_048;
const UPDATE_STEP: i16 = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Pause,
    Explore,
    Inspect,
    Groom,
}

impl Action {
    pub const ALL: [Self; ACTION_COUNT] = [Self::Pause, Self::Explore, Self::Inspect, Self::Groom];

    const fn index(self) -> usize {
        match self {
            Self::Pause => 0,
            Self::Explore => 1,
            Self::Inspect => 2,
            Self::Groom => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Feedback {
    Encourage,
    Discourage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PolicyContext {
    pub behavior: Behavior,
    pub recent_interaction: bool,
}

impl PolicyContext {
    fn index(self) -> usize {
        let behavior_bucket = match self.behavior {
            Behavior::Rest | Behavior::Quiet => 0,
            Behavior::Walk | Behavior::Reverse => 1,
            Behavior::Groom => 2,
            Behavior::Alert | Behavior::PreEscape | Behavior::Flight | Behavior::Landing => 3,
        };
        behavior_bucket * 3 + usize::from(self.recent_interaction)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LearningLedgerEntry {
    pub sequence: u64,
    pub unix_millis: u64,
    pub rule_version: String,
    pub context: PolicyContext,
    pub action: Action,
    pub feedback: Feedback,
    pub value_before: i16,
    pub value_after: i16,
    pub policy_before_sha256: String,
    pub policy_after_sha256: String,
    pub claim: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PetPolicy {
    pub enabled: bool,
    values: [[i16; ACTION_COUNT]; CONTEXT_COUNT],
    updates: [[u16; ACTION_COUNT]; CONTEXT_COUNT],
    pub ledger: Vec<LearningLedgerEntry>,
}

impl Default for PetPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            values: [[0; ACTION_COUNT]; CONTEXT_COUNT],
            updates: [[0; ACTION_COUNT]; CONTEXT_COUNT],
            ledger: Vec::new(),
        }
    }
}

impl PetPolicy {
    pub fn choose(&self, context: PolicyContext, sequence: u64, seed: u64) -> Action {
        let row = &self.values[context.index()];
        let best = row.iter().copied().max().unwrap_or_default();
        let tied: Vec<Action> = Action::ALL
            .into_iter()
            .filter(|action| row[action.index()] == best)
            .collect();
        tied[(mix64(seed ^ sequence) as usize) % tied.len()]
    }

    pub fn apply_feedback(
        &mut self,
        context: PolicyContext,
        action: Action,
        feedback: Feedback,
        unix_millis: u64,
    ) -> Option<&LearningLedgerEntry> {
        if !self.enabled {
            return None;
        }
        let policy_before_sha256 = self.digest();
        let context_index = context.index();
        let action_index = action.index();
        let value_before = self.values[context_index][action_index];
        let delta = match feedback {
            Feedback::Encourage => UPDATE_STEP,
            Feedback::Discourage => -UPDATE_STEP,
        };
        let value_after = value_before
            .saturating_add(delta)
            .clamp(-VALUE_LIMIT, VALUE_LIMIT);
        self.values[context_index][action_index] = value_after;
        self.updates[context_index][action_index] =
            self.updates[context_index][action_index].saturating_add(1);
        let sequence = self.ledger.len() as u64 + 1;
        let policy_after_sha256 = self.digest();
        self.ledger.push(LearningLedgerEntry {
            sequence,
            unix_millis,
            rule_version: LEARNING_RULE_VERSION.to_owned(),
            context,
            action,
            feedback,
            value_before,
            value_after,
            policy_before_sha256,
            policy_after_sha256,
            claim: "MODELED_SOFTWARE_LEARNING_FROM_EXPLICIT_FEEDBACK".to_owned(),
        });
        self.ledger.last()
    }

    pub fn reset(&mut self) {
        self.values = [[0; ACTION_COUNT]; CONTEXT_COUNT];
        self.updates = [[0; ACTION_COUNT]; CONTEXT_COUNT];
        self.ledger.clear();
    }

    pub fn digest(&self) -> String {
        let values: Vec<u8> = self
            .values
            .iter()
            .flatten()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        let updates: Vec<u8> = self
            .updates
            .iter()
            .flatten()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        sha256_hex([values, updates])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learning_requires_explicit_feedback_and_is_bounded() {
        let mut policy = PetPolicy::default();
        let context = PolicyContext {
            behavior: Behavior::Walk,
            recent_interaction: true,
        };
        let before = policy.digest();
        for time in 0..100 {
            policy.apply_feedback(context, Action::Explore, Feedback::Encourage, time);
        }
        assert_ne!(before, policy.digest());
        assert_eq!(
            policy.values[context.index()][Action::Explore.index()],
            VALUE_LIMIT
        );
        policy.enabled = false;
        let disabled = policy.digest();
        assert!(
            policy
                .apply_feedback(context, Action::Explore, Feedback::Discourage, 101)
                .is_none()
        );
        assert_eq!(disabled, policy.digest());
    }
}
