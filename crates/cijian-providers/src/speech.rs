use super::*;
fn valid_wave(bytes: &[u8]) -> bool {
    if bytes.len() < 44 || bytes.get(..4) != Some(b"RIFF") || bytes.get(8..12) != Some(b"WAVE") {
        return false;
    }
    let total = u32::from_le_bytes(bytes[4..8].try_into().expect("header length")) as usize;
    let Some(end) = total.checked_add(8).filter(|end| *end <= bytes.len()) else {
        return false;
    };
    let mut offset = 12;
    let mut format = false;
    let mut data = false;
    while offset + 8 <= end {
        let length = u32::from_le_bytes(
            bytes[offset + 4..offset + 8]
                .try_into()
                .expect("chunk length"),
        ) as usize;
        let Some(next) = offset
            .checked_add(8)
            .and_then(|v| v.checked_add(length))
            .filter(|v| *v <= end)
        else {
            return false;
        };
        if &bytes[offset..offset + 4] == b"fmt " {
            if length < 16 {
                return false;
            }
            let fields = &bytes[offset + 8..next];
            let encoding = u16::from_le_bytes([fields[0], fields[1]]);
            let channels = u16::from_le_bytes([fields[2], fields[3]]);
            let sample_rate = u32::from_le_bytes(fields[4..8].try_into().expect("fmt length"));
            let bits = u16::from_le_bytes([fields[14], fields[15]]);
            format = channels > 0
                && sample_rate > 0
                && match encoding {
                    1 => [8, 16, 24, 32].contains(&bits),
                    3 => [32, 64].contains(&bits),
                    0xfffe => length >= 40,
                    _ => false,
                };
        } else if &bytes[offset..offset + 4] == b"data" {
            data = length > 0;
        }
        offset = next + (length % 2);
    }
    format && data
}

impl<T: HttpTransport, L: CallRecorder> OpenAiCompatible<T, L> {
    pub async fn speech(
        &self,
        snapshot: &ProviderSnapshot,
        text: &str,
        mut call: CallStart,
    ) -> Result<Vec<u8>> {
        snapshot
            .config
            .validate()
            .map_err(|e| AppError::new(ErrorCode::InvalidInput, e.to_string()))?;
        if snapshot.config.kind != ProviderKind::Tts || text.trim().is_empty() {
            return Err(AppError::new(ErrorCode::InvalidInput, "朗读配置或正文无效"));
        }
        let voice = snapshot
            .config
            .voice
            .as_ref()
            .ok_or(AppError::new(ErrorCode::ConfigMissing, "请配置语音名称"))?;
        call.model = snapshot.config.model.clone();
        call.provider_fingerprint = format!(
            "{:x}",
            Sha256::digest(format!(
                "{}\n{}\n{}\nwav",
                snapshot.config.endpoint, snapshot.config.model, voice
            ))
        );
        let call_id = call.call_id.clone();
        self.recorder.start(call).await?;
        let started = Instant::now();
        let response=self.transport.post(HttpRequest{url:format!("{}/audio/speech",snapshot.config.endpoint.trim_end_matches('/')),bearer:snapshot.secret.clone(),body:json!({"model":snapshot.config.model,"voice":voice,"input":text,"response_format":"wav"})}).await;
        let (status, result) = match response {
            Err(_) => (
                CallStatus::TransportError,
                Err(AppError::new(
                    ErrorCode::ProviderError,
                    "朗读服务连接失败或超时",
                )),
            ),
            Ok(response) if !(200..300).contains(&response.status) => (
                CallStatus::HttpError,
                Err(AppError::new(ErrorCode::ProviderError, "朗读服务返回错误")),
            ),
            Ok(response) if !valid_wave(&response.body) => (
                CallStatus::InvalidOutput,
                Err(AppError::new(
                    ErrorCode::LlmOutputInvalid,
                    "服务未返回有效的WAV音频",
                )),
            ),
            Ok(response) => (CallStatus::Success, Ok(response.body)),
        };
        self.recorder
            .finish(CallFinish {
                call_id,
                status,
                tokens_in: None,
                tokens_out: None,
                latency_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
            })
            .await?;
        result
    }
}
