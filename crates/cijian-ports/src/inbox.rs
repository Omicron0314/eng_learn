use crate::reading::ReaderCommand;
use cijian_domain::reading::TextRange;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MarkAction {
    Known,
    Unknown,
    AddToReview,
}
#[derive(Debug, Clone)]
pub struct MarkWord {
    pub context: ReaderCommand,
    pub range: TextRange,
    pub action: MarkAction,
    pub expected_revision: u64,
    pub lexeme_id: String,
    pub card_id: String,
    pub inbox_id: String,
    pub meaning: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkResult {
    pub lexeme_id: String,
    pub revision: u64,
    pub inbox_id: Option<String>,
    pub card_id: Option<String>,
    pub card_created: bool,
    pub paused: bool,
    pub archived: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboxItem {
    pub id: String,
    pub lemma: String,
    pub article_id: Option<String>,
    pub article_title: Option<String>,
    pub context: String,
    pub body_hash: Option<String>,
    pub range: Option<TextRange>,
    pub revision: u64,
    pub meaning: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InboxAction {
    Add,
    Ignore,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboxSelection {
    pub id: String,
    pub revision: u64,
    pub card_id: String,
    pub event_id: String,
}
#[derive(Debug, Clone)]
pub struct BatchInbox {
    pub profile: String,
    pub operation: String,
    pub action: InboxAction,
    pub selections: Vec<InboxSelection>,
    pub now: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult {
    pub processed: u64,
    pub cards_created: u64,
    pub paused: u64,
    pub archived: u64,
}
