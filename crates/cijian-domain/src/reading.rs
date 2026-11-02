use crate::{
    ValidationError,
    analysis::{TokenKind, contraction, tokenize},
    lexicon::lemmas,
};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextRange {
    pub start: usize,
    pub end: usize,
}
impl TextRange {
    pub fn text(self, body: &str) -> Result<&str, ValidationError> {
        if self.start >= self.end {
            return Err(ValidationError::new("range", "文字范围为空或逆序"));
        }
        body.get(self.start..self.end).ok_or(ValidationError::new(
            "range",
            "文字范围越界或切开Unicode字符",
        ))
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Selection {
    pub range: TextRange,
    pub surface: String,
    pub lemma: String,
    pub sentence: String,
    pub sentence_range: TextRange,
}
pub fn sentences(body: &str) -> Vec<TextRange> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut push = |end: usize| {
        let raw = &body[start..end];
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            let left = raw.len() - raw.trim_start().len();
            result.push(TextRange {
                start: start + left,
                end: start + left + trimmed.len(),
            });
        }
        start = end;
    };
    for (byte, ch) in body.char_indices() {
        if matches!(ch, '.' | '!' | '?' | '\n') {
            push(byte + ch.len_utf8());
        }
    }
    push(body.len());
    result
}
pub fn select(body: &str, range: TextRange) -> Result<Selection, ValidationError> {
    let surface = range.text(body)?;
    if !tokenize(body)
        .iter()
        .any(|t| t.start == range.start && t.end == range.end && t.kind == TokenKind::Word)
    {
        return Err(ValidationError::new("range", "请选择完整英文词"));
    }
    let sentence_range = sentences(body)
        .into_iter()
        .find(|s| s.start <= range.start && s.end >= range.end)
        .ok_or(ValidationError::new("range", "无法定位所在句"))?;
    let normalized = contraction(surface);
    let lemma = lemmas(&normalized).into_iter().next().unwrap_or(normalized);
    Ok(Selection {
        range,
        surface: surface.into(),
        lemma,
        sentence: sentence_range.text(body)?.into(),
        sentence_range,
    })
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeedbackMode {
    Reading,
    Listening,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Difficulty {
    TooEasy,
    JustRight,
    TooHard,
}
pub fn validate_flag(body: &str, ranges: &[TextRange], note: &str) -> Result<(), ValidationError> {
    if note.chars().count() > 200 {
        return Err(ValidationError::new("note", "备注最多200字符"));
    }
    if ranges.is_empty() && note.trim().is_empty() {
        return Err(ValidationError::new("flag", "请选择句子或填写备注"));
    }
    let sentences = sentences(body);
    for (index, range) in ranges.iter().enumerate() {
        range.text(body)?;
        if !sentences.contains(range) || ranges[..index].contains(range) {
            return Err(ValidationError::new("range", "句子范围无效或重复"));
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_preserves_original_unicode_and_rejects_partial_words() {
        let body = "🙂 Cats sleep.\nDogs run!";
        let selected = select(body, TextRange { start: 5, end: 9 }).unwrap();
        assert_eq!(selected.surface, "Cats");
        assert_eq!(selected.lemma, "cat");
        assert_eq!(selected.sentence, "🙂 Cats sleep.");
        assert!(select(body, TextRange { start: 6, end: 9 }).is_err());
        assert!(TextRange { start: 1, end: 5 }.text(body).is_err());
    }
    #[test]
    fn flags_bind_to_complete_sentences_and_unicode_character_limit() {
        let body = "Cats sleep. Dogs run!";
        let ranges = sentences(body);
        assert!(validate_flag(body, &ranges, &"好".repeat(200)).is_ok());
        assert!(validate_flag(body, &ranges, &"好".repeat(201)).is_err());
        assert!(validate_flag(body, &[TextRange { start: 0, end: 4 }], "").is_err());
        assert!(validate_flag(body, &[], "").is_err());
        assert!(validate_flag(body, &[ranges[0], ranges[0]], "").is_err());
    }
}
