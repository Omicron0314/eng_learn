//! Commands shared by UI and platform composition roots. No IO is performed here.
use cijian_domain::{import::BookKind, srs::Rating};
use cijian_ports::{
    Result,
    library::{CardPage, CardView, ImportReport},
};
use futures::future::BoxFuture;
#[derive(Debug, Clone)]
pub enum Command {
    SetTheme {
        value: cijian_domain::settings::Theme,
    },
    SetPlaybackRate {
        rate: f64,
    },
    SetSystemVoice {
        voice: String,
    },
    SetPronunciationAccent {
        accent: cijian_domain::audio::Accent,
    },
    SetReviewAutoPlay {
        value: bool,
    },
    PlayStoredAudio {
        article: String,
        audio: String,
    },
    EnsureSystemAudio {
        article: String,
        voice: String,
    },
    EnsureAudio {
        article: String,
        operation: String,
    },
    /// Plays the reviewed word on its own sink; article playback is untouched.
    PlayWord {
        lexeme: String,
        accent: cijian_domain::audio::Accent,
    },
    AudioPause,
    AudioResume,
    AudioStop,
    AudioSeek {
        position: f64,
    },
    AudioRate {
        rate: f64,
    },
    AudioStatus,
    InspectLegacy,
    SelectLegacyProfile {
        selection: String,
        profile: String,
    },
    ImportLegacy {
        selection: String,
    },
    ExportBackup {
        include_audio: bool,
    },
    RestoreBackup,
    LoadStats {
        page: u32,
    },
    LoadArticles {
        from: Option<i64>,
        until: Option<i64>,
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
    SaveReadingPosition {
        article: String,
        body_hash: String,
        session: String,
        position: usize,
        revision: u64,
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
    FlagSentences {
        article: String,
        body_hash: String,
        flag: cijian_ports::reading::SentenceFlag,
        operation: String,
    },
    Mark {
        article: String,
        body_hash: String,
        range: cijian_domain::reading::TextRange,
        action: cijian_ports::inbox::MarkAction,
        revision: u64,
        meaning: String,
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
    /// Reads the device-local preferences, so a fresh window shows the accent,
    /// rate and theme that the backend will actually use.
    LoadPreferences,
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
    EditLibraryCard {
        card: String,
        revision: u64,
        meaning: String,
        example: String,
        suspended: bool,
        operation: String,
    },
    PreviewDeleteBook {
        book: String,
    },
    DeleteBook {
        book: String,
        fingerprint: String,
        operation: String,
    },
    LoadLibrary {
        filter: cijian_ports::library::CardFilter,
    },
    ChooseImport {
        word_list: bool,
    },
    ImportSelected {
        target_book: Option<String>,
        handle: String,
        name: String,
        kind: BookKind,
        word_list: bool,
    },
    ExplainCard {
        card: String,
        revision: u64,
        operation: String,
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
impl Command {
    /// Polling keeps a long-running job or the playback cursor in sync. It is
    /// scheduled by the UI itself, so it must stay invisible: no busy notice,
    /// no disabled controls and no error surface. The next tick retries.
    pub fn is_background_poll(&self) -> bool {
        matches!(self, Self::ObserveJob { .. } | Self::AudioStatus)
    }

    /// Local playback and preference writes are fire-and-forget from the user's
    /// point of view. They must never take the busy flag: that flashed the
    /// snackbar and disabled the card on every pronunciation click.
    /// What a slow foreground request is doing, so the busy notice never says
    /// only "processing". Local and background commands return None.
    pub fn busy_label(&self) -> Option<&'static str> {
        match self {
            Self::Lookup { .. } => Some("正在查询释义…"),
            Self::ExplainCard { .. } => Some("正在生成释义…"),
            Self::GenerateArticle { .. } => Some("正在生成文章…"),
            Self::EnsureAudio { .. } => Some("正在生成全文音频…"),
            Self::OpenArticle { .. } => Some("正在打开文章…"),
            Self::ImportLegacy { .. } => Some("正在迁入旧数据…"),
            Self::ChooseImport { .. } => Some("正在读取文件…"),
            Self::ImportSelected { .. } => Some("正在导入词书…"),
            Self::ExportBackup { .. } => Some("正在写入备份…"),
            Self::RestoreBackup => Some("正在恢复备份…"),
            Self::Mark { .. } | Self::BatchInbox { .. } => Some("正在保存…"),
            Self::Rate { .. } => Some("正在记录评分…"),
            Self::SaveSettings { .. } | Self::SaveProvider { .. } => Some("正在保存设置…"),
            _ => None,
        }
    }
    pub fn is_quiet(&self) -> bool {
        self.is_background_poll()
            || matches!(
                self,
                Self::LoadPreferences
                    | Self::PlayWord { .. }
                    | Self::PlayStoredAudio { .. }
                    | Self::AudioPause
                    | Self::AudioResume
                    | Self::AudioStop
                    | Self::AudioSeek { .. }
                    | Self::AudioRate { .. }
                    | Self::SetTheme { .. }
                    | Self::SetPlaybackRate { .. }
                    | Self::SetSystemVoice { .. }
                    | Self::SetPronunciationAccent { .. }
                    | Self::SetReviewAutoPlay { .. }
            )
    }
}
#[derive(Debug, Clone)]
pub struct SelectedFile {
    pub handle: String,
    pub name: String,
    /// A `.cijianpack` already carries its own words, meanings and purpose, so
    /// the dialog must not ask for an import kind or a merge target.
    pub word_pack: bool,
}
#[derive(Debug, Clone)]
pub enum Response {
    Preferences(cijian_domain::settings::DevicePreferences),
    Audio(cijian_domain::audio::Playback),
    WordAudio {
        accent: cijian_domain::audio::Accent,
    },
    LegacyProfiles {
        selection: String,
        profiles: Vec<cijian_ports::legacy::LegacyProfile>,
    },
    LegacyPreview {
        selection: String,
        report: Box<cijian_ports::legacy::LegacyReport>,
    },
    LegacyImported {
        already_present: bool,
        directory: String,
    },
    BackupSaved {
        include_audio: bool,
    },
    RestoreOpened,
    FileCancelled,
    Stats(Box<cijian_ports::stats::StatsSnapshot>),
    Articles(Vec<cijian_ports::articles::Article>),
    Article {
        article: Box<cijian_ports::articles::Article>,
        session: cijian_ports::reading::ReadingSession,
        audio: Vec<cijian_ports::audio::AudioAsset>,
        feedback: Vec<cijian_ports::reading::Feedback>,
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
    ReadingPositionSaved {
        session: String,
        position: usize,
        revision: u64,
    },
    Completed(cijian_ports::reading::ReadingSession),
    FeedbackSaved(cijian_ports::reading::Feedback),
    SentencesFlagged {
        article: String,
        body_hash: String,
    },
    InboxProcessed(cijian_ports::inbox::BatchResult),
    LearningSettingsSaved {
        value: cijian_domain::settings::Settings,
        revision: u64,
    },
    ProviderSaved {
        config: cijian_domain::provider::ProviderConfig,
        has_key: bool,
        source: cijian_ports::provider::SecretSource,
    },
    Settings {
        value: cijian_domain::settings::Settings,
        revision: u64,
        provider: Option<cijian_domain::provider::ProviderConfig>,
        has_session_key: bool,
        tts: Option<cijian_domain::provider::ProviderConfig>,
        has_tts_key: bool,
        source: cijian_ports::provider::SecretSource,
        voices: Vec<cijian_ports::audio::SystemVoice>,
        system_voice: Option<String>,
    },
    Library {
        page: CardPage,
        books: Vec<cijian_ports::library::BookView>,
        latest_import: Option<(String, ImportReport)>,
        entries: Option<cijian_ports::library::BookEntryPage>,
    },
    LibraryCardEdited(CardView),
    DeleteBookPreview(cijian_ports::library::DeletePreview),
    BookDeleted(cijian_ports::library::DeletePreview),
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
    CardExplained(CardView),
}
/// UI schedules dispatch on its background executor; browser adapters return an async transaction.
pub trait CommandBus: Send + Sync {
    fn dispatch(&self, command: Command) -> BoxFuture<'_, Result<Response>>;
}
