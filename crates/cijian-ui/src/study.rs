use super::*;
use crate::design::Icon;
use chrono::Datelike;
use cijian_domain::{
    reading::{Difficulty, FeedbackMode},
    settings::{Genre, Tier},
};
use cijian_ports::{
    articles::{GenerationJob, JobStatus},
    inbox::{InboxAction, MarkAction},
};
use gpui_kit_material::components::{MaterialBottomSheet, TextField};
impl Shell {
    pub(super) fn operation(&self, purpose: &str) -> String {
        format!(
            "{purpose}-{}-{}",
            self.request_id,
            self.review_started.elapsed().as_nanos()
        )
    }
    pub(super) fn receive_job(&mut self, job: GenerationJob, cx: &mut Context<Self>) {
        let active = matches!(job.status, JobStatus::Queued | JobStatus::Running);
        let result = job.result_ref.clone();
        self.generation_job = Some(job);
        if result.is_some() {
            self.articles_history = false;
            self.articles_page = 0;
            self.articles_date = None;
            self.articles_date_error = None;
        }
        if let Some(article) = result
            && self.navigation.current() == &Route::Articles
        {
            self.request(Command::OpenArticle { id: article }, cx);
        }
        if active {
            self.poll_generation(cx);
        }
    }
    /// Keep observing an active job while the article list stays visible. A
    /// transient UI request must reschedule instead of silently stopping the
    /// poll, and returning to Articles resumes an already running job.
    pub(super) fn poll_generation(&mut self, cx: &mut Context<Self>) {
        if self.generation_poll_pending || self.navigation.current() != &Route::Articles {
            return;
        }
        let Some(job) = &self.generation_job else {
            return;
        };
        if !matches!(job.status, JobStatus::Queued | JobStatus::Running) {
            return;
        }
        let id = job.id.clone();
        self.generation_poll_pending = true;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(750))
                .await;
            let _ = this.update(cx, |this, cx| {
                this.generation_poll_pending = false;
                let active = this.generation_job.as_ref().is_some_and(|job| {
                    job.id == id && matches!(job.status, JobStatus::Queued | JobStatus::Running)
                });
                if !active || this.navigation.current() != &Route::Articles {
                    return;
                }
                if this.loading {
                    this.poll_generation(cx);
                } else {
                    this.request(Command::ObserveJob { job: id }, cx);
                }
            });
        })
        .detach();
    }
    /// Word audio: the accent chip switches 英/美, the transcription itself replays.
    /// Both live inside the review card, so their clicks must not also reveal it.
    fn review_pronunciation(
        &mut self,
        card: &CardView,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let accent = self.review_accent;
        let ipa = card.pronunciation.ipa(accent).to_owned();
        let playable = card.pronunciation.playable(accent);
        if ipa.is_empty() && playable.is_none() {
            return div().into_any_element();
        }
        let speaker = if self.compact { 16. } else { 18. };
        let mut chip = div()
            .id("review-accent")
            .role(gpui_kit::Role::Button)
            .aria_label(format!("切换到{}式发音", accent.toggle().label()))
            .cursor_pointer()
            .h(px(30.))
            .px(px(10.))
            .rounded(px(15.))
            .bg(design::c(ColorRole::SECONDARY_CONTAINER))
            .text_color(design::c(ColorRole::ON_SECONDARY_CONTAINER))
            .flex()
            .items_center()
            .gap(px(6.))
            .when(playable.is_none(), |chip| chip.opacity(0.55))
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                let accent = this.review_accent.toggle();
                this.review_accent = accent;
                this.request(Command::SetPronunciationAccent { accent }, cx);
                this.play_word(accent, cx);
            }))
            .child(design::text(13., 18., 650.).child(accent.label()))
            .child(design::icon_role(
                Icon::Speaker,
                speaker,
                ColorRole::ON_SECONDARY_CONTAINER,
            ));
        if playable.is_none() {
            chip = chip.child(design::muted_text(12., 16.).child("暂无"));
        }
        let chip = chip.test_support();
        let transcription = div()
            .id("review-ipa")
            .role(gpui_kit::Role::Button)
            .aria_label("播放单词发音")
            .flex()
            .items_center()
            .child(
                design::mono(
                    if self.compact { 18. } else { 22. },
                    if self.compact { 26. } else { 30. },
                    400.,
                )
                .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                .child(ipa),
            );
        let transcription = if playable.is_some() {
            transcription
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| {
                    cx.stop_propagation();
                    this.play_word(accent, cx);
                }))
                .test_support()
        } else {
            transcription.test_support()
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(10.))
            .child(chip)
            .child(transcription)
            .into_any_element()
    }
    /// Reads the word out as soon as a card comes up, once per card.
    /// Silent when the card has no recording; it never blocks reveal or rating.
    pub(super) fn announce_review_card(&mut self, cx: &mut Context<Self>) {
        if !self.review_auto_play {
            return;
        }
        if !matches!(self.navigation.current(), Route::Review) {
            return;
        }
        let Some(card) = self.review.first() else {
            return;
        };
        if self.review_announced.as_deref() == Some(card.id.as_str()) {
            return;
        }
        let announced = card.id.clone();
        if card.pronunciation.playable(self.review_accent).is_none() {
            // Remember the card anyway: a missing recording must not retry on every frame.
            self.review_announced = Some(announced);
            return;
        }
        self.review_announced = Some(announced);
        let accent = self.review_accent;
        self.play_word(accent, cx);
    }
    /// Plays the current card, falling back to the accent that actually has audio.
    fn play_word(&mut self, accent: cijian_domain::audio::Accent, cx: &mut Context<Self>) {
        let Some((lexeme, recorded)) = self.review.first().and_then(|card| {
            card.pronunciation
                .playable(accent)
                .map(|recorded| (card.lexeme_id.clone(), recorded))
        }) else {
            return;
        };
        self.request(
            Command::PlayWord {
                lexeme,
                accent: recorded,
            },
            cx,
        );
    }
    /// Reveal the current card and, when its meaning is missing, request one
    /// bounded AI gloss. The request never blocks reveal or rating.
    pub(super) fn reveal_card(&mut self, cx: &mut Context<Self>) {
        if self.loading || self.revealed {
            return;
        }
        self.revealed = true;
        let Some(card) = self.review.first().cloned() else {
            cx.notify();
            return;
        };
        let already_failed = self
            .gloss_failed
            .as_ref()
            .is_some_and(|(id, _)| id == &card.id);
        if card.meaning.trim().is_empty()
            && !already_failed
            && self.gloss_pending.as_deref() != Some(&card.id)
            && self.provider_config.is_some()
        {
            self.explain_card(card, cx);
        }
        cx.notify();
    }
    fn explain_card(&mut self, card: CardView, cx: &mut Context<Self>) {
        let Some(backend) = self.backend.clone() else {
            return;
        };
        let card_id = card.id.clone();
        let revision = card.revision;
        let operation = self.operation("review-gloss");
        self.gloss_pending = Some(card_id.clone());
        self.gloss_failed = None;
        cx.spawn(async move |this, cx| {
            let result = backend
                .dispatch(Command::ExplainCard {
                    card: card_id.clone(),
                    revision,
                    operation,
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.gloss_pending.as_deref() != Some(&card_id) {
                    return;
                }
                this.gloss_pending = None;
                match result {
                    Ok(Response::CardExplained(card)) => {
                        if let Some(index) =
                            this.review.iter().position(|value| value.id == card.id)
                        {
                            this.review[index] = card;
                        }
                        this.gloss_failed = None;
                    }
                    Ok(_) => {}
                    Err(error) => this.gloss_failed = Some((card_id, error.message)),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn retry_card_gloss(&mut self, card: CardView, cx: &mut Context<Self>) {
        if self.gloss_pending.is_some() || self.loading {
            return;
        }
        self.gloss_failed = None;
        self.explain_card(card, cx);
        cx.notify();
    }
    pub(super) fn play_remote_audio(&mut self, article: String, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.last_audio_system = false;
        self.request(
            Command::EnsureAudio {
                article,
                operation: self.operation("audio"),
            },
            cx,
        );
    }
    pub(super) fn play_system_audio(&mut self, article: String, cx: &mut Context<Self>) {
        let Some(voice) = self.system_voices.get(self.system_voice_index) else {
            return;
        };
        let voice = voice.id.clone();
        self.last_audio_system = true;
        self.request(Command::EnsureSystemAudio { article, voice }, cx);
    }
    fn toggle_audio(&mut self, article: String, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        let playing = matches!(
            self.playback.state,
            cijian_domain::audio::PlaybackState::Playing
        );
        let ended = matches!(
            self.playback.state,
            cijian_domain::audio::PlaybackState::Ended
                | cijian_domain::audio::PlaybackState::Failed
        );
        if self.playback.session.is_none() || ended {
            if self.last_audio_system {
                self.play_system_audio(article, cx);
            } else {
                self.play_remote_audio(article, cx);
            }
        } else if playing {
            self.request(Command::AudioPause, cx);
        } else {
            self.request(Command::AudioResume, cx);
        }
    }
    fn toggle_remote_audio(&mut self, article: String, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        let playing = matches!(
            self.playback.state,
            cijian_domain::audio::PlaybackState::Playing
        );
        let ended = matches!(
            self.playback.state,
            cijian_domain::audio::PlaybackState::Ended
                | cijian_domain::audio::PlaybackState::Failed
        );
        if self.playback.session.is_none() || ended {
            self.play_remote_audio(article, cx);
        } else if playing {
            self.request(Command::AudioPause, cx);
        } else {
            self.request(Command::AudioResume, cx);
        }
    }
    fn article_range(&self) -> (Option<i64>, Option<i64>) {
        let zone = self
            .settings
            .timezone
            .parse::<chrono_tz::Tz>()
            .unwrap_or(chrono_tz::Tz::UTC);
        let now = chrono::Utc::now().timestamp_millis();
        let today = cijian_domain::time::study_day(now, zone, self.settings.day_start_hour).ok();
        if let Some(date) = self.articles_date {
            let noon = date.and_hms_opt(12, 0, 0).expect("valid noon");
            let Some(local) = chrono::TimeZone::from_local_datetime(&zone, &noon).single() else {
                return (None, None);
            };
            return cijian_domain::time::study_day(
                local.timestamp_millis(),
                zone,
                self.settings.day_start_hour,
            )
            .map(|day| (Some(day.start_ms), Some(day.end_ms)))
            .unwrap_or((None, None));
        }
        if self.articles_history {
            (None, today.map(|day| day.start_ms))
        } else {
            today
                .map(|day| (Some(day.start_ms), Some(day.end_ms)))
                .unwrap_or((None, None))
        }
    }
    pub(super) fn load_articles(&mut self, page: u32, cx: &mut Context<Self>) {
        self.articles_page = page;
        let (from, until) = self.article_range();
        self.request(Command::LoadArticles { from, until, page }, cx);
    }
    pub(super) fn prepare_articles(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.articles_history || self.articles_date_input.is_some() {
            return;
        }
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("YYYY-MM-DD")
                .default_value(
                    self.articles_date
                        .map(|date| date.format("%Y-%m-%d").to_string())
                        .unwrap_or_default(),
                )
        });
        cx.subscribe(&input, |this, _, event, cx| {
            if matches!(event, gpui_kit::base::input::InputEvent::PressEnter { .. }) {
                this.apply_article_date(cx);
            }
        })
        .detach();
        self.articles_date_input = Some(input);
    }
    fn set_articles_history(&mut self, history: bool, cx: &mut Context<Self>) {
        if self.articles_history == history {
            return;
        }
        self.articles_history = history;
        self.articles_date = None;
        self.articles_date_error = None;
        self.articles_date_input = None;
        self.load_articles(0, cx);
    }
    fn apply_article_date(&mut self, cx: &mut Context<Self>) {
        let Some(input) = &self.articles_date_input else {
            return;
        };
        let value = input.read(cx).value().trim().to_owned();
        match chrono::NaiveDate::parse_from_str(&value, "%Y-%m-%d") {
            Ok(date) => {
                self.articles_date = Some(date);
                self.articles_date_error = None;
                self.load_articles(0, cx);
            }
            Err(_) => {
                self.articles_date_error = Some("日期格式须为 YYYY-MM-DD".into());
                cx.notify();
            }
        }
    }
    fn clear_article_date(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.articles_date = None;
        self.articles_date_error = None;
        if let Some(input) = &self.articles_date_input {
            input.update(cx, |input, cx| input.set_value("", window, cx));
        }
        self.load_articles(0, cx);
    }
    fn article_tab(
        &self,
        history: bool,
        label: &'static str,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let selected = self.articles_history == history;
        div()
            .id(if history {
                "articles-history"
            } else {
                "articles-today"
            })
            .test_support()
            .role(gpui_kit::Role::Button)
            .aria_label(label)
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| this.set_articles_history(history, cx)))
            .child(design::chip(
                selected,
                label,
                selected.then_some(Icon::Check),
            ))
            .into_any_element()
    }
    pub(super) fn articles_body(&mut self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let active = self
            .generation_job
            .as_ref()
            .is_some_and(|j| matches!(j.status, JobStatus::Queued | JobStatus::Running));
        let tier_label = |tier: Tier| match tier {
            Tier::Easy => "轻松",
            Tier::Medium => "适中",
            Tier::Hard => "挑战",
        };
        let genre_label = |genre: Genre| match genre {
            Genre::Auto => "自动轮换",
            Genre::Story => "短故事",
            Genre::Dialogue => "对话",
            Genre::Monologue => "独白",
        };
        let coverage = |article: &cijian_ports::articles::Article| {
            article
                .analysis
                .coverage
                .map(|value| format!("{:.1}%", value * 100.))
                .unwrap_or_else(|| "暂无".into())
        };
        let today_count = self.today_article_count;
        let review_count = self
            .stats
            .as_ref()
            .map(|stats| stats.today.reviews)
            .unwrap_or(0);
        let completion = self
            .stats
            .as_ref()
            .and_then(|stats| stats.reading_completion)
            .map(|value| format!("{:.0}%", value * 100.))
            .unwrap_or_else(|| "—".into());

        let mut body = design::pane()
            .id("articles-page")
            .track_scroll(&self.page_scroll)
            .flex_1()
            .min_w_0()
            .h_full()
            .p(px(28.))
            .px(px(if self.compact { 20. } else { 32. }))
            .gap(px(24.))
            .overflow_y_scroll();
        body = body.child(
            div()
                .flex()
                .items_end()
                .gap(px(16.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .gap(px(8.))
                        .child(
                            design::text(14., 20., 600.)
                                .text_color(design::c(ColorRole::PRIMARY))
                                .child("READ / LISTEN / DISCOVER"),
                        )
                        .child(design::title(40., 48., 750.).child("文章"))
                        .child(
                            design::muted_text(16., 22.)
                                .child("把复习过的词，放回故事里。先听一遍，再慢慢读。"),
                        ),
                )
                .child(
                    design::tag(
                        ColorRole::PRIMARY_CONTAINER,
                        ColorRole::ON_PRIMARY_CONTAINER,
                    )
                    .h(px(32.))
                    .px(px(14.))
                    .child(format!("今日 {today_count} 篇")),
                ),
        );
        body = body.child(
            design::shaped(
                [24., 24., 24., 24.],
                design::c(ColorRole::PRIMARY_CONTAINER),
            )
            .p(px(18.))
            .px(px(22.))
            .flex()
            .items_center()
            .gap(px(14.))
            .text_color(design::c(ColorRole::ON_PRIMARY_CONTAINER))
            .child(design::icon_role(
                Icon::Spark,
                24.,
                ColorRole::ON_PRIMARY_CONTAINER,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .gap(px(4.))
                    .child(design::text(16., 22., 650.).child("今天的复习，成为下一篇的词汇"))
                    .child(design::text(13., 18., 400.).child(if review_count > 0 {
                        format!("已完成 {review_count} 次复习，可以开始生成。难度和体裁由你选择。")
                    } else {
                        "还没有复习记录。先复习几张卡片，生成时才有词可用。".to_owned()
                    })),
            )
            .child(
                design::tag(ColorRole::SUCCESS_BG, ColorRole::ON_SUCCESS)
                    .h(px(32.))
                    .child(format!("阅读完成率 {completion}")),
            )
            .child(
                div()
                    .id("articles-review")
                    .role(gpui_kit::Role::Button)
                    .aria_label("继续复习")
                    .cursor_pointer()
                    .h(px(40.))
                    .px(px(20.))
                    .rounded(px(20.))
                    .bg(design::c(ColorRole::SURFACE_CONTAINER_LOWEST))
                    .text_color(design::c(ColorRole::ON_PRIMARY_CONTAINER))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_size(px(14.))
                    .font_weight(gpui_kit::FontWeight(650.))
                    .on_click(cx.listener(|this, _, _, cx| this.navigate(Route::Review, cx)))
                    .child("继续复习")
                    .child(design::icon_role(Icon::Chev, 18., ColorRole::PRIMARY)),
            ),
        );

        let mut grid = div()
            .id("article-grid")
            .flex()
            .items_stretch()
            .gap(px(24.))
            .flex_1()
            .min_h_0()
            .when(self.compact, |grid| grid.flex_col().overflow_y_scroll());
        let mut generate = design::card()
            .w(px(380.))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .p(px(24.))
            .rounded(px(28.))
            .gap(px(22.))
            .when(self.compact, |card| card.w_full());
        generate = generate.child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(
                    design::shaped(
                        [16., 24., 16., 24.],
                        design::c(ColorRole::TERTIARY_CONTAINER),
                    )
                    .w(px(48.))
                    .h(px(48.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(design::c(ColorRole::ON_TERTIARY_CONTAINER))
                    .child(design::icon_role(
                        Icon::Spark,
                        24.,
                        ColorRole::ON_TERTIARY_CONTAINER,
                    )),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(design::title(20., 28., 700.).child("写一篇新文章"))
                        .child(
                            design::muted_text(13., 18.)
                                .child("复用今天复习过的词，写一段新的英文"),
                        ),
                ),
        );
        let mut tiers = div().flex().gap(px(2.));
        for (index, tier) in [Tier::Easy, Tier::Medium, Tier::Hard]
            .into_iter()
            .enumerate()
        {
            let selected = self.settings.tier == tier;
            let radii = match index {
                0 => [20., 6., 6., 20.],
                2 => [6., 20., 20., 6.],
                _ => [6., 6., 6., 6.],
            };
            let mut button = design::shaped(
                radii,
                design::c(if selected {
                    ColorRole::SECONDARY
                } else {
                    ColorRole::SURFACE_CONTAINER_HIGH
                }),
            )
            .id(gpui_kit::SharedString::from(format!(
                "generation-tier-{}",
                index
            )))
            .role(gpui_kit::Role::Button)
            .aria_label(tier_label(tier))
            .cursor_pointer()
            .flex_1()
            .h(px(40.))
            .text_color(design::c(if selected {
                ColorRole::SURFACE_CONTAINER_LOWEST
            } else {
                ColorRole::ON_SURFACE_VARIANT
            }))
            .text_size(px(14.))
            .font_weight(gpui_kit::FontWeight(if selected { 700. } else { 600. }))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.settings.tier = tier;
                cx.notify();
            }));
            button = button.child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(6.))
                    .when(selected, |row| {
                        row.child(design::icon_role(
                            Icon::Check,
                            18.,
                            ColorRole::SURFACE_CONTAINER_LOWEST,
                        ))
                    })
                    .child(tier_label(tier)),
            );
            tiers = tiers.child(button);
        }
        generate = generate.child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(design::text(14., 20., 650.).child("难度 · 已知词覆盖率"))
                .child(tiers)
                .child(design::muted_text(12., 16.).child(format!(
                    "{}：已知词覆盖率 {}",
                    tier_label(self.settings.tier),
                    match self.settings.tier {
                        Tier::Easy => "≥98%",
                        Tier::Medium => "95–98%",
                        Tier::Hard => "90–95%",
                    }
                ))),
        );
        let mut genres = div().flex().flex_wrap().gap(px(8.));
        for genre in [Genre::Auto, Genre::Story, Genre::Dialogue, Genre::Monologue] {
            let selected = self.settings.genre == genre;
            genres = genres.child(
                div()
                    .id(gpui_kit::SharedString::from(format!(
                        "generation-genre-{}",
                        match genre {
                            Genre::Auto => 0,
                            Genre::Story => 1,
                            Genre::Dialogue => 2,
                            Genre::Monologue => 3,
                        }
                    )))
                    .role(gpui_kit::Role::Button)
                    .aria_label(genre_label(genre))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.settings.genre = genre;
                        cx.notify();
                    }))
                    .child(design::chip(
                        selected,
                        genre_label(genre),
                        selected.then_some(Icon::Check),
                    )),
            );
        }
        generate = generate.child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(design::text(14., 20., 650.).child("体裁"))
                .child(genres),
        );
        generate = generate.child(div().flex_1());
        generate = generate.child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .flex_wrap()
                .child(
                    div()
                        .id("generate-article")
                        .role(gpui_kit::Role::Button)
                        .aria_label("生成文章")
                        .cursor_pointer()
                        .h(px(56.))
                        .px(px(28.))
                        .rounded(px(28.))
                        .bg(design::c(ColorRole::PRIMARY))
                        .text_color(design::c(ColorRole::ON_PRIMARY))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .text_size(px(16.))
                        .font_weight(gpui_kit::FontWeight(700.))
                        .when(self.loading || active, |button| button.opacity(0.38))
                        .on_click(cx.listener(|this, _, _, cx| {
                            if this.loading
                                || this.generation_job.as_ref().is_some_and(|job| {
                                    matches!(job.status, JobStatus::Queued | JobStatus::Running)
                                })
                            {
                                return;
                            }
                            this.request(
                                Command::GenerateArticle {
                                    tier: this.settings.tier,
                                    genre: this.settings.genre,
                                    operation: this.operation("generate"),
                                },
                                cx,
                            );
                        }))
                        .child(design::icon_role(Icon::Spark, 18., ColorRole::ON_PRIMARY))
                        .child("生成文章"),
                )
                .child(
                    design::muted_text(12., 16.)
                        .flex_1()
                        .child(if review_count > 0 {
                            "复用今天复习过的词，难度和体裁由你选择。"
                        } else {
                            "先复习几张卡片，再生成你的文章。"
                        }),
                ),
        );
        grid = grid.child(generate);

        let mut list = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap(px(12.))
            .when(self.compact, |list| list.flex_none());
        let mut tabs = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .flex_wrap()
            .child(self.article_tab(false, "今天", cx))
            .child(self.article_tab(true, "历史", cx));
        if self.articles_history {
            if let Some(input) = &self.articles_date_input {
                tabs = tabs.child(
                    div()
                        .w(px(190.))
                        .child(TextField::new("articles-date", input)),
                );
            }
            tabs = tabs.child(
                div()
                    .id("articles-date-apply")
                    .test_support()
                    .role(gpui_kit::Role::Button)
                    .aria_label("筛选日期")
                    .cursor_pointer()
                    .h(px(32.))
                    .px(px(12.))
                    .rounded(px(16.))
                    .bg(design::c(ColorRole::SECONDARY_CONTAINER))
                    .text_color(design::c(ColorRole::ON_SECONDARY_CONTAINER))
                    .flex()
                    .items_center()
                    .text_size(px(13.))
                    .on_click(cx.listener(|this, _, _, cx| this.apply_article_date(cx)))
                    .child("筛选"),
            );
            if self.articles_date.is_some() {
                tabs = tabs.child(
                    div()
                        .id("articles-date-clear")
                        .role(gpui_kit::Role::Button)
                        .aria_label("清除日期筛选")
                        .cursor_pointer()
                        .h(px(32.))
                        .px(px(12.))
                        .rounded(px(16.))
                        .text_color(design::c(ColorRole::PRIMARY))
                        .flex()
                        .items_center()
                        .text_size(px(13.))
                        .on_click(
                            cx.listener(|this, _, window, cx| this.clear_article_date(window, cx)),
                        )
                        .child("清除"),
                );
            }
            if let Some(error) = &self.articles_date_error {
                tabs = tabs.child(
                    design::muted_text(12., 16.)
                        .text_color(design::c(ColorRole::ERROR))
                        .child(error.clone()),
                );
            }
        }
        list = list.child(tabs);
        let title = match self.articles_date {
            Some(date) => format!("{} 的文章", date.format("%Y-%m-%d")),
            None if self.articles_history => "历史文章".to_owned(),
            None => "今天的文章".to_owned(),
        };
        list = list.child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(design::title(24., 32., 700.).child(title))
                .child(
                    design::tag(
                        ColorRole::SURFACE_CONTAINER_HIGH,
                        ColorRole::ON_SURFACE_VARIANT,
                    )
                    .child(self.articles.len().to_string()),
                )
                .child(div().flex_1())
                .when_some(
                    (!self.articles_history)
                        .then(|| self.articles.first().map(|article| article.id.clone()))
                        .flatten(),
                    |row, id| {
                        row.child(
                            div()
                                .id("continue-reading")
                                .role(gpui_kit::Role::Button)
                                .aria_label("继续阅读")
                                .cursor_pointer()
                                .h(px(40.))
                                .px(px(12.))
                                .rounded(px(20.))
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .text_color(design::c(ColorRole::PRIMARY))
                                .text_size(px(14.))
                                .font_weight(gpui_kit::FontWeight(650.))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.request(Command::OpenArticle { id: id.clone() }, cx)
                                }))
                                .child("继续阅读")
                                .child(design::icon_role(Icon::Chev, 18., ColorRole::PRIMARY)),
                        )
                    },
                ),
        );
        if self.articles.is_empty() {
            list = list.child(
                design::empty_state(if self.articles_date.is_some() {
                    "所选日期没有文章"
                } else if self.articles_history {
                    "还没有更早的文章"
                } else {
                    "下一篇会出现在这里 · 每篇都会尽量复用新的目标词"
                })
                .h(px(120.)),
            );
        }
        for (index, article) in self.articles.iter().enumerate() {
            let id = article.id.clone();
            let color = match index % 2 {
                0 => (
                    ColorRole::PRIMARY_CONTAINER,
                    ColorRole::ON_PRIMARY_CONTAINER,
                ),
                _ => (
                    ColorRole::SECONDARY_CONTAINER,
                    ColorRole::ON_SECONDARY_CONTAINER,
                ),
            };
            let icon = match article.genre {
                Genre::Dialogue => Icon::Chat,
                _ => Icon::Book,
            };
            list = list.child(
                div()
                    .id(gpui_kit::SharedString::from(format!("article-{id}")))
                    .role(gpui_kit::Role::Button)
                    .aria_label(article.title.clone())
                    .cursor_pointer()
                    .rounded(px(24.))
                    .bg(design::c(ColorRole::SURFACE_CONTAINER_LOWEST))
                    .shadow(design::shadow_card())
                    .p(px(16.))
                    .px(px(18.))
                    .flex()
                    .items_center()
                    .gap(px(16.))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.request(Command::OpenArticle { id: id.clone() }, cx)
                    }))
                    .child(
                        design::shaped(
                            if article.genre == Genre::Dialogue {
                                [16., 16., 16., 16.]
                            } else {
                                [28., 28., 28., 16.]
                            },
                            design::c(color.0),
                        )
                        .w(px(56.))
                        .h(px(56.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(design::c(color.1))
                        .child(design::icon_color(
                            icon,
                            24.,
                            design::c(color.1),
                        )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(
                                design::muted_text(12., 16.)
                                    .font_weight(gpui_kit::FontWeight(650.))
                                    .text_ellipsis()
                                    .child(format!(
                                        "{} · {} · 覆盖率 {} · {} 词",
                                        genre_label(article.genre),
                                        tier_label(article.tier),
                                        coverage(article),
                                        article.analysis.token_count
                                    )),
                            )
                            .child(
                                design::text(18., 24., 700.)
                                    .text_ellipsis()
                                    .child(article.title.clone()),
                            ),
                    )
                    .when(
                        article
                            .analysis
                            .coverage
                            .is_none_or(|value| value < article.tier.minimum_coverage()),
                        |row| {
                            row.child(
                                design::tag(ColorRole::WARNING_BG, ColorRole::ON_WARNING)
                                    .child("覆盖率偏低"),
                            )
                        },
                    )
                    .child(
                        design::tag(
                            if article.read_at.is_some() {
                                ColorRole::SUCCESS_BG
                            } else {
                                ColorRole::PRIMARY_CONTAINER
                            },
                            if article.read_at.is_some() {
                                ColorRole::ON_SUCCESS
                            } else {
                                ColorRole::ON_PRIMARY_CONTAINER
                            },
                        )
                        .child(if article.read_at.is_some() {
                            "已读"
                        } else {
                            "待阅读"
                        }),
                    )
                    .child(
                        div()
                            .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                            .child(design::icon_role(
                                Icon::Chev,
                                18.,
                                ColorRole::ON_SURFACE_VARIANT,
                            )),
                    ),
            );
        }
        if !self.articles.is_empty() {
            list = list.child(div().flex_1());
        }
        if let Some(job) = &self.generation_job {
            let status = match job.status {
                JobStatus::Queued | JobStatus::Running => "正在生成…",
                JobStatus::Succeeded => "生成已完成",
                JobStatus::Failed => "生成失败，请检查设置后重新提交",
                JobStatus::Cancelled => "已取消；已发送的请求可能仍计费",
                JobStatus::Interrupted => "任务中断，未自动重试",
            };
            list = list.child(design::muted_text(13., 18.).child(status));
            if active {
                let job_id = job.id.clone();
                list = list.child(
                    div()
                        .id("cancel-generation")
                        .role(gpui_kit::Role::Button)
                        .aria_label("取消生成")
                        .cursor_pointer()
                        .h(px(40.))
                        .px(px(16.))
                        .rounded(px(20.))
                        .bg(design::c(ColorRole::SECONDARY_CONTAINER))
                        .text_color(design::c(ColorRole::ON_SECONDARY_CONTAINER))
                        .flex()
                        .items_center()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.request(
                                Command::CancelGeneration {
                                    job: job_id.clone(),
                                },
                                cx,
                            )
                        }))
                        .child("取消生成"),
                );
            }
        }
        let mut pager = div()
            .flex()
            .items_center()
            .justify_end()
            .gap(px(8.))
            .w_full();
        pager = pager.child(
            div()
                .id("articles-prev")
                .test_support()
                .role(gpui_kit::Role::Button)
                .aria_label("上一页")
                .cursor_pointer()
                .h(px(32.))
                .px(px(12.))
                .rounded(px(16.))
                .bg(design::c(ColorRole::SURFACE_CONTAINER_HIGH))
                .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                .flex()
                .items_center()
                .text_size(px(13.))
                .when(self.articles_page == 0, |button| button.opacity(0.38))
                .on_click(cx.listener(|this, _, _, cx| {
                    if this.articles_page > 0 {
                        let page = this.articles_page - 1;
                        this.load_articles(page, cx);
                    }
                }))
                .child("上一页"),
        );
        pager = pager
            .child(design::muted_text(12., 16.).child(format!("第 {} 页", self.articles_page + 1)));
        pager = pager.child(
            div()
                .id("articles-next")
                .test_support()
                .role(gpui_kit::Role::Button)
                .aria_label("下一页")
                .cursor_pointer()
                .h(px(32.))
                .px(px(12.))
                .rounded(px(16.))
                .bg(design::c(ColorRole::SURFACE_CONTAINER_HIGH))
                .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                .flex()
                .items_center()
                .text_size(px(13.))
                .when(self.articles.len() < 30, |button| button.opacity(0.38))
                .on_click(cx.listener(|this, _, _, cx| {
                    if this.articles.len() >= 30 {
                        let page = this.articles_page + 1;
                        this.load_articles(page, cx);
                    }
                }))
                .child("下一页"),
        );
        list = list.child(pager);
        grid = grid.child(list);
        body = body.child(grid);
        body.into_any_element()
    }

    fn reader_header(&self, listening: bool, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        if self.compact {
            let mut seg = div().flex().gap(px(2.)).w_full();
            for (value, icon, label) in [(true, Icon::Headphones, "听"), (false, Icon::Eye, "读")]
            {
                let selected = listening == value;
                let radii = if value {
                    [20., 6., 6., 20.]
                } else {
                    [6., 20., 20., 6.]
                };
                let cell = design::shaped(
                    radii,
                    design::c(if selected {
                        ColorRole::SECONDARY
                    } else {
                        ColorRole::SURFACE_CONTAINER_HIGH
                    }),
                )
                .id(gpui_kit::SharedString::from(if value {
                    "reading-listening-mode"
                } else {
                    "reading-mode"
                }))
                .role(gpui_kit::Role::Button)
                .aria_label(if value { "先听" } else { "阅读" })
                .cursor_pointer()
                .h(px(36.))
                .flex_1()
                .flex_basis(px(0.))
                .min_w_0()
                .flex()
                .items_center()
                .justify_center()
                .gap(px(4.))
                .text_color(design::c(if selected {
                    ColorRole::SURFACE_CONTAINER_LOWEST
                } else {
                    ColorRole::ON_SURFACE_VARIANT
                }))
                .text_size(px(13.))
                .font_weight(gpui_kit::FontWeight(if selected { 700. } else { 600. }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Route::Article { id, .. } = this.navigation.current().clone() {
                        this.navigation.navigate(Route::Article {
                            id,
                            listening: value,
                        });
                        this.show_listening_text = false;
                        cx.notify();
                    }
                }))
                .child(design::icon_role(
                    icon,
                    16.,
                    if selected {
                        ColorRole::SURFACE_CONTAINER_LOWEST
                    } else {
                        ColorRole::ON_SURFACE_VARIANT
                    },
                ))
                .child(label);
                seg = seg.child(cell);
            }
            return div()
                .flex()
                .items_center()
                .gap(px(8.))
                .h(px(56.))
                .w_full()
                .child(
                    div()
                        .id("back")
                        .test_support()
                        .role(gpui_kit::Role::Button)
                        .aria_label("返回文章")
                        .cursor_pointer()
                        .w(px(40.))
                        .h(px(40.))
                        .rounded(px(20.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                        .on_click(cx.listener(|this, _, _, cx| this.go_back(cx)))
                        .child(design::icon_role(
                            Icon::Back,
                            24.,
                            ColorRole::ON_SURFACE_VARIANT,
                        )),
                )
                .child(div().flex_1())
                .child(div().w(px(180.)).child(seg))
                .child(div().flex_1())
                .child(
                    div()
                        .id("flag-sentences")
                        .test_support()
                        .role(gpui_kit::Role::Button)
                        .aria_label("标记不自然")
                        .cursor_pointer()
                        .w(px(40.))
                        .h(px(40.))
                        .rounded(px(20.))
                        .border_1()
                        .border_color(design::c(ColorRole::OUTLINE_VARIANT))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                        .on_click(cx.listener(|this, _, _, cx| {
                            if !this.loading {
                                this.reader_flags.open = true;
                                this.error = None;
                                cx.notify();
                            }
                        }))
                        .child(design::icon_role(
                            Icon::Flag,
                            18.,
                            ColorRole::ON_SURFACE_VARIANT,
                        )),
                )
                .into_any_element();
        }
        let mut seg = div().flex().gap(px(2.)).w_full();
        for (value, icon, label) in [(true, Icon::Headphones, "先听"), (false, Icon::Eye, "阅读")]
        {
            let selected = listening == value;
            let radii = if value {
                [20., 6., 6., 20.]
            } else {
                [6., 20., 20., 6.]
            };
            let mut cell = design::shaped(
                radii,
                design::c(if selected {
                    ColorRole::SECONDARY
                } else {
                    ColorRole::SURFACE_CONTAINER_HIGH
                }),
            )
            .id(gpui_kit::SharedString::from(if value {
                "reading-listening-mode"
            } else {
                "reading-mode"
            }))
            .role(gpui_kit::Role::Button)
            .aria_label(label)
            .cursor_pointer()
            .h(px(40.))
            .px(px(12.))
            .flex_1()
            .flex_basis(px(0.))
            .min_w_0()
            .text_color(design::c(if selected {
                ColorRole::SURFACE_CONTAINER_LOWEST
            } else {
                ColorRole::ON_SURFACE_VARIANT
            }))
            .text_size(px(14.))
            .font_weight(gpui_kit::FontWeight(if selected { 700. } else { 600. }))
            .on_click(cx.listener(move |this, _, _, cx| {
                if let Route::Article { id, .. } = this.navigation.current().clone() {
                    this.navigation.navigate(Route::Article {
                        id,
                        listening: value,
                    });
                    this.show_listening_text = false;
                    cx.notify();
                }
            }));
            cell = cell.child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(6.))
                    .child(design::icon_role(
                        icon,
                        18.,
                        if selected {
                            ColorRole::SURFACE_CONTAINER_LOWEST
                        } else {
                            ColorRole::ON_SURFACE_VARIANT
                        },
                    ))
                    .child(label),
            );
            seg = seg.child(cell);
        }
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(56.))
            .w_full()
            .child(
                div()
                    .id("back")
                    .test_support()
                    .role(gpui_kit::Role::Button)
                    .aria_label("返回文章")
                    .cursor_pointer()
                    .h(px(40.))
                    .px(px(14.))
                    .pl(px(8.))
                    .rounded(px(20.))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                    .text_size(px(14.))
                    .on_click(cx.listener(|this, _, _, cx| this.go_back(cx)))
                    .child(design::icon_role(
                        Icon::Back,
                        18.,
                        ColorRole::ON_SURFACE_VARIANT,
                    ))
                    .child("文章"),
            )
            .child(div().flex_1())
            .child(div().w(px(240.)).child(seg))
            .child(div().flex_1())
            .child(
                div()
                    .id("flag-sentences")
                    .test_support()
                    .role(gpui_kit::Role::Button)
                    .aria_label("标记不自然")
                    .cursor_pointer()
                    .h(px(40.))
                    .px(px(16.))
                    .rounded(px(20.))
                    .border_1()
                    .border_color(design::c(ColorRole::OUTLINE_VARIANT))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                    .text_size(px(14.))
                    .font_weight(gpui_kit::FontWeight(650.))
                    .when(self.loading, |button| button.opacity(0.38))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.loading {
                            return;
                        }
                        this.reader_flags.open = true;
                        this.error = None;
                        cx.notify();
                    }))
                    .child(design::icon_role(
                        Icon::Flag,
                        18.,
                        ColorRole::ON_SURFACE_VARIANT,
                    ))
                    .child("标记不自然"),
            )
            .into_any_element()
    }

    fn reader_meta(&self, article: &cijian_ports::articles::Article) -> gpui_kit::AnyElement {
        let genre = match article.genre {
            Genre::Auto => "自动",
            Genre::Story => "短故事",
            Genre::Dialogue => "对话",
            Genre::Monologue => "独白",
        };
        let tier = match article.tier {
            Tier::Easy => "轻松",
            Tier::Medium => "适中",
            Tier::Hard => "挑战",
        };
        let coverage = article
            .analysis
            .coverage
            .map(|value| format!("覆盖率 {:.1}%", value * 100.))
            .unwrap_or_else(|| "覆盖率 —".into());
        let below = article
            .analysis
            .coverage
            .is_none_or(|value| value < article.tier.minimum_coverage());
        let date = chrono::DateTime::from_timestamp_millis(article.created_at)
            .map(|value| {
                let date = value.date_naive();
                format!("{}月{}日", date.month(), date.day())
            })
            .unwrap_or_else(|| "—".into());
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                design::tag(
                    ColorRole::PRIMARY_CONTAINER,
                    ColorRole::ON_PRIMARY_CONTAINER,
                )
                .child(genre),
            )
            .child(
                design::tag(
                    ColorRole::SECONDARY_CONTAINER,
                    ColorRole::ON_SECONDARY_CONTAINER,
                )
                .child(format!(
                    "{tier} · {}",
                    match article.tier {
                        Tier::Easy => "≥98%",
                        Tier::Medium => "95–98%",
                        Tier::Hard => "90–95%",
                    }
                )),
            )
            .child(
                design::tag(
                    if below {
                        ColorRole::WARNING_BG
                    } else {
                        ColorRole::SUCCESS_BG
                    },
                    if below {
                        ColorRole::ON_WARNING
                    } else {
                        ColorRole::ON_SUCCESS
                    },
                )
                .child(coverage),
            )
            .child(
                design::muted_text(13., 18.)
                    .child(format!("· {} 词 · {}", article.analysis.token_count, date)),
            )
            .into_any_element()
    }

    fn reader_feedback(&self, listening: bool, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let mode = if listening {
            FeedbackMode::Listening
        } else {
            FeedbackMode::Reading
        };
        let saved = self
            .reading_feedback
            .iter()
            .find(|feedback| feedback.mode == mode)
            .map(|feedback| feedback.value);
        let completed = self
            .reading_session
            .as_ref()
            .is_some_and(|session| session.completed_at.is_some());
        let values = [
            (Difficulty::TooHard, "太难"),
            (Difficulty::JustRight, "刚好"),
            (Difficulty::TooEasy, "太简单"),
        ];
        let mut seg = div().flex().gap(px(2.)).flex_1();
        for (index, (value, label)) in values.into_iter().enumerate() {
            let selected = saved == Some(value);
            let radii = match index {
                0 => [20., 6., 6., 20.],
                2 => [6., 20., 20., 6.],
                _ => [6., 6., 6., 6.],
            };
            let mut cell = design::shaped(
                radii,
                design::c(if selected {
                    ColorRole::SECONDARY
                } else {
                    ColorRole::SURFACE_CONTAINER_LOWEST
                }),
            )
            .id(gpui_kit::SharedString::from(match value {
                Difficulty::TooHard => "feedback-hard",
                Difficulty::JustRight => "feedback-right",
                Difficulty::TooEasy => "feedback-easy",
            }))
            .role(gpui_kit::Role::Button)
            .aria_label(label)
            .cursor_pointer()
            .h(px(40.))
            .flex_1()
            .text_color(design::c(if selected {
                ColorRole::SURFACE_CONTAINER_LOWEST
            } else {
                ColorRole::ON_SURFACE_VARIANT
            }))
            .text_size(px(14.))
            .font_weight(gpui_kit::FontWeight(600.))
            .when(self.loading || saved == Some(value), |cell| {
                cell.opacity(0.8)
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                if this.loading || saved == Some(value) {
                    return;
                }
                if let Some(article) = &this.article {
                    this.request(
                        Command::Feedback {
                            article: article.id.clone(),
                            body_hash: article.body_hash.clone(),
                            value,
                            mode: if matches!(
                                this.navigation.current(),
                                Route::Article {
                                    listening: true,
                                    ..
                                }
                            ) {
                                FeedbackMode::Listening
                            } else {
                                FeedbackMode::Reading
                            },
                            operation: this.operation("feedback"),
                        },
                        cx,
                    );
                }
            }));
            cell = cell.child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(6.))
                    .when(selected, |row| {
                        row.child(design::icon_role(
                            Icon::Check,
                            16.,
                            ColorRole::SURFACE_CONTAINER_LOWEST,
                        ))
                    })
                    .child(label),
            );
            seg = seg.child(cell);
        }
        let mut row = div().flex().items_center().gap(px(12.)).w_full();
        if !listening {
            row = row.child(
                div()
                    .id("complete-reading")
                    .test_support()
                    .role(gpui_kit::Role::Button)
                    .aria_label("读完了")
                    .cursor_pointer()
                    .h(px(40.))
                    .px(px(16.))
                    .rounded(px(20.))
                    .bg(design::c(ColorRole::SECONDARY_CONTAINER))
                    .text_color(design::c(ColorRole::ON_SECONDARY_CONTAINER))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_size(px(14.))
                    .font_weight(gpui_kit::FontWeight(650.))
                    .when(self.loading || completed, |button| button.opacity(0.8))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.loading
                            || this
                                .reading_session
                                .as_ref()
                                .is_some_and(|s| s.completed_at.is_some())
                        {
                            return;
                        }
                        if let (Some(article), Some(session)) =
                            (&this.article, &this.reading_session)
                        {
                            this.request(
                                Command::CompleteReading {
                                    article: article.id.clone(),
                                    body_hash: article.body_hash.clone(),
                                    session: session.id.clone(),
                                },
                                cx,
                            );
                        }
                    }))
                    .child(design::icon_role(Icon::Check, 16., ColorRole::ON_PRIMARY))
                    .child(if completed { "已读完" } else { "读完了" }),
            );
        }
        row = row
            .child(design::text(14., 20., 650.).child(if listening {
                "听完后，感觉如何？"
            } else {
                "读完后，感觉如何？"
            }))
            .child(seg);
        row.into_any_element()
    }

    fn audio_toolbar(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let playing = matches!(
            self.playback.state,
            cijian_domain::audio::PlaybackState::Playing
        );
        let position = self.playback.position.max(0.);
        let duration = self.playback.duration.unwrap_or(0.);
        let fraction = if duration > 0. {
            (position / duration).clamp(0., 1.) as f32
        } else {
            0.
        };
        let clock = |seconds: f64| {
            let seconds = seconds.max(0.) as u64;
            format!("{}:{:02}", seconds / 60, seconds % 60)
        };
        div()
            .absolute()
            .bottom(px(20.))
            .left(px(0.))
            .right(px(0.))
            .flex()
            .justify_center()
            .child(
                design::shaped(
                    [36., 36., 36., 36.],
                    design::c(ColorRole::SURFACE_CONTAINER_HIGH),
                )
                .w(px(640.))
                .max_w_full()
                .h(px(72.))
                .shadow(design::shadow_float())
                .flex()
                .items_center()
                .gap(px(10.))
                .p(px(12.))
                .child(
                    div()
                        .id("audio-play")
                        .role(gpui_kit::Role::Button)
                        .aria_label(if playing { "暂停" } else { "播放" })
                        .cursor_pointer()
                        .w(px(52.))
                        .h(px(52.))
                        .rounded(px(18.))
                        .bg(design::c(ColorRole::PRIMARY))
                        .text_color(design::c(ColorRole::ON_PRIMARY))
                        .flex()
                        .items_center()
                        .justify_center()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(article) = &this.article {
                                let article = article.id.clone();
                                this.toggle_audio(article, cx);
                            }
                        }))
                        .child(design::icon_role(
                            if playing { Icon::Pause } else { Icon::Play },
                            22.,
                            ColorRole::ON_PRIMARY,
                        )),
                )
                .child(
                    div()
                        .id("audio-back-five")
                        .role(gpui_kit::Role::Button)
                        .aria_label("后退 5 秒")
                        .cursor_pointer()
                        .w(px(40.))
                        .h(px(40.))
                        .rounded(px(20.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.request(
                                Command::AudioSeek {
                                    position: (this.playback.position - 5.).max(0.),
                                },
                                cx,
                            )
                        }))
                        .child(design::icon_role(
                            Icon::Replay,
                            22.,
                            ColorRole::ON_SURFACE_VARIANT,
                        )),
                )
                .child(
                    design::mono(12., 16., 400.)
                        .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                        .child(clock(position)),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .flex_1()
                        .child(
                            design::shaped([4., 2., 2., 4.], design::c(ColorRole::PRIMARY))
                                .h(px(8.))
                                .w(gpui_kit::relative(fraction))
                                .when(fraction <= 0.001, |bar| bar.w(px(8.))),
                        )
                        .child(
                            design::shaped(
                                [2., 4., 4., 2.],
                                design::c(ColorRole::SECONDARY_CONTAINER),
                            )
                            .h(px(8.))
                            .flex_1(),
                        ),
                )
                .child(
                    design::mono(12., 16., 400.)
                        .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                        .child(clock(self.playback.duration.unwrap_or(0.))),
                )
                .child(
                    div()
                        .id("audio-speed")
                        .role(gpui_kit::Role::Button)
                        .aria_label("播放速度")
                        .cursor_pointer()
                        .h(px(36.))
                        .px(px(10.))
                        .rounded(px(12.))
                        .bg(design::c(ColorRole::SECONDARY_CONTAINER))
                        .text_color(design::c(ColorRole::ON_SECONDARY_CONTAINER))
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .text_size(px(13.))
                        .font_weight(gpui_kit::FontWeight(650.))
                        .on_click(cx.listener(|this, _, _, cx| {
                            let rates = cijian_domain::settings::PLAYBACK_RATES;
                            let index = rates
                                .iter()
                                .position(|rate| *rate == this.playback.rate)
                                .unwrap_or(1);
                            this.request(
                                Command::SetPlaybackRate {
                                    rate: rates[(index + 1) % rates.len()],
                                },
                                cx,
                            );
                        }))
                        .child(format!("{}×", self.playback.rate))
                        .child(design::icon_role(
                            Icon::Down,
                            16.,
                            ColorRole::ON_SECONDARY_CONTAINER,
                        )),
                )
                .child(self.article_accent_selector(cx)),
            )
            .into_any_element()
    }

    fn listening_panel(
        &self,
        article: &cijian_ports::articles::Article,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let article_id = article.id.clone();
        let cached = self.audio_ready || self.playback.session.is_some();
        let position = self.playback.position;
        let duration = self.playback.duration.unwrap_or(0.);
        let mut wave = div().flex().items_center().gap(px(2.)).h(px(56.)).flex_1();
        for index in 0..56 {
            let height = 8. + ((index * 37 + 13) % 40) as f32;
            wave = wave.child(
                div()
                    .w(px(5.))
                    .h(px(height))
                    .rounded(px(2.5))
                    .bg(design::c(
                        if (index as f64 / 56.)
                            < if duration > 0. {
                                position / duration
                            } else {
                                0.
                            }
                        {
                            ColorRole::PRIMARY
                        } else {
                            ColorRole::ON_PRIMARY_CONTAINER
                        },
                    ))
                    .opacity(
                        if (index as f64 / 56.)
                            < if duration > 0. {
                                position / duration
                            } else {
                                0.
                            }
                        {
                            1.
                        } else {
                            0.28
                        },
                    ),
            );
        }
        let mut card = design::shaped(
            [36., 36., 36., 36.],
            design::c(ColorRole::PRIMARY_CONTAINER),
        )
        .p(px(26.))
        .px(px(30.))
        .gap(px(18.))
        .flex()
        .flex_col()
        .text_color(design::c(ColorRole::ON_PRIMARY_CONTAINER))
        .child(self.article_accent_selector(cx))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(design::icon_role(
                    Icon::Headphones,
                    24.,
                    ColorRole::ON_PRIMARY_CONTAINER,
                ))
                .child(design::text(22., 28., 700.).child("先听，再看"))
                .child(div().flex_1())
                .child(
                    design::tag(
                        ColorRole::SURFACE_CONTAINER_LOWEST,
                        ColorRole::ON_PRIMARY_CONTAINER,
                    )
                    .child(if self.loading {
                        "正在准备全文音频…"
                    } else if cached {
                        "整篇音频 · 已缓存"
                    } else {
                        "首次播放将生成全文并缓存"
                    }),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(18.))
                .child(
                    div()
                        .id("audio-play")
                        .role(gpui_kit::Role::Button)
                        .aria_label("播放整篇音频")
                        .cursor_pointer()
                        .w(px(88.))
                        .h(px(88.))
                        .rounded(px(30.))
                        .bg(design::c(ColorRole::PRIMARY))
                        .text_color(design::c(ColorRole::ON_PRIMARY))
                        .flex()
                        .items_center()
                        .justify_center()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_remote_audio(article_id.clone(), cx);
                        }))
                        .child(design::icon_role(
                            if matches!(
                                self.playback.state,
                                cijian_domain::audio::PlaybackState::Playing
                            ) {
                                Icon::Pause
                            } else {
                                Icon::Play
                            },
                            32.,
                            ColorRole::ON_PRIMARY,
                        )),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .gap(px(8.))
                        .child(wave)
                        .child(design::mono(12., 16., 400.).child(format!(
                            "{} / {}",
                            {
                                let seconds = position.max(0.) as u64;
                                format!("{}:{:02}", seconds / 60, seconds % 60)
                            },
                            {
                                let seconds = duration.max(0.) as u64;
                                format!("{}:{:02}", seconds / 60, seconds % 60)
                            }
                        ))),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .id("audio-back-five")
                        .role(gpui_kit::Role::Button)
                        .aria_label("后退 5 秒")
                        .cursor_pointer()
                        .h(px(34.))
                        .px(px(14.))
                        .rounded(px(17.))
                        .bg(design::c(ColorRole::SURFACE_CONTAINER_LOWEST))
                        .text_color(design::c(ColorRole::ON_PRIMARY_CONTAINER))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .text_size(px(13.))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.request(
                                Command::AudioSeek {
                                    position: (this.playback.position - 5.).max(0.),
                                },
                                cx,
                            )
                        }))
                        .child(design::icon_role(
                            Icon::Replay,
                            16.,
                            ColorRole::ON_PRIMARY_CONTAINER,
                        ))
                        .child("后退 5 秒"),
                )
                .child(div().flex_1())
                .child(design::text(13., 18., 650.).child("速度"))
                .children(
                    cijian_domain::settings::PLAYBACK_RATES
                        .into_iter()
                        .enumerate()
                        .map(|(index, rate)| {
                            let selected = (self.playback.rate - rate).abs() < 0.001;
                            div()
                                .id(gpui_kit::SharedString::from(format!("audio-rate-{index}")))
                                .role(gpui_kit::Role::Button)
                                .aria_label(format!("{rate} 倍速"))
                                .cursor_pointer()
                                .h(px(32.))
                                .px(px(12.))
                                .rounded(px(8.))
                                .flex()
                                .items_center()
                                .text_size(px(13.))
                                .font_weight(gpui_kit::FontWeight(if selected {
                                    700.
                                } else {
                                    500.
                                }))
                                .when(selected, |cell| {
                                    cell.bg(design::c(ColorRole::PRIMARY))
                                        .text_color(design::c(ColorRole::ON_PRIMARY))
                                })
                                .when(!selected, |cell| {
                                    cell.text_color(design::c(ColorRole::ON_PRIMARY_CONTAINER))
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.request(Command::SetPlaybackRate { rate }, cx);
                                }))
                                .child(format!("{rate}"))
                        }),
                ),
        );
        if position > 0. && duration > 0. {
            card = card.child(
                design::muted_text(12., 16.)
                    .text_color(design::alpha(
                        design::c(ColorRole::ON_PRIMARY_CONTAINER),
                        0.8,
                    ))
                    .child(format!("已播放 {:.0}%", position / duration * 100.)),
            );
        }
        card.into_any_element()
    }

    fn article_accent_selector(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        div()
            .flex()
            .gap(px(8.))
            .children(cijian_domain::audio::Accent::ALL.into_iter().map(|accent| {
                let selected = self.review_accent == accent;
                div()
                    .id(gpui_kit::SharedString::from(format!(
                        "article-accent-{}",
                        accent.key()
                    )))
                    .role(gpui_kit::Role::Button)
                    .aria_label(format!("{}式发音", accent.label()))
                    .cursor_pointer()
                    .px(px(16.))
                    .py(px(8.))
                    .rounded(px(16.))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    // The listening card is PRIMARY_CONTAINER, so a selected chip
                    // painted in the same role looked like the unselected one:
                    // always use a filled primary chip with inverted text.
                    .bg(design::c(if selected {
                        ColorRole::PRIMARY
                    } else {
                        ColorRole::SURFACE_CONTAINER_LOWEST
                    }))
                    .text_color(design::c(if selected {
                        ColorRole::ON_PRIMARY
                    } else {
                        ColorRole::ON_SURFACE_VARIANT
                    }))
                    .when(!selected, |cell| {
                        cell.border_1()
                            .border_color(design::c(ColorRole::OUTLINE_VARIANT))
                    })
                    .when(selected, |cell| {
                        cell.child(design::icon_role(Icon::Check, 14., ColorRole::ON_PRIMARY))
                    })
                    .when(self.loading, |cell| cell.opacity(0.38))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.loading || this.review_accent == accent {
                            return;
                        }
                        this.review_accent = accent;
                        this.audio_ready = false;
                        this.playback = Default::default();
                        this.request(Command::SetPronunciationAccent { accent }, cx);
                    }))
                    .child(format!("{}式发音", accent.label()))
            }))
            .into_any_element()
    }

    /// Article facts beside the text. There is no audio-source picker any more:
    /// whole-article speech is always the local model, chosen above the text.
    fn reader_side_facts(&self, article: &cijian_ports::articles::Article) -> gpui_kit::AnyElement {
        let coverage = article.analysis.coverage.unwrap_or(0.);
        let mut article_card = design::pane()
            .w(px(384.))
            .flex_shrink_0()
            .h_full()
            .p(px(22.))
            .gap(px(14.));
        article_card = article_card
            .child(design::muted_text(14., 20.).child("这篇文章"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(16.))
                    .child(design::ring(coverage as f32, 88., ColorRole::PRIMARY))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(design::text(15., 20., 650.).child("已知词覆盖率"))
                            .child(design::muted_text(13., 18.).child("目标区间 95–98%"))
                            .child(
                                design::muted_text(13., 18.)
                                    .child(format!("{} 词", article.analysis.token_count)),
                            ),
                    ),
            )
            .child(design::divider());
        for (label, value) in [
            (
                "体裁",
                match article.genre {
                    Genre::Auto => "自动",
                    Genre::Story => "短故事",
                    Genre::Dialogue => "对话",
                    Genre::Monologue => "独白",
                },
            ),
            (
                "难度档位",
                match article.tier {
                    Tier::Easy => "轻松",
                    Tier::Medium => "适中",
                    Tier::Hard => "挑战",
                },
            ),
        ] {
            article_card = article_card.child(
                div()
                    .flex()
                    .gap(px(8.))
                    .text_size(px(13.))
                    .line_height(px(18.))
                    .child(design::muted_text(13., 18.).flex_1().child(label))
                    .child(design::text(13., 18., 650.).child(value)),
            );
        }
        for (label, mode, fallback) in [
            ("听力反馈", FeedbackMode::Listening, "听完后记录"),
            ("阅读反馈", FeedbackMode::Reading, "读完后记录"),
        ] {
            article_card = article_card.child(
                div()
                    .flex()
                    .gap(px(8.))
                    .text_size(px(13.))
                    .line_height(px(18.))
                    .child(design::muted_text(13., 18.).flex_1().child(label))
                    .child(
                        self.reading_feedback
                            .iter()
                            .find(|feedback| feedback.mode == mode)
                            .map(|feedback| match feedback.value {
                                Difficulty::TooHard => "太难",
                                Difficulty::JustRight => "刚好",
                                Difficulty::TooEasy => "太简单",
                            })
                            .unwrap_or(fallback),
                    ),
            );
        }
        article_card.into_any_element()
    }

    fn reader_lookup_panel(&self, cx: &mut Context<Self>) -> Option<gpui_kit::AnyElement> {
        let (context, revision, meaning) = self.lookup.as_ref()?;
        let selection = &context.selection;
        let sentence = selection.sentence.clone();
        let mut highlights = Vec::new();
        if let Some(start) = sentence.find(&selection.surface) {
            highlights.push((
                start..start + selection.surface.len(),
                gpui_kit::HighlightStyle {
                    background_color: Some(design::c(ColorRole::PRIMARY_CONTAINER)),
                    color: Some(design::c(ColorRole::ON_PRIMARY_CONTAINER)),
                    ..Default::default()
                },
            ));
        }
        let mut pane = design::pane()
            .id("word-sheet")
            .w(px(384.))
            .flex_shrink_0()
            .h_full()
            .p(px(20.))
            .px(px(22.))
            .pb(px(22.))
            .gap(px(16.));
        pane = pane
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(
                        design::muted_text(14., 20.)
                            .font_weight(gpui_kit::FontWeight(650.))
                            .flex_1()
                            .child("语境释义"),
                    )
                    .child(
                        div()
                            .id("dismiss-lookup")
                            .role(gpui_kit::Role::Button)
                            .aria_label("关闭词语面板")
                            .cursor_pointer()
                            .w(px(40.))
                            .h(px(40.))
                            .rounded(px(20.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .on_click(
                                cx.listener(|this, _, window, cx| this.dismiss_lookup(window, cx)),
                            )
                            .child(design::icon_role(
                                Icon::Close,
                                24.,
                                ColorRole::ON_SURFACE_VARIANT,
                            )),
                    ),
            )
            .child(
                design::shaped(
                    [20., 20., 20., 20.],
                    design::c(ColorRole::SURFACE_CONTAINER_LOWEST),
                )
                .p(px(14.))
                .px(px(16.))
                .text_size(px(15.))
                .line_height(px(23.))
                .child(gpui_kit::StyledText::new(sentence).with_highlights(highlights)),
            )
            .child(
                div()
                    .flex()
                    .items_end()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .child(design::title(36., 42., 760.).child(selection.surface.clone()))
                            .child(
                                design::mono(13., 18., 400.)
                                    .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                                    .child(format!("词元 {}", selection.lemma)),
                            ),
                    )
                    .child(
                        div()
                            .id("word-audio")
                            .role(gpui_kit::Role::Button)
                            .aria_label("发音")
                            .cursor_pointer()
                            .w(px(48.))
                            .h(px(48.))
                            .rounded(px(24.))
                            .bg(design::c(ColorRole::SECONDARY_CONTAINER))
                            .text_color(design::c(ColorRole::ON_SECONDARY_CONTAINER))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(design::icon_role(
                                Icon::Speaker,
                                24.,
                                ColorRole::ON_SECONDARY_CONTAINER,
                            )),
                    ),
            )
            .child(design::text(20., 28., 600.).child(if meaning.is_empty() {
                "暂无释义".to_owned()
            } else {
                meaning.clone()
            }))
            .child(
                div()
                    .flex()
                    .gap(px(6.))
                    .when(!meaning.is_empty(), |row| {
                        row.child(
                            design::tag(
                                ColorRole::TERTIARY_CONTAINER,
                                ColorRole::ON_TERTIARY_CONTAINER,
                            )
                            .child(design::icon_role(
                                Icon::Spark,
                                16.,
                                ColorRole::ON_TERTIARY_CONTAINER,
                            ))
                            .child("AI 生成"),
                        )
                    })
                    .child(
                        design::tag(
                            ColorRole::SURFACE_CONTAINER_HIGH,
                            ColorRole::ON_SURFACE_VARIANT,
                        )
                        .child("语境释义 · 新生成"),
                    ),
            );
        let mut actions = div().flex().gap(px(2.));
        for (index, (id, label, action, primary)) in [
            ("mark-known", "认识", MarkAction::Known, false),
            ("mark-unknown", "不认识", MarkAction::Unknown, false),
            ("add-review", "加入复习", MarkAction::AddToReview, true),
        ]
        .into_iter()
        .enumerate()
        {
            let radii = match index {
                0 => [24., 8., 8., 24.],
                2 => [8., 24., 24., 8.],
                _ => [8., 8., 8., 8.],
            };
            let mut button = design::shaped(
                radii,
                design::c(if primary {
                    ColorRole::PRIMARY
                } else {
                    ColorRole::SURFACE_CONTAINER_LOWEST
                }),
            )
            .id(id)
            .test_support()
            .role(gpui_kit::Role::Button)
            .aria_label(label)
            .cursor_pointer()
            .flex_1()
            .h(px(48.))
            .text_color(design::c(if primary {
                ColorRole::ON_PRIMARY
            } else {
                ColorRole::ON_SURFACE_VARIANT
            }))
            .text_size(px(15.))
            .font_weight(gpui_kit::FontWeight(650.))
            .when(self.loading, |button| button.opacity(0.38))
            .on_click(cx.listener(move |this, _, _, cx| {
                if this.loading {
                    return;
                }
                if let Some((context, revision, meaning)) = &this.lookup {
                    this.request(
                        Command::Mark {
                            article: context.article_id.clone(),
                            body_hash: context.body_hash.clone(),
                            range: context.selection.range,
                            action,
                            revision: *revision,
                            meaning: meaning.clone(),
                            operation: this.operation("mark"),
                        },
                        cx,
                    );
                }
            }));
            button = button.child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(6.))
                    .when(primary, |row| {
                        row.child(design::icon_role(Icon::Plus, 18., ColorRole::ON_PRIMARY))
                    })
                    .child(label),
            );
            actions = actions.child(button);
        }
        pane = pane.child(actions);
        if let Some(result) = &self.mark_result {
            let message = if result.card_id.is_some() {
                "已加入复习"
            } else if result.inbox_id.is_some() {
                "已放入生词箱"
            } else {
                "已标记认识"
            };
            pane = pane.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .rounded(px(14.))
                    .bg(design::c(ColorRole::SUCCESS_BG))
                    .text_color(design::c(ColorRole::ON_SUCCESS))
                    .p(px(10.))
                    .px(px(14.))
                    .child(design::icon_role(Icon::Check, 18., ColorRole::ON_SUCCESS))
                    .child(design::text(13., 18., 650.).child(message)),
            );
        }
        let mode = if self.settings.lookup_mode == cijian_domain::settings::LookupMode::MarkOnly {
            "只标记"
        } else {
            "自动建卡"
        };
        pane = pane.child(div().flex_1()).child(
            div()
                .rounded(px(20.))
                .bg(design::c(ColorRole::SURFACE_CONTAINER_HIGH))
                .p(px(14.))
                .px(px(16.))
                .gap(px(6.))
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(design::icon_role(Icon::Info, 16., ColorRole::PRIMARY))
                        .child(
                            design::text(13., 18., 650.)
                                .child(format!("当前点词模式：{mode}")),
                        )
                        .child(div().flex_1())
                        .child(
                            div()
                                .id("lookup-settings")
                                .role(gpui_kit::Role::Button)
                                .aria_label("更改点词模式")
                                .cursor_pointer()
                                .text_color(design::c(ColorRole::PRIMARY))
                                .text_size(px(13.))
                                .font_weight(gpui_kit::FontWeight(650.))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.navigate(Route::Settings, cx)
                                }))
                                .child("更改"),
                        ),
                )
                .child(
                    design::muted_text(12., 16.).child(
                        "“不认识”只放进生词箱，不建卡。切换到“自动建卡”后会直接加入复习，并计入每日新词上限。",
                    ),
                ),
        );
        let _ = revision;
        Some(pane.into_any_element())
    }

    pub(super) fn reader_body(&mut self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let listening = matches!(
            self.navigation.current(),
            Route::Article {
                listening: true,
                ..
            }
        );
        let Some(article) = self.article.clone().filter(
            |article| matches!(self.navigation.current(),Route::Article{id,..} if id==&article.id),
        ) else {
            let mut pane = design::pane()
                .flex_1()
                .min_w_0()
                .h_full()
                .p(px(16.))
                .px(px(24.))
                .gap(px(0.));
            pane = pane.child(self.reader_header(listening, cx));
            return pane
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(design::muted_text(16., 24.).child("文章未加载，可返回文章列表。")),
                )
                .into_any_element();
        };
        let side = if self.compact {
            None
        } else if self.lookup.is_some() && !listening {
            self.reader_lookup_panel(cx)
        } else if listening {
            Some(self.reader_side_facts(&article))
        } else {
            None
        };
        let side_width = if side.is_some() { 396. } else { 0. };
        let center_width = if self.compact {
            self.viewport_width - 32.
        } else if listening {
            760.
        } else {
            700.
        }
        .min((self.viewport_width - 96. - 24. - side_width).max(360.));
        let mut center = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .items_center()
            .gap(px(16.))
            .pt(px(20.))
            .px(px(if self.compact { 16. } else { 0. }));
        center = center.child(
            div()
                .w(px(center_width))
                .max_w_full()
                .child(self.reader_meta(&article)),
        );
        if !article.diagnostics.is_empty() {
            center = center.child(
                div().w(px(center_width)).max_w_full().child(
                    design::tag(ColorRole::WARNING_BG, ColorRole::ON_WARNING)
                        .h_auto()
                        .py(px(6.))
                        .child(article.diagnostics.join("；")),
                ),
            );
        }
        center = center.child(
            design::title(
                if self.compact { 28. } else { 44. },
                if self.compact { 34. } else { 50. },
                760.,
            )
            .w(px(center_width))
            .max_w_full()
            .child(article.title.clone()),
        );
        if listening {
            center = center.child(
                div()
                    .w(px(center_width))
                    .max_w_full()
                    .child(self.listening_panel(&article, cx)),
            );
            center = center.child(
                div()
                    .w(px(center_width))
                    .max_w_full()
                    .child(self.reader_feedback(true, cx)),
            );
            if self.show_listening_text {
                center = center.child(design::divider().w(px(center_width))).child(
                    div()
                        .id("article-body")
                        .w(px(center_width))
                        .max_w_full()
                        .child(self.reader_text_body(cx)),
                );
            } else {
                center = center.child(
                    div()
                        .w(px(center_width))
                        .max_w_full()
                        .h(px(76.))
                        .rounded(px(24.))
                        .border_1()
                        .border_dashed()
                        .border_color(design::c(ColorRole::OUTLINE_VARIANT))
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .px(px(18.))
                        .child(design::icon_role(
                            Icon::Eye,
                            20.,
                            ColorRole::ON_SURFACE_VARIANT,
                        ))
                        .child(
                            design::muted_text(14., 20.)
                                .flex_1()
                                .child("原文暂时收起。准备好了，就打开它。"),
                        )
                        .child(
                            div()
                                .id("show-listening-text")
                                .role(gpui_kit::Role::Button)
                                .aria_label("显示原文")
                                .cursor_pointer()
                                .h(px(40.))
                                .px(px(18.))
                                .rounded(px(20.))
                                .bg(design::c(ColorRole::SECONDARY_CONTAINER))
                                .text_color(design::c(ColorRole::ON_SECONDARY_CONTAINER))
                                .flex()
                                .items_center()
                                .text_size(px(14.))
                                .font_weight(gpui_kit::FontWeight(650.))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.show_listening_text = true;
                                    cx.notify();
                                }))
                                .child("显示原文"),
                        ),
                );
            }
        } else {
            center = center.child(
                div()
                    .id("article-body")
                    .w(px(center_width))
                    .max_w_full()
                    .child(self.reader_text_body(cx)),
            );
            center = center.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(16.))
                    .w(px(center_width))
                    .max_w_full()
                    .text_color(design::c(ColorRole::OUTLINE))
                    .child(design::divider().flex_1())
                    .child(design::mono(12., 16., 650.).child("END"))
                    .child(design::divider().flex_1()),
            );
            center = center.child(
                div()
                    .w(px(center_width))
                    .max_w_full()
                    .child(self.reader_feedback(false, cx)),
            );
        }
        let scroll = div()
            .id("reader-page")
            .track_scroll(&self.reader_scroll)
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .pb(px(110.))
            .child(center);
        let mut pane = design::pane()
            .flex_1()
            .min_w_0()
            .h_full()
            .p(px(16.))
            .px(px(if self.compact { 16. } else { 24. }))
            .pb(px(if listening { 28. } else { 0. }))
            .gap(px(0.))
            .relative()
            .overflow_hidden();
        pane = pane.child(self.reader_header(listening, cx));
        if let Some(notice) = self.position_notice(cx) {
            pane = pane.child(notice);
        }
        pane = pane.child(scroll);
        if !listening {
            pane = pane.child(self.audio_toolbar(cx));
        }
        let mut main = div().flex().flex_1().min_w_0().h_full().gap(px(12.));
        main = main.child(pane);
        if let Some(side) = side {
            main = main.child(side);
        }
        let mut result = div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .child(main.into_any_element());
        if self.compact
            && let Some((context, _, meaning)) = &self.lookup
        {
            let weak = cx.entity().downgrade();
            let dismiss = move |window: &mut Window, cx: &mut App| {
                let _ = weak.update(cx, |this, cx| this.dismiss_lookup(window, cx));
            };
            let mut actions = div().flex().gap(px(2.));
            for (index, (id, label, action, primary)) in [
                ("mark-known", "认识", MarkAction::Known, false),
                ("mark-unknown", "不认识", MarkAction::Unknown, false),
                ("add-review", "加入复习", MarkAction::AddToReview, true),
            ]
            .into_iter()
            .enumerate()
            {
                let radii = match index {
                    0 => [24., 8., 8., 24.],
                    2 => [8., 24., 24., 8.],
                    _ => [8., 8., 8., 8.],
                };
                let mut button = design::shaped(
                    radii,
                    design::c(if primary {
                        ColorRole::PRIMARY
                    } else {
                        ColorRole::SURFACE_CONTAINER_LOWEST
                    }),
                )
                .id(id)
                .test_support()
                .role(gpui_kit::Role::Button)
                .aria_label(label)
                .cursor_pointer()
                .flex_1()
                .h(px(56.))
                .flex()
                .items_center()
                .justify_center()
                .gap(px(6.))
                .text_color(design::c(if primary {
                    ColorRole::ON_PRIMARY
                } else {
                    ColorRole::ON_SURFACE_VARIANT
                }))
                .text_size(px(15.))
                .font_weight(gpui_kit::FontWeight(650.))
                .when(self.loading, |button| button.opacity(0.38))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if this.loading {
                        return;
                    }
                    if let Some((context, revision, meaning)) = &this.lookup {
                        this.request(
                            Command::Mark {
                                article: context.article_id.clone(),
                                body_hash: context.body_hash.clone(),
                                range: context.selection.range,
                                action,
                                revision: *revision,
                                meaning: meaning.clone(),
                                operation: this.operation("mark"),
                            },
                            cx,
                        );
                    }
                }));
                if primary {
                    button =
                        button.child(design::icon_role(Icon::Plus, 18., ColorRole::ON_PRIMARY));
                }
                actions = actions.child(button.child(label));
            }
            let mut content = div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .p(px(8.))
                .px(px(22.))
                .pb(px(18.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w_0()
                                .child(
                                    design::title(30., 36., 760.)
                                        .child(context.selection.surface.clone()),
                                )
                                .child(
                                    design::mono(12., 16., 400.)
                                        .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                                        .child(format!("词元 {}", context.selection.lemma)),
                                ),
                        )
                        .child(
                            div()
                                .id("word-audio")
                                .role(gpui_kit::Role::Button)
                                .aria_label("发音")
                                .cursor_pointer()
                                .w(px(48.))
                                .h(px(48.))
                                .rounded(px(24.))
                                .bg(design::c(ColorRole::SECONDARY_CONTAINER))
                                .text_color(design::c(ColorRole::ON_SECONDARY_CONTAINER))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(design::icon_role(
                                    Icon::Speaker,
                                    24.,
                                    ColorRole::ON_SECONDARY_CONTAINER,
                                )),
                        ),
                )
                .child(design::text(18., 24., 600.).child(if meaning.is_empty() {
                    "语境释义暂不可用，仍可标记".to_owned()
                } else {
                    meaning.clone()
                }))
                .child(
                    div()
                        .flex()
                        .gap(px(6.))
                        .when(!meaning.is_empty(), |row| {
                            row.child(
                                design::tag(
                                    ColorRole::TERTIARY_CONTAINER,
                                    ColorRole::ON_TERTIARY_CONTAINER,
                                )
                                .child(design::icon_role(
                                    Icon::Spark,
                                    16.,
                                    ColorRole::ON_TERTIARY_CONTAINER,
                                ))
                                .child("AI 生成"),
                            )
                        })
                        .child(
                            design::tag(
                                ColorRole::SURFACE_CONTAINER_HIGH,
                                ColorRole::ON_SURFACE_VARIANT,
                            )
                            .child("语境释义"),
                        ),
                )
                .child(actions);
            if let Some(result) = &self.mark_result {
                content = content.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .rounded(px(14.))
                        .bg(design::c(ColorRole::SUCCESS_BG))
                        .text_color(design::c(ColorRole::ON_SUCCESS))
                        .p(px(10.))
                        .px(px(14.))
                        .child(design::icon_role(Icon::Check, 18., ColorRole::ON_SUCCESS))
                        .child(
                            design::text(13., 18., 650.).child(if result.card_id.is_some() {
                                "已加入复习"
                            } else if result.inbox_id.is_some() {
                                "已放入生词箱"
                            } else {
                                "已标记认识"
                            }),
                        ),
                );
            }
            let mark_only =
                self.settings.lookup_mode == cijian_domain::settings::LookupMode::MarkOnly;
            content = content.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                    .child(design::icon_role(Icon::Info, 16., ColorRole::PRIMARY))
                    .child(if mark_only {
                        "只标记模式：“不认识”进入生词箱"
                    } else {
                        "自动建卡模式：直接加入复习"
                    })
                    .child(div().flex_1())
                    .child(
                        div()
                            .id("lookup-settings")
                            .role(gpui_kit::Role::Button)
                            .aria_label("更改点词模式")
                            .cursor_pointer()
                            .text_color(design::c(ColorRole::PRIMARY))
                            .text_size(px(12.))
                            .font_weight(gpui_kit::FontWeight(650.))
                            .on_click(
                                cx.listener(|this, _, _, cx| this.navigate(Route::Settings, cx)),
                            )
                            .child("更改"),
                    ),
            );
            result = div()
                .flex()
                .flex_col()
                .w_full()
                .h_full()
                .child(result)
                .child(
                    MaterialBottomSheet::new("word-sheet")
                        .open(true)
                        .modal(true)
                        .label(context.selection.surface.clone())
                        .dismiss_on_escape(!self.loading || self.lookup_loading)
                        .dismiss_on_scrim(!self.loading || self.lookup_loading)
                        .drag_handle("展开词语面板")
                        .child(content)
                        .on_dismiss(dismiss),
                );
        }
        result.child(self.flag_dialog(cx)).into_any_element()
    }

    pub(super) fn inbox_body(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let wide = self.viewport_width >= 1200.;
        let card_width = if self.compact {
            self.viewport_width - 32.
        } else {
            ((self.viewport_width - 96. - 12. - 64. - 28.) / 3.).max(240.)
        };
        let mut body = design::pane()
            .id("inbox-page")
            .track_scroll(&self.page_scroll)
            .flex_1()
            .min_w_0()
            .h_full()
            .p(px(24.))
            .px(px(if self.compact { 16. } else { 32. }))
            .pb(px(28.))
            .gap(px(18.))
            .overflow_y_scroll();
        let mode_hint =
            if self.settings.lookup_mode == cijian_domain::settings::LookupMode::MarkOnly {
                "只标记"
            } else {
                "自动建卡"
            };
        body = body.child(
            div()
                .flex()
                .items_end()
                .gap(px(16.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .gap(px(4.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(12.))
                                .child(design::title(40., 48., 750.).child("生词箱"))
                                .child(
                                    design::tag(
                                        ColorRole::TERTIARY_CONTAINER,
                                        ColorRole::ON_TERTIARY_CONTAINER,
                                    )
                                    .h(px(28.))
                                    .px(px(12.))
                                    .text_size(px(13.))
                                    .child(format!("{} 待整理", self.inbox_total)),
                                ),
                        )
                        .child(
                            design::muted_text(16., 22.)
                                .child("阅读时的一个小标记，在这里慢慢整理。"),
                        ),
                )
                .when(wide, |row| {
                    row.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .rounded(px(20.))
                            .bg(design::c(ColorRole::SURFACE_CONTAINER_HIGH))
                            .p(px(10.))
                            .pr(px(12.))
                            .pl(px(16.))
                            .child(
                                div()
                                    .text_color(design::c(ColorRole::PRIMARY))
                                    .child(design::icon_role(Icon::Info, 18., ColorRole::PRIMARY)),
                            )
                            .child(
                                design::text(13., 18., 400.).child(
                                    div()
                                        .flex()
                                        .gap(px(3.))
                                        .child("点词模式：")
                                        .child(design::text(13., 18., 650.).child(mode_hint))
                                        .child("——“不认识”的词会来到这里"),
                                ),
                            )
                            .child(
                                div()
                                    .id("inbox-settings")
                                    .role(gpui_kit::Role::Button)
                                    .aria_label("更改点词模式")
                                    .cursor_pointer()
                                    .h(px(32.))
                                    .px(px(12.))
                                    .rounded(px(16.))
                                    .flex()
                                    .items_center()
                                    .text_color(design::c(ColorRole::PRIMARY))
                                    .text_size(px(14.))
                                    .font_weight(gpui_kit::FontWeight(650.))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.navigate(Route::Settings, cx)
                                    }))
                                    .child("更改"),
                            ),
                    )
                }),
        );

        let selected_count = self.inbox_selected.len();
        let all_selected = !self.inbox.is_empty() && selected_count == self.inbox.len();
        let mut bar = design::shaped(
            [32., 32., 32., 32.],
            design::c(ColorRole::PRIMARY_CONTAINER),
        )
        .h(px(64.))
        .w_full()
        .flex()
        .items_center()
        .gap(px(12.))
        .p(px(10.))
        .pl(px(8.))
        .text_color(design::c(ColorRole::ON_PRIMARY_CONTAINER));
        bar = bar
            .child(
                div()
                    .id("inbox-clear")
                    .test_support()
                    .role(gpui_kit::Role::Button)
                    .aria_label("清空选择")
                    .cursor_pointer()
                    .w(px(48.))
                    .h(px(48.))
                    .rounded(px(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.inbox_selected.clear();
                        cx.notify();
                    }))
                    .child(design::icon_role(
                        Icon::Close,
                        24.,
                        ColorRole::ON_PRIMARY_CONTAINER,
                    )),
            )
            .child(
                div()
                    .id("inbox-select-page")
                    .test_support()
                    .role(gpui_kit::Role::Button)
                    .aria_label("全选本页")
                    .cursor_pointer()
                    .w(px(20.))
                    .h(px(20.))
                    .rounded(px(3.))
                    .border_2()
                    .border_color(design::c(ColorRole::ON_PRIMARY_CONTAINER))
                    .when(all_selected || selected_count > 0, |checkbox| {
                        checkbox
                            .bg(design::c(ColorRole::PRIMARY))
                            .border_color(design::c(ColorRole::PRIMARY))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(design::c(ColorRole::ON_PRIMARY))
                            .when(all_selected, |checkbox| {
                                checkbox.child(design::icon_role(
                                    Icon::Check,
                                    16.,
                                    ColorRole::ON_PRIMARY,
                                ))
                            })
                            .when(!all_selected && selected_count > 0, |checkbox| {
                                checkbox.child(
                                    div()
                                        .w(px(10.))
                                        .h(px(2.))
                                        .rounded(px(1.))
                                        .bg(design::c(ColorRole::ON_PRIMARY)),
                                )
                            })
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.loading {
                            return;
                        }
                        if this.inbox_selected.len() == this.inbox.len() {
                            this.inbox_selected.clear();
                        } else {
                            this.inbox_selected =
                                this.inbox.iter().map(|item| item.id.clone()).collect();
                        }
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        design::text(16., 22., 650.)
                            .child(format!("已选 {selected_count} / {}", self.inbox.len())),
                    )
                    .child(design::text(13., 18., 400.).opacity(0.8).child("· 全选")),
            )
            .child(div().flex_1());
        for (id, label, action, primary) in [
            ("inbox-ignore", "忽略", InboxAction::Ignore, false),
            ("inbox-add", "加入复习", InboxAction::Add, true),
        ] {
            let disabled = self.loading || selected_count == 0;
            let mut action_button = div()
                .id(id)
                .test_support()
                .role(gpui_kit::Role::Button)
                .aria_label(label)
                .cursor_pointer()
                .h(px(44.))
                .px(px(20.))
                .flex()
                .items_center()
                .gap(px(8.))
                .text_size(px(14.))
                .font_weight(gpui_kit::FontWeight(650.))
                .when(primary, |button| {
                    button
                        .rounded(px(22.))
                        .bg(design::c(ColorRole::PRIMARY))
                        .text_color(design::c(ColorRole::ON_PRIMARY))
                })
                .when(!primary, |button| {
                    button
                        .rounded(px(22.))
                        .bg(design::c(ColorRole::SURFACE_CONTAINER_LOWEST))
                        .text_color(design::c(ColorRole::ON_PRIMARY_CONTAINER))
                })
                .when(disabled, |button| button.opacity(0.38))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if this.loading || this.inbox_selected.is_empty() {
                        return;
                    }
                    let selections = this
                        .inbox
                        .iter()
                        .filter(|item| this.inbox_selected.contains(&item.id))
                        .map(|item| (item.id.clone(), item.revision))
                        .collect();
                    this.request(
                        Command::BatchInbox {
                            action,
                            selections,
                            operation: this.operation("inbox"),
                        },
                        cx,
                    );
                }));
            if primary {
                action_button =
                    action_button.child(design::icon_role(Icon::Plus, 18., ColorRole::ON_PRIMARY));
            }
            bar = bar.child(action_button.child(label));
        }
        if selected_count > 0 {
            body = body.child(bar);
        }

        if self.inbox.is_empty() {
            body = body.child(design::empty_state("读文章时把不熟悉的词留在这里").h(px(220.)));
            body = body.child(
                div()
                    .id("inbox-articles")
                    .role(gpui_kit::Role::Button)
                    .aria_label("去文章")
                    .cursor_pointer()
                    .self_start()
                    .h(px(40.))
                    .px(px(20.))
                    .rounded(px(20.))
                    .bg(design::c(ColorRole::SECONDARY_CONTAINER))
                    .text_color(design::c(ColorRole::ON_SECONDARY_CONTAINER))
                    .flex()
                    .items_center()
                    .on_click(cx.listener(|this, _, _, cx| this.navigate(Route::Articles, cx)))
                    .child("去文章")
                    .child(design::icon_role(
                        Icon::Chev,
                        18.,
                        ColorRole::ON_SECONDARY_CONTAINER,
                    )),
            );
            return body.into_any_element();
        }

        let mut grid = div().flex().flex_wrap().gap(px(14.));
        for item in &self.inbox {
            let id = item.id.clone();
            let selected = self.inbox_selected.contains(&id);
            let article = item.article_id.clone();
            let reveal = article
                .clone()
                .zip(item.range.zip(item.body_hash.clone()))
                .map(|(article, (range, hash))| (article, hash, range.start));
            let mut card = div()
                .id(gpui_kit::SharedString::from(format!(
                    "inbox-card-{}",
                    item.id
                )))
                .w(px(card_width))
                .min_w(px(200.))
                .flex_shrink_0()
                .flex()
                .flex_col()
                .gap(px(10.))
                .p(px(20.))
                .px(px(22.))
                .rounded(px(24.))
                .bg(if selected {
                    design::alpha(design::c(ColorRole::PRIMARY_CONTAINER), 0.35)
                } else {
                    design::c(ColorRole::SURFACE_CONTAINER_LOWEST)
                })
                .when(selected, |card| {
                    card.border_2().border_color(design::c(ColorRole::PRIMARY))
                })
                .when(!selected, |card| card.shadow(design::shadow_card()));
            card = card.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .id(gpui_kit::SharedString::from(format!(
                                "select-inbox-{}",
                                item.id
                            )))
                            .test_support()
                            .role(gpui_kit::Role::Button)
                            .aria_label(format!("选择 {}", item.lemma))
                            .cursor_pointer()
                            .w(px(18.))
                            .h(px(18.))
                            .rounded(px(3.))
                            .border_2()
                            .border_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                            .when(selected, |checkbox| {
                                checkbox
                                    .bg(design::c(ColorRole::PRIMARY))
                                    .border_color(design::c(ColorRole::PRIMARY))
                                    .text_color(design::c(ColorRole::ON_PRIMARY))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(design::icon_role(
                                        Icon::Check,
                                        14.,
                                        ColorRole::ON_PRIMARY,
                                    ))
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if !this.inbox_selected.remove(&id) {
                                    this.inbox_selected.insert(id.clone());
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        design::title(22., 28., 750.)
                            .flex_1()
                            .child(item.lemma.clone()),
                    )
                    .child(design::muted_text(12., 16.).child("待整理")),
            );
            card = card.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        design::text(15., 22., 400.).child(if item.meaning.is_empty() {
                            "释义未生成".to_owned()
                        } else {
                            item.meaning.clone()
                        }),
                    )
                    .when(!item.meaning.is_empty(), |row| {
                        row.child(
                            design::tag(
                                ColorRole::TERTIARY_CONTAINER,
                                ColorRole::ON_TERTIARY_CONTAINER,
                            )
                            .h(px(20.))
                            .text_size(px(11.))
                            .child(source_label(&item.meaning_source)),
                        )
                    }),
            );
            let highlighted = gpui_kit::StyledText::new(item.context.clone()).with_highlights(
                item.context
                    .to_lowercase()
                    .find(&item.lemma.to_lowercase())
                    .map(|start| {
                        (
                            start..start + item.lemma.len(),
                            gpui_kit::HighlightStyle {
                                background_color: Some(design::c(ColorRole::WARNING_BG)),
                                color: Some(design::c(ColorRole::ON_WARNING)),
                                ..Default::default()
                            },
                        )
                    }),
            );
            card = card.child(
                div()
                    .text_size(px(13.))
                    .line_height(px(20.))
                    .font_weight(gpui_kit::FontWeight(400.))
                    .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                    .child(highlighted),
            );
            if let Some(article) = article {
                let article_id = article.clone();
                card = card.child(
                    div()
                        .id(gpui_kit::SharedString::from(format!("source-{}", item.id)))
                        .test_support()
                        .role(gpui_kit::Role::Button)
                        .aria_label(
                            item.article_title
                                .clone()
                                .unwrap_or_else(|| "来源文章不可用".into()),
                        )
                        .cursor_pointer()
                        .mt(px(6.))
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .text_color(design::c(ColorRole::PRIMARY))
                        .text_size(px(13.))
                        .line_height(px(18.))
                        .font_weight(gpui_kit::FontWeight(650.))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.source_reveal = reveal.clone();
                            this.request(
                                Command::OpenArticle {
                                    id: article_id.clone(),
                                },
                                cx,
                            );
                        }))
                        .child(design::icon_role(Icon::Book, 16., ColorRole::PRIMARY))
                        .child(
                            item.article_title
                                .clone()
                                .unwrap_or_else(|| "来源文章不可用".into()),
                        )
                        .child(design::icon_role(Icon::Chev, 16., ColorRole::PRIMARY)),
                );
            } else {
                card = card.child(design::muted_text(13., 18.).child("来源文章暂不可用"));
            }
            grid = grid.child(card);
        }
        body = body.child(grid);
        body.into_any_element()
    }
}

impl Shell {
    fn review_heatmap_totals(&self) -> (u64, u64, u64) {
        let mut learning = 0;
        let mut review = 0;
        let mut new = 0;
        for card in &self.review {
            match card.srs.state {
                cijian_domain::srs::State::Learning | cijian_domain::srs::State::Relearning => {
                    learning += 1
                }
                cijian_domain::srs::State::Review => review += 1,
                cijian_domain::srs::State::New => new += 1,
            }
        }
        (learning, review, new)
    }

    fn review_progress(&self, width: f32) -> gpui_kit::AnyElement {
        let total = (self.review_done + self.review.len() as u32).max(1) as f32;
        let fraction = (self.review_done as f32 / total).clamp(0., 1.);
        div()
            .relative()
            .w_full()
            .h(px(12.))
            .child(
                gpui_kit::canvas(
                    move |_, _, _| (),
                    move |bounds, _, window, _| {
                        let start = bounds.origin.x;
                        let end = start + px(width);
                        let mid = bounds.origin.y + px(6.);
                        let split = start + px(width * fraction);
                        let mut done = gpui_kit::PathBuilder::stroke(px(4.));
                        let mut x = start + px(2.);
                        let mut up = true;
                        done.move_to(gpui_kit::point(x, mid));
                        while x < split {
                            let next = (x + px(15.)).min(split);
                            let y = if up { mid - px(5.) } else { mid + px(5.) };
                            done.curve_to(
                                gpui_kit::point(next, mid),
                                gpui_kit::point((x + next) / 2., y),
                            );
                            x = next;
                            up = !up;
                        }
                        if let Ok(path) = done.build() {
                            window.paint_path(path, design::c(ColorRole::PRIMARY));
                        }
                        if split < end {
                            let mut rest = gpui_kit::PathBuilder::stroke(px(4.));
                            rest.move_to(gpui_kit::point(split, mid));
                            rest.line_to(gpui_kit::point(end, mid));
                            if let Ok(path) = rest.build() {
                                window.paint_path(path, design::c(ColorRole::SECONDARY_CONTAINER));
                            }
                            window.paint_quad(gpui_kit::quad(
                                gpui_kit::Bounds {
                                    origin: gpui_kit::point(end - px(2.), mid - px(2.)),
                                    size: gpui_kit::size(px(4.), px(4.)),
                                },
                                gpui_kit::Corners::all(px(2.)),
                                design::c(ColorRole::PRIMARY),
                                gpui_kit::Edges::all(px(0.)),
                                gpui_kit::transparent_black(),
                                gpui_kit::BorderStyle::Solid,
                            ));
                        }
                    },
                )
                .absolute()
                .inset_0(),
            )
            .into_any_element()
    }

    pub(super) fn review_body(&mut self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let wide = self.viewport_width >= 1180.;
        let remaining = self.review.len() as u64;
        let (learning, review_count, new) = self.review_heatmap_totals();
        let card_width = if self.compact {
            (self.viewport_width - 40.).max(280.)
        } else if wide {
            780.
        } else {
            (self.viewport_width - 96. - 24. - 64.).clamp(420., 780.)
        };
        let mut pane = design::pane()
            .flex_1()
            .min_w_0()
            .h_full()
            .p(px(20.))
            .px(px(if self.compact { 20. } else { 32. }))
            .pb(px(28.))
            .items_center()
            .gap(px(0.));
        let mut header = div()
            .w_full()
            .flex()
            .items_center()
            .gap(px(16.))
            .child(
                div()
                    .id("review-close")
                    .role(gpui_kit::Role::Button)
                    .aria_label("结束复习")
                    .cursor_pointer()
                    .w(px(48.))
                    .h(px(48.))
                    .rounded(px(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                    .on_click(cx.listener(|this, _, _, cx| this.navigate(Route::Today, cx)))
                    .child(design::icon_role(
                        Icon::Close,
                        24.,
                        ColorRole::ON_SURFACE_VARIANT,
                    )),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(
                        div()
                            .flex()
                            .gap(px(4.))
                            .text_size(px(13.))
                            .line_height(px(18.))
                            .child(
                                design::text(13., 18., 650.)
                                    .child(format!("第 {} 张", self.review_done + 1)),
                            )
                            .child(
                                design::muted_text(13., 18.)
                                    .child(format!("· 还剩 {remaining} 张")),
                            ),
                    )
                    .child(self.review_progress(card_width.max(200.))),
            );
        if !self.compact
            && let Some(card) = self.review.first()
        {
            header = header.child(design::chip(
                true,
                source_label(&card.text_source),
                Some(Icon::Book),
            ));
        }
        pane = pane.child(header);
        if self.compact && !self.review.is_empty() {
            pane = pane.child(
                div()
                    .flex()
                    .justify_center()
                    .mt(px(12.))
                    .child(design::chip(
                        true,
                        source_label(&self.review[0].text_source),
                        Some(Icon::Book),
                    )),
            );
        }

        let Some(card) = self.review.first().cloned() else {
            pane = pane.child(div().flex_1().flex().items_center().justify_center().child(
                design::muted_text(16., 24.).child(if self.waiting {
                    "学习步骤尚未到期，请稍后再来"
                } else if self.loading {
                    "正在读取复习队列…"
                } else {
                    "当前没有待复习词卡，可以导入词书或稍后再来"
                }),
            ));
            return pane.into_any_element();
        };

        let long_word = card.front.chars().count() > 16;
        let mut front = design::shaped(
            if self.compact {
                [36., 36., 36., 36.]
            } else {
                [40., 40., 40., 40.]
            },
            design::c(ColorRole::SURFACE_CONTAINER_LOWEST),
        )
        .id("review-card")
        .test_support()
        .w(px(card_width))
        .max_w_full()
        .shadow(design::shadow_card())
        .mt(px(if self.compact { 56. } else { 40. }))
        .when(self.compact, |card| {
            card.flex_none()
                .h(px((self.viewport_height - 380.).clamp(360., 512.)))
                .when(self.revealed, |card| {
                    card.justify_start().overflow_y_scroll()
                })
                .when(!self.revealed, |card| card.justify_center().items_center())
        })
        .when(!self.compact, |card| {
            card.max_h(px((self.viewport_height - 360.).max(240.)))
                .overflow_y_scroll()
        })
        .p(px(if self.compact { 24. } else { 44. }))
        .px(px(if self.compact { 24. } else { 56. }))
        .pb(px(40.))
        .flex()
        .flex_col()
        .gap(px(22.))
        .cursor_pointer()
        .on_click(cx.listener(|this, _, _, cx| this.reveal_card(cx)))
        .child(
            design::text(14., 20., 600.)
                .id("reveal-hint")
                .text_color(design::c(ColorRole::PRIMARY))
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| this.reveal_card(cx)))
                .child("试着想起它的意思"),
        )
        .child(
            design::title(
                if long_word || self.compact { 52. } else { 76. },
                if long_word || self.compact { 58. } else { 80. },
                760.,
            )
            .id("reveal-word")
            .font_family("Cijian Display")
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| this.reveal_card(cx)))
            .child(card.front.clone()),
        )
        .child(self.review_pronunciation(&card, cx));
        if self.revealed {
            let pending = self.gloss_pending.as_deref() == Some(&card.id);
            let failed = self
                .gloss_failed
                .as_ref()
                .filter(|(id, _)| id == &card.id)
                .map(|(_, message)| message.clone());
            let mut meaning_row = div().flex().items_baseline().gap(px(12.)).w_full();
            if card.meaning.is_empty() {
                let status = if pending {
                    "正在生成释义…"
                } else if failed.is_some() {
                    "释义生成失败，可照常评分"
                } else if self.provider_config.is_none() {
                    "未配置 AI 服务，可在词库中编辑释义"
                } else {
                    "暂无释义"
                };
                let mut missing = div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(6.))
                    .child(design::muted_text(17., 24.).child(status));
                if failed.is_some() && !pending {
                    let retry = card.clone();
                    missing = missing.child(
                        div()
                            .id("retry-review-gloss")
                            .role(gpui_kit::Role::Button)
                            .aria_label("重试生成释义")
                            .cursor_pointer()
                            .h(px(32.))
                            .px(px(12.))
                            .rounded(px(16.))
                            .bg(design::c(ColorRole::SECONDARY_CONTAINER))
                            .text_color(design::c(ColorRole::ON_SECONDARY_CONTAINER))
                            .flex()
                            .items_center()
                            .text_size(px(13.))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.retry_card_gloss(retry.clone(), cx)
                            }))
                            .child("重试生成"),
                    );
                }
                meaning_row = meaning_row.child(missing);
            } else {
                meaning_row = meaning_row.child(
                    design::text(28., 36., 650.)
                        .flex_1()
                        .min_w_0()
                        .child(card.meaning.clone()),
                );
            }
            meaning_row = meaning_row.child(
                div()
                    .id("review-source")
                    .test_support()
                    .role(gpui_kit::Role::Label)
                    .aria_label(format!("来源：{}", card.text_source))
                    .flex_shrink_0()
                    .child(
                        design::tag(
                            ColorRole::SECONDARY_CONTAINER,
                            ColorRole::ON_SECONDARY_CONTAINER,
                        )
                        .child(source_label(&card.text_source)),
                    ),
            );
            front = front.child(design::divider()).child(meaning_row);
            if !card.example.is_empty() {
                front = front.child(
                    div()
                        .flex()
                        .items_start()
                        .gap(px(14.))
                        .rounded(px(20.))
                        .bg(design::c(ColorRole::SURFACE_CONTAINER))
                        .p(px(16.))
                        .px(px(18.))
                        .child(
                            div()
                                .text_color(design::c(ColorRole::PRIMARY))
                                .child(design::icon_role(Icon::Quote, 24., ColorRole::PRIMARY)),
                        )
                        .child(design::text(17., 26., 400.).child(card.example.clone())),
                );
            }
        } else if !self.compact {
            front = front.child(
                design::muted_text(14., 20.)
                    .id("reveal-caption")
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| this.reveal_card(cx)))
                    .child("点按卡片或下方按钮翻面"),
            );
        }
        pane = pane.child(front);

        if self.revealed {
            let mut ratings = div()
                .w(px(card_width))
                .max_w_full()
                .flex()
                .gap(px(4.))
                .mt(px(22.));
            // All four buttons share one width: the design's wider 1.25 Good
            // button read as a layout glitch next to the other three.
            for (index, (label, english, rating, bg, fg, radii, font_size)) in [
                (
                    "重来",
                    "Again",
                    Rating::Again,
                    ColorRole::ERROR_CONTAINER,
                    ColorRole::ON_ERROR_CONTAINER,
                    [32., 10., 10., 32.],
                    19.,
                ),
                (
                    "困难",
                    "Hard",
                    Rating::Hard,
                    ColorRole::WARNING_BG,
                    ColorRole::ON_WARNING,
                    [10., 10., 10., 10.],
                    19.,
                ),
                (
                    "记得",
                    "Good",
                    Rating::Good,
                    ColorRole::PRIMARY,
                    ColorRole::ON_PRIMARY,
                    [10., 10., 10., 10.],
                    21.,
                ),
                (
                    "轻松",
                    "Easy",
                    Rating::Easy,
                    ColorRole::SUCCESS_BG,
                    ColorRole::ON_SUCCESS,
                    [10., 32., 32., 10.],
                    19.,
                ),
            ]
            .into_iter()
            .enumerate()
            {
                let interval = self
                    .intervals
                    .map(|values| format_interval(values[index]))
                    .unwrap_or_default();
                let mut button = div()
                    .id(gpui_kit::SharedString::from(format!("rating-{index}")))
                    .test_support()
                    .role(gpui_kit::Role::Button)
                    .aria_label(format!("{label} · {english}"))
                    .cursor_pointer()
                    .flex_grow(1.)
                    .flex_basis(px(0.))
                    .min_w_0()
                    .h(px(100.))
                    .text_color(design::c(fg))
                    .when(self.loading, |button| button.opacity(0.38))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.loading {
                            this.rate(rating, cx);
                        }
                    }));
                if index == 2 {
                    // The middle "记得" button paints its own background, so
                    // it needs the same 10px radius as the shaped corners.
                    button = button.bg(design::c(bg)).rounded(px(10.));
                } else {
                    button =
                        button.child(design::shaped(radii, design::c(bg)).absolute().inset_0());
                }
                let key_alpha = if index == 2 { 0.22 } else { 0.6 };
                let key_color = if index == 2 {
                    gpui_kit::white()
                } else {
                    design::c(fg)
                };
                button = button
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                design::mono(12., 24., 600.)
                                    .min_w(px(24.))
                                    .h(px(24.))
                                    .px(px(6.))
                                    .rounded(px(7.))
                                    .bg(design::alpha(key_color, key_alpha))
                                    .text_color(key_color)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child((index + 1).to_string()),
                            )
                            .child(design::text(font_size, 26., 700.).child(label)),
                    )
                    .child(
                        design::text(
                            if self.compact { 12. } else { 13. },
                            if self.compact { 16. } else { 18. },
                            400.,
                        )
                        .id(gpui_kit::SharedString::from(format!("rating-meta-{index}")))
                        .test_support()
                        .child(format!("{english} · {interval}")),
                    );
                ratings = ratings.child(button);
            }
            pane = pane.child(ratings);
            pane = pane.child(
                design::muted_text(13., 18.)
                    .mt(px(16.))
                    .child("按真实记忆选择 · 间隔按 Anki 调度"),
            );
        } else {
            pane = pane.child(
                div()
                    .id("reveal")
                    .role(gpui_kit::Role::Button)
                    .aria_label("显示答案")
                    .cursor_pointer()
                    .w(px(card_width))
                    .max_w_full()
                    .mt(px(22.))
                    .h(px(72.))
                    .rounded(px(36.))
                    .bg(design::c(ColorRole::SECONDARY_CONTAINER))
                    .text_color(design::c(ColorRole::ON_SECONDARY_CONTAINER))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(20.))
                    .font_weight(gpui_kit::FontWeight(700.))
                    .when(self.loading, |button| button.opacity(0.38))
                    .on_click(cx.listener(|this, _, _, cx| this.reveal_card(cx)))
                    .child("显示答案"),
            );
            pane = pane.child(
                design::muted_text(13., 18.)
                    .mt(px(12.))
                    .child("轻点卡片或按钮翻面"),
            );
        }

        let mut main = div().flex().flex_1().min_w_0().h_full().gap(px(12.));
        main = main.child(pane);
        if wide {
            let mut aside = div()
                .w(px(320.))
                .flex_shrink_0()
                .h_full()
                .flex()
                .flex_col()
                .gap(px(12.));
            let mut queue = design::pane().p(px(24.)).gap(px(14.));
            queue = queue
                .child(
                    design::muted_text(14., 20.)
                        .font_weight(gpui_kit::FontWeight(650.))
                        .child("今日队列"),
                )
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .gap(px(10.))
                        .child(design::title(56., 60., 750.).child(remaining.to_string()))
                        .child(design::muted_text(16., 22.).child("张剩余")),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(3.))
                        .h(px(12.))
                        .child(
                            div()
                                .flex_grow(learning.max(1) as f32)
                                .h(px(12.))
                                .rounded(px(6.))
                                .bg(design::c(ColorRole::TERTIARY)),
                        )
                        .child(
                            div()
                                .flex_grow(review_count.max(1) as f32)
                                .h(px(12.))
                                .rounded(px(3.))
                                .bg(design::c(ColorRole::PRIMARY)),
                        )
                        .child(
                            div()
                                .flex_grow(new.max(1) as f32)
                                .h(px(12.))
                                .rounded(px(6.))
                                .bg(design::c(ColorRole::OUTLINE_VARIANT)),
                        ),
                );
            for (label, value, role) in [
                ("学习中", learning, ColorRole::TERTIARY),
                ("待复习", review_count, ColorRole::PRIMARY),
                ("新词", new, ColorRole::OUTLINE_VARIANT),
            ] {
                queue = queue.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .text_size(px(15.))
                        .line_height(px(22.))
                        .child(design::dot(role, 10.))
                        .child(div().flex_1().child(label))
                        .child(design::text(15., 22., 700.).child(value.to_string())),
                );
            }
            aside = aside.child(queue);
            let mut keys = design::pane().p(px(22.)).px(px(24.)).gap(px(12.));
            keys = keys
                .child(design::muted_text(14., 20.).child("快捷键"))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .text_size(px(15.))
                        .child(design::kbd("Space"))
                        .child("显示答案"),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .text_size(px(15.))
                        .child(design::kbd("1"))
                        .child(design::kbd("2"))
                        .child(design::kbd("3"))
                        .child(design::kbd("4"))
                        .child("重来 / 困难 / 记得 / 轻松"),
                );
            aside = aside.child(keys);
            aside = aside.child(
                design::shaped(
                    [28., 28., 64., 28.],
                    design::c(ColorRole::TERTIARY_CONTAINER),
                )
                .flex_1()
                .p(px(22.))
                .px(px(24.))
                .gap(px(10.))
                .flex()
                .flex_col()
                .text_color(design::c(ColorRole::ON_TERTIARY_CONTAINER))
                .child(
                    div()
                        .w(px(44.))
                        .h(px(44.))
                        .rounded(px(14.))
                        .bg(design::c(ColorRole::TERTIARY))
                        .text_color(design::c(ColorRole::TERTIARY_CONTAINER))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(design::icon_role(Icon::Spark, 18., ColorRole::ON_PRIMARY)),
                )
                .child(design::text(16., 22., 650.).child("没有释义的词卡"))
                .child(
                    design::text(13., 18., 400.).child(
                        "翻面时自动用 AI 生成释义，并标注“AI 生成”。生成失败也可以照常评分，稍后在词库里编辑。",
                    ),
                ),
            );
            main = main.child(aside);
        }
        main.into_any_element()
    }
}

pub(super) fn source_label(source: &str) -> String {
    match source {
        "" => "本机".into(),
        "imported" => "导入".into(),
        "missing" => "待生成".into(),
        "user" | "user_edit" | "user-edited" => "用户编辑".into(),
        "generated" | "ai" | "lookup" => "AI 生成".into(),
        "legacy" | "legacy:import" => "旧版导入".into(),
        other => other.to_owned(),
    }
}
