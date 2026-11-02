use crate::Result;
use cijian_domain::{
    import::{BookKind, ParsedImport},
    srs::{Rating, SrsState, State},
};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone)]
pub struct ImportIds {
    pub lexeme: String,
    pub card: String,
    pub entry: String,
}
#[derive(Debug, Clone)]
pub struct ImportWords {
    pub profile_id: String,
    pub operation_id: String,
    pub event_id: String,
    pub book_id: String,
    pub import_id: String,
    pub name: String,
    pub kind: BookKind,
    pub source_name: String,
    pub source_hash: String,
    pub format: String,
    pub now: i64,
    pub parsed: ParsedImport,
    pub row_ids: Vec<ImportIds>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImportReport {
    pub parsed: u64,
    pub words_added: u64,
    pub cards_added: u64,
    pub entries_added: u64,
    pub duplicates: u64,
    pub phrases: u64,
    pub skipped: u64,
    pub errors: Vec<cijian_domain::import::RowError>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardView {
    pub id: String,
    pub lexeme_id: String,
    pub front: String,
    pub meaning: String,
    pub example: String,
    pub text_source: String,
    pub suspended: bool,
    pub archived: bool,
    pub revision: u64,
    pub created_at: i64,
    pub srs: SrsState,
}
#[derive(Debug, Clone, Default)]
pub struct CardFilter {
    pub book_id: Option<String>,
    pub search: String,
    pub state: Option<State>,
    pub suspended: Option<bool>,
    pub page: u32,
    pub page_size: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardPage {
    pub total: u64,
    pub cards: Vec<CardView>,
}
#[derive(Debug, Clone)]
pub struct RateCard {
    pub profile_id: String,
    pub operation_id: String,
    pub event_id: String,
    pub review_id: String,
    pub card_id: String,
    pub expected_revision: u64,
    pub rating: Rating,
    pub duration_ms: u32,
    pub revealed: bool,
    pub now: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookView {
    pub id: String,
    pub name: String,
    pub kind: cijian_domain::import::BookKind,
    pub entries: u64,
    pub revision: u64,
}
#[derive(Debug, Clone)]
pub struct EditCard {
    pub profile_id: String,
    pub operation_id: String,
    pub event_id: String,
    pub card_id: String,
    pub expected_revision: u64,
    pub meaning: String,
    pub example: String,
    pub suspended: bool,
    pub now: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeletePreview {
    pub book_id: String,
    pub removed_entries: u64,
    pub shared_cards: u64,
    pub deleted_cards: u64,
    pub archived_cards: u64,
    pub fingerprint: String,
}
#[derive(Debug, Clone)]
pub struct DeleteBook {
    pub profile_id: String,
    pub operation_id: String,
    pub event_id: String,
    pub book_id: String,
    pub fingerprint: String,
    pub now: i64,
}

pub trait LibraryRepository {
    fn import_words(
        &self,
        command: ImportWords,
    ) -> impl std::future::Future<Output = Result<ImportReport>>;
    fn query_cards(
        &self,
        profile: &str,
        filter: CardFilter,
    ) -> impl std::future::Future<Output = Result<CardPage>>;
    fn rate_card(&self, command: RateCard) -> impl std::future::Future<Output = Result<CardView>>;
}
