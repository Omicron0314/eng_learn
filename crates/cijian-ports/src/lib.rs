//! IO contracts. Platform resources stay behind these interfaces.
pub mod articles;
pub mod audio;
pub mod backup;
pub mod inbox;
pub mod library;
pub mod provider;
pub mod reading;
pub mod stats;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    InvalidInput,
    NoTargets,
    ConfigMissing,
    ProviderError,
    LlmOutputInvalid,
    QuotaExceeded,
    StaleState,
    NotFound,
    StorageUnavailable,
    UnsupportedCapability,
    Cancelled,
}
#[derive(Debug, Clone, thiserror::Error, Serialize, Deserialize)]
#[error("{message}")]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    pub retryable: bool,
}
impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retryable: false,
        }
    }
}
pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub kind: String,
    pub schema_version: u32,
    pub payload: serde_json::Value,
}

/// Non-secret platform observations. Absence is an explicit product state.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Capabilities {
    pub file_picker: bool,
    pub persistent_secrets: bool,
    pub audio_playback: bool,
    pub audio_seek: bool,
    pub audio_rate: bool,
    pub system_english_voice: bool,
}

/// Browser implementations may await completion of an actual IndexedDB transaction.
/// No synchronous SQL callback is exposed across the platform boundary.
pub trait SettingsRepository {
    fn load_settings(
        &self,
        profile: &str,
    ) -> impl std::future::Future<Output = Result<(cijian_domain::settings::Settings, u64)>>;
    fn save_settings(
        &self,
        command: SaveSettings,
    ) -> impl std::future::Future<Output = Result<u64>>;
}
#[derive(Debug, Clone)]
pub struct SaveDevicePreferences {
    pub profile: String,
    pub device: String,
    pub operation: String,
    pub event: String,
    pub expected_revision: u64,
    pub value: cijian_domain::settings::DevicePreferences,
    pub now: i64,
}

#[derive(Debug, Clone)]
pub struct SaveSettings {
    pub profile_id: String,
    pub operation_id: String,
    pub event_id: String,
    pub expected_revision: u64,
    pub now: i64,
    pub value: cijian_domain::settings::ValidatedSettings,
}
