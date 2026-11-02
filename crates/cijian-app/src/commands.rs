//! Commands shared by UI and platform composition roots. No IO is performed here.
use cijian_domain::{import::BookKind, srs::Rating};
use cijian_ports::{
    Result,
    library::{CardPage, CardView, ImportReport},
};
use futures::future::BoxFuture;
#[derive(Debug, Clone)]
pub enum Command {
    LoadLibrary,
    ChooseImport {
        word_list: bool,
    },
    ImportSelected {
        handle: String,
        name: String,
        kind: BookKind,
        word_list: bool,
    },
    LoadReview,
    Rate {
        card: String,
        revision: u64,
        rating: Rating,
        operation: String,
        duration_ms: u32,
    },
}
#[derive(Debug, Clone)]
pub struct SelectedFile {
    pub handle: String,
    pub name: String,
}
#[derive(Debug, Clone)]
pub enum Response {
    Library(CardPage),
    Selected(Option<SelectedFile>),
    Imported(ImportReport),
    Review {
        cards: Vec<CardView>,
        waiting: bool,
        intervals: Option<[i64; 4]>,
    },
    Rated(CardView),
}
/// UI schedules dispatch on its background executor; browser adapters return an async transaction.
pub trait CommandBus: Send + Sync {
    fn dispatch(&self, command: Command) -> BoxFuture<'_, Result<Response>>;
}
