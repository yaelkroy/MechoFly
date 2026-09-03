use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ClaimLayer {
    DerivedConnectomeStructure,
    ModeledNeuralDynamics,
    ModeledSoftwareLearning,
    AuthoredPresentation,
    AuthoredIntervention,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProvenanceRecord {
    pub layer: ClaimLayer,
    pub label: String,
    pub source: String,
    pub version: String,
    pub sha256: String,
    pub measured_activity: bool,
    pub live_hardware_authority: bool,
}

pub fn sha256_hex(parts: impl IntoIterator<Item = impl AsRef<[u8]>>) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        let bytes = part.as_ref();
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    format!("{:x}", hasher.finalize())
}
