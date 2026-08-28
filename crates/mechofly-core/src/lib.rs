#![forbid(unsafe_code)]

pub mod connectome;
pub mod graph;
pub mod learning;
pub mod model;
pub mod provenance;
pub mod replay;
pub mod stimulation;

pub use connectome::{ConnectomeImport, ConnectomeImportError, ImportManifest};
pub use graph::{GraphIdentity, ModelGraph, ModelTier};
pub use learning::{Action, Feedback, LearningLedgerEntry, PetPolicy, PolicyContext};
pub use model::{Behavior, FrameSummary, ModelEngine, ModelState, StepInput};
pub use provenance::{ClaimLayer, ProvenanceRecord, sha256_hex};
pub use replay::{BoundedReplay, ModelCheckpoint};
pub use stimulation::{
    ComparisonFrame, ComparisonReceipt, ComparisonResult, NeuronSample, StimulationPolicy,
    StimulationRequest, StimulationValidationError,
};

pub const MODEL_STEP_MS: u32 = 33;
pub const MODEL_VERSION: &str = "mechofly-fixedpoint-v2-loom";
