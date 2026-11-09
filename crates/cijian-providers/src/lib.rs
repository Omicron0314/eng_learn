//! OpenAI-compatible transport. All calls are explicit, recorded and have no retry policy.
mod speech;
pub mod youdao;
use cijian_domain::provider::ProviderKind;
use cijian_ports::{AppError, ErrorCode, Result, provider::*};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};

pub struct NativeHttp {
    client: reqwest::Client,
}
impl NativeHttp {
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .build()
            .map_err(|_| AppError::new(ErrorCode::ProviderError, "无法初始化网络连接"))?;
        Ok(Self { client })
    }
}
impl HttpTransport for NativeHttp {
    async fn post(&self, request: HttpRequest) -> Result<HttpResponse> {
        let start = Instant::now();
        let mut builder = self.client.post(&request.url).json(&request.body);
        if let Some(secret) = request.bearer {
            builder = builder.bearer_auth(secret.expose());
        }
        let mut response = builder
            .send()
            .await
            .map_err(|_| AppError::new(ErrorCode::ProviderError, "服务连接失败或超时"))?;
        let status = response.status().as_u16();
        const MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
        if response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
        {
            return Err(AppError::new(
                ErrorCode::ProviderError,
                "服务响应超出64MiB资源保护",
            ));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| AppError::new(ErrorCode::ProviderError, "无法读取服务响应"))?
        {
            if bytes
                .len()
                .checked_add(chunk.len())
                .is_none_or(|size| size > MAX_RESPONSE_BYTES)
            {
                return Err(AppError::new(
                    ErrorCode::ProviderError,
                    "服务响应超出64MiB资源保护",
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(HttpResponse {
            status,
            body: bytes,
            latency_ms: u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX),
        })
    }
}

pub struct OpenAiCompatible<T, L> {
    pub transport: T,
    pub recorder: L,
}
impl<T: HttpTransport, L: CallRecorder> OpenAiCompatible<T, L> {
    /// One underlying request only. Repair/rewrite budgets belong to the application job.
    pub async fn complete(
        &self,
        snapshot: Option<&ProviderSnapshot>,
        input: CompletionInput,
        mut call: CallStart,
    ) -> Result<String> {
        input.validate()?;
        let snapshot =
            snapshot.ok_or(AppError::new(ErrorCode::ConfigMissing, "请先配置 AI 服务"))?;
        snapshot
            .config
            .validate()
            .map_err(|e| AppError::new(ErrorCode::InvalidInput, e.to_string()))?;
        if snapshot.config.kind != ProviderKind::Llm {
            return Err(AppError::new(ErrorCode::InvalidInput, "服务类型不匹配"));
        }
        if call.profile_id.is_empty()
            || call.call_id.is_empty()
            || call.purpose.is_empty()
            || call.prompt_version.is_empty()
        {
            return Err(AppError::new(ErrorCode::InvalidInput, "调用记录信息不完整"));
        }
        call.model = snapshot.config.model.clone();
        call.provider_fingerprint = format!(
            "{:x}",
            Sha256::digest(format!(
                "{}\n{}",
                snapshot.config.endpoint, snapshot.config.model
            ))
        );
        let call_id = call.call_id.clone();
        self.recorder.start(call).await?;
        let mut body = json!({"model":snapshot.config.model,"messages":[{"role":"system","content":input.system},{"role":"user","content":input.user}]});
        if input.json {
            body["response_format"] = json!({"type":"json_object"});
        }
        let started = Instant::now();
        let response = self
            .transport
            .post(HttpRequest {
                url: format!(
                    "{}/chat/completions",
                    snapshot.config.endpoint.trim_end_matches('/')
                ),
                bearer: snapshot.secret.clone(),
                body,
            })
            .await;
        let mut finish = CallFinish {
            call_id,
            status: CallStatus::TransportError,
            tokens_in: None,
            tokens_out: None,
            latency_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        };
        let result = match response {
            Err(_) => Err(AppError::new(
                ErrorCode::ProviderError,
                "服务连接失败或超时",
            )),
            Ok(response) => {
                finish.latency_ms = response.latency_ms;
                if !(200..300).contains(&response.status) {
                    finish.status = CallStatus::HttpError;
                    Err(AppError::new(
                        ErrorCode::ProviderError,
                        format!("服务返回错误（HTTP {}）", response.status),
                    ))
                } else {
                    let value = serde_json::from_slice::<Value>(&response.body).ok();
                    finish.tokens_in = value
                        .as_ref()
                        .and_then(|v| v["usage"]["prompt_tokens"].as_u64());
                    finish.tokens_out = value
                        .as_ref()
                        .and_then(|v| v["usage"]["completion_tokens"].as_u64());
                    match value
                        .as_ref()
                        .and_then(|v| v["choices"][0]["message"]["content"].as_str())
                        .filter(|s| !s.trim().is_empty())
                    {
                        Some(text) => {
                            finish.status = CallStatus::Success;
                            Ok(text.to_owned())
                        }
                        None => {
                            finish.status = CallStatus::InvalidOutput;
                            Err(AppError::new(
                                ErrorCode::LlmOutputInvalid,
                                "服务响应缺少有效内容",
                            ))
                        }
                    }
                }
            }
        };
        self.recorder.finish(finish).await?;
        result
    }
}

impl<T: HttpTransport, L: CallRecorder> LanguageModel for OpenAiCompatible<T, L> {
    async fn complete(
        &self,
        snapshot: &ProviderSnapshot,
        input: CompletionInput,
        call: CallStart,
    ) -> Result<String> {
        OpenAiCompatible::complete(self, Some(snapshot), input, call).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    struct Stub {
        calls: Mutex<usize>,
        status: u16,
        body: Vec<u8>,
    }
    impl HttpTransport for Stub {
        async fn post(&self, _: HttpRequest) -> Result<HttpResponse> {
            *self.calls.lock().unwrap() += 1;
            Ok(HttpResponse {
                status: self.status,
                body: self.body.clone(),
                latency_ms: 4,
            })
        }
    }
    #[derive(Default)]
    struct Log {
        starts: Mutex<Vec<CallStart>>,
        finishes: Mutex<Vec<CallFinish>>,
    }
    impl CallRecorder for Log {
        async fn start(&self, call: CallStart) -> Result<()> {
            self.starts.lock().unwrap().push(call);
            Ok(())
        }
        async fn finish(&self, call: CallFinish) -> Result<()> {
            self.finishes.lock().unwrap().push(call);
            Ok(())
        }
    }
    fn input() -> CompletionInput {
        CompletionInput {
            system: "原创测试".into(),
            user: "test".into(),
            json: false,
        }
    }
    fn call() -> CallStart {
        CallStart {
            profile_id: "p".into(),
            call_id: "call".into(),
            purpose: "test".into(),
            model: String::new(),
            prompt_version: "v1".into(),
            provider_fingerprint: String::new(),
            now: 0,
        }
    }
    fn snapshot() -> ProviderSnapshot {
        ProviderSnapshot {
            config: cijian_domain::provider::ProviderConfig {
                kind: ProviderKind::Llm,
                endpoint: "https://example.invalid/v1".into(),
                model: "fixture-configured".into(),
                voice: None,
                revision: 1,
            },
            secret: Some(Secret::new("test-secret-do-not-log".into()).unwrap()),
        }
    }
    #[tokio::test]
    async fn missing_configuration_never_calls_transport() {
        let p = OpenAiCompatible {
            transport: Stub {
                calls: Mutex::new(0),
                status: 200,
                body: vec![],
            },
            recorder: Log::default(),
        };
        assert_eq!(
            p.complete(None, input(), call()).await.unwrap_err().code,
            ErrorCode::ConfigMissing
        );
        assert_eq!(*p.transport.calls.lock().unwrap(), 0);
    }
    #[tokio::test]
    async fn speech_invalid_output_and_http_errors_are_logged_without_retry() {
        for (status, body, expected) in [
            (
                200,
                b"RIFF invalid WAVE".to_vec(),
                CallStatus::InvalidOutput,
            ),
            (
                429,
                b"test-secret-do-not-log".to_vec(),
                CallStatus::HttpError,
            ),
        ] {
            let provider = OpenAiCompatible {
                transport: Stub {
                    calls: Mutex::new(0),
                    status,
                    body,
                },
                recorder: Log::default(),
            };
            let mut config = snapshot();
            config.config.kind = ProviderKind::Tts;
            config.config.voice = Some("configured-voice".into());
            let error = provider
                .speech(&config, "An original sentence.", call())
                .await
                .unwrap_err();
            assert!(!error.message.contains("test-secret-do-not-log"));
            assert_eq!(*provider.transport.calls.lock().unwrap(), 1);
            assert_eq!(
                provider.recorder.finishes.lock().unwrap()[0].status,
                expected
            );
            assert_eq!(provider.recorder.starts.lock().unwrap().len(), 1);
        }
    }
    #[tokio::test]
    async fn success_and_failures_are_recorded_once_without_secret() {
        for (status, body, expected) in [
            (
                200,
                br#"{"choices":[{"message":{"content":"original"}}]}"#.to_vec(),
                CallStatus::Success,
            ),
            (
                429,
                b"test-secret-do-not-log".to_vec(),
                CallStatus::HttpError,
            ),
            (200, b"invalid".to_vec(), CallStatus::InvalidOutput),
        ] {
            let p = OpenAiCompatible {
                transport: Stub {
                    calls: Mutex::new(0),
                    status,
                    body,
                },
                recorder: Log::default(),
            };
            let result = p.complete(Some(&snapshot()), input(), call()).await;
            assert_eq!(*p.transport.calls.lock().unwrap(), 1);
            assert_eq!(p.recorder.starts.lock().unwrap().len(), 1);
            assert_eq!(p.recorder.finishes.lock().unwrap()[0].status, expected);
            assert!(
                !format!("{result:?} {:?}", p.recorder.starts.lock().unwrap())
                    .contains("test-secret")
            );
        }
    }
}
