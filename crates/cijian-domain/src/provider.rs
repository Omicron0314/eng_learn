use crate::ValidationError;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Llm,
    Tts,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub endpoint: String,
    pub model: String,
    pub voice: Option<String>,
    pub revision: u64,
}
impl ProviderConfig {
    pub fn validate(&self) -> Result<(), ValidationError> {
        let url = url::Url::parse(&self.endpoint)
            .map_err(|_| ValidationError::new("endpoint", "服务地址无效"))?;
        if !["http", "https"].contains(&url.scheme())
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || url.query().is_some()
        {
            return Err(ValidationError::new(
                "endpoint",
                "服务地址必须为HTTP(S)，不能包含凭据、查询或片段",
            ));
        }
        if self.model.trim().is_empty() || self.model.chars().any(char::is_control) {
            return Err(ValidationError::new("model", "请输入有效模型名称"));
        }
        if self.kind == ProviderKind::Tts
            && self
                .voice
                .as_ref()
                .is_none_or(|v| v.trim().is_empty() || v.chars().any(char::is_control))
        {
            return Err(ValidationError::new("voice", "请输入有效语音名称"));
        }
        if self.revision == 0 {
            return Err(ValidationError::new("revision", "配置版本无效"));
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_endpoint_or_model_defaults_and_no_embedded_secret() {
        let config = ProviderConfig {
            kind: ProviderKind::Llm,
            endpoint: "https://example.invalid/v1".into(),
            model: "configured-model".into(),
            voice: None,
            revision: 1,
        };
        assert!(config.validate().is_ok());
        for endpoint in [
            "",
            "file:///tmp/test",
            "https://user:key@example.invalid/v1",
            "https://example.invalid/v1?key=test",
        ] {
            assert!(
                ProviderConfig {
                    endpoint: endpoint.into(),
                    ..config.clone()
                }
                .validate()
                .is_err()
            );
        }
        assert!(
            ProviderConfig {
                model: String::new(),
                ..config.clone()
            }
            .validate()
            .is_err()
        );
        assert!(
            ProviderConfig {
                kind: ProviderKind::Tts,
                ..config
            }
            .validate()
            .is_err()
        );
    }
}
