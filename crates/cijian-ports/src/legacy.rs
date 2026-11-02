use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyProfile {
    pub id: String,
    pub created_at: i64,
    pub cards: u64,
    pub articles: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyReport {
    pub profile: String,
    pub source_fingerprint: String,
    pub mapping_version: String,
    pub source_counts: BTreeMap<String, u64>,
    pub target_counts: BTreeMap<String, u64>,
    pub warnings: Vec<String>,
    pub blockers: Vec<String>,
    pub audio_bytes: u64,
}
