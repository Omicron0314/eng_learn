use crate::{ValidationError, lexicon::lemmas};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TokenKind {
    Word,
    Number,
    Other,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Token {
    pub start: usize,
    pub end: usize,
    pub kind: TokenKind,
    pub sentence_start: bool,
}
pub fn tokenize(body: &str) -> Vec<Token> {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let pattern = PATTERN
        .get_or_init(|| Regex::new(r"[A-Za-z]+(?:['’\-][A-Za-z]+)*|[0-9]+(?:\.[0-9]+)?").unwrap());
    let mut result = Vec::new();
    let mut previous = 0;
    let mut start = true;
    for found in pattern.find_iter(body) {
        if previous < found.start() {
            let gap = &body[previous..found.start()];
            result.push(Token {
                start: previous,
                end: found.start(),
                kind: TokenKind::Other,
                sentence_start: false,
            });
            if gap.contains(['.', '?', '!', '\n']) {
                start = true;
            }
        }
        let word = found.as_str().as_bytes()[0].is_ascii_alphabetic();
        result.push(Token {
            start: found.start(),
            end: found.end(),
            kind: if word {
                TokenKind::Word
            } else {
                TokenKind::Number
            },
            sentence_start: start,
        });
        if word {
            start = false;
        }
        previous = found.end();
    }
    if previous < body.len() {
        result.push(Token {
            start: previous,
            end: body.len(),
            kind: TokenKind::Other,
            sentence_start: false,
        });
    }
    result
}
pub fn utf16_to_utf8(body: &str, offset: usize) -> Result<usize, ValidationError> {
    let mut units = 0;
    for (byte, ch) in body.char_indices() {
        if units == offset {
            return Ok(byte);
        }
        units += ch.len_utf16();
        if units > offset {
            return Err(ValidationError::new("range", "UTF-16位置切开了代理对"));
        }
    }
    if units == offset {
        Ok(body.len())
    } else {
        Err(ValidationError::new("range", "文字位置超出正文"))
    }
}
pub fn contraction(word: &str) -> String {
    let word = word.to_lowercase().replace('’', "'");
    match word.as_str() {
        "won't" => return "will".into(),
        "can't" => return "can".into(),
        "shan't" => return "shall".into(),
        "ain't" => return "be".into(),
        _ => {}
    }
    for suffix in ["n't", "'m", "'re", "'ve", "'ll", "'d", "'s"] {
        if let Some(base) = word.strip_suffix(suffix).filter(|s| !s.is_empty()) {
            return base.to_owned();
        }
    }
    word
}
const FUNCTION_WORDS: &str = "a an the i me my mine myself you your yours yourself yourselves he him his himself she her hers herself it its itself we us our ours ourselves they them their theirs themselves this that these those who whom whose which what whatever whoever whichever there here be am is are was were been being have has had having do does did doing done will would shall should can could may might must ought not no nor and or but so yet if then than because although though while whereas unless until since as whether either neither both of in on at by for with about against between into through during before after above below to from up down out off over under again further once around among along across behind beyond near toward towards upon within without onto some any each every all few more most other such own same only very too just also even still already ever never how when where why one ones yes oh ok okay";
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Analysis {
    pub version: String,
    pub token_count: u64,
    pub coverage: Option<f64>,
    pub type_coverage: Option<f64>,
    pub unknown: BTreeMap<String, u64>,
    pub occurrences: BTreeMap<String, u64>,
    pub excluded_names: BTreeSet<String>,
}
pub fn analyze(
    body: &str,
    known: &BTreeSet<String>,
    exposed: &BTreeSet<String>,
    control: &BTreeSet<String>,
    names: &BTreeSet<String>,
) -> Analysis {
    let mut result = Analysis {
        version: "cijian-wordnet-v1".into(),
        token_count: 0,
        coverage: None,
        type_coverage: None,
        unknown: BTreeMap::new(),
        occurrences: exposed.union(control).map(|s| (s.clone(), 0)).collect(),
        excluded_names: BTreeSet::new(),
    };
    let function_words: BTreeSet<_> = FUNCTION_WORDS.split_whitespace().collect();
    let mut known_count = 0;
    let mut types = BTreeMap::<String, bool>::new();
    let is_known = |word: &str| {
        function_words.contains(word) || known.contains(word) || exposed.contains(word)
    };
    for token in tokenize(body) {
        if token.kind != TokenKind::Word {
            continue;
        }
        let surface = &body[token.start..token.end];
        let stem = contraction(surface);
        if names.contains(&stem)
            || (!token.sentence_start && stem != "i" && surface.as_bytes()[0].is_ascii_uppercase())
        {
            result.excluded_names.insert(stem);
            continue;
        }
        result.token_count += 1;
        let candidates = lemmas(&stem);
        let primary = candidates[0].clone();
        for (target, count) in &mut result.occurrences {
            if candidates.contains(target) {
                *count += 1;
            }
        }
        let known = candidates.iter().any(|s| is_known(s))
            || (primary.contains('-')
                && primary
                    .split('-')
                    .all(|part| lemmas(part).iter().any(|s| is_known(s))));
        if known {
            known_count += 1;
        } else {
            *result.unknown.entry(primary.clone()).or_default() += 1;
        }
        *types.entry(primary).or_default() |= known;
    }
    if result.token_count > 0 {
        result.coverage = Some(f64::from(known_count) / result.token_count as f64);
    }
    if !types.is_empty() {
        result.type_coverage =
            Some(types.values().filter(|known| **known).count() as f64 / types.len() as f64);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offsets_preserve_every_original_byte() {
        let text = "你好🙂 Don't run—it's 3.5!\nNext.";
        let tokens = tokenize(text);
        assert_eq!(
            tokens
                .iter()
                .map(|t| &text[t.start..t.end])
                .collect::<String>(),
            text
        );
        assert!(utf16_to_utf8(text, 3).is_err());
        assert_eq!(utf16_to_utf8(text, 4).unwrap(), 10);
    }
    #[test]
    fn empty_denominator_and_controls_are_not_known() {
        let empty = BTreeSet::new();
        assert_eq!(
            analyze("123…", &empty, &empty, &empty, &empty).coverage,
            None
        );
        let exposed = BTreeSet::from(["cat".into()]);
        let controls = BTreeSet::from(["dog".into()]);
        let analysis = analyze("cats dogs", &empty, &exposed, &controls, &empty);
        assert_eq!(analysis.coverage, Some(0.5));
        assert_eq!(analysis.occurrences["cat"], 1);
        assert_eq!(analysis.occurrences["dog"], 1);
    }
}
