//! Window chrome: the 96px navigation rail, compact bottom bar and page area.
use super::*;
use crate::design::{self, Icon};
use gpui_kit::{ElementId, FocusHandle, FontWeight, prelude::*, px};
use gpui_kit_material::color::ColorRole;

#[derive(Clone, Copy)]
enum NavSlot {
    Today,
    Review,
    Articles,
    Library,
    Inbox,
    Stats,
}

const NAV_SLOTS: [NavSlot; 6] = [
    NavSlot::Today,
    NavSlot::Review,
    NavSlot::Articles,
    NavSlot::Library,
    NavSlot::Inbox,
    NavSlot::Stats,
];

impl NavSlot {
    fn route(self) -> Route {
        match self {
            Self::Today => Route::Today,
            Self::Review => Route::Review,
            Self::Articles => Route::Articles,
            Self::Library => Route::Library {
                book: None,
                search: String::new(),
            },
            Self::Inbox => Route::Inbox,
            Self::Stats => Route::Stats,
        }
    }
    fn id(self) -> &'static str {
        match self {
            Self::Today => "today",
            Self::Review => "review",
            Self::Articles => "articles",
            Self::Library => "library",
            Self::Inbox => "inbox",
            Self::Stats => "stats",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Today => "今日",
            Self::Review => "复习",
            Self::Articles => "文章",
            Self::Library => "词库",
            Self::Inbox => "生词箱",
            Self::Stats => "统计",
        }
    }
    fn icon(self) -> Icon {
        match self {
            Self::Today => Icon::Today,
            Self::Review => Icon::Cards,
            Self::Articles => Icon::File,
            Self::Library => Icon::Book,
            Self::Inbox => Icon::Inbox,
            Self::Stats => Icon::Chart,
        }
    }
}

impl Shell {
    pub(super) fn navigation_focus(&self, cx: &mut Context<Self>) -> Vec<FocusHandle> {
        let mut handles = self.nav_focus.borrow_mut();
        if handles.len() != NAV_SLOTS.len() + 2 {
            *handles = (0..NAV_SLOTS.len() + 2)
                .map(|_| cx.focus_handle())
                .collect();
        }
        handles.clone()
    }

    fn navigate_slot(&mut self, slot: NavSlot, cx: &mut Context<Self>) {
        let route = slot.route();
        if self.navigation.current() == &route {
            match slot {
                NavSlot::Today | NavSlot::Review => self.request(Command::LoadReview, cx),
                NavSlot::Articles => self.request(Command::LoadArticles { page: 0 }, cx),
                NavSlot::Library => self.request(
                    Command::LoadLibrary {
                        filter: self.library_filter(),
                    },
                    cx,
                ),
                NavSlot::Inbox => self.request(Command::LoadInbox { page: 0 }, cx),
                NavSlot::Stats => self.request(Command::LoadStats { page: 0 }, cx),
            }
            return;
        }
        self.navigate(route, cx);
    }

    fn rail_item(
        &mut self,
        slot: NavSlot,
        selected: bool,
        focus: FocusHandle,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let focus = focus.tab_stop(true);
        let badge = matches!(slot, NavSlot::Inbox)
            .then_some(self.inbox_total)
            .filter(|count| *count > 0);
        let mut item = div()
            .id(ElementId::NamedChild(
                ElementId::from("main-nav").into(),
                slot.id().into(),
            ))
            .test_support()
            .role(gpui_kit::Role::Button)
            .aria_label(slot.label())
            .track_focus(&focus)
            .tab_index(0)
            .w(px(80.))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(4.))
            .pt(px(4.))
            .pb(px(9.))
            .relative()
            .cursor_pointer()
            .text_color(if selected {
                design::c(ColorRole::ON_SECONDARY_CONTAINER)
            } else {
                design::c(ColorRole::ON_SURFACE_VARIANT)
            })
            .hover(|style| style.opacity(0.9))
            .on_click(cx.listener(move |this, _, _, cx| this.navigate_slot(slot, cx)));
        let mut indicator = div()
            .w(px(56.))
            .h(px(32.))
            .rounded(px(16.))
            .flex()
            .items_center()
            .justify_center();
        if selected {
            indicator = indicator.bg(design::c(ColorRole::SECONDARY_CONTAINER));
        }
        indicator = indicator.child(design::icon_role(
            slot.icon(),
            24.,
            if selected {
                ColorRole::ON_SECONDARY_CONTAINER
            } else {
                ColorRole::ON_SURFACE_VARIANT
            },
        ));
        item = item.child(indicator).child(
            div()
                .text_size(px(12.))
                .line_height(px(16.))
                .font_weight(FontWeight(if selected { 750. } else { 500. }))
                .child(slot.label()),
        );
        if let Some(count) = badge {
            item = item.child(
                div()
                    .absolute()
                    .top(px(0.))
                    .left(px(46.))
                    .min_w(px(18.))
                    .h(px(18.))
                    .px(px(5.))
                    .rounded(px(9.))
                    .border_2()
                    .border_color(design::c(ColorRole::SURFACE))
                    .bg(design::c(ColorRole::ERROR))
                    .text_color(gpui_kit::white())
                    .text_size(px(11.))
                    .line_height(px(14.))
                    .font_weight(FontWeight(700.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(if count > 99 {
                        "99+".to_owned()
                    } else {
                        count.to_string()
                    }),
            );
        }
        item.into_any_element()
    }

    fn rail(&mut self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let selected = self.navigation.current().section();
        let handles = self.navigation_focus(cx);
        let settings_focus = handles[NAV_SLOTS.len() + 1].clone();
        let mut rail = div()
            .w(px(design::RAIL_WIDTH))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .items_center()
            .pt(px(20.))
            .pb(px(16.))
            .gap(px(2.));
        let home_focus = handles[NAV_SLOTS.len()].clone().tab_stop(true);
        rail = rail.child(
            div()
                .id("nav-home")
                .role(gpui_kit::Role::Button)
                .aria_label("今日")
                .track_focus(&home_focus)
                .tab_stop(true)
                .cursor_pointer()
                .w(px(48.))
                .h(px(48.))
                .relative()
                .text_color(design::c(ColorRole::ON_PRIMARY))
                .on_click(cx.listener(|this, _, _, cx| this.navigate(Route::Today, cx)))
                .child(
                    design::shaped([24., 24., 24., 14.], design::c(ColorRole::PRIMARY))
                        .w(px(48.))
                        .h(px(48.)),
                )
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(22.))
                        .line_height(px(26.))
                        .font_weight(FontWeight(800.))
                        .child("词"),
                ),
        );
        rail = rail.child(div().h(px(36.)).flex_shrink_0());
        for (index, slot) in NAV_SLOTS.iter().enumerate() {
            let is_selected = slot.id() == selected
                || (matches!(slot, NavSlot::Articles) && selected == "articles");
            rail = rail.child(self.rail_item(*slot, is_selected, handles[index].clone(), cx));
        }
        rail = rail.child(div().flex_1());
        let settings_selected = selected == "settings";
        let mut settings = div()
            .id("settings")
            .test_support()
            .role(gpui_kit::Role::Button)
            .aria_label("设置")
            .track_focus(&settings_focus)
            .tab_index(0)
            .cursor_pointer()
            .w(px(80.))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(4.))
            .pt(px(4.))
            .pb(px(9.))
            .text_color(if settings_selected {
                design::c(ColorRole::ON_SECONDARY_CONTAINER)
            } else {
                design::c(ColorRole::ON_SURFACE_VARIANT)
            })
            .hover(|style| style.opacity(0.9))
            .on_click(cx.listener(|this, _, _, cx| this.navigate(Route::Settings, cx)));
        let mut indicator = div()
            .w(px(56.))
            .h(px(32.))
            .rounded(px(16.))
            .flex()
            .items_center()
            .justify_center();
        if settings_selected {
            indicator = indicator.bg(design::c(ColorRole::SECONDARY_CONTAINER));
        }
        settings = settings
            .child(indicator.child(design::icon_role(
                Icon::Tune,
                24.,
                if settings_selected {
                    ColorRole::ON_SECONDARY_CONTAINER
                } else {
                    ColorRole::ON_SURFACE_VARIANT
                },
            )))
            .child(
                div()
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .font_weight(FontWeight(if settings_selected { 750. } else { 500. }))
                    .child("设置"),
            );
        rail.child(settings).into_any_element()
    }

    fn compact_bar(&mut self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let handles = self.navigation_focus(cx);
        let home_focus = handles[NAV_SLOTS.len()].clone().tab_stop(true);
        let settings_focus = handles[NAV_SLOTS.len() + 1].clone().tab_stop(true);
        div()
            .flex()
            .items_center()
            .gap(px(12.))
            .w_full()
            .px(px(16.))
            .pt(px(12.))
            .pb(px(4.))
            .child(
                div()
                    .id("nav-home")
                    .role(gpui_kit::Role::Button)
                    .aria_label("今日")
                    .track_focus(&home_focus)
                    .tab_stop(true)
                    .cursor_pointer()
                    .w(px(40.))
                    .h(px(40.))
                    .relative()
                    .text_color(design::c(ColorRole::ON_PRIMARY))
                    .on_click(cx.listener(|this, _, _, cx| this.navigate(Route::Today, cx)))
                    .child(
                        design::shaped([20., 20., 20., 12.], design::c(ColorRole::PRIMARY))
                            .w(px(40.))
                            .h(px(40.)),
                    )
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(18.))
                            .line_height(px(22.))
                            .font_weight(FontWeight(800.))
                            .child("词"),
                    ),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id("settings")
                    .test_support()
                    .role(gpui_kit::Role::Button)
                    .aria_label("设置")
                    .track_focus(&settings_focus)
                    .tab_stop(true)
                    .cursor_pointer()
                    .w(px(40.))
                    .h(px(40.))
                    .rounded(px(20.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                    .on_click(cx.listener(|this, _, _, cx| this.navigate(Route::Settings, cx)))
                    .child(design::icon_role(
                        Icon::Tune,
                        24.,
                        ColorRole::ON_SURFACE_VARIANT,
                    )),
            )
            .into_any_element()
    }

    fn compact_nav(&mut self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let selected = self.navigation.current().section();
        let handles = self.navigation_focus(cx);
        let mut bar = div()
            .w_full()
            .h(px(80.))
            .flex_shrink_0()
            .flex()
            .items_start()
            .justify_around()
            .pt(px(12.))
            .bg(design::c(ColorRole::SURFACE_CONTAINER))
            .text_color(design::c(ColorRole::ON_SURFACE_VARIANT));
        for (index, slot) in NAV_SLOTS.iter().enumerate() {
            let is_selected = slot.id() == selected;
            let badge = matches!(slot, NavSlot::Inbox)
                .then_some(self.inbox_total)
                .filter(|count| *count > 0);
            let mut item = div()
                .id(ElementId::NamedChild(
                    ElementId::from("main-nav").into(),
                    slot.id().into(),
                ))
                .test_support()
                .role(gpui_kit::Role::Button)
                .aria_label(slot.label())
                .track_focus(&handles[index].clone().tab_stop(true))
                .tab_index(0)
                .cursor_pointer()
                .w(px(60.))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(4.))
                .relative()
                .text_color(if is_selected {
                    design::c(ColorRole::ON_SECONDARY_CONTAINER)
                } else {
                    design::c(ColorRole::ON_SURFACE_VARIANT)
                })
                .on_click(cx.listener(move |this, _, _, cx| this.navigate_slot(*slot, cx)));
            let mut indicator = div()
                .w(px(48.))
                .h(px(32.))
                .rounded(px(16.))
                .flex()
                .items_center()
                .justify_center();
            if is_selected {
                indicator = indicator.bg(design::c(ColorRole::SECONDARY_CONTAINER));
            }
            item = item
                .child(indicator.child(design::icon_role(
                    slot.icon(),
                    22.,
                    if is_selected {
                        ColorRole::ON_SECONDARY_CONTAINER
                    } else {
                        ColorRole::ON_SURFACE_VARIANT
                    },
                )))
                .child(
                    div()
                        .text_size(px(11.))
                        .line_height(px(16.))
                        .font_weight(FontWeight(if is_selected { 700. } else { 500. }))
                        .child(slot.label()),
                );
            if let Some(count) = badge {
                item = item.child(
                    div()
                        .absolute()
                        .top(px(0.))
                        .left(px(42.))
                        .min_w(px(18.))
                        .h(px(18.))
                        .px(px(5.))
                        .rounded(px(9.))
                        .bg(design::c(ColorRole::ERROR))
                        .text_color(gpui_kit::white())
                        .text_size(px(11.))
                        .line_height(px(14.))
                        .font_weight(FontWeight(700.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(if count > 99 {
                            "99+".to_owned()
                        } else {
                            count.to_string()
                        }),
                );
            }
            bar = bar.child(item);
        }
        bar.into_any_element()
    }

    pub(super) fn screen(&mut self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        match self.navigation.current() {
            Route::Today => self.today_body(cx),
            Route::Review => self.review_body(cx),
            Route::Settings => self.settings_body(cx),
            Route::Articles => self.articles_body(cx),
            Route::Article { .. } => self.reader_body(cx),
            Route::Inbox => self.inbox_body(cx),
            Route::Stats => self.stats_body(cx),
            Route::Library { .. } => self.library_body(cx),
        }
    }

    fn desktop_area(&mut self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        div()
            .flex()
            .w_full()
            .h_full()
            .child(self.rail(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .py(px(12.))
                    .pr(px(12.))
                    .child(self.screen(cx)),
            )
            .into_any_element()
    }

    fn compact_area(&mut self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let route = self.navigation.current().clone();
        let full_bleed = matches!(route, Route::Review | Route::Article { .. });
        let mut content = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .w_full()
            .child(div().flex_1().min_h_0().w_full().child(self.screen(cx)));
        if !full_bleed {
            content = content.child(self.compact_nav(cx));
        }
        let mut area = div().flex().flex_col().w_full().h_full();
        if !full_bleed {
            area = area.child(self.compact_bar(cx));
        }
        area.child(content).into_any_element()
    }

    fn snackbar(&self) -> Option<gpui_kit::AnyElement> {
        let (message, color) = if let Some(error) = &self.error {
            (error.clone(), ColorRole::ERROR_CONTAINER)
        } else if self.loading {
            ("正在处理，请稍候…".to_owned(), ColorRole::INVERSE_SURFACE)
        } else {
            (self.notice.clone()?, ColorRole::INVERSE_SURFACE)
        };
        Some(
            div()
                .absolute()
                .bottom(px(28.))
                .left(px(0.))
                .right(px(0.))
                .flex()
                .justify_center()
                .child(
                    div()
                        .min_h(px(48.))
                        .max_w(px(560.))
                        .px(px(16.))
                        .py(px(10.))
                        .rounded(px(12.))
                        .bg(design::c(color))
                        .text_color(if color == ColorRole::ERROR_CONTAINER {
                            design::c(ColorRole::ON_ERROR_CONTAINER)
                        } else {
                            design::c(ColorRole::INVERSE_ON_SURFACE)
                        })
                        .text_size(px(14.))
                        .line_height(px(20.))
                        .shadow(design::shadow_float())
                        .child(message),
                )
                .into_any_element(),
        )
    }

    pub(super) fn render_content(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        #[cfg(test)]
        {
            self.rendered_dark = gpui_kit_material::theme::is_dark();
        }
        self.prepare_library_inputs(window, cx);
        if self.key_intercept.is_none() {
            let weak = cx.entity().downgrade();
            self.key_intercept = Some(cx.intercept_keystrokes(move |event, _, cx| {
                if event.keystroke.key != "escape" {
                    return;
                }
                let _ = weak.update(cx, |this, cx| {
                    if this.navigation.current().section() == "library"
                        && this.library_ui.editor.is_some()
                        && !this.library_ui.discard
                    {
                        this.close_card_editor(cx);
                        cx.stop_propagation();
                    }
                });
            }));
        }
        if self.focus.is_none() {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            self.focus = Some(focus);
            let weak = cx.entity().downgrade();
            window.on_window_should_close(cx, move |window, cx| {
                weak.update(cx, |this, cx| {
                    this.may_close(cx) && this.close_reader_when_saved(window, cx)
                })
                .unwrap_or(true)
            });
        }
        self.initialize_positions(cx);
        self.prepare_settings(window, cx);
        self.prepare_flag(window, cx);
        self.prepare_reader(window, cx);
        let viewport = window.viewport_size();
        self.viewport_width = viewport.width.into();
        self.viewport_height = viewport.height.into();
        self.compact = viewport.width < px(600.);
        self.compact_reader = viewport.width
            < gpui_kit_material::tokens::typography::rem(840.).to_pixels(window.rem_size());
        let body = if self.compact {
            self.compact_area(cx)
        } else {
            self.desktop_area(cx)
        };
        let background = div()
            .capture_key_down(
                cx.listener(|this, event: &gpui_kit::KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape"
                        && !this.compact_reader
                        && !this.reader_flags.open
                        && this.lookup.is_some()
                        && matches!(this.navigation.current(), Route::Article { .. })
                        && (!this.loading || this.lookup_loading)
                    {
                        this.dismiss_lookup(window, cx);
                        cx.stop_propagation();
                    }
                }),
            )
            .track_focus(self.focus.as_ref().expect("initialized focus"))
            .tab_stop(false)
            .on_key_down(cx.listener(|this, event: &gpui_kit::KeyDownEvent, _, cx| {
                if this.navigation.current().section() == "library"
                    && this.library_ui.editor.is_some()
                    && !this.library_ui.discard
                    && event.keystroke.key == "escape"
                {
                    this.close_card_editor(cx);
                    cx.stop_propagation();
                    return;
                }
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
            .w(viewport.width)
            .h(viewport.height)
            .flex()
            .font(design::base_font())
            .bg(design::c(ColorRole::SURFACE))
            .text_color(design::c(ColorRole::ON_SURFACE))
            .child(body);
        let background = match self.snackbar() {
            Some(snack) => background.child(snack),
            None => background,
        };
        background
            .child(self.position_dialog(cx))
            .into_any_element()
    }
}
