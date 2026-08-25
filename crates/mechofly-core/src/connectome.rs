use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use csv::{Reader, ReaderBuilder, StringRecord};
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    graph::{GraphIdentity, ModelGraph, graph_digest, mix64},
    provenance::sha256_hex,
};

const FAFB_CONNECTIONS_URL: &str = "https://codex.flywire.ai/api/download_resource?data_product=connections_princeton&dataset=fafb";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ColumnMapping {
    pub source_root_id: String,
    pub target_root_id: String,
    pub synapse_count: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImportManifest {
    pub schema_version: u32,
    pub dataset: String,
    pub snapshot: String,
    pub product: String,
    pub source_url: String,
    pub source_file: String,
    pub source_file_sha256: String,
    pub retrieved_utc: String,
    pub filter_declaration: String,
    pub transform: String,
    pub column_mapping: ColumnMapping,
    pub neuron_count: usize,
    pub edge_row_count: usize,
    pub graph_sha256: String,
    pub validation_warnings: Vec<String>,
    pub citation_required: bool,
    pub measured_activity: bool,
}

#[derive(Clone, Debug)]
pub struct ConnectomeImport {
    pub graph: ModelGraph,
    pub manifest: ImportManifest,
}

#[derive(Debug, Error)]
pub enum ConnectomeImportError {
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot parse CSV: {0}")]
    Csv(#[from] csv::Error),
    #[error("missing required column; expected one of {0}")]
    MissingColumn(String),
    #[error("row {row}: invalid {column} value {value:?}")]
    InvalidValue {
        row: usize,
        column: String,
        value: String,
    },
    #[error("connection table contains no rows")]
    Empty,
    #[error("graph validation failed: {0}")]
    InvalidGraph(String),
}

#[derive(Clone, Copy, Debug)]
struct RawEdge {
    source: u64,
    target: u64,
    synapses: u32,
    ordinal: u32,
}

impl ConnectomeImport {
    pub fn fafb_v783(
        path: impl AsRef<Path>,
        retrieved_utc: impl Into<String>,
    ) -> Result<Self, ConnectomeImportError> {
        Self::from_connection_table(
            path,
            "fafb",
            "v783",
            "connections_princeton",
            FAFB_CONNECTIONS_URL,
            retrieved_utc,
            "provider-filtered table; provider default is at least five synapses",
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_connection_table(
        path: impl AsRef<Path>,
        dataset: impl Into<String>,
        snapshot: impl Into<String>,
        product: impl Into<String>,
        source_url: impl Into<String>,
        retrieved_utc: impl Into<String>,
        filter_declaration: impl Into<String>,
    ) -> Result<Self, ConnectomeImportError> {
        let path = path.as_ref();
        let source_file_sha256 = file_sha256(path)?;
        let mut reader = csv_reader(path)?;
        let headers = reader.headers()?.clone();
        let source_index = find_column(
            &headers,
            &["pre_root_id", "pre_root", "source_root_id", "source"],
        )?;
        let target_index = find_column(
            &headers,
            &["post_root_id", "post_root", "target_root_id", "target"],
        )?;
        let synapse_index = find_column(
            &headers,
            &["syn_count", "synapse_count", "n_synapses", "weight"],
        )?;
        let mapping = ColumnMapping {
            source_root_id: headers[source_index].to_owned(),
            target_root_id: headers[target_index].to_owned(),
            synapse_count: headers[synapse_index].to_owned(),
        };

        let mut raw_edges = Vec::new();
        let mut neuron_ids = Vec::new();
        for (record_index, record) in reader.records().enumerate() {
            let record = record?;
            let row = record_index + 2;
            let source = parse::<u64>(&record, source_index, row, &mapping.source_root_id)?;
            let target = parse::<u64>(&record, target_index, row, &mapping.target_root_id)?;
            let synapses = parse::<u32>(&record, synapse_index, row, &mapping.synapse_count)?;
            raw_edges.push(RawEdge {
                source,
                target,
                synapses,
                ordinal: record_index as u32,
            });
            neuron_ids.push(source);
            neuron_ids.push(target);
        }
        if raw_edges.is_empty() {
            return Err(ConnectomeImportError::Empty);
        }

        neuron_ids.sort_unstable();
        neuron_ids.dedup();
        let index_by_id: HashMap<u64, u32> = neuron_ids
            .iter()
            .enumerate()
            .map(|(index, root_id)| (*root_id, index as u32))
            .collect();
        raw_edges.sort_unstable_by_key(|edge| (edge.target, edge.source, edge.ordinal));

        let mut incoming_offsets = Vec::with_capacity(neuron_ids.len() + 1);
        let mut incoming_sources = Vec::with_capacity(raw_edges.len());
        let mut modeled_weights = Vec::with_capacity(raw_edges.len());
        incoming_offsets.push(0);
        let mut edge_cursor = 0;
        for target_id in &neuron_ids {
            while edge_cursor < raw_edges.len() && raw_edges[edge_cursor].target == *target_id {
                let edge = raw_edges[edge_cursor];
                incoming_sources.push(index_by_id[&edge.source]);
                modeled_weights.push(structural_weight_to_modeled(edge.synapses));
                edge_cursor += 1;
            }
            incoming_offsets.push(incoming_sources.len() as u32);
        }
        let positions = neuron_ids
            .iter()
            .map(|root_id| position_from_root_id(*root_id))
            .collect();
        let graph_sha256 = graph_digest(
            &neuron_ids,
            &incoming_offsets,
            &incoming_sources,
            &modeled_weights,
        );
        let dataset = dataset.into();
        let snapshot = snapshot.into();
        let product = product.into();
        let source_url = source_url.into();
        let transform = "incoming-csr-retain-rows_unsigned-strength-q12-v1".to_owned();
        let identity = GraphIdentity {
            graph_id: format!("{}:{}:{}", dataset, snapshot, &graph_sha256[..12]),
            dataset: dataset.clone(),
            snapshot: snapshot.clone(),
            product: product.clone(),
            source_url: source_url.clone(),
            transform: transform.clone(),
            sha256: graph_sha256.clone(),
            neuron_count: neuron_ids.len(),
            edge_count: incoming_sources.len(),
            structure_claim: "DERIVED_CONNECTOME_STRUCTURE".to_owned(),
        };
        let graph = ModelGraph {
            identity,
            neuron_ids,
            incoming_offsets,
            incoming_sources,
            modeled_weights,
            positions,
        };
        graph
            .validate()
            .map_err(ConnectomeImportError::InvalidGraph)?;

        let mut validation_warnings = Vec::new();
        if dataset.eq_ignore_ascii_case("fafb") && snapshot.eq_ignore_ascii_case("v783") {
            if graph.identity.neuron_count != 139_255 {
                validation_warnings.push(format!(
                    "Codex lists 139255 FAFB v783 neurons; this table resolves {} unique IDs",
                    graph.identity.neuron_count
                ));
            }
            if graph.identity.edge_count != 3_732_460 {
                validation_warnings.push(format!(
                    "Codex lists 3732460 filtered FAFB v783 connections; this file has {} rows",
                    graph.identity.edge_count
                ));
            }
        }
        let manifest = ImportManifest {
            schema_version: 1,
            dataset,
            snapshot,
            product,
            source_url,
            source_file: path.to_string_lossy().into_owned(),
            source_file_sha256,
            retrieved_utc: retrieved_utc.into(),
            filter_declaration: filter_declaration.into(),
            transform,
            column_mapping: mapping,
            neuron_count: graph.identity.neuron_count,
            edge_row_count: graph.identity.edge_count,
            graph_sha256,
            validation_warnings,
            citation_required: true,
            measured_activity: false,
        };
        Ok(Self { graph, manifest })
    }

    pub fn manifest_digest(&self) -> String {
        let json = serde_json::to_vec(&self.manifest).expect("manifest serialization cannot fail");
        sha256_hex([json])
    }
}

fn csv_reader(path: &Path) -> Result<Reader<Box<dyn Read>>, ConnectomeImportError> {
    let file = File::open(path).map_err(|source| ConnectomeImportError::Io {
        path: path.to_owned(),
        source,
    })?;
    let reader: Box<dyn Read> = if path
        .extension()
        .is_some_and(|extension| extension.to_string_lossy().eq_ignore_ascii_case("gz"))
    {
        Box::new(GzDecoder::new(BufReader::new(file)))
    } else {
        Box::new(BufReader::new(file))
    };
    Ok(ReaderBuilder::new().flexible(false).from_reader(reader))
}

fn file_sha256(path: &Path) -> Result<String, ConnectomeImportError> {
    let file = File::open(path).map_err(|source| ConnectomeImportError::Io {
        path: path.to_owned(),
        source,
    })?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 128 * 1_024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|source| ConnectomeImportError::Io {
                path: path.to_owned(),
                source,
            })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn normalize_header(header: &str) -> String {
    header.trim().to_ascii_lowercase().replace([' ', '-'], "_")
}

fn find_column(
    headers: &StringRecord,
    candidates: &[&str],
) -> Result<usize, ConnectomeImportError> {
    headers
        .iter()
        .position(|header| candidates.contains(&normalize_header(header).as_str()))
        .ok_or_else(|| ConnectomeImportError::MissingColumn(candidates.join(", ")))
}

fn parse<T: std::str::FromStr>(
    record: &StringRecord,
    index: usize,
    row: usize,
    column: &str,
) -> Result<T, ConnectomeImportError> {
    let value = record.get(index).unwrap_or_default().trim();
    value
        .parse::<T>()
        .map_err(|_| ConnectomeImportError::InvalidValue {
            row,
            column: column.to_owned(),
            value: value.to_owned(),
        })
}

fn structural_weight_to_modeled(synapses: u32) -> i32 {
    ((synapses.clamp(1, 2_048) as i32) * 24).clamp(24, 1_024)
}

fn position_from_root_id(root_id: u64) -> [f32; 2] {
    let h1 = mix64(root_id ^ 0xC6A4_A793_5BD1_E995);
    let h2 = mix64(h1 ^ 0x9E37_79B9_7F4A_7C15);
    let side = if h1 & 1 == 0 { -1.0_f32 } else { 1.0_f32 };
    let radial = (((h1 >> 8) & 0xffff) as f32 / 65_535.0).sqrt();
    let theta = ((h2 & 0xffff) as f32 / 65_535.0) * std::f32::consts::TAU;
    [
        (side * (0.30 + 0.52 * radial) + 0.18 * theta.cos() * radial).clamp(-1.0, 1.0),
        (0.70 * theta.sin() * radial).clamp(-1.0, 1.0),
    ]
}

#[cfg(test)]
mod tests {
    use std::{fs, time::SystemTime};

    use super::*;

    #[test]
    fn imports_flexible_connection_headers_and_retains_rows() {
        let path = std::env::temp_dir().join(format!(
            "mechofly-connectome-{}.csv",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(
            &path,
            "pre_root_id,post_root_id,neuropil,syn_count\n100,200,AL,6\n100,200,MB,7\n200,300,MB,9\n",
        )
        .unwrap();
        let imported = ConnectomeImport::from_connection_table(
            &path,
            "fixture",
            "v1",
            "connections",
            "fixture://connections",
            "2026-08-25T00:00:00Z",
            "none",
        )
        .unwrap();
        fs::remove_file(path).ok();
        assert_eq!(imported.graph.identity.neuron_count, 3);
        assert_eq!(imported.graph.identity.edge_count, 3);
        assert_eq!(imported.graph.neuron_ids, vec![100, 200, 300]);
        assert_eq!(imported.manifest.column_mapping.synapse_count, "syn_count");
        assert!(!imported.manifest.measured_activity);
    }
}
