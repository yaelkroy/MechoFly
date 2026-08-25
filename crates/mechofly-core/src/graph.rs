use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::provenance::sha256_hex;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelTier {
    Demo4096,
    Standard12615,
    Extended65536,
    FafbV783Full,
}

impl ModelTier {
    pub const fn neuron_count(self) -> Option<usize> {
        match self {
            Self::Demo4096 => Some(4_096),
            Self::Standard12615 => Some(12_615),
            Self::Extended65536 => Some(65_536),
            Self::FafbV783Full => None,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Demo4096 => "demo-4096",
            Self::Standard12615 => "standard-12615",
            Self::Extended65536 => "extended-65536",
            Self::FafbV783Full => "fafb-v783-full",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GraphIdentity {
    pub graph_id: String,
    pub dataset: String,
    pub snapshot: String,
    pub product: String,
    pub source_url: String,
    pub transform: String,
    pub sha256: String,
    pub neuron_count: usize,
    pub edge_count: usize,
    pub structure_claim: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelGraph {
    pub identity: GraphIdentity,
    pub neuron_ids: Vec<u64>,
    pub incoming_offsets: Vec<u32>,
    pub incoming_sources: Vec<u32>,
    pub modeled_weights: Vec<i32>,
    pub positions: Vec<[f32; 2]>,
}

impl ModelGraph {
    pub fn synthetic(tier: ModelTier, seed: u64) -> Self {
        let count = tier
            .neuron_count()
            .expect("a full FAFB graph must be imported, never synthesized");
        let fan_in = if count <= 4_096 { 8 } else { 10 };
        let mut incoming_offsets = Vec::with_capacity(count + 1);
        let mut incoming_sources = Vec::with_capacity(count * fan_in);
        let mut modeled_weights = Vec::with_capacity(count * fan_in);
        let mut positions = Vec::with_capacity(count);
        incoming_offsets.push(0);

        for target in 0..count {
            let mut unique = BTreeSet::new();
            let mut lane = 0_u64;
            while unique.len() < fan_in {
                let h = mix64(seed ^ (target as u64).wrapping_mul(0x9E37_79B9) ^ lane);
                unique.insert((h as usize) % count);
                lane += 1;
            }
            for source in unique {
                let h = mix64(seed ^ ((source as u64) << 32) ^ target as u64);
                let magnitude = 96 + (h % 225) as i32;
                let inhibitory = (h >> 11) % 5 == 0;
                incoming_sources.push(source as u32);
                modeled_weights.push(if inhibitory { -magnitude } else { magnitude });
            }
            incoming_offsets.push(incoming_sources.len() as u32);
            positions.push(brain_position(target, count, seed));
        }

        let neuron_ids: Vec<u64> = (0..count as u64).collect();
        let digest = graph_digest(
            &neuron_ids,
            &incoming_offsets,
            &incoming_sources,
            &modeled_weights,
        );
        let identity = GraphIdentity {
            graph_id: format!("synthetic:{}:{}", tier.label(), &digest[..12]),
            dataset: "mechofly-synthetic-demo".to_owned(),
            snapshot: "v1".to_owned(),
            product: tier.label().to_owned(),
            source_url: "repository://procedural-graph-v1".to_owned(),
            transform: "deterministic-incoming-csr-v1".to_owned(),
            sha256: digest,
            neuron_count: count,
            edge_count: incoming_sources.len(),
            structure_claim: "SYNTHETIC_DEMO_TOPOLOGY".to_owned(),
        };

        Self {
            identity,
            neuron_ids,
            incoming_offsets,
            incoming_sources,
            modeled_weights,
            positions,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let count = self.neuron_ids.len();
        if count == 0 {
            return Err("graph has no neurons".to_owned());
        }
        if self.incoming_offsets.len() != count + 1 {
            return Err("incoming offset count must equal neuron count + 1".to_owned());
        }
        if self.incoming_sources.len() != self.modeled_weights.len() {
            return Err("source and weight arrays differ in length".to_owned());
        }
        if self.positions.len() != count {
            return Err("position count differs from neuron count".to_owned());
        }
        if self.incoming_offsets[0] != 0
            || self.incoming_offsets[count] as usize != self.incoming_sources.len()
        {
            return Err("incoming offsets do not bound the edge arrays".to_owned());
        }
        if self
            .incoming_offsets
            .windows(2)
            .any(|pair| pair[0] > pair[1])
        {
            return Err("incoming offsets are not monotonic".to_owned());
        }
        if self
            .incoming_sources
            .iter()
            .any(|source| *source as usize >= count)
        {
            return Err("edge source is outside the neuron table".to_owned());
        }
        Ok(())
    }
}

pub fn graph_digest(
    neuron_ids: &[u64],
    offsets: &[u32],
    sources: &[u32],
    weights: &[i32],
) -> String {
    sha256_hex([
        as_le_bytes_u64(neuron_ids),
        as_le_bytes_u32(offsets),
        as_le_bytes_u32(sources),
        as_le_bytes_i32(weights),
    ])
}

fn as_le_bytes_u64(values: &[u64]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn as_le_bytes_u32(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn as_le_bytes_i32(values: &[i32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

pub(crate) fn mix64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

fn brain_position(index: usize, count: usize, seed: u64) -> [f32; 2] {
    let h1 = mix64(seed ^ index as u64);
    let h2 = mix64(h1 ^ 0xA076_1D64_78BD_642F);
    let side = if index % 2 == 0 { -1.0_f32 } else { 1.0_f32 };
    let radial = ((h1 & 0xffff) as f32 / 65_535.0).sqrt();
    let theta = ((h2 & 0xffff) as f32 / 65_535.0) * std::f32::consts::TAU;
    let density = (count as f32).log10().max(1.0);
    let x = side * (0.28 + radial * 0.58) + theta.cos() * 0.17 * radial;
    let y = theta.sin() * 0.72 * radial / (1.0 + 0.02 * density);
    [x.clamp(-1.0, 1.0), y.clamp(-1.0, 1.0)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_graph_is_reproducible_and_valid() {
        let first = ModelGraph::synthetic(ModelTier::Demo4096, 7);
        let second = ModelGraph::synthetic(ModelTier::Demo4096, 7);
        assert_eq!(first.identity.sha256, second.identity.sha256);
        assert_eq!(first.incoming_sources, second.incoming_sources);
        assert!(first.validate().is_ok());
    }
}
