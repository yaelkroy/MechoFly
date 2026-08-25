use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::{graph::GraphIdentity, model::FrameSummary, model::ModelState};

pub const MAX_REPLAY_FRAMES: usize = 240;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModelCheckpoint {
    pub graph: GraphIdentity,
    pub model_identity: String,
    pub state: ModelState,
    pub summary: FrameSummary,
}

#[derive(Clone, Debug)]
pub struct BoundedReplay {
    capacity: usize,
    frames: VecDeque<ModelCheckpoint>,
}

impl Default for BoundedReplay {
    fn default() -> Self {
        Self::new(MAX_REPLAY_FRAMES)
    }
}

impl BoundedReplay {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.clamp(1, MAX_REPLAY_FRAMES),
            frames: VecDeque::new(),
        }
    }

    pub fn push(&mut self, checkpoint: ModelCheckpoint) {
        while self.frames.len() >= self.capacity {
            self.frames.pop_front();
        }
        self.frames.push_back(checkpoint);
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn newest(&self) -> Option<&ModelCheckpoint> {
        self.frames.back()
    }

    pub fn get_from_newest(&self, frames_back: usize) -> Option<&ModelCheckpoint> {
        self.frames
            .len()
            .checked_sub(frames_back + 1)
            .and_then(|index| self.frames.get(index))
    }

    pub fn summaries(&self) -> impl Iterator<Item = &FrameSummary> {
        self.frames.iter().map(|frame| &frame.summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Behavior, GraphIdentity};

    fn checkpoint(frame: u64) -> ModelCheckpoint {
        let state = ModelState {
            frame,
            seed: 1,
            activation: vec![frame as i32],
            spikes: vec![0],
            behavior: Behavior::Rest,
            behavior_age_frames: frame as u32,
        };
        ModelCheckpoint {
            graph: GraphIdentity {
                graph_id: "g".into(),
                dataset: "d".into(),
                snapshot: "v".into(),
                product: "p".into(),
                source_url: "u".into(),
                transform: "t".into(),
                sha256: "h".into(),
                neuron_count: 1,
                edge_count: 0,
                structure_claim: "SYNTHETIC_DEMO_TOPOLOGY".into(),
            },
            model_identity: "m".into(),
            summary: FrameSummary {
                frame,
                spike_count: 0,
                mean_activation_q15: frame as i32,
                behavior: Behavior::Rest,
                state_digest: state.digest(),
            },
            state,
        }
    }

    #[test]
    fn replay_is_bounded() {
        let mut replay = BoundedReplay::new(3);
        for frame in 0..5 {
            replay.push(checkpoint(frame));
        }
        assert_eq!(replay.len(), 3);
        assert_eq!(replay.get_from_newest(2).unwrap().state.frame, 2);
        assert_eq!(replay.newest().unwrap().state.frame, 4);
    }
}
