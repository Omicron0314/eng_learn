use serde::{Deserialize, Serialize};
#[derive(Debug, Clone)]
pub struct ReserveAudio {
    pub profile: String,
    pub operation: String,
    pub event: String,
    pub reservation: String,
    pub article: String,
    pub body_hash: String,
    pub now: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioReservation {
    pub id: String,
    pub article: String,
    pub chars: u64,
    pub outcome: String,
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
pub struct SaveAudio {
    pub profile: String,
    pub operation: String,
    pub event: String,
    pub reservation: String,
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
