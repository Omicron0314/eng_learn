use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupPackage {
    pub format: String,
    pub version: u32,
    pub checksum: String,
    pub payload: serde_json::Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreReport {
    pub profile: String,
    pub counts: std::collections::BTreeMap<String, u64>,
    pub audio_included: bool,
    pub credentials_included: bool,
}
