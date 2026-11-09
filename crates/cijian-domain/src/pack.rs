//! Word pack contents: a manifest plus one tab-separated row per word.
//!
//! The pack is the distribution unit for a word book: words, meaning and the
//! recordings for both accents. Audio is content-addressed, so a key is the
//! sha256 of its own bytes and duplicate words across packs cost nothing.
use crate::ValidationError;
use serde::{Deserialize, Serialize};

pub const PACK_FORMAT: &str = "cijian-pack";
pub const PACK_VERSION: u32 = 1;
pub const MANIFEST_NAME: &str = "manifest.json";
pub const WORDS_NAME: &str = "words.tsv";
pub const AUDIO_DIRS: [&str; 2] = ["audio/uk", "audio/us"];
const MAX_WORDS: usize = 50_000;
const MAX_AUDIO_BYTES: usize = 512 * 1024;
const MAX_WORD_CHARS: usize = 64;
const MAX_MEANING_CHARS: usize = 1000;
const MAX_IPA_CHARS: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub kind: String,
    pub word_count: usize,
    pub accents: Vec<String>,
    pub source: String,
    pub created_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
}

impl Manifest {
    pub fn new(
        name: String,
        kind: String,
        source: String,
        created_at: i64,
        word_count: usize,
    ) -> Self {
        Self {
            format: PACK_FORMAT.into(),
            version: PACK_VERSION,
            name,
            kind,
            word_count,
            accents: crate::audio::Accent::ALL
                .iter()
                .map(|accent| accent.key().to_owned())
                .collect(),
            source,
            created_at,
            license: None,
        }
    }
    pub fn validate(&self) -> Result<(), ValidationError> {
        let problem = if self.format != PACK_FORMAT {
            Some("词包格式标识不支持")
        } else if self.version != PACK_VERSION {
            Some("词包版本不支持")
        } else if self.name.trim().is_empty() || self.name.chars().count() > 80 {
            Some("词包名称无效")
        } else if !["study", "baseline"].contains(&self.kind.as_str()) {
            Some("词包用途无效")
        } else if self.word_count > MAX_WORDS {
            Some("词包词条数量超出上限")
        } else if self.accents.is_empty()
            || self
                .accents
                .iter()
                .any(|accent| crate::audio::Accent::parse(accent).is_none())
        {
            Some("词包口音列表无效")
        } else {
            None
        };
        match problem {
            Some(message) => Err(ValidationError::new("pack", message)),
            None => Ok(()),
        }
    }
}

/// One pack row. `uk`/`us` are blob keys, empty when that accent is absent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackWord {
    pub word: String,
    pub meaning: String,
    pub ipa_uk: String,
    pub ipa_us: String,
    pub uk: String,
    pub us: String,
}

/// A blob key is `<sha256>.<mp3|wav>`; anything else never reaches the store.
pub fn valid_blob_key(key: &str) -> bool {
    if key.is_empty() {
        return true;
    }
    let Some((stem, extension)) = key.rsplit_once('.') else {
        return false;
    };
    stem.len() == 64
        && stem.bytes().all(|b| b.is_ascii_hexdigit())
        && ["mp3", "wav"].contains(&extension)
}

pub fn max_audio_bytes() -> usize {
    MAX_AUDIO_BYTES
}

fn check_ipa(value: &str) -> bool {
    value.chars().count() <= MAX_IPA_CHARS && !value.contains('\t') && !value.contains('\n')
}

impl PackWord {
    pub fn validate(&self) -> Result<(), ValidationError> {
        let problem = if self.word.trim().is_empty() || self.word.chars().count() > MAX_WORD_CHARS {
            Some("词条为空或过长")
        } else if self.meaning.chars().count() > MAX_MEANING_CHARS {
            Some("词条释义过长")
        } else if !check_ipa(&self.ipa_uk) || !check_ipa(&self.ipa_us) {
            Some("词条音标过长或含换行")
        } else if !valid_blob_key(&self.uk) || !valid_blob_key(&self.us) {
            Some("词条音频键无效")
        } else if self.word.contains('\t') || self.meaning.contains('\t') {
            Some("词条含制表符")
        } else {
            None
        };
        match problem {
            Some(message) => Err(ValidationError::new("pack", message)),
            None => Ok(()),
        }
    }
    pub fn to_line(&self) -> String {
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            self.word, self.meaning, self.ipa_uk, self.ipa_us, self.uk, self.us
        )
    }
}

/// Parses one pack row. Extra columns are rejected so a future format change
/// never silently drops data.
pub fn parse_line(line: &str) -> Result<PackWord, ValidationError> {
    let line = line.trim_end_matches('\r');
    if line.trim().is_empty() {
        return Err(ValidationError::new("pack", "词条行内容为空"));
    }
    let fields = line.split('\t').collect::<Vec<_>>();
    if fields.len() != 6 {
        return Err(ValidationError::new(
            "pack",
            "词条行必须为词、释义、英音标、美音标、英音频、美音频六列",
        ));
    }
    let word = PackWord {
        word: fields[0].trim().to_owned(),
        meaning: fields[1].trim().to_owned(),
        ipa_uk: fields[2].trim().to_owned(),
        ipa_us: fields[3].trim().to_owned(),
        uk: fields[4].trim().to_owned(),
        us: fields[5].trim().to_owned(),
    };
    word.validate()?;
    Ok(word)
}

/// Header row of a pack; the first column keeps word lists from being mistaken
/// for data by generic importers.
pub fn header() -> String {
    "word\tmeaning\tipaUk\tipaUs\tukAudio\tusAudio".to_owned()
}
