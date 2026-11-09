//! GPUI views issue application commands; all IO runs on the background executor.
#[cfg(test)]
extern crate gpui_kit as gpui;
mod audio;
mod design;
mod library;
mod reader_flags;
mod reader_positions;
mod reader_text;
mod settings;
mod shell;
mod stats;
mod study;
#[cfg(test)]
mod tests;
mod theme;
use cijian_app::{
    Navigation, Route,
    commands::{Command, CommandBus, Response, SelectedFile},
};
use cijian_domain::{import::BookKind, srs::Rating};
use cijian_ports::{
    AppError,
    library::{CardPage, CardView, ImportReport},
};
use gpui_kit::base::input::InputState;
use gpui_kit::{
    App, Context, Entity, TestSupportExt as _, Window, WindowOptions, div, prelude::*, px,
};
use gpui_kit_material::{
    color::ColorRole,
    components::{ButtonVariant, material_button, title_bar_window_options},
    icons::Assets,
    theme::init_component_theme,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

struct Shell {
    focus: Option<gpui_kit::FocusHandle>,
    navigation: Navigation,
    nav_focus: std::cell::RefCell<Vec<gpui_kit::FocusHandle>>,
    compact: bool,
    viewport_width: f32,
    viewport_height: f32,
    dark: bool,
    #[cfg(test)]
    caption: Option<gpui_kit_material::components::Caption>,
    #[cfg(test)]
    rendered_dark: bool,
    backend: Option<Arc<dyn CommandBus>>,
    loading: bool,
    compact_reader: bool,
    reader_scroll: gpui_kit::ScrollHandle,
    heatmap_focus: std::cell::RefCell<Vec<gpui_kit::FocusHandle>>,
    positions: reader_positions::Positions,
    source_reveal: Option<(String, String, usize)>,
    lookup_loading: bool,
    reader_text: Option<reader_text::ReaderText>,
    error: Option<String>,
    request_id: u64,
    library_ui: library::LibraryState,
    library: Option<CardPage>,
    selected: Option<SelectedFile>,
    word_list: bool,
    import_kind: BookKind,
    report: Option<ImportReport>,
    review: Vec<CardView>,
    waiting: bool,
    intervals: Option<[i64; 4]>,
    revealed: bool,
    review_accent: cijian_domain::audio::Accent,
    review_started: Instant,
    review_done: u32,
    key_intercept: Option<gpui_kit::Subscription>,
    settings: cijian_domain::settings::Settings,
    settings_revision: u64,
    legacy_selection: Option<String>,
    legacy_profiles: Vec<cijian_ports::legacy::LegacyProfile>,
    legacy_report: Option<cijian_ports::legacy::LegacyReport>,
    stored_audio: Vec<cijian_ports::audio::AudioAsset>,
    audio_poll_epoch: u64,
    audio_ready: bool,
    last_audio_system: bool,
    appearance_subscription: Option<gpui_kit::Subscription>,
    system_voices: Vec<cijian_ports::audio::SystemVoice>,
    system_voice_index: usize,
    show_listening_text: bool,
    playback: cijian_domain::audio::Playback,
    provider_kind: cijian_domain::provider::ProviderKind,
    tts_config: Option<cijian_domain::provider::ProviderConfig>,
    has_tts_key: bool,
    key_source: cijian_ports::provider::SecretSource,
    backup_audio: bool,
    generation_poll_pending: bool,
    gloss_pending: Option<String>,
    gloss_failed: Option<(String, String)>,
    provider_config: Option<cijian_domain::provider::ProviderConfig>,
    has_session_key: bool,
    settings_ui: settings::SettingsState,
    page_scroll: gpui_kit::ScrollHandle,
    settings_editor: Option<settings::Editor>,
    articles: Vec<cijian_ports::articles::Article>,
    articles_history: bool,
    articles_page: u32,
    articles_date: Option<chrono::NaiveDate>,
    articles_date_error: Option<String>,
    articles_date_input: Option<Entity<InputState>>,
    today_article_count: u64,
    article: Option<cijian_ports::articles::Article>,
    reading_session: Option<cijian_ports::reading::ReadingSession>,
    reading_feedback: Vec<cijian_ports::reading::Feedback>,
    reader_flags: reader_flags::FlagState,
    lookup: Option<(cijian_ports::reading::LookupContext, u64, String)>,
    mark_result: Option<cijian_ports::inbox::MarkResult>,
    generation_job: Option<cijian_ports::articles::GenerationJob>,
    inbox: Vec<cijian_ports::inbox::InboxItem>,
    inbox_total: u64,
    inbox_selected: std::collections::BTreeSet<String>,
    notice: Option<String>,
    notice_dismissal: Option<String>,
    stats: Option<cijian_ports::stats::StatsSnapshot>,
    stats_page: u32,
    books: Vec<cijian_ports::library::BookView>,
    heatmap_day: Option<usize>,
}
impl Shell {
    fn new() -> Self {
        Self {
            focus: None,
            navigation: Navigation::default(),
            nav_focus: Default::default(),
            compact: false,
            viewport_width: 1440.,
            viewport_height: 900.,
            dark: false,
            #[cfg(test)]
            caption: None,
            #[cfg(test)]
            rendered_dark: false,
            backend: None,
            loading: false,
            compact_reader: false,
            reader_scroll: gpui_kit::ScrollHandle::new(),
            heatmap_focus: Default::default(),
            positions: Default::default(),
            source_reveal: None,
            lookup_loading: false,
            reader_text: None,
            error: None,
            request_id: 0,
            library_ui: Default::default(),
            library: None,
            selected: None,
            word_list: false,
            import_kind: BookKind::Study,
            report: None,
            review: Vec::new(),
            waiting: false,
            intervals: None,
            revealed: false,
            review_accent: cijian_domain::audio::Accent::default(),
            review_started: Instant::now(),
            review_done: 0,
            key_intercept: None,
            settings: Default::default(),
            settings_revision: 1,
            legacy_selection: None,
            legacy_profiles: Vec::new(),
            legacy_report: None,
            stored_audio: Vec::new(),
            audio_poll_epoch: 0,
            audio_ready: false,
            last_audio_system: false,
            appearance_subscription: None,
            system_voices: Vec::new(),
            system_voice_index: 0,
            show_listening_text: false,
            playback: Default::default(),
            provider_kind: cijian_domain::provider::ProviderKind::Llm,
            tts_config: None,
            has_tts_key: false,
            key_source: cijian_ports::provider::SecretSource::Session,
            backup_audio: false,
            generation_poll_pending: false,
            gloss_pending: None,
            gloss_failed: None,
            provider_config: None,
            has_session_key: false,
            settings_ui: Default::default(),
            page_scroll: gpui_kit::ScrollHandle::new(),
            settings_editor: None,
            articles: Vec::new(),
            articles_history: false,
            articles_page: 0,
            articles_date: None,
            articles_date_error: None,
            articles_date_input: None,
            today_article_count: 0,
            article: None,
            reading_session: None,
            reading_feedback: Vec::new(),
            reader_flags: Default::default(),
            lookup: None,
            mark_result: None,
            generation_job: None,
            inbox: Vec::new(),
            inbox_total: 0,
            inbox_selected: Default::default(),
            notice: None,
            notice_dismissal: None,
            stats: None,
            stats_page: 0,
            books: Vec::new(),
            heatmap_day: None,
        }
    }
    fn request(&mut self, command: Command, cx: &mut Context<Self>) {
        let Some(backend) = self.backend.clone() else {
            return;
        };
        self.loading = true;
        self.lookup_loading = matches!(command, Command::Lookup { .. });
        self.error = None;
        self.request_id += 1;
        let request_id = self.request_id;
        let rating = matches!(command, Command::Rate { .. });
        let task = cx
            .background_executor()
            .spawn(async move { backend.dispatch(command).await });
        cx.spawn(async move |this, cx| {
            let response = task.await;
            let _ = this.update(cx, |this, cx| {
                if request_id != this.request_id {
                    return;
                }
                this.loading = false;
                this.lookup_loading = false;
                match response {
                    Err(error) => {
                        if rating && error.code == cijian_ports::ErrorCode::StaleState {
                            this.notice = Some("词卡已更新，已重新加载；请重新评分".into());
                            this.request(Command::LoadReview, cx);
                        } else {
                            this.error = Some(error.message);
                        }
                    }
                    Ok(Response::Preferences(value)) => {
                        this.settings.theme = value.theme;
                        this.settings.playback_rate = value.playback_rate;
                        let _ = this.playback.set_rate(value.playback_rate);
                        this.poll_audio(cx);
                        if let Some(voice) = value.system_voice {
                            this.system_voice_index = this
                                .system_voices
                                .iter()
                                .position(|available| available.id == voice)
                                .unwrap_or(0);
                        }
                    }
                    Ok(Response::Audio(state)) => this.receive_audio(state, cx),
                    Ok(Response::WordAudio { accent }) => this.review_accent = accent,
                    Ok(Response::LegacyProfiles {
                        selection,
                        profiles,
                    }) => {
                        this.legacy_selection = Some(selection);
                        this.legacy_profiles = profiles;
                        this.legacy_report = None;
                    }
                    Ok(Response::LegacyPreview { selection, report }) => {
                        this.legacy_selection = Some(selection);
                        this.legacy_profiles.clear();
                        this.legacy_report = Some(*report);
                    }
                    Ok(Response::LegacyImported {
                        already_present,
                        directory,
                    }) => {
                        this.notice = Some(if already_present {
                            format!("相同快照已迁入，没有重复创建学习记录。位置：{directory}")
                        } else {
                            format!(
                                "迁入完成，新学习库已打开。原 Web 数据未改动。位置：{directory}"
                            )
                        });
                    }
                    Ok(Response::BackupSaved { include_audio }) => {
                        this.notice = Some(
                            if include_audio {
                                "学习备份已保存，包含音频缓存，不包含密钥"
                            } else {
                                "学习备份已保存，不包含密钥和音频缓存"
                            }
                            .into(),
                        )
                    }
                    Ok(Response::RestoreOpened) => {
                        this.notice =
                            Some("恢复完成，新学习库已在另一个窗口打开；当前学习库保留".into())
                    }
                    Ok(Response::FileCancelled) => {}
                    Ok(Response::Stats(stats)) => this.stats = Some(*stats),
                    Ok(Response::Articles(articles)) => {
                        if !this.articles_history && this.articles_page == 0 {
                            this.today_article_count = articles.len() as u64;
                        }
                        this.articles = articles;
                    }
                    Ok(Response::Article {
                        article,
                        session,
                        audio,
                        feedback,
                    }) => {
                        if this
                            .source_reveal
                            .as_ref()
                            .is_some_and(|(id, hash, _)| id != &article.id || hash != &article.body_hash)
                        {
                            this.source_reveal = None;
                            this.notice = Some("来源文章正文已变化，无法定位原词".into());
                        }
                        this.reading_feedback = feedback;
                        this.reader_flags.open = false;
                        this.audio_ready = !audio.is_empty();
                        this.last_audio_system = false;
                        this.stored_audio = audio;
                        let listening=matches!(this.navigation.current(),Route::Article{id,listening:true} if id==&article.id);
                        this.navigation.navigate(Route::Article {
                            id: article.id.clone(),
                            listening,
                        });
                        this.article = Some(*article);
                        this.reading_session = Some(session);
                        this.lookup = None;
                        cx.spawn(async move |this, cx| {
                            cx.background_executor()
                                .timer(std::time::Duration::from_millis(24))
                                .await;
                            let _ = this.update(cx, |_, cx| cx.notify());
                        })
                        .detach();
                    }
                    Ok(Response::Lookup {
                        context,
                        revision,
                        meaning,
                    }) => {this.mark_result = None;this.lookup = Some((context, revision, meaning));},
                    Ok(Response::Marked(result)) => {
                        if let Some((_, revision, _)) = &mut this.lookup {
                            *revision = result.revision;
                        }
                        this.notice = Some(
                            if result.archived {
                                "词卡已归档，未自动恢复"
                            } else if result.paused {
                                "词卡已暂停，可在词库恢复"
                            } else if result.card_id.is_some() {
                                "已加入复习"
                            } else if result.inbox_id.is_some() {
                                "已放入生词箱"
                            } else {
                                "已标记认识"
                            }
                            .into(),
                        );
                    }
                    Ok(Response::Inbox { items, total }) => {
                        this.inbox = items;
                        this.inbox_total = total;
                        this.inbox_selected.clear();
                    }
                    Ok(Response::InboxProcessed(_)) => {
                        this.request(Command::LoadInbox { page: 0 }, cx)
                    }
                    Ok(Response::ReadingPositionSaved {..}) => {}
                    Ok(Response::Completed(session)) => {
                        this.reading_session = Some(session);
                        this.notice = Some("已记录本次阅读完成".into());
                        this.request(Command::LoadStats { page: 0 }, cx);
                    }
                    Ok(Response::FeedbackSaved(feedback)) => {
                        this.reading_feedback.retain(|value| value.mode != feedback.mode);
                        this.reading_feedback.push(feedback);
                        this.notice = Some("反馈已保存".into());
                        this.request(Command::LoadStats { page: 0 }, cx);
                    }
                    Ok(Response::SentencesFlagged {article,body_hash}) => this.flag_saved(article,body_hash),
                    Ok(Response::Job(job)) => this.receive_job(job, cx),
                    Ok(Response::LearningSettingsSaved {
                        mut value,
                        revision,
                    }) => {
                        value.theme = this.settings.theme;
                        value.playback_rate = this.settings.playback_rate;
                        this.settings = value;
                        this.settings_revision = revision;
                        this.settings_ui.refresh_learning = true;
                        this.notice = Some("学习设置已保存".into());
                    }
                    Ok(Response::ProviderSaved {
                        config,
                        has_key,
                        source,
                    }) => {
                        let kind = config.kind;
                        this.key_source = source;
                        if kind == cijian_domain::provider::ProviderKind::Tts {
                            this.tts_config = Some(config);
                            this.has_tts_key = has_key;
                        } else {
                            this.provider_config = Some(config);
                            this.has_session_key = has_key;
                        }
                        this.settings_ui.refresh_provider = kind == this.provider_kind;
                        this.notice = Some("AI 配置已保存".into());
                    }
                    Ok(Response::Settings {
                        value,
                        revision,
                        provider,
                        has_session_key,
                        tts,
                        has_tts_key,
                        source,
                        voices,
                        system_voice,
                    }) => {
                        this.key_source = source;
                        this.settings = value;
                        this.settings_revision = revision;
                        this.provider_config = provider;
                        this.has_session_key = has_session_key;
                        this.tts_config = tts;
                        this.has_tts_key = has_tts_key;
                        this.system_voices = voices;
                        this.system_voice_index = this
                            .system_voice_index
                            .min(this.system_voices.len().saturating_sub(1));
                        if let Some(voice) = system_voice {
                            this.system_voice_index = this
                                .system_voices
                                .iter()
                                .position(|available| available.id == voice)
                                .unwrap_or(0);
                        }
                        this.settings_editor = None;
                        this.dark =
                            matches!(this.settings.theme, cijian_domain::settings::Theme::Dark);
                        if this.navigation.current() == &Route::Today {
                            this.request(Command::LoadReview, cx);
                        }
                    }
                    Ok(Response::Library {
                        page,
                        books,
                        entries,
                        latest_import,
                    }) => {
                        if let Some((source, report)) = latest_import {
                            this.library_ui.import_source = Some(source);
                            this.report = Some(report);
                        }
                        this.library = Some(page);
                        this.library_ui.books = books;
                        this.library_ui.entries = entries;
                    }
                    Ok(Response::LibraryCardEdited(_)) => {
                        this.library_ui.editor = None;
                        this.library_ui.discard = false;
                        this.notice = Some("词卡已保存".into());
                        this.request(
                            Command::LoadLibrary {
                                filter: this.library_filter(),
                            },
                            cx,
                        );
                    }
                    Ok(Response::DeleteBookPreview(preview)) => {
                        this.library_ui.deletion = Some(preview)
                    }
                    Ok(Response::BookDeleted(preview)) => {
                        this.library_ui.deletion = None;
                        if this.library_ui.target.as_ref() == Some(&preview.book_id) {
                            this.library_ui.target = None;
                        }
                        this.library_ui.page = 0;
                        this.notice = Some(format!(
                            "词书已删除；保留共享卡 {}，归档历史卡 {}",
                            preview.shared_cards, preview.archived_cards
                        ));
                        this.navigate(
                            Route::Library {
                                book: None,
                                search: String::new(),
                            },
                            cx,
                        );
                    }
                    Ok(Response::Selected(selected)) => {
                        if selected.is_some() {
                            this.selected = selected;
                        }
                    }
                    Ok(Response::Imported(report)) => {
                        this.library_ui.import_source = None;
                        this.library_ui.error_page = 0;
                        this.library_ui.import_open = false;
                        this.library_ui.report_open = true;
                        this.report = Some(report);
                        this.selected = None;
                        this.request(
                            Command::LoadLibrary {
                                filter: this.library_filter(),
                            },
                            cx,
                        );
                    }
                    Ok(Response::Review {
                        cards,
                        waiting,
                        intervals,
                        stats,
                        books,
                        inbox_pending,
                    }) => {
                        this.stats = Some(*stats);
                        this.books = books;
                        this.inbox_total = inbox_pending;
                        let empty_inbox = this.inbox.is_empty();
                        this.review = cards;
                        this.waiting = waiting;
                        this.intervals = intervals;
                        this.revealed = false;
                        this.gloss_pending = None;
                        this.gloss_failed = None;
                        this.review_started = Instant::now();
                        if this.navigation.current() == &Route::Today && empty_inbox && inbox_pending > 0 {
                            this.request(Command::LoadInbox { page: 0 }, cx);
                        }
                    }
                    Ok(Response::Rated(_)) => {
                        this.review_done = this.review_done.saturating_add(1);
                        this.request(Command::LoadReview, cx)
                    }
                    Ok(Response::CardExplained(card)) => {
                        if let Some(index) = this.review.iter().position(|value| value.id == card.id)
                        {
                            this.review[index] = card;
                        }
                        this.gloss_pending = None;
                        this.gloss_failed = None;
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn library_filter(&self) -> cijian_ports::library::CardFilter {
        let (book_id, search) = match self.navigation.current() {
            Route::Library { book, search } => (book.clone(), search.clone()),
            _ => (None, String::new()),
        };
        cijian_ports::library::CardFilter {
            book_id,
            search,
            page_size: 30,
            page: self.library_ui.page,
            state: self.library_ui.state,
            suspended: self.library_ui.suspended,
        }
    }
    fn navigate(&mut self, route: Route, cx: &mut Context<Self>) {
        self.navigate_to(route, false, cx);
    }
    fn navigate_to(&mut self, route: Route, back: bool, cx: &mut Context<Self>) {
        if !back
            && self.navigation.current() == &route
            && (self.loading || route == Route::Settings)
        {
            return;
        }
        let entering = self.navigation.current().section() != route.section();
        if route == Route::Review && entering {
            self.review_done = 0;
        }
        if route == Route::Articles && entering {
            self.articles_history = false;
            self.articles_page = 0;
            self.articles_date = None;
            self.articles_date_error = None;
            self.articles_date_input = None;
        }
        if self.navigation.current() != &route
            && self.guard_settings_exit(
                if back {
                    settings::PendingSettings::Back
                } else {
                    settings::PendingSettings::Navigate(route.clone())
                },
                cx,
            )
        {
            return;
        }
        if self.loading && self.library_ui.editor.is_some() {
            return;
        }
        if self.navigation.current() != &route
            && self
                .library_ui
                .editor
                .as_ref()
                .is_some_and(|editor| editor.dirty(cx))
        {
            self.library_ui.pending_close = false;
            self.library_ui.pending_route = if back { None } else { Some(route) };
            self.library_ui.pending_back = back;
            self.library_ui.discard = true;
            cx.notify();
            return;
        }
        if self.navigation.current() != &route {
            self.library_ui.editor = None;
            self.library_ui.deletion = None;
            self.library_ui.suspended_open = false;
            if self.navigation.current().section() != route.section() {
                self.notice = None;
                self.notice_dismissal = None;
            }
        }
        if let Route::Library { book, search } = &route {
            let scope = (book.clone(), search.clone());
            if self.library_ui.scope.as_ref() != Some(&scope) {
                self.library_ui.page = 0;
                self.library_ui.scope = Some(scope);
                self.library = None;
                self.library_ui.entries = None;
            }
            if book
                .as_ref()
                .and_then(|id| self.library_ui.books.iter().find(|book| &book.id == id))
                .is_some_and(|book| book.kind == BookKind::Baseline)
            {
                self.library_ui.state = None;
                self.library_ui.suspended = None;
            }
        }
        if matches!(self.navigation.current(), Route::Article { .. })
            && self.navigation.current() != &route
        {
            self.flush_positions(cx);
            self.playback.stop();
            if let Some(backend) = self.backend.clone() {
                cx.background_executor()
                    .spawn(async move {
                        let _ = backend.dispatch(Command::AudioStop).await;
                    })
                    .detach();
            }
        }
        if back {
            self.navigation.back();
        } else {
            self.navigation.navigate(route);
        }
        match self.navigation.current() {
            Route::Articles => {
                self.load_articles(0, cx);
                self.poll_generation(cx);
            }
            Route::Article { id, .. } => self.request(Command::OpenArticle { id: id.clone() }, cx),
            Route::Inbox => self.request(Command::LoadInbox { page: 0 }, cx),
            Route::Stats => self.load_stats(0, cx),
            Route::Settings => self.request(Command::LoadSettings, cx),
            Route::Today | Route::Review => self.request(Command::LoadReview, cx),
            Route::Library { .. } => self.request(
                Command::LoadLibrary {
                    filter: self.library_filter(),
                },
                cx,
            ),
        }
        cx.notify();
    }
    fn rate(&mut self, rating: Rating, cx: &mut Context<Self>) {
        if self.loading || !self.revealed {
            return;
        }
        if let Some(card) = self.review.first() {
            self.request(
                Command::Rate {
                    card: card.id.clone(),
                    revision: card.revision,
                    rating,
                    operation: format!("rating-{}-{}", card.id, card.revision),
                    duration_ms: self
                        .review_started
                        .elapsed()
                        .as_millis()
                        .min(u128::from(u32::MAX)) as u32,
                },
                cx,
            );
        }
    }
}
fn format_interval(ms: i64) -> String {
    // Round up like Anki's shortened labels: 5.5 minutes is shown as 6m.
    let minutes = (ms.max(0) as u64).div_ceil(60_000).max(1);
    if minutes < 60 {
        format!("{minutes}分钟")
    } else if minutes < 1440 {
        format!("{}小时", minutes / 60)
    } else {
        format!("{}天", minutes / 1440)
    }
}
impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.appearance_subscription.is_none() {
            self.appearance_subscription =
                Some(cx.observe_window_appearance(window, |this, window, cx| {
                    this.apply_theme(window, cx)
                }));
        }
        self.apply_theme(window, cx);
        let scope = gpui_kit_material::scope::Scope::new("window-theme")
            .scheme(theme::scheme(self.dark))
            .resolve(window, cx);
        let content = scope.build(window, cx, |window, cx| self.render_content(window, cx));
        scope.child(content)
    }
}

pub fn run(application: gpui_kit::Application, backend: Result<Arc<dyn CommandBus>, AppError>) {
    application
        .with_assets(Assets::new())
        .run(move |cx: &mut App| {
            gpui_kit::init(cx);
            gpui_kit_material::init(cx);
            design::install_fonts(cx);
            init_component_theme(theme::scheme(false), 1.0, cx);
            gpui_kit::open_window(
                WindowOptions {
                    window_min_size: Some(gpui_kit::size(gpui_kit::px(360.), gpui_kit::px(600.))),
                    ..title_bar_window_options(
                        "词间",
                        gpui_kit::px(design::TITLE_BAR_HEIGHT),
                        gpui_kit::size(gpui_kit::px(1440.), gpui_kit::px(900.)),
                        cx,
                    )
                },
                cx,
                move |window, cx| {
                    window.set_window_title("词间");
                    let view = cx.new(|_| {
                        let mut shell = Shell::new();
                        match backend {
                            Ok(backend) => shell.backend = Some(backend),
                            Err(error) => shell.error = Some(error.message),
                        }
                        shell
                    });
                    view.update(cx, |shell, cx| shell.request(Command::LoadSettings, cx));
                    view
                },
            )
            .expect("无法创建词间窗口");
            cx.activate(true);
        });
}
