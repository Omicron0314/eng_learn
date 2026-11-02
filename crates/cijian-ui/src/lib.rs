//! GPUI views issue application commands; all IO runs on the background executor.
#[cfg(test)]
extern crate gpui_kit as gpui;
mod audio;
mod settings;
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
use gpui_kit::{App, Context, Window, WindowOptions, div, prelude::*, px};
use gpui_kit_material::{
    color::ColorRole,
    components::{ButtonVariant, NavIcon, NavItem, NavigationBar, NavigationRail, material_button},
    icons::{Assets, Symbol},
    theme::{color, init_component_theme},
};
use std::{sync::Arc, time::Instant};

struct Shell {
    focus: Option<gpui_kit::FocusHandle>,
    navigation: Navigation,
    items: Vec<NavItem>,
    dark: bool,
    backend: Option<Arc<dyn CommandBus>>,
    loading: bool,
    error: Option<String>,
    request_id: u64,
    library: Option<CardPage>,
    selected: Option<SelectedFile>,
    word_list: bool,
    import_kind: BookKind,
    report: Option<ImportReport>,
    review: Vec<CardView>,
    waiting: bool,
    intervals: Option<[i64; 4]>,
    revealed: bool,
    review_started: Instant,
    settings: cijian_domain::settings::Settings,
    settings_revision: u64,
    legacy_selection: Option<String>,
    legacy_profiles: Vec<cijian_ports::legacy::LegacyProfile>,
    legacy_report: Option<cijian_ports::legacy::LegacyReport>,
    stored_audio: Vec<cijian_ports::audio::AudioAsset>,
    audio_poll_epoch: u64,
    appearance_subscription: Option<gpui_kit::Subscription>,
    system_voices: Vec<cijian_ports::audio::SystemVoice>,
    system_voice_index: usize,
    show_listening_text: bool,
    playback: cijian_domain::audio::Playback,
    provider_kind: cijian_domain::provider::ProviderKind,
    tts_config: Option<cijian_domain::provider::ProviderConfig>,
    has_tts_key: bool,
    provider_config: Option<cijian_domain::provider::ProviderConfig>,
    has_session_key: bool,
    settings_editor: Option<settings::Editor>,
    articles: Vec<cijian_ports::articles::Article>,
    article: Option<cijian_ports::articles::Article>,
    reading_session: Option<cijian_ports::reading::ReadingSession>,
    lookup: Option<(cijian_ports::reading::LookupContext, u64, String)>,
    generation_job: Option<cijian_ports::articles::GenerationJob>,
    inbox: Vec<cijian_ports::inbox::InboxItem>,
    inbox_total: u64,
    inbox_selected: std::collections::BTreeSet<String>,
    notice: Option<String>,
    stats: Option<cijian_ports::stats::StatsSnapshot>,
    books: Vec<cijian_ports::library::BookView>,
    heatmap_day: Option<usize>,
}
impl Shell {
    fn new() -> Self {
        let items = [
            ("today", "今日", "today"),
            ("review", "复习", "school"),
            ("articles", "文章", "article"),
            ("library", "词库", "menu_book"),
            ("inbox", "生词箱", "inbox"),
            ("stats", "统计", "bar_chart"),
        ]
        .into_iter()
        .map(|(value, label, icon)| {
            NavItem::new(
                value,
                label,
                NavIcon::symbol(Symbol::named(icon).expect("bundled Material symbol")),
            )
        })
        .collect();
        Self {
            focus: None,
            navigation: Navigation::default(),
            items,
            dark: false,
            backend: None,
            loading: false,
            error: None,
            request_id: 0,
            library: None,
            selected: None,
            word_list: false,
            import_kind: BookKind::Study,
            report: None,
            review: Vec::new(),
            waiting: false,
            intervals: None,
            revealed: false,
            review_started: Instant::now(),
            settings: Default::default(),
            settings_revision: 1,
            legacy_selection: None,
            legacy_profiles: Vec::new(),
            legacy_report: None,
            stored_audio: Vec::new(),
            audio_poll_epoch: 0,
            appearance_subscription: None,
            system_voices: Vec::new(),
            system_voice_index: 0,
            show_listening_text: false,
            playback: Default::default(),
            provider_kind: cijian_domain::provider::ProviderKind::Llm,
            tts_config: None,
            has_tts_key: false,
            provider_config: None,
            has_session_key: false,
            settings_editor: None,
            articles: Vec::new(),
            article: None,
            reading_session: None,
            lookup: None,
            generation_job: None,
            inbox: Vec::new(),
            inbox_total: 0,
            inbox_selected: Default::default(),
            notice: None,
            stats: None,
            books: Vec::new(),
            heatmap_day: None,
        }
    }
    fn request(&mut self, command: Command, cx: &mut Context<Self>) {
        let Some(backend) = self.backend.clone() else {
            return;
        };
        self.loading = true;
        self.error = None;
        self.request_id += 1;
        let request_id = self.request_id;
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
                match response {
                    Err(error) => this.error = Some(error.message),
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
                    Ok(Response::BackupSaved) => {
                        this.notice = Some("学习备份已保存，不包含密钥和音频缓存".into())
                    }
                    Ok(Response::RestoreOpened) => {
                        this.notice =
                            Some("恢复完成，新学习库已在另一个窗口打开；当前学习库保留".into())
                    }
                    Ok(Response::FileCancelled) => {}
                    Ok(Response::Stats(stats)) => this.stats = Some(*stats),
                    Ok(Response::Articles(articles)) => this.articles = articles,
                    Ok(Response::Article {
                        article,
                        session,
                        audio,
                    }) => {
                        this.stored_audio = audio;
                        this.navigation.navigate(Route::Article {
                            id: article.id.clone(),
                            listening: false,
                        });
                        this.article = Some(*article);
                        this.reading_session = Some(session);
                        this.lookup = None;
                    }
                    Ok(Response::Lookup {
                        context,
                        revision,
                        meaning,
                    }) => this.lookup = Some((context, revision, meaning)),
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
                    Ok(Response::Completed) => {
                        if let Some(session) = &mut this.reading_session {
                            session.completed_at = Some(0);
                        }
                        this.notice = Some("已记录本次阅读完成".into());
                    }
                    Ok(Response::FeedbackSaved) => this.notice = Some("反馈已保存".into()),
                    Ok(Response::Job(job)) => this.receive_job(job, cx),
                    Ok(Response::Settings {
                        value,
                        revision,
                        provider,
                        has_session_key,
                        tts,
                        has_tts_key,
                        voices,
                        system_voice,
                    }) => {
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
                        init_component_theme(theme::scheme(this.dark), 1.0, cx);
                        if this.navigation.current() == &Route::Today {
                            this.request(Command::LoadReview, cx);
                        }
                    }
                    Ok(Response::Library(page)) => this.library = Some(page),
                    Ok(Response::Selected(selected)) => {
                        if selected.is_some() {
                            this.selected = selected;
                        }
                    }
                    Ok(Response::Imported(report)) => {
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
                        this.review = cards;
                        this.waiting = waiting;
                        this.intervals = intervals;
                        this.revealed = false;
                        this.review_started = Instant::now();
                    }
                    Ok(Response::Rated(_)) => this.request(Command::LoadReview, cx),
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
            page_size: 100,
            ..Default::default()
        }
    }
    fn navigate(&mut self, route: Route, cx: &mut Context<Self>) {
        if matches!(self.navigation.current(), Route::Article { .. })
            && self.navigation.current() != &route
        {
            self.playback.stop();
            if let Some(backend) = self.backend.clone() {
                cx.background_executor()
                    .spawn(async move {
                        let _ = backend.dispatch(Command::AudioStop).await;
                    })
                    .detach();
            }
        }
        self.navigation.navigate(route);
        match self.navigation.current() {
            Route::Articles => self.request(Command::LoadArticles { page: 0 }, cx),
            Route::Article { id, .. } => self.request(Command::OpenArticle { id: id.clone() }, cx),
            Route::Inbox => self.request(Command::LoadInbox { page: 0 }, cx),
            Route::Stats => self.request(Command::LoadStats { page: 0 }, cx),
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
    fn body(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let mut body = div().flex().flex_col().gap_4();
        match self.navigation.current() {
            Route::Today => {
                body = body.child(self.today_metrics(cx));
                body = body.child(div().text_size(px(24.)).child("一点复习，一段故事。"));
                if !self.loading {
                    body = body.child(format!("当前可复习 {} 张词卡", self.review.len()));
                }
                body = body
                    .child(
                        material_button("start-review", ButtonVariant::Filled)
                            .child("开始复习")
                            .disabled(self.loading || self.review.is_empty())
                            .on_click(
                                cx.listener(|this, _, _, cx| this.navigate(Route::Review, cx)),
                            ),
                    )
                    .child(
                        material_button("open-library", ButtonVariant::Tonal)
                            .child("管理词库")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.navigate(
                                    Route::Library {
                                        book: None,
                                        search: String::new(),
                                    },
                                    cx,
                                )
                            })),
                    );
            }
            Route::Library { .. } => {
                body = body.child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            material_button(
                                "mode-cards",
                                if self.word_list {
                                    ButtonVariant::Tonal
                                } else {
                                    ButtonVariant::Filled
                                },
                            )
                            .child("词卡")
                            .disabled(self.loading)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.word_list = false;
                                this.import_kind = BookKind::Study;
                                this.selected = None;
                                cx.notify();
                            })),
                        )
                        .child(
                            material_button(
                                "mode-wordlist",
                                if self.word_list {
                                    ButtonVariant::Filled
                                } else {
                                    ButtonVariant::Tonal
                                },
                            )
                            .child("词表")
                            .disabled(self.loading)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.word_list = true;
                                this.selected = None;
                                cx.notify();
                            })),
                        ),
                );
                if self.word_list {
                    body = body.child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                material_button(
                                    "kind-baseline",
                                    if self.import_kind == BookKind::Baseline {
                                        ButtonVariant::Filled
                                    } else {
                                        ButtonVariant::Tonal
                                    },
                                )
                                .child("基线：作为已知词")
                                .disabled(self.loading)
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.import_kind = BookKind::Baseline;
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                material_button(
                                    "kind-study",
                                    if self.import_kind == BookKind::Study {
                                        ButtonVariant::Filled
                                    } else {
                                        ButtonVariant::Tonal
                                    },
                                )
                                .child("学习：加入复习")
                                .disabled(self.loading)
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.import_kind = BookKind::Study;
                                        cx.notify();
                                    },
                                )),
                            ),
                    );
                }
                body = body.child(
                    material_button("choose-file", ButtonVariant::Tonal)
                        .child(if self.word_list {
                            "选择词表 CSV / TXT"
                        } else {
                            "选择词卡 CSV / APKG"
                        })
                        .disabled(self.loading)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.request(
                                Command::ChooseImport {
                                    word_list: this.word_list,
                                },
                                cx,
                            )
                        })),
                );
                if let Some(selected) = &self.selected {
                    body = body.child(format!("词书：{}", selected.name));
                }
                body = body.child(
                    material_button("import-file", ButtonVariant::Filled)
                        .child("开始导入")
                        .disabled(self.loading || self.selected.is_none())
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(file) = this.selected.clone() {
                                this.request(
                                    Command::ImportSelected {
                                        handle: file.handle,
                                        name: file.name,
                                        kind: this.import_kind,
                                        word_list: this.word_list,
                                    },
                                    cx,
                                );
                            }
                        })),
                );
                if let Some(report) = &self.report {
                    body = body.child(format!(
                        "新增 {} 个词、{} 张词卡；重复 {}，跳过 {}",
                        report.words_added, report.cards_added, report.duplicates, report.skipped
                    ));
                }
                if let Some(page) = &self.library {
                    body = body.child(format!("共 {} 张词卡", page.total));
                    for card in &page.cards {
                        body = body.child(
                            div()
                                .rounded(px(16.))
                                .p_4()
                                .bg(color(ColorRole::SURFACE_CONTAINER_LOWEST))
                                .child(card.front.clone())
                                .child(
                                    div()
                                        .text_color(color(ColorRole::ON_SURFACE_VARIANT))
                                        .child(card.meaning.clone()),
                                ),
                        );
                    }
                }
            }
            Route::Review => {
                if let Some(card) = self.review.first() {
                    body = body.child(div().text_size(px(36.)).child(card.front.clone()));
                    if self.revealed {
                        body = body
                            .child(if card.meaning.is_empty() {
                                "暂无释义，可继续评分".to_owned()
                            } else {
                                card.meaning.clone()
                            })
                            .child(card.example.clone());
                        let mut ratings = div().flex().flex_wrap().gap_2();
                        for (index, (label, rating)) in [
                            ("重来", Rating::Again),
                            ("困难", Rating::Hard),
                            ("记得", Rating::Good),
                            ("轻松", Rating::Easy),
                        ]
                        .into_iter()
                        .enumerate()
                        {
                            let interval = self
                                .intervals
                                .map(|values| format_interval(values[index]))
                                .unwrap_or_default();
                            ratings = ratings.child(
                                material_button(
                                    ("rating", index),
                                    if rating == Rating::Good {
                                        ButtonVariant::Filled
                                    } else {
                                        ButtonVariant::Tonal
                                    },
                                )
                                .child(format!("{label} · {interval}"))
                                .disabled(self.loading)
                                .on_click(cx.listener(move |this, _, _, cx| this.rate(rating, cx))),
                            );
                        }
                        body = body.child(ratings);
                    } else {
                        body = body.child(
                            material_button("reveal", ButtonVariant::Filled)
                                .child("显示答案")
                                .disabled(self.loading)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.revealed = true;
                                    cx.notify();
                                })),
                        );
                    }
                } else if !self.loading {
                    body = body.child(if self.waiting {
                        "学习步骤尚未到期，请稍后再来"
                    } else {
                        "当前没有待复习词卡，可以导入词书或稍后再来"
                    });
                }
            }
            Route::Settings => return self.settings_body(cx),
            Route::Articles => return self.articles_body(cx),
            Route::Article { .. } => return self.reader_body(cx),
            Route::Inbox => return self.inbox_body(cx),
            Route::Stats => return self.stats_body(cx),
        }
        body.into_any_element()
    }
}
fn format_interval(ms: i64) -> String {
    let minutes = (ms / 60000).max(1);
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
        if self.focus.is_none() {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            self.focus = Some(focus);
        }
        if self.navigation.current() == &Route::Settings && self.settings_editor.is_none() {
            self.settings_editor = Some(settings::Editor::new(
                &self.settings,
                if self.provider_kind == cijian_domain::provider::ProviderKind::Tts {
                    self.tts_config.as_ref()
                } else {
                    self.provider_config.as_ref()
                },
                window,
                cx,
            ));
        }
        let compact = window.viewport_size().width < px(600.);
        let selected = self.navigation.current().section();
        let entity = cx.entity().downgrade();
        let on_change = move |value: &gpui_kit::SharedString, _: &mut Window, cx: &mut App| {
            if let Some(route) = Route::section_route(value.as_ref()) {
                let _ = entity.update(cx, |this, cx| this.navigate(route, cx));
            }
        };
        let navigation = if compact {
            NavigationBar::new("main-nav", "主导航")
                .items(self.items.clone())
                .selected(selected)
                .on_change(on_change)
                .into_any_element()
        } else {
            NavigationRail::new("main-nav", "主导航")
                .items(self.items.clone())
                .selected(selected)
                .on_change(on_change)
                .into_any_element()
        };
        let mut header = div().flex().items_center().justify_between().gap_4().child(
            div()
                .text_size(px(32.))
                .child(self.navigation.current().label()),
        );
        if compact {
            header = header.child(
                material_button("settings", ButtonVariant::Text)
                    .child("设置")
                    .on_click(cx.listener(|this, _, _, cx| this.navigate(Route::Settings, cx))),
            );
        }
        let mut content = div()
            .id("page")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .p_6()
            .flex()
            .flex_col()
            .gap_6()
            .child(header);
        if self.navigation.can_back() {
            content = content.child(
                material_button("back", ButtonVariant::Text)
                    .child("返回")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.navigation.back();
                        let route = this.navigation.current().clone();
                        this.navigate(route, cx);
                    })),
            )
        }
        if let Some(notice) = &self.notice {
            content = content.child(notice.clone());
        }
        if self.loading {
            content = content.child("正在处理，请稍候…");
        }
        if let Some(error) = &self.error {
            content = content.child(
                div()
                    .p_4()
                    .rounded(px(16.))
                    .bg(color(ColorRole::ERROR_CONTAINER))
                    .text_color(color(ColorRole::ON_ERROR_CONTAINER))
                    .child(error.clone()),
            );
        }
        content = content.child(
            div()
                .rounded(px(28.))
                .p_6()
                .bg(color(ColorRole::SURFACE_CONTAINER_LOW))
                .child(self.body(cx)),
        );
        let background = div()
            .track_focus(self.focus.as_ref().expect("initialized focus"))
            .tab_stop(false)
            .on_key_down(cx.listener(|this, event: &gpui_kit::KeyDownEvent, _, cx| {
                if this.navigation.current() == &Route::Review && !this.loading {
                    match event.keystroke.key.as_str() {
                        "space" if !this.revealed => {
                            this.revealed = true;
                            cx.notify();
                            cx.stop_propagation();
                        }
                        "1" => this.rate(Rating::Again, cx),
                        "2" => this.rate(Rating::Hard, cx),
                        "3" => this.rate(Rating::Good, cx),
                        "4" => this.rate(Rating::Easy, cx),
                        _ => {}
                    }
                }
            }))
            .w(window.viewport_size().width)
            .h(window.viewport_size().height)
            .flex()
            .bg(color(ColorRole::SURFACE))
            .text_color(color(ColorRole::ON_SURFACE));
        if compact {
            background
                .flex_col()
                .child(content)
                .child(navigation)
                .into_any_element()
        } else {
            background
                .child(
                    div()
                        .w(px(96.))
                        .h_full()
                        .flex()
                        .flex_col()
                        .justify_between()
                        .child(
                            div()
                                .flex_1()
                                .flex_basis(px(0.))
                                .min_h_0()
                                .overflow_hidden()
                                .child(navigation),
                        )
                        .child(
                            div().flex_shrink_0().pb_2().child(
                                material_button("settings", ButtonVariant::Text)
                                    .child("设置")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.navigate(Route::Settings, cx)
                                    })),
                            ),
                        ),
                )
                .child(content)
                .into_any_element()
        }
    }
}
pub fn run(application: gpui_kit::Application, backend: Result<Arc<dyn CommandBus>, AppError>) {
    application
        .with_assets(Assets::new())
        .run(move |cx: &mut App| {
            gpui_kit::init(cx);
            gpui_kit_material::init(cx);
            init_component_theme(theme::scheme(false), 1.0, cx);
            gpui_kit::open_window(WindowOptions::default(), cx, move |window, cx| {
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
            })
            .expect("无法创建词间窗口");
            cx.activate(true);
        });
}
