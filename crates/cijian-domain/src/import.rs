use crate::ValidationError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BookKind {
    Baseline,
    Study,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRow {
    pub word: String,
    pub raw: String,
    pub front: String,
    pub meaning: String,
    pub example: String,
    pub line: u64,
    pub phrase: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowError {
    pub line: u64,
    pub reason: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParsedImport {
    pub rows: Vec<ImportRow>,
    pub parsed: u64,
    pub duplicates: u64,
    pub skipped: u64,
    pub phrases: u64,
    pub errors: Vec<RowError>,
}

pub fn normalize_headword(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('’', "'")
        .to_lowercase()
}
pub fn book_name(file_name: &str) -> Result<String, ValidationError> {
    let base = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
    let stem = base
        .rsplit_once('.')
        .filter(|(stem, _)| !stem.is_empty())
        .map(|(stem, _)| stem)
        .unwrap_or(base)
        .trim();
    if stem.is_empty() || stem.chars().any(char::is_control) {
        return Err(ValidationError::new("fileName", "无法从文件名确定词书名称"));
    }
    Ok(stem.to_owned())
}

fn add(
    result: &mut ParsedImport,
    seen: &mut HashSet<String>,
    word: &str,
    meaning: &str,
    example: &str,
    line: u64,
) {
    let key = normalize_headword(word);
    if key.is_empty() {
        result.skipped += 1;
        result.errors.push(RowError {
            line,
            reason: "单词不能为空".into(),
        });
        return;
    }
    result.parsed += 1;
    if meaning.chars().count() > 1000
        || example.chars().count() > 5000
        || key.chars().any(char::is_control)
    {
        result.skipped += 1;
        result.errors.push(RowError {
            line,
            reason: "词条或释义/例句长度无效".into(),
        });
        return;
    }
    if !seen.insert(key.clone()) {
        result.duplicates += 1;
        return;
    }
    let phrase = key.contains(' ');
    result.phrases += u64::from(phrase);
    result.rows.push(ImportRow {
        word: key,
        raw: word.trim().to_owned(),
        front: word.trim().to_owned(),
        meaning: meaning.trim().to_owned(),
        example: example.trim().to_owned(),
        line,
        phrase,
    });
}

pub fn parse_rows(rows: impl IntoIterator<Item = ImportRow>) -> ParsedImport {
    let mut result = ParsedImport::default();
    let mut seen = HashSet::new();
    for row in rows {
        let before = result.rows.len();
        add(
            &mut result,
            &mut seen,
            &row.word,
            &row.meaning,
            &row.example,
            row.line,
        );
        if result.rows.len() > before {
            let added = result.rows.last_mut().expect("inserted row");
            added.front = if row.front.trim().is_empty() {
                row.word.trim().to_owned()
            } else {
                row.front.trim().to_owned()
            };
            added.raw = row.raw;
        }
    }
    result
}

pub fn parse_card_csv(bytes: &[u8]) -> Result<ParsedImport, ValidationError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| ValidationError::new("file", "文件必须为UTF-8"))?
        .trim_start_matches('\u{feff}');
    let mut line_starts = vec![0_u64];
    for (index, byte) in text.bytes().enumerate() {
        if byte == b'\n' || (byte == b'\r' && text.as_bytes().get(index + 1) != Some(&b'\n')) {
            line_starts.push((index + 1) as u64);
        }
    }
    let source_line = |position: &csv::Position| {
        // csv may report the boundary between CR and LF as the next record's position.
        let mut byte = position.byte() as usize;
        while matches!(text.as_bytes().get(byte), Some(b'\r' | b'\n')) {
            byte += 1;
        }
        line_starts.partition_point(|start| *start <= byte as u64) as u64
    };
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut result = ParsedImport::default();
    let mut seen = HashSet::new();
    let mut first = true;
    for record in reader.records() {
        match record {
            Err(error) => {
                result.skipped += 1;
                result.errors.push(RowError {
                    line: error.position().map_or(1, source_line),
                    reason: "CSV格式无效".into(),
                });
            }
            Ok(record) => {
                if record.iter().all(|cell| cell.trim().is_empty()) {
                    continue;
                }
                let word = record.get(0).unwrap_or("");
                if first && word.trim().eq_ignore_ascii_case("word") {
                    first = false;
                    continue;
                }
                first = false;
                add(
                    &mut result,
                    &mut seen,
                    word,
                    record.get(1).unwrap_or(""),
                    record.get(2).unwrap_or(""),
                    record.position().map_or(1, source_line),
                );
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn csv_multiline_duplicate_bad_row_and_unicode() {
        let parsed=parse_card_csv("\u{feff}word,meaning,example\r\nApple,苹果,\"line one\nline two\"\r\napple,不覆盖,\r\n,错误,\r\ntake off,起飞,\r\n".as_bytes()).unwrap();
        assert_eq!(parsed.rows.len(), 2);
        assert_eq!(parsed.duplicates, 1);
        assert_eq!(parsed.phrases, 1);
        assert_eq!(parsed.errors[0].line, 5);
        assert_eq!(parsed.rows[0].example, "line one\nline two");
        assert!(parse_card_csv(&[0xff]).is_err());
    }
    #[test]
    fn filenames_and_apostrophes() {
        assert_eq!(book_name("/tmp/我的词书.csv").unwrap(), "我的词书");
        assert_eq!(normalize_headword("  Don’t  "), "don't");
    }
}
