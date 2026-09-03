//! Deterministic motor substates inside the controller-authorized `Groom` macrostate.
//!
//! The sequence is literature-inspired and deliberately labeled as an engineering
//! motor program. It does not claim that these authored timings are a biological fit.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroomingMotorSubstate {
    Prepare,
    HeadSweep,
    ForelegRub,
    AbdomenBrush,
    WingClean,
    Reset,
}

impl GroomingMotorSubstate {
    pub const ALL: [Self; 6] = [
        Self::Prepare,
        Self::HeadSweep,
        Self::ForelegRub,
        Self::AbdomenBrush,
        Self::WingClean,
        Self::Reset,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Prepare => "PREPARE",
            Self::HeadSweep => "HEAD SWEEP",
            Self::ForelegRub => "FORELEG RUB",
            Self::AbdomenBrush => "ABDOMEN BRUSH",
            Self::WingClean => "WING CLEAN",
            Self::Reset => "RESET",
        }
    }

    pub const fn body_region(self) -> &'static str {
        match self {
            Self::Prepare | Self::Reset => "whole-body posture",
            Self::HeadSweep => "head/eyes/antennae",
            Self::ForelegRub => "forelegs",
            Self::AbdomenBrush => "abdomen/hind legs",
            Self::WingClean => "wing/hind leg",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroomingProgramFrame {
    pub substate: GroomingMotorSubstate,
    pub cycle: u32,
    pub segment_progress: f32,
    pub cycle_progress: f32,
}

const SEGMENTS: [(GroomingMotorSubstate, u32); 6] = [
    (GroomingMotorSubstate::Prepare, 8),
    (GroomingMotorSubstate::HeadSweep, 30),
    (GroomingMotorSubstate::ForelegRub, 15),
    (GroomingMotorSubstate::AbdomenBrush, 8),
    (GroomingMotorSubstate::WingClean, 8),
    (GroomingMotorSubstate::Reset, 6),
];

pub const GROOMING_MOTOR_CYCLE_FRAMES: u32 = 75;

pub fn grooming_program_at(age_frames: u32) -> GroomingProgramFrame {
    let cycle = age_frames / GROOMING_MOTOR_CYCLE_FRAMES;
    let frame = age_frames % GROOMING_MOTOR_CYCLE_FRAMES;
    let mut start = 0_u32;
    for (substate, duration) in SEGMENTS {
        let end = start + duration;
        if frame < end {
            return GroomingProgramFrame {
                substate,
                cycle,
                segment_progress: (frame - start) as f32 / duration as f32,
                cycle_progress: frame as f32 / GROOMING_MOTOR_CYCLE_FRAMES as f32,
            };
        }
        start = end;
    }
    unreachable!("grooming segment durations must span the declared cycle")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_cycle_covers_every_frame_and_substate() {
        let mut observed = Vec::new();
        for frame in 0..GROOMING_MOTOR_CYCLE_FRAMES {
            let program = grooming_program_at(frame);
            assert!(program.segment_progress >= 0.0 && program.segment_progress < 1.0);
            assert!(program.cycle_progress >= 0.0 && program.cycle_progress < 1.0);
            if observed.last() != Some(&program.substate) {
                observed.push(program.substate);
            }
        }
        assert_eq!(observed, GroomingMotorSubstate::ALL);
    }

    #[test]
    fn cycle_repeats_without_losing_authoritative_age() {
        for frame in 0..GROOMING_MOTOR_CYCLE_FRAMES {
            let first = grooming_program_at(frame);
            let second = grooming_program_at(frame + GROOMING_MOTOR_CYCLE_FRAMES);
            assert_eq!(first.substate, second.substate);
            assert_eq!(first.segment_progress, second.segment_progress);
            assert_eq!(second.cycle, first.cycle + 1);
        }
    }
}
