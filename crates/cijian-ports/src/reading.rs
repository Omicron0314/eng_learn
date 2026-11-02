use cijian_domain::reading::{Difficulty, FeedbackMode, Selection, TextRange};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone)]
pub struct ReaderCommand {
    pub profile: String,
    pub operation: String,
    pub event: String,
    pub article: String,
    pub body_hash: String,
    pub now: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadingSession {
    pub id: String,
    pub article_id: String,
    pub position: usize,
    pub opened_at: i64,
    pub completed_at: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feedback {
    pub mode: FeedbackMode,
    pub value: Difficulty,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentenceFlag {
    pub ranges: Vec<TextRange>,
    pub note: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupContext {
    pub article_id: String,
    pub body_hash: String,
    pub selection: Selection,
}
