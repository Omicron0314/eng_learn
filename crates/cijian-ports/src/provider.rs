use crate::{AppError, ErrorCode, Result};
use cijian_domain::provider::ProviderConfig;
use zeroize::Zeroizing;

/// Never serialized or printed. The only exposure is to the authenticated transport.
#[derive(Clone)]
pub struct Secret(Zeroizing<String>);
impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret([REDACTED])")
    }
}
impl Secret {
    pub fn new(value: String) -> Result<Self> {
        if value.trim().is_empty() || value.chars().any(char::is_control) {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                "密钥不能为空或包含控制字符",
            ));
        }
        Ok(Self(Zeroizing::new(value)))
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}
#[derive(Debug)]
pub enum SecretPatch {
    Keep,
    Replace(Secret),
    Clear,
}
#[derive(Debug, Clone)]
pub struct ProviderSnapshot {
    pub config: ProviderConfig,
    pub secret: Option<Secret>,
}
pub trait SecretStore {
    fn read(&self, reference: &str) -> impl std::future::Future<Output = Result<Option<Secret>>>;
    fn write(
        &self,
        reference: &str,
        patch: SecretPatch,
    ) -> impl std::future::Future<Output = Result<()>>;
}
pub struct HttpRequest {
    pub url: String,
    pub bearer: Option<Secret>,
    pub body: serde_json::Value,
}
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
    pub latency_ms: u64,
}
pub trait HttpTransport {
    fn post(&self, request: HttpRequest)
    -> impl std::future::Future<Output = Result<HttpResponse>>;
}

#[derive(Debug, Clone)]
pub struct CallStart {
    pub profile_id: String,
    pub call_id: String,
    pub purpose: String,
    pub model: String,
    pub prompt_version: String,
    pub provider_fingerprint: String,
    pub now: i64,
}
#[derive(Debug, Clone)]
pub struct CallFinish {
    pub call_id: String,
    pub status: CallStatus,
    pub tokens_in: Option<u64>,
    pub tokens_out: Option<u64>,
    pub latency_ms: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallStatus {
    Success,
    TransportError,
    HttpError,
    InvalidOutput,
    Interrupted,
}
/// Start must durably succeed before network IO. A failed finish leaves an auditable started call.
pub trait CallRecorder {
    fn start(&self, call: CallStart) -> impl std::future::Future<Output = Result<()>>;
    fn finish(&self, call: CallFinish) -> impl std::future::Future<Output = Result<()>>;
}
pub trait LanguageModel {
    fn complete(
        &self,
        snapshot: &ProviderSnapshot,
        input: CompletionInput,
        call: CallStart,
    ) -> impl std::future::Future<Output = Result<String>>;
}

#[derive(Debug, Clone)]
pub struct CompletionInput {
    pub system: String,
    pub user: String,
    pub json: bool,
}
impl CompletionInput {
    pub fn validate(&self) -> Result<()> {
        if self.system.trim().is_empty() || self.user.trim().is_empty() {
            Err(AppError::new(ErrorCode::InvalidInput, "提示内容不能为空"))
        } else {
            Ok(())
        }
    }
}
