//! WordNet morphology adapted from wink-lemmatizer 3.0.4.
//! Copyright and WordNet terms are preserved in assets/lexicon/NOTICE.txt.
use std::{collections::HashMap, sync::OnceLock};
struct Lexicon {
    words: HashMap<&'static str, u8>,
    verb: HashMap<&'static str, &'static str>,
    noun: HashMap<&'static str, &'static str>,
    adjective: HashMap<&'static str, &'static str>,
}
fn map(text: &'static str) -> HashMap<&'static str, &'static str> {
    text.lines()
        .filter_map(|line| line.split_once('\t'))
        .collect()
}
fn lexicon() -> &'static Lexicon {
    static LEXICON: OnceLock<Lexicon> = OnceLock::new();
    LEXICON.get_or_init(|| Lexicon {
        words: map(include_str!("../assets/lexicon/words.tsv"))
            .into_iter()
            .map(|(word, flags)| (word, flags.parse().expect("generated word flags")))
            .collect(),
        verb: map(include_str!("../assets/lexicon/verb.tsv")),
        noun: map(include_str!("../assets/lexicon/noun.tsv")),
        adjective: map(include_str!("../assets/lexicon/adjective.tsv")),
    })
}
fn inflect(
    word: &str,
    exceptions: &HashMap<&str, &str>,
    rules: &[(&str, &str)],
    flag: u8,
) -> String {
    if let Some(value) = exceptions.get(word) {
        return (*value).to_owned();
    }
    for (suffix, replacement) in rules {
        if let Some(base) = word.strip_suffix(suffix) {
            let candidate = format!("{base}{replacement}");
            if lexicon()
                .words
                .get(candidate.as_str())
                .is_some_and(|flags| flags & flag != 0)
            {
                return candidate;
            }
        }
    }
    word.to_owned()
}
pub fn lemmas(surface: &str) -> Vec<String> {
    let word = surface.to_lowercase();
    if !word.as_bytes().first().is_some_and(u8::is_ascii_lowercase) {
        return vec![word];
    }
    let lex = lexicon();
    let verb = inflect(
        &word,
        &lex.verb,
        &[
            ("s", ""),
            ("ies", "y"),
            ("es", ""),
            ("es", "e"),
            ("ed", ""),
            ("ed", "e"),
            ("ing", ""),
            ("ing", "e"),
        ],
        4,
    );
    let noun = inflect(
        &word,
        &lex.noun,
        &[
            ("s", ""),
            ("ses", "s"),
            ("xes", "x"),
            ("zes", "s"),
            ("ves", "f"),
            ("ches", "ch"),
            ("shes", "sh"),
            ("men", "man"),
            ("ies", "y"),
        ],
        2,
    );
    let adjective = inflect(
        &word,
        &lex.adjective,
        &[("est", ""), ("est", "e"), ("er", ""), ("er", "e")],
        1,
    );
    let keep = [
        "data",
        "media",
        "news",
        "glasses",
        "clothes",
        "goods",
        "means",
        "series",
        "species",
        "politics",
        "physics",
        "mathematics",
        "economics",
        "people",
        "police",
        "trousers",
        "jeans",
        "scissors",
        "thanks",
        "congratulations",
        "headquarters",
        "boss",
    ];
    let mut output = Vec::new();
    if keep.contains(&word.as_str()) {
        output.push(word.clone());
    }
    for candidate in [verb, noun, adjective] {
        if candidate != word && !output.contains(&candidate) {
            output.push(candidate);
        }
    }
    if !output.contains(&word) {
        output.push(word);
    }
    output
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_original_lemma_probes() {
        let probes: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/lemma/probes.json")).unwrap();
        for probe in probes.as_array().unwrap() {
            let surface = probe["surface"].as_str().unwrap();
            let expected = probe["lemma"].as_str().unwrap();
            assert!(
                lemmas(surface).iter().any(|s| s == expected),
                "{surface}: {expected} {:?}",
                lemmas(surface)
            );
        }
    }
}
