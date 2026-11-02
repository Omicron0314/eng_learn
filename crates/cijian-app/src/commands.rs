//! Commands shared by UI and platform composition roots. No IO is performed here.
use cijian_domain::{import::BookKind, srs::Rating};
use cijian_ports::{
    Result,
    library::{CardPage, CardView, ImportReport},
};
use futures::future::BoxFuture;
#[derive(Debug, Clone)]
pub enum Command {
    LoadStats {
        page: u32,
    },
    LoadArticles {
        page: u32,
    },
    GenerateArticle {
        tier: cijian_domain::settings::Tier,
        genre: cijian_domain::settings::Genre,
        operation: String,
    },
    ObserveJob {
        job: String,
    },
    CancelGeneration {
        job: String,
    },
    OpenArticle {
        id: String,
    },
    Lookup {
        article: String,
        body_hash: String,
        range: cijian_domain::reading::TextRange,
    },
    CompleteReading {
        article: String,
        body_hash: String,
        session: String,
    },
    Feedback {
        article: String,
        body_hash: String,
        value: cijian_domain::reading::Difficulty,
        mode: cijian_domain::reading::FeedbackMode,
        operation: String,
    },
    Mark {
        article: String,
        body_hash: String,
        range: cijian_domain::reading::TextRange,
        action: cijian_ports::inbox::MarkAction,
        revision: u64,
        operation: String,
    },
    LoadInbox {
        page: u32,
    },
    BatchInbox {
        action: cijian_ports::inbox::InboxAction,
        selections: Vec<(String, u64)>,
        operation: String,
    },
    LoadSettings,
    SaveSettings {
        value: cijian_domain::settings::Settings,
        revision: u64,
        operation: String,
    },
    SaveProvider {
        config: cijian_domain::provider::ProviderConfig,
        revision: u64,
        secret: cijian_ports::provider::SecretPatch,
        operation: String,
    },
    LoadLibrary {
        filter: cijian_ports::library::CardFilter,
    },
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
    Stats(Box<cijian_ports::stats::StatsSnapshot>),
    Articles(Vec<cijian_ports::articles::Article>),
    Article {
        article: Box<cijian_ports::articles::Article>,
        session: cijian_ports::reading::ReadingSession,
    },
    Lookup {
        context: cijian_ports::reading::LookupContext,
        revision: u64,
        meaning: String,
    },
    Marked(cijian_ports::inbox::MarkResult),
    Inbox {
        items: Vec<cijian_ports::inbox::InboxItem>,
        total: u64,
    },
    Job(cijian_ports::articles::GenerationJob),
    Completed,
    FeedbackSaved,
    InboxProcessed(cijian_ports::inbox::BatchResult),
    Settings {
        value: cijian_domain::settings::Settings,
        revision: u64,
        provider: Option<cijian_domain::provider::ProviderConfig>,
        has_session_key: bool,
    },
    Library(CardPage),
    Selected(Option<SelectedFile>),
    Imported(ImportReport),
    Review {
        cards: Vec<CardView>,
        waiting: bool,
        intervals: Option<[i64; 4]>,
        stats: Box<cijian_ports::stats::StatsSnapshot>,
        books: Vec<cijian_ports::library::BookView>,
        inbox_pending: u64,
    },
    Rated(CardView),
}
/// UI schedules dispatch on its background executor; browser adapters return an async transaction.
pub trait CommandBus: Send + Sync {
    fn dispatch(&self, command: Command) -> BoxFuture<'_, Result<Response>>;
}
