#![forbid(unsafe_code)]

pub mod behavior_dynamics;
pub mod behavior_intent;
pub mod behavior_parameters;
pub mod behavior_selection;
pub mod behavior_telemetry;
pub mod behavior_validation;
pub mod connectome;
pub mod graph;
pub mod learning;
pub mod model;
pub mod neural_evidence;
pub mod provenance;
pub mod replay;
pub mod stimulation;

pub use behavior_intent::{BEHAVIOR_PIPELINE_VERSION, BehaviorContext, BehaviorIntentBuilder};
pub use behavior_selection::{BehaviorDecision, LegacyBehaviorSelector};
pub use behavior_telemetry::{
    BEHAVIOR_TELEMETRY_CLAIM_BOUNDARY, BEHAVIOR_TELEMETRY_CONTROLLER,
    BEHAVIOR_TELEMETRY_SCHEMA_VERSION, BehaviorIntentSnapshot, BehaviorTelemetryLedger,
    BehaviorTelemetrySnapshot, BehaviorTransitionEvent, BehaviorTransitionReason,
    MAX_BEHAVIOR_TRANSITION_EVENTS,
};
pub use connectome::{ConnectomeImport, ConnectomeImportError, ImportManifest};
pub use graph::{GraphIdentity, ModelGraph, ModelTier};
pub use learning::{Action, Feedback, LearningLedgerEntry, PetPolicy, PolicyContext};
pub use model::{Behavior, FrameSummary, ModelEngine, ModelState, StepComponentTimings, StepInput};
pub use neural_evidence::NeuralEvidence;
pub use provenance::{ClaimLayer, ProvenanceRecord, sha256_hex};
pub use replay::{BoundedReplay, ModelCheckpoint};
pub use stimulation::{
    ComparisonFrame, ComparisonReceipt, ComparisonResult, NeuronSample, StimulationPolicy,
    StimulationRequest, StimulationValidationError,
};

pub const MODEL_STEP_MS: u32 = 33;
pub const MODEL_VERSION: &str = "mechofly-fixedpoint-v4-authoritative-actions";
