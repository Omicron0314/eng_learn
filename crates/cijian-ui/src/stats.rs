#[cfg(test)]
mod tests;
use super::*;
use crate::design::Icon;
use chrono::{Datelike, NaiveDate, Timelike};
use cijian_domain::srs::State;
use cijian_ports::stats::StatsSnapshot;

fn local_now(settings: &cijian_domain::settings::Settings) -> chrono::DateTime<chrono_tz::Tz> {
    let zone: chrono_tz::Tz = settings
        .timezone
        .parse()
        .unwrap_or(chrono_tz::Asia::Shanghai);
    chrono::Utc::now().with_timezone(&zone)
}

pub(super) fn study_date(settings: &cijian_domain::settings::Settings) -> NaiveDate {
    let now = local_now(settings);
    let mut date = now.date_naive();
    if now.hour() < settings.day_start_hour {
        date -= chrono::Duration::days(1);
    }
    date
}

fn greeting(hour: u32) -> &'static str {
    match hour {
        5..=10 => "早上好",
        11..=12 => "中午好",
        13..=17 => "下午好",
        _ => "晚上好",
    }
}

fn weekday_label(date: NaiveDate) -> &'static str {
    match date.weekday().num_days_from_monday() {
        0 => "星期一",
        1 => "星期二",
        2 => "星期三",
        3 => "星期四",
        4 => "星期五",
        5 => "星期六",
        _ => "星期日",
    }
}

fn calendar_label(date: NaiveDate) -> String {
    format!(
        "{}月{}日 · {}",
        date.month(),
        date.day(),
        weekday_label(date)
    )
}

fn review_counts(cards: &[cijian_ports::library::CardView]) -> (u64, u64, u64) {
    let mut learning = 0;
    let mut review = 0;
    let mut new = 0;
    for card in cards {
        match card.srs.state {
            State::Learning | State::Relearning => learning += 1,
            State::Review => review += 1,
            State::New => new += 1,
        }
    }
    (learning, review, new)
}

fn stat_number(value: impl Into<String>, size: f32, line: f32) -> gpui_kit::Div {
    design::disp(size, line).child(value.into())
}

fn heat_level(count: u64, max: u64) -> u8 {
    if count == 0 || max == 0 {
        0
    } else if count * 4 <= max {
        1
    } else if count * 2 <= max {
        2
    } else if count * 4 <= max * 3 {
        3
    } else {
        4
    }
}

impl Shell {
    fn inbox_preview(&self, cx: &mut Context<Self>, limit: usize) -> Vec<gpui_kit::AnyElement> {
        self.inbox
            .iter()
            .take(limit)
            .map(|item| {
                let context = item.context.clone();
                let article = item.article_id.clone();
                let reveal = article
                    .clone()
                    .zip(item.range.zip(item.body_hash.clone()))
                    .map(|(article, (range, hash))| (article, hash, range.start));
                div()
                    .id(gpui_kit::SharedString::from(format!(
                        "today-inbox-{}",
                        item.id
                    )))
                    .role(gpui_kit::Role::Button)
                    .aria_label(format!("{}：{}", item.lemma, item.meaning))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .p(px(10.))
                    .rounded(px(20.))
                    .cursor_pointer()
                    .hover(|style| style.bg(design::c(ColorRole::SURFACE_CONTAINER_LOWEST)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(article) = article.clone() {
                            this.source_reveal = reveal.clone();
                            this.request(Command::OpenArticle { id: article }, cx);
                        } else {
                            this.navigate(Route::Inbox, cx);
                        }
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(
                                design::text(16., 22., 650.)
                                    .text_color(design::c(ColorRole::ON_SURFACE))
                                    .child(item.lemma.clone()),
                            )
                            .child(design::muted_text(13., 18.).text_ellipsis().child(
                                if item.meaning.is_empty() {
                                    context.clone()
                                } else {
                                    item.meaning.clone()
                                },
                            )),
                    )
                    .child(design::muted_text(12., 16.).flex_shrink_0().child("待整理"))
                    .into_any_element()
            })
            .collect()
    }

    fn heatmap_cells(
        &self,
        stats: &StatsSnapshot,
        cell: f32,
        gap: f32,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let today = stats
            .heatmap
            .iter()
            .rposition(|day| !day.future)
            .unwrap_or(0);
        let selected = self.heatmap_day.unwrap_or(today).min(today);
        {
            let mut handles = self.heatmap_focus.borrow_mut();
            if handles.len() != stats.heatmap.len() {
                *handles = stats.heatmap.iter().map(|_| cx.focus_handle()).collect();
            }
        }
        let max = stats.heatmap.iter().map(|day| day.count).max().unwrap_or(0);
        let mut weeks = div().flex().gap(px(gap));
        for (week_index, week) in stats.heatmap.chunks(7).enumerate() {
            let mut column = div().flex().flex_col().gap(px(gap));
            for (offset, day) in week.iter().enumerate() {
                let index = week_index * 7 + offset;
                let level = heat_level(day.count, max);
                let role = match level {
                    0 => ColorRole::SURFACE_CONTAINER_HIGH,
                    1 => ColorRole::SECONDARY_CONTAINER,
                    2 => ColorRole::TERTIARY_CONTAINER,
                    3 => ColorRole::PRIMARY_CONTAINER,
                    _ => ColorRole::PRIMARY,
                };
                let label = format!(
                    "{}：{} 次活动{}",
                    day.date,
                    day.count,
                    if day.future {
                        "，未来日期不可选择"
                    } else {
                        ""
                    }
                );
                let mut background = design::c(role);
                if day.future {
                    background = background.opacity(0.32);
                }
                let mut square = div().w(px(cell)).h(px(cell)).rounded(px(4.)).bg(background);
                if index == selected && !day.future {
                    square = square
                        .border_2()
                        .border_color(design::c(ColorRole::PRIMARY));
                }
                let mut target = div()
                    .id(gpui_kit::SharedString::from(format!("activity-{index}")))
                    .test_support()
                    .role(gpui_kit::Role::Button)
                    .aria_label(label)
                    .w(px(cell))
                    .h(px(cell))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer();
                if !day.future {
                    let focus = self.heatmap_focus.borrow()[index].clone();
                    let click_focus = focus.clone();
                    target = target
                        .track_focus(&focus)
                        .tab_stop(index == selected)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.heatmap_day = Some(index);
                            window.focus(&click_focus, cx);
                            cx.notify();
                        }))
                        .on_key_down(cx.listener(
                            move |this, event: &gpui_kit::KeyDownEvent, window, cx| {
                                let next = match event.keystroke.key.as_str() {
                                    "left" => index.checked_sub(1),
                                    "right" => Some((index + 1).min(today)),
                                    "up" => index.checked_sub(7),
                                    "down" => Some((index + 7).min(today)),
                                    "home" => Some(0),
                                    "end" => Some(today),
                                    _ => return,
                                };
                                if let Some(next) = next.filter(|next| *next != index) {
                                    this.heatmap_day = Some(next);
                                    if let Some(focus) =
                                        this.heatmap_focus.borrow().get(next).cloned()
                                    {
                                        window.focus(&focus, cx);
                                    }
                                    cx.notify();
                                }
                                cx.stop_propagation();
                            },
                        ));
                }
                column = column.child(target.child(square));
            }
            weeks = weeks.child(column);
        }
        weeks.into_any_element()
    }

    fn compact_today(&mut self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let date = study_date(&self.settings);
        let hour = local_now(&self.settings).hour();
        let (learning, review, new) = review_counts(&self.review);
        let total = self.review.len() as u64;
        let stats = self.stats.clone();
        let mut page = div()
            .id("today-page")
            .track_scroll(&self.page_scroll)
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .w_full()
            .gap(px(14.))
            .p(px(16.))
            .overflow_y_scroll();
        page = page.child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .px(px(4.))
                .child(
                    design::text(13., 18., 600.)
                        .text_color(design::c(ColorRole::PRIMARY))
                        .child(calendar_label(date)),
                )
                .child(
                    design::title(36., 42., 750.)
                        .font_family("Cijian Display")
                        .child(greeting(hour)),
                ),
        );
        let mut hero = design::shaped(
            [28., 28., 28., 28.],
            design::c(ColorRole::PRIMARY_CONTAINER),
        )
        .p(px(20.))
        .gap(px(12.))
        .flex()
        .flex_col()
        .text_color(design::c(ColorRole::ON_PRIMARY_CONTAINER));
        hero = hero
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(design::text(16., 22., 650.).flex_1().child("今日复习"))
                    .child(
                        design::text(13., 18., 400.)
                            .child(format!("学习中 {learning} · 待复习 {review} · 新词 {new}")),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(px(10.))
                    .child(design::title(56., 62., 750.).child(total.to_string()))
                    .child(design::text(15., 20., 400.).child("张卡片等待复习")),
            )
            .child(
                div()
                    .id("start-review")
                    .role(gpui_kit::Role::Button)
                    .aria_label("开始复习")
                    .cursor_pointer()
                    .w_full()
                    .h(px(56.))
                    .rounded(px(28.))
                    .bg(design::c(ColorRole::PRIMARY))
                    .text_color(design::c(ColorRole::ON_PRIMARY))
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(8.))
                    .text_size(px(16.))
                    .font_weight(gpui_kit::FontWeight(700.))
                    .when(self.loading || total == 0, |button| button.opacity(0.38))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if !this.loading && !this.review.is_empty() {
                            this.navigate(Route::Review, cx);
                        }
                    }))
                    .child(design::icon_role(Icon::Play, 20., ColorRole::ON_PRIMARY))
                    .child("开始复习"),
            );
        page = page.child(hero);
        page = page.child(
            div()
                .id("today-inbox-open")
                .role(gpui_kit::Role::Button)
                .aria_label("整理生词箱")
                .cursor_pointer()
                .rounded(px(24.))
                .bg(design::c(ColorRole::SURFACE_CONTAINER_LOWEST))
                .p(px(16.))
                .flex()
                .items_center()
                .gap(px(12.))
                .on_click(cx.listener(|this, _, _, cx| this.navigate(Route::Inbox, cx)))
                .child(
                    div()
                        .w(px(44.))
                        .h(px(44.))
                        .rounded(px(14.))
                        .bg(design::c(ColorRole::TERTIARY_CONTAINER))
                        .text_color(design::c(ColorRole::ON_TERTIARY_CONTAINER))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(design::icon_role(
                            Icon::Inbox,
                            24.,
                            ColorRole::ON_TERTIARY_CONTAINER,
                        )),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .child(design::text(16., 22., 650.).child("整理生词箱"))
                        .child(
                            design::muted_text(13., 18.)
                                .child(format!("{} 个生词，等你决定", self.inbox_total)),
                        ),
                )
                .child(design::icon_role(
                    Icon::Chev,
                    18.,
                    ColorRole::ON_SURFACE_VARIANT,
                )),
        );
        if let Some(stats) = stats.as_ref() {
            let today_index = stats
                .heatmap
                .iter()
                .rposition(|day| !day.future)
                .unwrap_or(0);
            let mut card = design::card()
                .rounded(px(24.))
                .p(px(18.))
                .gap(px(10.))
                .flex()
                .flex_col();
            card = card.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(design::text(16., 22., 650.).flex_1().child("学习热力图"))
                    .child(
                        design::tag(
                            ColorRole::SECONDARY_CONTAINER,
                            ColorRole::ON_SECONDARY_CONTAINER,
                        )
                        .child("近 13 周"),
                    ),
            );
            let mut month_row = div().flex().justify_between().w_full();
            for month in ["8 月", "9 月", "10 月", "11 月"] {
                month_row = month_row.child(design::muted_text(12., 16.).child(month));
            }
            card = card.child(month_row);
            card = card.child(self.heatmap_cells(stats, 19., 3., cx));
            let active = stats.heatmap.iter().filter(|day| day.count > 0).count();
            card = card.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .child(
                        design::muted_text(12., 16.)
                            .flex_1()
                            .child(format!("{active} 天有学习记录")),
                    )
                    .child(design::muted_text(12., 16.).child("少"))
                    .children((0..5).map(|level| {
                        let role = match level {
                            0 => ColorRole::SURFACE_CONTAINER_HIGH,
                            1 => ColorRole::SECONDARY_CONTAINER,
                            2 => ColorRole::TERTIARY_CONTAINER,
                            3 => ColorRole::PRIMARY_CONTAINER,
                            _ => ColorRole::PRIMARY,
                        };
                        div()
                            .w(px(12.))
                            .h(px(12.))
                            .rounded(px(4.))
                            .bg(design::c(role))
                    }))
                    .child(design::muted_text(12., 16.).child("多")),
            );
            if let Some(day) = stats
                .heatmap
                .get(self.heatmap_day.unwrap_or(today_index).min(today_index))
            {
                card = card.child(
                    design::muted_text(12., 16.)
                        .child(format!("{}：{} 次学习活动", day.date, day.count)),
                );
            }
            page = page.child(card);
        }
        page.into_any_element()
    }

    pub(super) fn today_body(&mut self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        if self.compact {
            return self.compact_today(cx);
        }
        let wide = self.viewport_width >= 1280.;
        let date = study_date(&self.settings);
        let hour = local_now(&self.settings).hour();
        let (learning, review, new) = review_counts(&self.review);
        let total = self.review.len() as u64;
        let stats = self.stats.clone();
        let books = self.books.clone();
        let inbox_preview = self.inbox_preview(cx, 2);
        let mut pane_content = div()
            .id("today-page")
            .track_scroll(&self.page_scroll)
            .flex()
            .flex_col()
            .gap(px(22.))
            .overflow_y_scroll()
            .flex_1()
            .min_h_0();

        pane_content = pane_content.child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(
                    design::text(14., 20., 600.)
                        .text_color(design::c(ColorRole::PRIMARY))
                        .child(calendar_label(date)),
                )
                .child(design::disp(52., 58.).child(greeting(hour)))
                .child(
                    design::muted_text(16., 22.).child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(px(4.))
                            .child("一点复习，一段故事。今天还有")
                            .child(
                                design::text(16., 22., 650.)
                                    .text_color(design::c(ColorRole::ON_SURFACE))
                                    .child(total.to_string()),
                            )
                            .child("张卡片在等你。"),
                    ),
                ),
        );

        let mut hero = design::shaped(
            [32., 32., 32., 32.],
            design::c(ColorRole::PRIMARY_CONTAINER),
        )
        .flex()
        .flex_col()
        .h(px(272.))
        .gap(px(20.))
        .p(px(26.))
        .px(px(28.))
        .text_color(design::c(ColorRole::ON_PRIMARY_CONTAINER));
        hero = hero.child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(design::icon_role(
                    Icon::Cards,
                    24.,
                    ColorRole::ON_PRIMARY_CONTAINER,
                ))
                .child(design::title(20., 28., 700.).child("今日复习"))
                .child(div().flex_1())
                .child(
                    div()
                        .h(px(24.))
                        .px(px(10.))
                        .rounded(px(12.))
                        .bg(design::c(ColorRole::SURFACE_CONTAINER_LOWEST))
                        .text_color(design::c(ColorRole::ON_PRIMARY_CONTAINER))
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .font_weight(gpui_kit::FontWeight(650.))
                        .flex()
                        .items_center()
                        .child("按自己的节奏"),
                ),
        );
        let mut figures = div().flex().items_center().gap(px(28.)).child(
            div()
                .flex()
                .items_baseline()
                .gap(px(10.))
                .flex_1()
                .child(stat_number(total.to_string(), 76., 84.))
                .child(design::text(16., 22., 400.).child("张卡片等待复习")),
        );
        for (value, label) in [(learning, "学习中"), (review, "待复习"), (new, "新词")] {
            figures = figures.child(
                div()
                    .w(px(96.))
                    .rounded(px(20.))
                    .p(px(14.))
                    .px(px(18.))
                    .bg(design::glass(self.dark))
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(design::text(28., 34., 700.).child(value.to_string()))
                    .child(design::text(13., 18., 400.).child(label)),
            );
        }
        hero = hero.child(figures);
        hero = hero.child(
            div()
                .flex()
                .items_center()
                .child(
                    div()
                        .id("start-review")
                        .role(gpui_kit::Role::Button)
                        .aria_label("开始复习")
                        .cursor_pointer()
                        .h(px(64.))
                        .px(px(32.))
                        .rounded(px(32.))
                        .bg(design::c(ColorRole::PRIMARY))
                        .text_color(design::c(ColorRole::ON_PRIMARY))
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .text_size(px(17.))
                        .line_height(px(22.))
                        .font_weight(gpui_kit::FontWeight(700.))
                        .when(self.loading || total == 0, |button| button.opacity(0.38))
                        .on_click(cx.listener(|this, _, _, cx| {
                            if !this.loading && !this.review.is_empty() {
                                this.navigate(Route::Review, cx);
                            }
                        }))
                        .child(design::icon_role(Icon::Play, 22., ColorRole::ON_PRIMARY))
                        .child("开始复习"),
                )
                .child(div().flex_1())
                .child(
                    design::muted_text(13., 18.)
                        .text_color(design::alpha(
                            design::c(ColorRole::ON_PRIMARY_CONTAINER),
                            0.8,
                        ))
                        .child("空格翻面 · 数字 1–4 评分"),
                ),
        );
        pane_content = pane_content.child(hero);

        let stats_today = stats
            .as_ref()
            .map(|stats| stats.today.clone())
            .unwrap_or_default();
        let stats_week = stats
            .as_ref()
            .map(|stats| stats.week.clone())
            .unwrap_or_default();
        let mut metrics = div().flex().gap(px(12.));
        for (label, value, sub) in [
            (
                "今日新学",
                stats_today.new_cards,
                format!("本周 {} 个", stats_week.new_cards),
            ),
            (
                "今日复习",
                stats_today.reviews,
                format!("本周 {} 次", stats_week.reviews),
            ),
            (
                "待整理生词",
                self.inbox_total,
                "从生词箱加入复习".to_owned(),
            ),
        ] {
            metrics = metrics.child(
                design::card()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .h(px(117.))
                    .p(px(16.))
                    .px(px(20.))
                    .child(design::muted_text(13., 18.).child(label))
                    .child(design::disp(32., 45.).child(value.to_string()))
                    .child(design::muted_text(12., 16.).child(sub)),
            );
        }
        pane_content = pane_content.child(metrics);

        let mut books_area = div().flex().flex_col().gap(px(14.)).flex_1().min_h_0();
        books_area = books_area.child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(design::title(24., 32., 700.).child("我的词书"))
                .child(div().flex_1())
                .child(
                    div()
                        .id("open-library")
                        .role(gpui_kit::Role::Button)
                        .aria_label("管理词库")
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
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.navigate(
                                Route::Library {
                                    book: None,
                                    search: String::new(),
                                },
                                cx,
                            )
                        }))
                        .child("管理词库")
                        .child(design::icon_role(
                            Icon::Chev,
                            18.,
                            ColorRole::ON_SURFACE_VARIANT,
                        )),
                ),
        );
        let mut book_row = div().flex().gap(px(12.)).flex_1().min_h(px(157.));
        if books.is_empty() {
            book_row = book_row.child(design::empty_state("导入词表后，词书会出现在这里"));
        } else {
            for (index, book) in books.iter().take(3).enumerate() {
                let id = book.id.clone();
                let (bg, fg) = match index % 3 {
                    0 => (
                        ColorRole::PRIMARY_CONTAINER,
                        ColorRole::ON_PRIMARY_CONTAINER,
                    ),
                    1 => (
                        ColorRole::SECONDARY_CONTAINER,
                        ColorRole::ON_SECONDARY_CONTAINER,
                    ),
                    _ => (
                        ColorRole::TERTIARY_CONTAINER,
                        ColorRole::ON_TERTIARY_CONTAINER,
                    ),
                };
                book_row = book_row.child(
                    div()
                        .id(gpui_kit::SharedString::from(format!("today-book-{id}")))
                        .role(gpui_kit::Role::Button)
                        .aria_label(book.name.clone())
                        .flex_1()
                        .min_w_0()
                        .cursor_pointer()
                        .rounded(px(24.))
                        .bg(design::c(ColorRole::SURFACE_CONTAINER_LOWEST))
                        .shadow(design::shadow_card())
                        .p(px(18.))
                        .px(px(20.))
                        .flex()
                        .flex_col()
                        .gap(px(10.))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.navigate(
                                Route::Library {
                                    book: Some(id.clone()),
                                    search: String::new(),
                                },
                                cx,
                            )
                        }))
                        .child(
                            div()
                                .w(px(40.))
                                .h(px(40.))
                                .rounded(px(14.))
                                .bg(design::c(bg))
                                .text_color(design::c(fg))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(design::icon_role(Icon::Book, 18., fg)),
                        )
                        .child(design::text(16., 22., 650.).child(book.name.clone()))
                        .child(design::muted_text(13., 18.).child(format!(
                            "{} {}",
                            book.entries,
                            if book.kind == cijian_domain::import::BookKind::Baseline {
                                "词条"
                            } else {
                                "张词卡"
                            }
                        ))),
                );
            }
        }
        books_area = books_area.child(book_row);
        pane_content = pane_content.child(books_area);

        let pane = design::pane()
            .flex_1()
            .min_w_0()
            .p(px(28.))
            .px(px(32.))
            .gap(px(22.))
            .child(pane_content);

        let mut main = div().flex().flex_1().min_w_0().h_full().gap(px(12.));
        main = main.child(pane);
        if wide {
            let mut aside = div()
                .w(px(340.))
                .flex_shrink_0()
                .h_full()
                .flex()
                .flex_col()
                .gap(px(12.));
            let mut inbox = design::pane()
                .h(px(260.))
                .flex_shrink_0()
                .p(px(22.))
                .px(px(20.))
                .gap(px(8.));
            inbox = inbox.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(4.))
                    .pb(px(6.))
                    .child(design::title(20., 28., 700.).flex_1().child("生词箱"))
                    .child(
                        design::tag(
                            ColorRole::TERTIARY_CONTAINER,
                            ColorRole::ON_TERTIARY_CONTAINER,
                        )
                        .child(format!("{} 待整理", self.inbox_total)),
                    ),
            );
            if inbox_preview.is_empty() {
                inbox = inbox.child(
                    design::muted_text(13., 18.)
                        .p(px(10.))
                        .child("阅读时标记的生词会出现在这里"),
                );
            } else {
                for (index, row) in inbox_preview.into_iter().enumerate() {
                    inbox = inbox.child(
                        div()
                            .rounded(px(20.))
                            .when(index == 0, |row| {
                                row.bg(design::c(ColorRole::SURFACE_CONTAINER_LOWEST))
                            })
                            .child(row),
                    );
                }
            }
            inbox = inbox.child(
                div().mt(px(6.)).ml(px(4.)).w(px(105.)).child(
                    div()
                        .id("today-inbox-open")
                        .role(gpui_kit::Role::Button)
                        .aria_label("去整理生词箱")
                        .cursor_pointer()
                        .h(px(40.))
                        .px(px(20.))
                        .rounded(px(20.))
                        .bg(design::c(ColorRole::SECONDARY_CONTAINER))
                        .text_color(design::c(ColorRole::ON_SECONDARY_CONTAINER))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .text_size(px(14.))
                        .font_weight(gpui_kit::FontWeight(650.))
                        .on_click(cx.listener(|this, _, _, cx| this.navigate(Route::Inbox, cx)))
                        .child("去整理")
                        .child(design::icon_role(
                            Icon::Chev,
                            18.,
                            ColorRole::ON_SURFACE_VARIANT,
                        )),
                ),
            );
            aside = aside.child(inbox);

            let mut rhythm = design::shaped(
                [28., 28., 28., 28.],
                design::c(ColorRole::SECONDARY_CONTAINER),
            )
            .h(px(220.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap(px(14.))
            .p(px(22.))
            .px(px(24.))
            .text_color(design::c(ColorRole::ON_SECONDARY_CONTAINER));
            let active_days = stats
                .as_ref()
                .map(|stats| {
                    stats
                        .heatmap
                        .iter()
                        .rev()
                        .take(7)
                        .filter(|day| !day.future && day.count > 0)
                        .count()
                })
                .unwrap_or(0);
            rhythm = rhythm.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(design::icon_role(
                        Icon::Chart,
                        24.,
                        ColorRole::ON_SECONDARY_CONTAINER,
                    ))
                    .child(design::title(20., 28., 700.).child("本周节奏"))
                    .child(div().flex_1())
                    .child(design::text(13., 18., 650.).child(format!("{active_days} / 5 天"))),
            );
            let mut days = div().flex().items_center().justify_between().w_full();
            let today_index = stats
                .as_ref()
                .and_then(|stats| stats.heatmap.iter().rposition(|day| !day.future))
                .unwrap_or(0);
            for (offset, day) in stats
                .as_ref()
                .map(|stats| {
                    let len = stats.heatmap.len();
                    stats.heatmap[len.saturating_sub(7)..].to_vec()
                })
                .unwrap_or_default()
                .iter()
                .enumerate()
            {
                let index = today_index.saturating_sub(6) + offset;
                let is_today = index == today_index;
                let complete = day.count > 0;
                let mut shape = div()
                    .w(px(34.))
                    .h(px(34.))
                    .flex()
                    .items_center()
                    .justify_center();
                if is_today {
                    shape = shape
                        .rounded(px(17.))
                        .bg(design::c(ColorRole::SURFACE_CONTAINER_LOWEST))
                        .text_color(design::c(ColorRole::PRIMARY))
                        .border_2()
                        .border_color(design::c(ColorRole::PRIMARY))
                        .text_size(px(12.))
                        .font_weight(gpui_kit::FontWeight(800.))
                        .child("今");
                } else if complete {
                    shape = shape
                        .rounded(px(12.))
                        .bg(design::c(ColorRole::SECONDARY))
                        .text_color(design::c(ColorRole::SURFACE_CONTAINER_LOW))
                        .child(design::icon_role(
                            Icon::Check,
                            16.,
                            ColorRole::SURFACE_CONTAINER_LOWEST,
                        ));
                } else {
                    shape = shape
                        .rounded(px(12.))
                        .border_1()
                        .border_dashed()
                        .border_color(design::c(ColorRole::OUTLINE));
                }
                let label = ["一", "二", "三", "四", "五", "六", "日"]
                    .get(
                        day.date
                            .parse::<NaiveDate>()
                            .ok()
                            .map(|date| date.weekday().num_days_from_monday() as usize)
                            .unwrap_or(offset),
                    )
                    .copied()
                    .unwrap_or("");
                days = days.child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(6.))
                        .child(shape)
                        .child(
                            design::text(12., 16., if is_today { 650. } else { 400. }).child(label),
                        ),
                );
            }
            rhythm = rhythm.child(days);
            let week_new = stats
                .as_ref()
                .map(|stats| stats.week.new_cards)
                .unwrap_or(0);
            let week_reviews = stats.as_ref().map(|stats| stats.week.reviews).unwrap_or(0);
            rhythm = rhythm.child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .rounded(px(16.))
                            .bg(design::c(ColorRole::SURFACE_CONTAINER_LOWEST))
                            .text_color(design::c(ColorRole::ON_SURFACE))
                            .p(px(10.))
                            .px(px(14.))
                            .flex()
                            .flex_col()
                            .child(design::muted_text(12., 16.).child("本周新学"))
                            .child(design::title(22., 28., 750.).child(week_new.to_string())),
                    )
                    .child(
                        div()
                            .flex_1()
                            .rounded(px(16.))
                            .bg(design::c(ColorRole::SURFACE_CONTAINER_LOWEST))
                            .text_color(design::c(ColorRole::ON_SURFACE))
                            .p(px(10.))
                            .px(px(14.))
                            .flex()
                            .flex_col()
                            .child(design::muted_text(12., 16.).child("本周复习"))
                            .child(design::title(22., 28., 750.).child(week_reviews.to_string())),
                    ),
            );
            aside = aside.child(rhythm);

            let mut heat = design::pane().flex_1().min_h_0().p(px(22.)).gap(px(14.));
            heat = heat.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(design::title(20., 28., 700.).flex_1().child("学习热力图"))
                    .child(
                        design::tag(
                            ColorRole::SECONDARY_CONTAINER,
                            ColorRole::ON_SECONDARY_CONTAINER,
                        )
                        .child("近 13 周"),
                    ),
            );
            if let Some(stats) = stats.as_ref() {
                let mut months: Vec<String> = Vec::new();
                for day in &stats.heatmap {
                    if let Ok(date) = day.date.parse::<NaiveDate>()
                        && months
                            .last()
                            .map(|last| last != &format!("{}月", date.month()))
                            != Some(false)
                    {
                        months.push(format!("{}月", date.month()));
                    }
                }
                months.dedup();
                let mut month_row = div().flex().justify_between().w_full();
                for month in months.iter().take(4) {
                    month_row = month_row.child(design::muted_text(12., 16.).child(month.clone()));
                }
                heat = heat.child(month_row);
                let active = stats.heatmap.iter().filter(|day| day.count > 0).count();
                heat = heat.child(self.heatmap_cells(stats, 18., 4., cx));
                heat = heat.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(5.))
                        .child(
                            design::muted_text(12., 16.)
                                .flex_1()
                                .child(format!("{active} 天有学习记录")),
                        )
                        .child(design::muted_text(12., 16.).child("少"))
                        .children((0..5).map(|level| {
                            let role = match level {
                                0 => ColorRole::SURFACE_CONTAINER_HIGH,
                                1 => ColorRole::SECONDARY_CONTAINER,
                                2 => ColorRole::TERTIARY_CONTAINER,
                                3 => ColorRole::PRIMARY_CONTAINER,
                                _ => ColorRole::PRIMARY,
                            };
                            div()
                                .w(px(12.))
                                .h(px(12.))
                                .rounded(px(4.))
                                .bg(design::c(role))
                        }))
                        .child(design::muted_text(12., 16.).child("多")),
                );
                if let Some(day) = stats
                    .heatmap
                    .get(self.heatmap_day.unwrap_or(today_index).min(today_index))
                {
                    heat = heat.child(
                        design::muted_text(12., 16.)
                            .child(format!("{}：{} 次学习活动", day.date, day.count)),
                    );
                }
                heat = heat.child(
                    design::muted_text(12., 16.).child("色阶表示每日学习活动次数。悬停查看详情。"),
                );
            } else {
                heat = heat.child(design::muted_text(13., 18.).child("正在读取学习记录…"));
            }
            aside = aside.child(heat);
            main = main.child(aside);
        }
        main.into_any_element()
    }
}

impl Shell {
    fn percent_or(value: Option<f64>, fallback: &'static str) -> String {
        value
            .map(|value| format!("{:.0}%", value * 100.))
            .unwrap_or_else(|| fallback.into())
    }

    fn stats_card_number(value: impl Into<String>, size: f32, line: f32) -> gpui_kit::Div {
        design::disp(size, line).child(value.into())
    }

    pub(super) fn load_stats(&mut self, page: u32, cx: &mut Context<Self>) {
        self.stats_page = page;
        self.request(Command::LoadStats { page }, cx);
    }
    pub(super) fn stats_body(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let Some(stats) = &self.stats else {
            return design::pane()
                .flex_1()
                .h_full()
                .p(px(28.))
                .child(design::muted_text(16., 24.).child("正在读取学习记录…"))
                .into_any_element();
        };
        let mut body = design::pane()
            .id("stats-page")
            .track_scroll(&self.page_scroll)
            .flex_1()
            .min_w_0()
            .h_full()
            .p(px(28.))
            .px(px(if self.compact { 20. } else { 32. }))
            .gap(px(20.))
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
                        .gap(px(4.))
                        .child(design::title(40., 48., 750.).child("看见积累"))
                        .child(
                            design::muted_text(16., 22.)
                                .child("数字是线索，不是分数。慢慢观察，找到有效的学习方式。"),
                        ),
                )
                .child(design::chip(
                    false,
                    format!(
                        "{} · 每日 {}:00 开始",
                        self.settings.timezone, self.settings.day_start_hour
                    ),
                    Some(Icon::Clock),
                )),
        );

        let completion = stats.reading_completion;
        let just_right = stats.feedback_just_right;
        let mut metrics = div().flex().gap(px(14.));
        metrics = metrics
            .child(
                design::shaped([28., 28., 28., 8.], design::c(ColorRole::PRIMARY_CONTAINER))
                    .flex_1()
                    .min_w_0()
                    .p(px(20.))
                    .px(px(22.))
                    .gap(px(4.))
                    .text_color(design::c(ColorRole::ON_PRIMARY_CONTAINER))
                    .flex()
                    .flex_col()
                    .child(design::text(13., 18., 650.).child("今日新学"))
                    .child(Self::stats_card_number(
                        stats.today.new_cards.to_string(),
                        48.,
                        54.,
                    ))
                    .child(
                        design::text(13., 18., 400.)
                            .opacity(0.8)
                            .child(format!("本周 {} 个", stats.week.new_cards)),
                    ),
            )
            .child(
                design::shaped(
                    [28., 8., 28., 28.],
                    design::c(ColorRole::SECONDARY_CONTAINER),
                )
                .flex_1()
                .min_w_0()
                .p(px(20.))
                .px(px(22.))
                .gap(px(4.))
                .text_color(design::c(ColorRole::ON_SECONDARY_CONTAINER))
                .flex()
                .flex_col()
                .child(design::text(13., 18., 650.).child("今日复习"))
                .child(Self::stats_card_number(
                    stats.today.reviews.to_string(),
                    48.,
                    54.,
                ))
                .child(
                    design::text(13., 18., 400.)
                        .opacity(0.8)
                        .child(format!("本周 {} 次", stats.week.reviews)),
                ),
            );
        let mut reading_card = design::card()
            .flex_1()
            .min_w_0()
            .rounded(px(28.))
            .p(px(20.))
            .px(px(22.))
            .flex()
            .items_center()
            .gap(px(14.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .gap(px(4.))
                    .child(
                        design::muted_text(13., 18.)
                            .font_weight(gpui_kit::FontWeight(650.))
                            .child("阅读完成率"),
                    )
                    .child(Self::stats_card_number(
                        Self::percent_or(completion, "—"),
                        40.,
                        48.,
                    ))
                    .child(design::muted_text(13., 18.).child("目标 ≥60%")),
            );
        if let Some(value) = completion {
            reading_card = reading_card.child(design::ring(
                value as f32,
                76.,
                if value >= 0.6 {
                    ColorRole::SUCCESS
                } else {
                    ColorRole::WARNING
                },
            ));
        }
        metrics = metrics.child(reading_card);
        let mut feedback_card = design::card()
            .flex_1()
            .min_w_0()
            .rounded(px(28.))
            .p(px(20.))
            .px(px(22.))
            .flex()
            .items_center()
            .gap(px(14.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .gap(px(4.))
                    .child(
                        design::muted_text(13., 18.)
                            .font_weight(gpui_kit::FontWeight(650.))
                            .child("难度「刚好」"),
                    )
                    .child(Self::stats_card_number(
                        Self::percent_or(just_right, "—"),
                        40.,
                        48.,
                    ))
                    .child(
                        design::text(13., 18., 400.)
                            .text_color(design::c(
                                if just_right.is_some_and(|value| value >= 0.6) {
                                    ColorRole::SUCCESS
                                } else {
                                    ColorRole::WARNING
                                },
                            ))
                            .child(if just_right.is_some_and(|value| value >= 0.6) {
                                "目标 ≥60%"
                            } else {
                                "目标 ≥60% · 差一点"
                            }),
                    ),
            );
        if let Some(value) = just_right {
            feedback_card = feedback_card.child(design::ring(
                value as f32,
                76.,
                if value >= 0.6 {
                    ColorRole::SUCCESS
                } else {
                    ColorRole::WARNING
                },
            ));
        }
        metrics = metrics.child(feedback_card);
        body = body.child(metrics);

        let mut trend_row = div().flex().gap(px(14.)).items_stretch();
        let mut trend = design::card()
            .flex_grow(1.7)
            .flex_basis(px(0.))
            .min_w_0()
            .rounded(px(28.))
            .p(px(20.))
            .px(px(22.))
            .flex()
            .flex_col()
            .gap(px(10.));
        trend = trend.child(
            div()
                .flex()
                .items_center()
                .child(
                    design::title(20., 28., 700.)
                        .flex_1()
                        .child("文章覆盖率趋势"),
                )
                .child(
                    design::tag(
                        ColorRole::SECONDARY_CONTAINER,
                        ColorRole::ON_SECONDARY_CONTAINER,
                    )
                    .child(format!(
                        "目标区间 {}",
                        match self.settings.tier {
                            cijian_domain::settings::Tier::Easy => "≥98%",
                            cijian_domain::settings::Tier::Medium => "95–98%",
                            cijian_domain::settings::Tier::Hard => "90–95%",
                        }
                    )),
                ),
        );
        if stats.coverage.len() >= 2 {
            let (low, high, band) = match self.settings.tier {
                cijian_domain::settings::Tier::Easy => (0.92, 1.0, (0.98, 1.0)),
                cijian_domain::settings::Tier::Medium => (0.93, 0.99, (0.95, 0.98)),
                cijian_domain::settings::Tier::Hard => (0.88, 0.96, (0.90, 0.95)),
            };
            let values: Vec<f32> = stats
                .coverage
                .iter()
                .rev()
                .take(14)
                .rev()
                .map(|point| {
                    ((point.coverage.unwrap_or(low) - low) / (high - low)).clamp(0., 1.) as f32
                })
                .collect();
            let chart = design::chart(
                values,
                760.,
                200.,
                design::c(ColorRole::PRIMARY),
                Some((
                    ((band.0 - low) / (high - low)) as f32,
                    ((band.1 - low) / (high - low)) as f32,
                )),
                false,
            );
            let mut labels = div()
                .absolute()
                .top(px(20.))
                .left(px(0.))
                .w(px(38.))
                .h(px(160.))
                .flex()
                .flex_col()
                .justify_between()
                .text_color(design::c(ColorRole::ON_SURFACE_VARIANT));
            let (low, high) = match self.settings.tier {
                cijian_domain::settings::Tier::Easy => (0.92, 1.0),
                cijian_domain::settings::Tier::Medium => (0.93, 0.99),
                cijian_domain::settings::Tier::Hard => (0.88, 0.96),
            };
            for index in 0..4 {
                let value = high - (high - low) * index as f64 / 3.;
                labels = labels
                    .child(design::mono(11., 14., 400.).child(format!("{:.0}%", value * 100.)));
            }
            trend = trend.child(chart.child(labels));
            let first = stats.coverage.first().map(|point| point.created_at);
            let last = stats.coverage.last().map(|point| point.created_at);
            let span = match (first, last) {
                (Some(from), Some(to)) => {
                    let from = chrono::DateTime::from_timestamp_millis(from)
                        .map(|value| value.date_naive())
                        .map(|date| format!("{:02}/{:02}", date.month(), date.day()))
                        .unwrap_or_else(|| "—".into());
                    let to = chrono::DateTime::from_timestamp_millis(to)
                        .map(|value| value.date_naive())
                        .map(|date| format!("{:02}/{:02}", date.month(), date.day()))
                        .unwrap_or_else(|| "—".into());
                    format!("{from} → {to}")
                }
                _ => "—".into(),
            };
            trend = trend.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(18.))
                    .text_size(px(13.))
                    .line_height(px(18.))
                    .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                    .child(span)
                    .child(
                        div()
                            .flex()
                            .gap(px(4.))
                            .child("近 7 天未知词占比中位数")
                            .child(
                                design::text(13., 18., 650.)
                                    .text_color(design::c(ColorRole::ON_SURFACE))
                                    .child(Self::percent_or(stats.unknown_median_week, "—")),
                            ),
                    )
                    .child(
                        div().flex().gap(px(4.)).child("全部").child(
                            design::text(13., 18., 650.)
                                .text_color(design::c(ColorRole::ON_SURFACE))
                                .child(Self::percent_or(stats.unknown_median_all, "—")),
                        ),
                    )
                    .child("目标 ≤5%"),
            );
        } else if let Some(point) = stats.coverage.first() {
            let coverage = point
                .coverage
                .map(|value| format!("{:.1}%", value * 100.))
                .unwrap_or_else(|| "暂无".into());
            trend = trend.child(design::text(15., 22., 650.).child(format!(
                "已记录 1 篇：{} · 覆盖率 {}",
                point.title, coverage
            )));
            trend = trend
                .child(design::muted_text(13., 18.).child("再生成一篇后，这里会显示覆盖率趋势。"));
        } else {
            trend = trend
                .child(design::muted_text(13., 18.).child("生成文章后，这里会出现覆盖率趋势。"));
        }
        trend_row = trend_row.child(trend);

        let exposed = &stats.experiment.exposed;
        let control = &stats.experiment.control;
        let mut experiment = design::shaped(
            [28., 28., 28., 64.],
            design::c(ColorRole::TERTIARY_CONTAINER),
        )
        .flex_grow(1.)
        .flex_basis(px(0.))
        .min_w_0()
        .p(px(22.))
        .px(px(24.))
        .gap(px(12.))
        .flex()
        .flex_col()
        .text_color(design::c(ColorRole::ON_TERTIARY_CONTAINER));
        experiment = experiment.child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(design::icon_role(
                    Icon::Beaker,
                    24.,
                    ColorRole::ON_TERTIARY_CONTAINER,
                ))
                .child(design::title(20., 28., 700.).flex_1().child("对照实验"))
                .child(
                    div()
                        .id("experiment-settings")
                        .test_support()
                        .role(gpui_kit::Role::Button)
                        .aria_label("调整实验设置")
                        .cursor_pointer()
                        .h(px(32.))
                        .px(px(10.))
                        .rounded(px(16.))
                        .bg(design::c(ColorRole::SURFACE_CONTAINER_LOWEST))
                        .text_color(design::c(ColorRole::ON_TERTIARY_CONTAINER))
                        .flex()
                        .items_center()
                        .text_size(px(12.))
                        .font_weight(gpui_kit::FontWeight(650.))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.settings_ui.advanced = true;
                            this.settings_ui.focus_advanced = true;
                            this.navigate(Route::Settings, cx);
                        }))
                        .child("调整"),
                ),
        );
        experiment =
            experiment
                .child(design::text(13., 18., 400.).child(
                    "文章中出现的目标词 vs 刻意留出的对照词，之后 7 天内首次复习的 Again 率。",
                ))
                .child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .items_stretch()
                        .child(
                            div()
                                .flex_1()
                                .rounded(px(20.))
                                .bg(design::c(ColorRole::SURFACE_CONTAINER_LOWEST))
                                .text_color(design::c(ColorRole::ON_SURFACE))
                                .p(px(12.))
                                .px(px(14.))
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(
                                    design::muted_text(12., 16.)
                                        .font_weight(gpui_kit::FontWeight(650.))
                                        .child("出现组"),
                                )
                                .child(
                                    design::title(30., 36., 750.)
                                        .child(Self::percent_or(exposed.again_rate, "—")),
                                )
                                .child(design::muted_text(12., 16.).child(format!(
                                    "样本 {} · Again {} · 未复习 {}",
                                    exposed.sample, exposed.again, exposed.unreviewed
                                ))),
                        )
                        .child(
                            div()
                                .flex_1()
                                .rounded(px(20.))
                                .bg(design::glass(self.dark))
                                .p(px(12.))
                                .px(px(14.))
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(design::text(12., 16., 650.).child("对照组"))
                                .child(
                                    design::title(30., 36., 750.)
                                        .child(Self::percent_or(control.again_rate, "—")),
                                )
                                .child(design::text(12., 16., 400.).child(format!(
                                    "样本 {} · Again {} · 未复习 {}",
                                    control.sample, control.again, control.unreviewed
                                ))),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(design::text(15., 22., 400.).child("差值（出现 − 对照）"))
                        .child(div().flex_1())
                        .child(
                            design::title(22., 28., 750.).child(
                                stats
                                    .experiment
                                    .difference_percentage_points
                                    .map(|value| format!("{value:+.1}%"))
                                    .unwrap_or_else(|| "—".into()),
                            ),
                        ),
                )
                .child(
                    design::text(12., 16., 400.)
                        .opacity(0.8)
                        .child("样本小，仅作参考；不代表因果结论。"),
                );
        trend_row = trend_row.child(experiment);
        body = body.child(trend_row);

        let mut lower = div().flex().gap(px(14.)).items_stretch();
        let mut side = design::card()
            .w(px(300.))
            .flex_shrink_0()
            .rounded(px(28.))
            .p(px(20.))
            .gap(px(14.))
            .flex()
            .flex_col();
        side = side.child(
            div()
                .flex()
                .flex_col()
                .child(design::title(20., 28., 700.).child("每周活跃"))
                .child(design::muted_text(13., 18.).child("目标 每周 ≥5 天")),
        );
        let weeks: Vec<_> = stats.heatmap.chunks(7).rev().take(4).collect();
        let mut week_rows = div().flex().flex_col().gap(px(12.));
        for week in weeks.into_iter().rev() {
            let active = week
                .iter()
                .filter(|day| day.count > 0 && !day.future)
                .count();
            let start = week
                .first()
                .and_then(|day| day.date.parse::<NaiveDate>().ok())
                .map(|date| format!("{:02}/{:02}", date.month(), date.day()))
                .unwrap_or_else(|| "—".into());
            let mut bars = div().flex().gap(px(4.)).flex_1();
            for day in week {
                bars = bars.child(
                    div()
                        .flex_1()
                        .h(px(14.))
                        .rounded(px(5.))
                        .bg(design::c(if day.count > 0 {
                            if active >= 5 {
                                ColorRole::SUCCESS
                            } else {
                                ColorRole::PRIMARY
                            }
                        } else {
                            ColorRole::SURFACE_CONTAINER_HIGH
                        }))
                        .when(day.future, |bar| bar.opacity(0.35)),
                );
            }
            week_rows = week_rows.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .text_size(px(13.))
                    .line_height(px(18.))
                    .child(
                        design::mono(13., 18., 400.)
                            .w(px(44.))
                            .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                            .child(start),
                    )
                    .child(bars)
                    .child(
                        div()
                            .w(px(36.))
                            .text_align(gpui_kit::TextAlign::Right)
                            .child(format!("{active} 天")),
                    ),
            );
        }
        side = side.child(week_rows);
        side = side.child(design::divider()).child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(design::title(20., 28., 700.).child("难度反馈"))
                .child(design::muted_text(12., 16.).child("阅读与听力分别取最后一次")),
        );
        for (label, value, role) in [
            ("太难", None, ColorRole::ERROR),
            ("刚好", stats.feedback_just_right, ColorRole::SUCCESS),
            ("太简单", None, ColorRole::SECONDARY),
        ] {
            let fraction = value.unwrap_or(0.) as f32;
            side = side.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_size(px(13.))
                    .line_height(px(18.))
                    .child(div().w(px(44.)).child(label))
                    .child(design::bar(fraction, role))
                    .child(
                        design::text(13., 18., 400.)
                            .w(px(32.))
                            .text_align(gpui_kit::TextAlign::Right)
                            .child(
                                value
                                    .map(|value| format!("{:.0}%", value * 100.))
                                    .unwrap_or_else(|| "—".into()),
                            ),
                    ),
            );
        }
        lower = lower.child(side);

        let mut learned = design::card()
            .flex_1()
            .min_w_0()
            .rounded(px(28.))
            .p(px(20.))
            .px(px(14.))
            .pb(px(14.))
            .gap(px(6.))
            .flex()
            .flex_col();
        learned = learned.child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .px(px(8.))
                .pb(px(8.))
                .child(design::title(20., 28., 700.).child("已学词汇"))
                .child(
                    design::tag(
                        ColorRole::SURFACE_CONTAINER_HIGH,
                        ColorRole::ON_SURFACE_VARIANT,
                    )
                    .child(stats.learned_total.to_string()),
                )
                .child(div().flex_1())
                .child(design::muted_text(12., 16.).child("可提取率 = FSRS 预测此刻能想起的概率")),
        );
        learned = learned.child(
            div()
                .flex()
                .px(px(12.))
                .py(px(4.))
                .text_size(px(12.))
                .line_height(px(16.))
                .font_weight(gpui_kit::FontWeight(650.))
                .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                .child(div().flex_1().child("词 / 释义"))
                .child(div().w(px(110.)).child("状态"))
                .child(div().w(px(240.)).child("可提取率")),
        );
        for word in &stats.learned {
            let value = word.retrievability.unwrap_or(0.);
            let role = if word.archived {
                ColorRole::OUTLINE
            } else if value >= 0.85 {
                ColorRole::SUCCESS
            } else if value >= 0.6 {
                ColorRole::WARNING
            } else {
                ColorRole::ERROR
            };
            learned = learned.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(0.))
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(14.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(design::text(15., 20., 650.).child(word.lemma.clone()))
                            .child(design::muted_text(12., 16.).child(if word.archived {
                                "已归档"
                            } else {
                                "本地学习词"
                            })),
                    )
                    .child(div().w(px(110.)).child(if word.archived {
                        design::tag(
                            ColorRole::SURFACE_CONTAINER_HIGH,
                            ColorRole::ON_SURFACE_VARIANT,
                        )
                        .child("归档")
                    } else {
                        design::tag(
                            ColorRole::PRIMARY_CONTAINER,
                            ColorRole::ON_PRIMARY_CONTAINER,
                        )
                        .child("已学")
                    }))
                    .child(
                        div()
                            .w(px(240.))
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .child(design::bar(value as f32, role))
                            .child(
                                design::mono(13., 18., 400.)
                                    .w(px(48.))
                                    .text_align(gpui_kit::TextAlign::Right)
                                    .child(Self::percent_or(word.retrievability, "—")),
                            ),
                    ),
            );
        }
        if stats.learned.is_empty() {
            learned = learned.child(
                design::muted_text(13., 18.)
                    .p(px(12.))
                    .child("完成复习后，已学词汇会出现在这里。"),
            );
        }
        let page = self.stats_page;
        let start = page.saturating_mul(50).saturating_add(1);
        let end = (page.saturating_mul(50) + stats.learned.len() as u32)
            .min(stats.learned_total.min(u64::from(u32::MAX)) as u32);
        let has_previous = page > 0;
        let has_next = u64::from(page.saturating_add(1)) * 50 < stats.learned_total;
        let mut pager = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .mt(px(4.))
            .px(px(6.))
            .child(
                design::muted_text(12., 16.)
                    .flex_1()
                    .child("长期留存与真实生成质量尚待自用验证。"),
            )
            .child(
                design::mono(13., 18., 400.)
                    .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                    .child(format!("{start}–{end} / {}", stats.learned_total)),
            );
        for (id, icon, enabled, target) in [
            (
                "stats-prev",
                Icon::Back,
                has_previous,
                page.saturating_sub(1),
            ),
            ("stats-next", Icon::Chev, has_next, page.saturating_add(1)),
        ] {
            pager = pager.child(
                div()
                    .id(id)
                    .test_support()
                    .role(gpui_kit::Role::Button)
                    .aria_label(if target < page {
                        "上一页"
                    } else {
                        "下一页"
                    })
                    .cursor_pointer()
                    .w(px(40.))
                    .h(px(40.))
                    .rounded(px(20.))
                    .border_1()
                    .border_color(design::c(ColorRole::OUTLINE_VARIANT))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(!enabled, |button| button.opacity(0.38))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if enabled {
                            this.load_stats(target, cx);
                        }
                    }))
                    .child(design::icon_role(icon, 18., ColorRole::ON_SURFACE_VARIANT)),
            );
        }
        learned = learned.child(pager);
        lower = lower.child(learned);
        body = body.child(lower);
        body.into_any_element()
    }
}
