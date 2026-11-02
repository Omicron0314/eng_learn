use crate::{
    ValidationError,
    import::{
        ImportAccumulator, ImportRow, ParsedImport, RowError, normalize_headword, parse_rows,
    },
};
use regex::Regex;
use serde::Serialize;
use std::sync::OnceLock;
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WordListEntry {
    pub headword: String,
    pub variants: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pos: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meaning: Option<String>,
    pub is_phrase: bool,
    pub raw: String,
}
struct Patterns {
    prefix: Regex,
    variants: Regex,
    phonetics: Regex,
    cjk: Regex,
    pos: Regex,
    parens: Regex,
    valid: Regex,
    heading: Regex,
}
fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(||Patterns{
    prefix:Regex::new(r"^(?:\d+\s*[.、)）:]\s*|[-*•]\s+)").unwrap(),
    variants:Regex::new(r"^([A-Za-z'’.-]+(?:/[A-Za-z'’.-]+)+)(?:\s|$)").unwrap(),
    phonetics:Regex::new(r"/[^/]*/|\[[^\]]*\]").unwrap(),cjk:Regex::new(r"[\x{3000}-\x{9fff}\x{ff00}-\x{ffef}]").unwrap(),
    pos:Regex::new(r"(?i)^(?:n|v|vt|vi|adj|a|adv|ad|prep|conj|pron|art|num|int|interj|aux|abbr|modal\s*v)\.?(?:\s*(?:&|/|,)\s*(?:n|v|vt|vi|adj|a|adv|ad|prep|conj|pron|art|num|int|interj|aux|abbr)\.?)*$").unwrap(),
    parens:Regex::new(r"\(([^)]*)\)").unwrap(),valid:Regex::new(r"^[A-Za-z][A-Za-z'’.\- ]*$").unwrap(),heading:Regex::new(r"(?i)^(unit|lesson|chapter|list|part)\b").unwrap(),
})
}
pub fn parse_line(line: &str) -> Option<WordListEntry> {
    let raw = line.trim_start_matches('\u{feff}').trim();
    if raw.is_empty() || raw.starts_with('#') {
        return None;
    }
    let p = patterns();
    let mut rest = p.prefix.replace(raw, "").trim().to_owned();
    if p.heading.is_match(&rest) && raw.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    let separator = if rest.contains('\t') {
        Some('\t')
    } else if rest.contains(',') {
        Some(',')
    } else {
        None
    };
    let mut column = None;
    if let Some(separator) = separator
        && let Some((head, meaning)) = rest.split_once(separator)
    {
        column = Some(meaning.trim().trim_matches('"').to_owned());
        rest = head.trim().to_owned();
    }
    let prefix = p
        .variants
        .captures(&rest)
        .and_then(|c| c.get(1))
        .map_or(0, |m| m.end());
    rest = format!(
        "{}{}",
        &rest[..prefix],
        p.phonetics.replace_all(&rest[prefix..], " ")
    );
    rest = rest.split_whitespace().collect::<Vec<_>>().join(" ");
    let split = p.cjk.find(&rest).map_or(rest.len(), |m| m.start());
    let mut english = rest[..split].trim().to_owned();
    let mut meaning = rest[split..].trim().to_owned();
    let mut pos = None;
    let words = english.split_whitespace().collect::<Vec<_>>();
    if let Some(index) = words
        .iter()
        .enumerate()
        .find_map(|(i, w)| (i > 0 && p.pos.is_match(w)).then_some(i))
    {
        let mut end = index;
        while end < words.len() && p.pos.is_match(words[end]) {
            end += 1;
        }
        pos = Some(words[index..end].join(" "));
        let tail = words[end..].join(" ");
        if !tail.is_empty() {
            meaning = format!("{tail} {meaning}").trim().to_owned();
        }
        english = words[..index].join(" ");
    }
    if let Some(column) = column.filter(|c| !c.is_empty()) {
        meaning = format!("{meaning} {column}").trim().to_owned();
    }
    let mut variants = Vec::new();
    english = p
        .parens
        .replace_all(&english, |c: &regex::Captures<'_>| {
            variants.extend(
                c[1].split([',', '/'])
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned),
            );
            " "
        })
        .to_string();
    english = english.split_whitespace().collect::<Vec<_>>().join(" ");
    if english.contains('/') && !english.contains(' ') {
        let parts = english.split('/').map(str::to_owned).collect::<Vec<_>>();
        variants.extend(parts.iter().skip(1).cloned());
        english = parts[0].clone();
    }
    if !p.valid.is_match(&english) {
        return None;
    }
    if english.len() == 1
        && english.as_bytes()[0].is_ascii_uppercase()
        && meaning.is_empty()
        && pos.is_none()
    {
        return None;
    }
    let headword = normalize_headword(&english);
    let mut normalized = Vec::new();
    for variant in variants {
        let variant = normalize_headword(&variant);
        if variant != headword && p.valid.is_match(&variant) && !normalized.contains(&variant) {
            normalized.push(variant);
        }
    }
    Some(WordListEntry {
        is_phrase: headword.contains(' '),
        headword,
        variants: normalized,
        pos,
        meaning: (!meaning.is_empty()).then_some(meaning),
        raw: raw.to_owned(),
    })
}
pub fn parse_csv(bytes: &[u8]) -> Result<ParsedImport, ValidationError> {
    Ok(from_csv(crate::import::parse_card_csv(bytes)?))
}
pub fn from_csv(source: ParsedImport) -> ParsedImport {
    let mut rows = Vec::new();
    let mut errors = source.errors;
    for row in source.rows {
        let raw = format!("{}\t{}", row.front, row.meaning);
        if let Some(entry) = parse_line(&raw) {
            rows.push(ImportRow {
                word: entry.headword.clone(),
                front: entry.headword,
                raw,
                meaning: entry.meaning.unwrap_or_default(),
                example: row.example,
                line: row.line,
                phrase: entry.is_phrase,
            });
        } else {
            errors.push(RowError {
                line: row.line,
                reason: "无法识别词条".into(),
            });
        }
    }
    let mut parsed = parse_rows(rows);
    parsed.duplicates += source.duplicates;
    parsed.skipped += errors.len() as u64;
    parsed.errors = errors;
    parsed
}

#[derive(Default)]
pub struct WordListImporter {
    rows: ImportAccumulator,
}
impl WordListImporter {
    pub fn push_line(&mut self, line: &str, number: u64) {
        if line.trim().is_empty()
            || (number == 1 && line.trim().to_lowercase().starts_with("word,"))
        {
            return;
        }
        match parse_line(line) {
            Some(entry) => self.rows.push(ImportRow {
                word: entry.headword.clone(),
                front: entry.headword,
                raw: entry.raw,
                meaning: entry.meaning.unwrap_or_default(),
                example: String::new(),
                line: number,
                phrase: entry.is_phrase,
            }),
            None => {
                if !(line.trim().len() == 1 && line.trim().as_bytes()[0].is_ascii_uppercase())
                    && !patterns().heading.is_match(line.trim())
                {
                    self.rows.error(number, "无法识别词条");
                }
            }
        }
    }
    pub fn finish(self) -> ParsedImport {
        self.rows.finish()
    }
}
pub fn parse(bytes: &[u8]) -> Result<ParsedImport, ValidationError> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| ValidationError::new("file", "词表必须为UTF-8"))?;
    let mut importer = WordListImporter::default();
    for (index, line) in text.lines().enumerate() {
        importer.push_line(line, index as u64 + 1);
    }
    Ok(importer.finish())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_original_wordlist_cases() {
        let cases: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/wordlist/cases.json")).unwrap();
        for case in cases.as_array().unwrap() {
            let input = case["input"].as_str().unwrap();
            let actual = serde_json::to_value(parse_line(input)).unwrap();
            if case["expected"].is_null() {
                assert!(actual.is_null(), "{input}: {actual}");
            } else {
                for (field, value) in case["expected"].as_object().unwrap() {
                    assert_eq!(&actual[field], value, "{input}: {field}");
                }
            }
        }
    }
}
