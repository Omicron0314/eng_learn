use crate::ValidationError;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Gloss {
    pub lemma: String,
    pub pos: String,
    pub meaning_zh: String,
    pub meaning_en: String,
    pub ipa: String,
}
impl Gloss {
    pub fn parse(text: &str, lemma: &str) -> Result<Self, ValidationError> {
        let value = Self::parse_any(text)?;
        if value.lemma != lemma {
            return Err(ValidationError::new("gloss", "释义字段无效"));
        }
        Ok(value)
    }
    /// Card context has no surface range to bind; the prompt's base form may
    /// differ for phrases, so only field constraints apply here.
    pub fn parse_any(text: &str) -> Result<Self, ValidationError> {
        let value: Self = serde_json::from_str(text)
            .map_err(|_| ValidationError::new("gloss", "释义JSON结构无效"))?;
        if value.lemma.trim().is_empty()
            || value.meaning_zh.trim().is_empty()
            || value.meaning_zh.chars().count() > 30
            || value.meaning_en.trim().is_empty()
            || value.meaning_en.chars().count() > 200
            || value.pos.chars().count() > 40
            || value.ipa.chars().count() > 100
        {
            return Err(ValidationError::new("gloss", "释义字段无效"));
        }
        Ok(value)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_requires_requested_lemma_and_short_meaning() {
        let valid = r#"{"lemma":"cat","pos":"n.","meaningZh":"猫","meaningEn":"a small feline","ipa":"/kæt/"}"#;
        assert!(Gloss::parse(valid, "cat").is_ok());
        assert!(Gloss::parse(valid, "dog").is_err());
        assert!(Gloss::parse(&valid.replace("猫", &"字".repeat(31)), "cat").is_err());
    }
}
