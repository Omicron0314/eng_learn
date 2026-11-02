//! Persisted article and generation-job contracts. Snapshots never contain secrets.
use cijian_domain::{
    analysis::Analysis,
    settings::{Genre, Tier},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    pub id: String,
    pub title: String,
    pub body: String,
    pub body_hash: String,
    pub genre: Genre,
    pub tier: Tier,
    pub analysis: Analysis,
    pub diagnostics: Vec<String>,
    pub created_at: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleTarget {
    pub lexeme_id: String,
    pub role: TargetRole,
    pub occurrences: u32,
    pub selection_reason: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetRole {
    Exposed,
    Control,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationJob {
    pub id: String,
    pub operation_id: String,
    pub status: JobStatus,
    pub result_ref: Option<String>,
    pub error: Option<String>,
    pub config_revision: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationSnapshot {
    pub tier: Tier,
    pub genre: Genre,
    pub exposed: Vec<String>,
    pub control: Vec<String>,
    pub known: Vec<String>,
    pub settings: cijian_domain::settings::Settings,
    pub provider: cijian_domain::provider::ProviderConfig,
}
#[derive(Debug, Clone)]
pub struct StartGeneration {
    pub profile: String,
    pub operation: String,
    pub event: String,
    pub job: String,
    pub now: i64,
    pub snapshot: GenerationSnapshot,
}
#[derive(Debug, Clone)]
pub struct SaveArticle {
    pub profile: String,
    pub operation: String,
    pub event: String,
    pub job: String,
    pub id: String,
    pub title: String,
    pub body: String,
    pub genre: Genre,
    pub tier: Tier,
    pub analysis: Analysis,
    pub diagnostics: Vec<String>,
    pub targets: Vec<ArticleTarget>,
    pub model: String,
    pub provider_fingerprint: String,
    pub now: i64,
}
