//! Optional dictionary audio source for word packs.
//!
//! This is deliberately **not** part of the configured provider surface: it has no
//! model, no secret and no per-call budget, and it is only reached by the pack build
//! tool, never by the running app. Every fetch is one request with no retry.
use super::{NativeHttp, Result};
use cijian_ports::{AppError, ErrorCode};
use serde_json::Value;
use std::time::Duration;

const AUDIO_ENDPOINT: &str = "https://dict.youdao.com/dictvoice";
const ENTRY_ENDPOINT: &str = "https://dict.youdao.com/jsonapi";
const MAX_AUDIO_BYTES: usize = 512 * 1024;
const MAX_ENTRY_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Default)]
pub struct WordEntry {
    pub ipa_uk: String,
    pub ipa_us: String,
    pub uk: Vec<u8>,
    pub us: Vec<u8>,
}

fn failed(message: &str) -> AppError {
    AppError::new(ErrorCode::ProviderError, message)
}

/// Percent-encodes everything outside the unreserved set so multi-word entries and
/// apostrophes survive the query string unchanged.
pub fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Anything outside the accepted containers is rejected instead of being cached.
pub fn audio_format(bytes: &[u8]) -> Option<&'static str> {
    cijian_domain::audio::container(bytes)
}

/// Extracts the two transcriptions from the dictionary entry payload.
pub fn transcriptions(value: &Value) -> (String, String) {
    let text = |key: &str| {
        value
            .get("ec")
            .and_then(|ec| ec.get("word"))
            .and_then(Value::as_array)
            .and_then(|words| words.first())
            .and_then(|word| word.get(key))
            .and_then(Value::as_str)
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty() && v.chars().count() <= 100)
            .map(|v| {
                if v.starts_with('/') {
                    v
                } else {
                    format!("/{v}/")
                }
            })
            .unwrap_or_default()
    };
    (text("ukphone"), text("usphone"))
}

impl NativeHttp {
    /// One GET with the same guards as the model transport: no redirect, size capped.
    pub async fn get_bytes(&self, url: &str, limit: usize) -> Result<Vec<u8>> {
        let mut response = self
            .client
            .get(url)
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .map_err(|_| failed("词典服务连接失败或超时"))?;
        if response.status().as_u16() != 200 {
            return Err(failed("词典服务返回非 200"));
        }
        if response
            .content_length()
            .is_some_and(|length| length > limit as u64)
        {
            return Err(failed("词典服务响应超出资源保护"));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| failed("无法读取词典服务响应"))?
        {
            if bytes
                .len()
                .checked_add(chunk.len())
                .is_none_or(|size| size > limit)
            {
                return Err(failed("词典服务响应超出资源保护"));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
    async fn audio(&self, word: &str, accent: u8) -> Result<Vec<u8>> {
        let url = format!("{AUDIO_ENDPOINT}?audio={}&type={accent}", encode(word));
        let bytes = self.get_bytes(&url, MAX_AUDIO_BYTES).await?;
        if audio_format(&bytes).is_none() {
            return Err(failed("词典音频格式无法识别"));
        }
        Ok(bytes)
    }
    /// Fetches both accents and the transcriptions for one headword.
    pub async fn word_entry(&self, word: &str) -> Result<WordEntry> {
        let word = word.trim();
        if word.is_empty() || word.chars().count() > 64 {
            return Err(AppError::new(ErrorCode::InvalidInput, "词条为空或过长"));
        }
        let entry = format!("{ENTRY_ENDPOINT}?q={}", encode(word));
        let body = self.get_bytes(&entry, MAX_ENTRY_BYTES).await?;
        let value: Value =
            serde_json::from_slice(&body).map_err(|_| failed("词典条目响应无法解析"))?;
        let (ipa_uk, ipa_us) = transcriptions(&value);
        let uk = self.audio(word, 1).await?;
        let us = self.audio(word, 2).await?;
        Ok(WordEntry {
            ipa_uk,
            ipa_us,
            uk,
            us,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn encodes_reserved_characters() {
        assert_eq!(encode("ice cream"), "ice%20cream");
        assert_eq!(encode("can't"), "can%27t");
        assert_eq!(encode("plain"), "plain");
    }

    #[test]
    fn recognises_only_decodable_containers() {
        assert_eq!(audio_format(b"RIFF\0\0\0\0WAVEfmt "), Some("wav"));
        assert_eq!(audio_format(b"ID3\x03\0\0"), Some("mp3"));
        assert_eq!(audio_format(&[0xFF, 0xFB, 0x90, 0x00]), Some("mp3"));
        assert_eq!(audio_format(b"<html>"), None);
        assert_eq!(audio_format(b"\x89PNG"), None);
    }

    #[test]
    fn wraps_bare_phones_in_slashes_and_rejects_oversized() {
        let value = json!({"ec":{"word":[{"ukphone":"ˌəʊvəˈlʊk","usphone":"/ˌoʊvərˈlʊk/"}]}});
        assert_eq!(
            transcriptions(&value),
            ("/ˌəʊvəˈlʊk/".into(), "/ˌoʊvərˈlʊk/".into())
        );
        let long = "x".repeat(101);
        let value = json!({"ec":{"word":[{"ukphone":long,"usphone":""}]}});
        assert_eq!(transcriptions(&value), (String::new(), String::new()));
        assert_eq!(transcriptions(&json!({})), (String::new(), String::new()));
    }
}
