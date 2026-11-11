use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemVoice {
    pub id: String,
    pub label: String,
    pub language: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioAsset {
    pub id: String,
    pub cache_key: String,
    pub text_hash: String,
    pub provider_fingerprint: String,
    pub model: String,
    pub voice: String,
    pub format: String,
    pub blob_key: String,
    pub duration: Option<f64>,
    pub chars: u64,
}
#[derive(Debug, Clone)]
pub struct SaveLocalAudio {
    pub profile: String,
    pub operation: String,
    pub event: String,
    pub article: String,
    pub asset: AudioAsset,
    pub now: i64,
}
#[derive(Debug, Clone)]
pub struct AudioSessionCommand {
    pub profile: String,
    pub operation: String,
    pub event: String,
    pub session: String,
    pub article: String,
    pub audio: Option<String>,
    pub source: String,
    pub rate: f64,
    pub now: i64,
}
