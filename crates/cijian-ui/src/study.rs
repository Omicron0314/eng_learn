use super::*;
use cijian_domain::{
    analysis::{TokenKind, tokenize},
    reading::{Difficulty, FeedbackMode, TextRange},
    settings::{Genre, Tier},
};
use cijian_ports::{
    articles::{GenerationJob, JobStatus},
    inbox::{InboxAction, MarkAction},
};
use gpui_kit::{InteractiveText, StyledText};
use gpui_kit_material::components::MaterialSideSheet;
impl Shell {
    fn operation(&self, purpose: &str) -> String {
        format!(
            "{purpose}-{}-{}",
            self.request_id,
            self.review_started.elapsed().as_nanos()
        )
    }
    pub(super) fn receive_job(&mut self, job: GenerationJob, cx: &mut Context<Self>) {
        let active = matches!(job.status, JobStatus::Queued | JobStatus::Running);
        let id = job.id.clone();
        let result = job.result_ref.clone();
        self.generation_job = Some(job);
        if let Some(article) = result {
            if self.navigation.current() == &Route::Articles {
                self.request(Command::OpenArticle { id: article }, cx);
            }
        } else if active {
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(750))
                    .await;
                let _ = this.update(cx, |this, cx| {
                    if this.navigation.current() == &Route::Articles
                        && !this.loading
                        && this.generation_job.as_ref().is_some_and(|job| {
                            job.id == id
                                && matches!(job.status, JobStatus::Queued | JobStatus::Running)
                        })
                    {
                        this.request(Command::ObserveJob { job: id }, cx);
                    }
                });
            })
            .detach();
        }
    }
    pub(super) fn articles_body(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let active = self
            .generation_job
            .as_ref()
            .is_some_and(|j| matches!(j.status, JobStatus::Queued | JobStatus::Running));
        let mut body = div()
            .flex()
            .flex_col()
            .gap_4()
            .child("把复习过的词，放回故事里。")
            .child(
                material_button("generation-tier", ButtonVariant::Tonal)
                    .child(format!(
                        "难度：{}",
                        match self.settings.tier {
                            Tier::Easy => "轻松",
                            Tier::Medium => "适中",
                            Tier::Hard => "挑战",
                        }
                    ))
                    .disabled(active)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.settings.tier = match this.settings.tier {
                            Tier::Easy => Tier::Medium,
                            Tier::Medium => Tier::Hard,
                            Tier::Hard => Tier::Easy,
                        };
                        cx.notify();
                    })),
            )
            .child(
                material_button("generation-genre", ButtonVariant::Tonal)
                    .child(format!(
                        "体裁：{}",
                        match self.settings.genre {
                            Genre::Auto => "自动轮换",
                            Genre::Story => "故事",
                            Genre::Dialogue => "对话",
                            Genre::Monologue => "独白",
                        }
                    ))
                    .disabled(active)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.settings.genre = match this.settings.genre {
                            Genre::Auto => Genre::Story,
                            Genre::Story => Genre::Dialogue,
                            Genre::Dialogue => Genre::Monologue,
                            Genre::Monologue => Genre::Auto,
                        };
                        cx.notify();
                    })),
            )
            .child(
                material_button("generate-article", ButtonVariant::Filled)
                    .child("生成文章")
                    .disabled(self.loading || active)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.request(
                            Command::GenerateArticle {
                                tier: this.settings.tier,
                                genre: this.settings.genre,
                                operation: this.operation("generate"),
                            },
                            cx,
                        );
                    })),
            )
            .child("生成会调用已配置的 AI 服务，可能产生费用。");
        if let Some(job) = &self.generation_job {
            body = body.child(match job.status {
                JobStatus::Queued | JobStatus::Running => "正在生成…",
                JobStatus::Succeeded => "生成已完成",
                JobStatus::Failed => "生成失败，请检查设置后重新提交",
                JobStatus::Cancelled => "已取消；已发送的请求可能仍计费",
                JobStatus::Interrupted => "任务中断，未自动重试",
            });
            if active {
                let job_id = job.id.clone();
                body = body.child(
                    material_button("cancel-generation", ButtonVariant::Text)
                        .child("取消生成")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.request(
                                Command::CancelGeneration {
                                    job: job_id.clone(),
                                },
                                cx,
                            )
                        })),
                );
            }
        }
        if self.articles.is_empty() {
            body = body.child("还没有文章。先复习一些词，再生成今天的阅读。");
        }
        for article in &self.articles {
            let id = article.id.clone();
            body = body.child(
                material_button(
                    gpui_kit::ElementId::Name(format!("article-{id}").into()),
                    ButtonVariant::Tonal,
                )
                .child(article.title.clone())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.request(Command::OpenArticle { id: id.clone() }, cx)
                })),
            );
        }
        body.into_any_element()
    }
    pub(super) fn reader_body(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let mut body = div().flex().flex_col().gap_4();
        let Some(article) = &self.article else {
            return body
                .child("文章未加载，可返回文章列表。")
                .into_any_element();
        };
        body = body.child(div().text_size(px(28.)).child(article.title.clone()));
        for diagnostic in &article.diagnostics {
            body = body.child(
                div()
                    .text_color(color(ColorRole::ON_SURFACE_VARIANT))
                    .child(diagnostic.clone()),
            );
        }
        let ranges = tokenize(&article.body)
            .into_iter()
            .filter(|t| t.kind == TokenKind::Word)
            .map(|t| t.start..t.end)
            .collect::<Vec<_>>();
        let click_ranges = ranges.clone();
        let article_id = article.id.clone();
        let body_hash = article.body_hash.clone();
        let weak = cx.entity().downgrade();
        body =
            body.child(
                InteractiveText::new("article-body", StyledText::new(article.body.clone()))
                    .on_click(ranges, move |index, _, cx| {
                        let range = click_ranges[index].clone();
                        let _ = weak.update(cx, |this, cx| {
                            this.request(
                                Command::Lookup {
                                    article: article_id.clone(),
                                    body_hash: body_hash.clone(),
                                    range: TextRange {
                                        start: range.start,
                                        end: range.end,
                                    },
                                },
                                cx,
                            )
                        });
                    }),
            );
        let completed = self
            .reading_session
            .as_ref()
            .is_some_and(|s| s.completed_at.is_some());
        body = body.child(
            material_button("complete-reading", ButtonVariant::Filled)
                .child(if completed {
                    "本次阅读已完成"
                } else {
                    "读完了"
                })
                .disabled(self.loading || completed)
                .on_click(cx.listener(|this, _, _, cx| {
                    if let (Some(article), Some(session)) = (&this.article, &this.reading_session) {
                        this.request(
                            Command::CompleteReading {
                                article: article.id.clone(),
                                body_hash: article.body_hash.clone(),
                                session: session.id.clone(),
                            },
                            cx,
                        );
                    }
                })),
        );
        let mut feedback = div().flex().flex_wrap().gap_2();
        for (id, label, value) in [
            ("feedback-easy", "太简单", Difficulty::TooEasy),
            ("feedback-right", "刚刚好", Difficulty::JustRight),
            ("feedback-hard", "有点难", Difficulty::TooHard),
        ] {
            feedback = feedback.child(
                material_button(id, ButtonVariant::Tonal)
                    .child(label)
                    .disabled(self.loading)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(article) = &this.article {
                            this.request(
                                Command::Feedback {
                                    article: article.id.clone(),
                                    body_hash: article.body_hash.clone(),
                                    value,
                                    mode: FeedbackMode::Reading,
                                    operation: this.operation("feedback"),
                                },
                                cx,
                            );
                        }
                    })),
            );
        }
        body = body.child(feedback);
        if let Some((context, _, meaning)) = &self.lookup {
            let mut sheet = MaterialSideSheet::new("word-sheet")
                .open(true)
                .modal(true)
                .headline(context.selection.surface.clone())
                .child(format!("词元：{}", context.selection.lemma))
                .child(context.selection.sentence.clone())
                .child(if meaning.is_empty() {
                    "暂无本地释义，仍可标记".to_owned()
                } else {
                    meaning.clone()
                })
                .child("发音暂不可用");
            for (id, label, action) in [
                ("mark-known", "认识", MarkAction::Known),
                ("mark-unknown", "不认识", MarkAction::Unknown),
                ("add-review", "加入复习", MarkAction::AddToReview),
            ] {
                sheet = sheet.child(
                    material_button(id, ButtonVariant::Tonal)
                        .child(label)
                        .disabled(self.loading)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some((context, revision, _)) = &this.lookup {
                                this.request(
                                    Command::Mark {
                                        article: context.article_id.clone(),
                                        body_hash: context.body_hash.clone(),
                                        range: context.selection.range,
                                        action,
                                        revision: *revision,
                                        operation: this.operation("mark"),
                                    },
                                    cx,
                                );
                            }
                        })),
                );
            }
            let weak = cx.entity().downgrade();
            body = body.child(sheet.on_dismiss(move |_, cx| {
                let _ = weak.update(cx, |this, cx| {
                    this.lookup = None;
                    cx.notify();
                });
            }));
        }
        body.into_any_element()
    }
    pub(super) fn inbox_body(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let mut body = div()
            .flex()
            .flex_col()
            .gap_4()
            .child(format!("待整理 {} 个词", self.inbox_total));
        if self.inbox.is_empty() {
            return body
                .child("读文章时把不熟悉的词留在这里。")
                .child(
                    material_button("inbox-articles", ButtonVariant::Tonal)
                        .child("去文章")
                        .on_click(cx.listener(|this, _, _, cx| this.navigate(Route::Articles, cx))),
                )
                .into_any_element();
        }
        body = body.child(
            material_button("inbox-select-page", ButtonVariant::Text)
                .child("全选本页 / 清空选择")
                .disabled(self.loading)
                .on_click(cx.listener(|this, _, _, cx| {
                    if this.inbox_selected.len() == this.inbox.len() {
                        this.inbox_selected.clear();
                    } else {
                        this.inbox_selected = this.inbox.iter().map(|i| i.id.clone()).collect();
                    }
                    cx.notify();
                })),
        );
        if !self.inbox_selected.is_empty() {
            for (id, label, action) in [
                ("inbox-add", "加入复习", InboxAction::Add),
                ("inbox-ignore", "忽略", InboxAction::Ignore),
            ] {
                body = body.child(
                    material_button(id, ButtonVariant::Filled)
                        .child(format!("{label}（{}）", self.inbox_selected.len()))
                        .disabled(self.loading)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let selections = this
                                .inbox
                                .iter()
                                .filter(|i| this.inbox_selected.contains(&i.id))
                                .map(|i| (i.id.clone(), i.revision))
                                .collect();
                            this.request(
                                Command::BatchInbox {
                                    action,
                                    selections,
                                    operation: this.operation("inbox"),
                                },
                                cx,
                            );
                        })),
                );
            }
        }
        for item in &self.inbox {
            let id = item.id.clone();
            let selected = self.inbox_selected.contains(&id);
            let mut row = div()
                .p_4()
                .rounded(px(16.))
                .bg(color(ColorRole::SURFACE_CONTAINER_LOWEST))
                .child(
                    material_button(
                        gpui_kit::ElementId::Name(format!("select-inbox-{id}").into()),
                        if selected {
                            ButtonVariant::Filled
                        } else {
                            ButtonVariant::Tonal
                        },
                    )
                    .child(format!(
                        "{} {}",
                        if selected { "✓" } else { "○" },
                        item.lemma
                    ))
                    .disabled(self.loading)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.inbox_selected.remove(&id) {
                            this.inbox_selected.insert(id.clone());
                        }
                        cx.notify();
                    })),
                )
                .child(item.context.clone())
                .child(if item.meaning.is_empty() {
                    "暂无释义".into()
                } else {
                    item.meaning.clone()
                });
            if let Some(article_id) = &item.article_id {
                let id = article_id.clone();
                row = row.child(
                    material_button(
                        gpui_kit::ElementId::Name(format!("source-{}", item.id).into()),
                        ButtonVariant::Text,
                    )
                    .child(
                        item.article_title
                            .clone()
                            .unwrap_or("来源文章不可用".into()),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.request(Command::OpenArticle { id: id.clone() }, cx)
                    })),
                );
            } else {
                row = row.child("来源文章不可用");
            }
            body = body.child(row);
        }
        body.into_any_element()
    }
}
