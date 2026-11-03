#[cfg(test)]
mod tests;
use super::*;
use cijian_ports::stats::StatsSnapshot;
use gpui_kit::TestSupportExt as _;
fn percent(value: Option<f64>) -> String {
    value
        .map(|v| format!("{:.1}%", v * 100.))
        .unwrap_or("暂无数据".into())
}
impl Shell {
    pub(super) fn today_metrics(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let mut body = div().flex().flex_col().gap_4();
        if let Some(stats) = &self.stats {
            body = body
                .child(format!(
                    "今日新学 {} · 复习 {}",
                    stats.today.new_cards, stats.today.reviews
                ))
                .child(self.heatmap(stats, cx));
        }
        body = body.child(
            material_button("today-inbox", ButtonVariant::Tonal)
                .child(format!("整理生词（{}）", self.inbox_total))
                .on_click(cx.listener(|this, _, _, cx| this.navigate(Route::Inbox, cx))),
        );
        for book in &self.books {
            let id = book.id.clone();
            body = body.child(
                material_button(
                    gpui_kit::ElementId::Name(format!("today-book-{id}").into()),
                    ButtonVariant::Text,
                )
                .child(format!("{} · {} 词", book.name, book.entries))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.navigate(
                        Route::Library {
                            book: Some(id.clone()),
                            search: String::new(),
                        },
                        cx,
                    )
                })),
            );
        }
        body.into_any_element()
    }
    fn heatmap(&self, stats: &StatsSnapshot, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let today = stats.heatmap.iter().rposition(|d| !d.future).unwrap_or(0);
        let selected = self.heatmap_day.unwrap_or(today).min(today);
        {
            let mut handles = self.heatmap_focus.borrow_mut();
            if handles.len() != stats.heatmap.len() {
                *handles = stats.heatmap.iter().map(|_| cx.focus_handle()).collect();
            }
        }
        let mut weeks = div().flex().gap_1();
        for (index, day) in stats.heatmap.iter().enumerate() {
            let role = match day.count {
                0 => ColorRole::SURFACE_CONTAINER_HIGHEST,
                1..=13 => ColorRole::PRIMARY_CONTAINER,
                14..=26 => ColorRole::SECONDARY_CONTAINER,
                27..=39 => ColorRole::TERTIARY_CONTAINER,
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
            let mut cell = div()
                .id(gpui_kit::SharedString::from(format!("activity-{index}")))
                .test_support()
                .role(gpui_kit::Role::Button)
                .aria_label(label)
                .w(px(24.))
                .h(px(24.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(12.))
                .bg(if index == selected && !day.future {
                    color(ColorRole::SURFACE_CONTAINER_HIGHEST)
                } else {
                    color(ColorRole::SURFACE)
                })
                .child(
                    div()
                        .w(px(14.))
                        .h(px(14.))
                        .rounded(px(3.))
                        .bg(color(role))
                        .opacity(if day.future { 0.25 } else { 1. }),
                );
            if !day.future {
                let focus = self.heatmap_focus.borrow()[index].clone();
                let click_focus = focus.clone();
                cell = cell
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
                                if let Some(focus) = this.heatmap_focus.borrow().get(next).cloned()
                                {
                                    window.focus(&focus, cx);
                                }
                                cx.notify();
                            }
                            cx.stop_propagation();
                        },
                    ));
            }
            weeks = weeks.child(cell);
        }
        let Some(date) = stats.heatmap.get(selected) else {
            return div().child("暂无学习活动").into_any_element();
        };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child("近 13 周学习活动")
            .child(weeks)
            .child(format!("{}：{} 次活动", date.date, date.count))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        material_button("activity-previous", ButtonVariant::Text)
                            .child("前一天")
                            .disabled(selected == 0)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.heatmap_day = Some(selected.saturating_sub(1));
                                cx.notify();
                            })),
                    )
                    .child(
                        material_button("activity-next", ButtonVariant::Text)
                            .child("后一天")
                            .disabled(selected >= today)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.heatmap_day = Some((selected + 1).min(today));
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
    }
    pub(super) fn stats_body(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let mut body = div().flex().flex_col().gap_4();
        let Some(stats) = &self.stats else {
            return body.child("正在读取学习记录…").into_any_element();
        };
        body = body
            .child(format!(
                "今日新学 {} · 复习 {}",
                stats.today.new_cards, stats.today.reviews
            ))
            .child(format!(
                "本周新学 {} · 复习 {}",
                stats.week.new_cards, stats.week.reviews
            ))
            .child(format!(
                "累计阅读完成率：{}",
                percent(stats.reading_completion)
            ))
            .child(format!(
                "最新阅读／听力反馈：刚好 {}",
                percent(stats.feedback_just_right)
            ))
            .child(self.heatmap(stats, cx))
            .child(format!(
                "近 7 学习日未知词占比中位数：{}",
                percent(stats.unknown_median_week)
            ))
            .child(format!(
                "全部文章未知词占比中位数：{}",
                percent(stats.unknown_median_all)
            ));
        body = body.child("生成时的覆盖率记录");
        for point in &stats.coverage {
            body = body.child(format!(
                "{} · {:?} · {}",
                point.title,
                point.tier,
                percent(point.coverage)
            ));
        }
        body = body
            .child(format!("已学词（{}）", stats.learned_total))
            .child("可提取率是当前记忆预测，不代表掌握认证。");
        for word in &stats.learned {
            body = body.child(format!(
                "{} · {}{}",
                word.lemma,
                percent(word.retrievability),
                if word.archived { " · 已归档" } else { "" }
            ));
        }
        body = body
            .child(format!("对照观察 · {}", stats.experiment.version))
            .child("这里只比较观察样本，不能据此断言生成文章造成了效果差异。");
        for (name, group) in [
            ("出现组", &stats.experiment.exposed),
            ("对照组", &stats.experiment.control),
        ] {
            body = body.child(format!(
                "{name}：已复习 {}，Again {}（{}），未复习 {}，排除 {}",
                group.sample,
                group.again,
                percent(group.again_rate),
                group.unreviewed,
                group.excluded
            ));
        }
        body = body.child(
            stats
                .experiment
                .difference_percentage_points
                .map(|v| format!("Again 比例差：{v:+.1} 个百分点"))
                .unwrap_or("组间样本不足，暂无比例差".into()),
        );
        body = body.child(
            material_button("experiment-settings", ButtonVariant::Tonal)
                .child("调整实验设置")
                .disabled(self.loading)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.settings_ui.advanced = true;
                    this.settings_ui.focus_advanced = true;
                    this.navigate(Route::Settings, cx);
                })),
        );
        body.into_any_element()
    }
}
