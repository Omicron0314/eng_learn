//! Pure learning rules; all time and randomness supplied by callers.
pub mod analysis;
pub mod audio;
pub mod gloss;
pub mod import;
pub mod json;
pub mod knowledge;
pub mod lexicon;
pub mod pack;
pub mod provider;
pub mod reading;
pub mod settings;
pub mod srs;
pub mod time;
pub mod wordlist;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{field}: {message}")]
pub struct ValidationError {
    pub field: &'static str,
    pub message: &'static str,
}

impl ValidationError {
    pub fn new(field: &'static str, message: &'static str) -> Self {
        Self { field, message }
    }
}

/// IDs retain their original value during migration; creation belongs to an injected source.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Id(String);
impl Id {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for Id {
    type Error = ValidationError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
            return Err(ValidationError::new("id", "标识符无效"));
        }
        Ok(Self(value))
    }
}
impl From<Id> for String {
    fn from(value: Id) -> Self {
        value.0
    }
}
pub type ProfileId = Id;
pub type CardId = Id;

pub trait IdSource {
    fn next_id(&mut self) -> Id;
}
pub trait RandomSource {
    fn next_u64(&mut self) -> u64;
}
