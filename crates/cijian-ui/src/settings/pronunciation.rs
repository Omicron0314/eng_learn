//! 学习偏好：发音口音与自动朗读。两项都只写本机设备偏好，
//! 不参与云端同步，也不影响复习调度。
use super::*;

impl Shell {
    pub(super) fn pronunciation_preferences(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let mut card = design::card()
            .rounded(px(28.))
            .p(px(20.))
            .px(px(22.))
            .gap(px(14.))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(design::text(16., 22., 650.).child("学习偏好"))
                    .child(design::muted_text(13., 18.).child("发音只影响本机播放")),
            );
        let mut accents = div().flex().gap(px(10.));
        for (index, accent) in cijian_domain::audio::Accent::ALL.into_iter().enumerate() {
            let selected = self.review_accent == accent;
            let label = match accent {
                cijian_domain::audio::Accent::Uk => "英式",
                cijian_domain::audio::Accent::Us => "美式",
            };
            let radii = match index {
                0 => [20., 6., 6., 20.],
                _ => [6., 20., 20., 6.],
            };
            let mut cell = design::shaped(
                radii,
                design::c(if selected {
                    ColorRole::SECONDARY
                } else {
                    ColorRole::SURFACE_CONTAINER_HIGH
                }),
            )
            .id(gpui_kit::SharedString::from(match accent {
                cijian_domain::audio::Accent::Uk => "pronunciation-uk",
                cijian_domain::audio::Accent::Us => "pronunciation-us",
            }))
            .test_support()
            .role(gpui_kit::Role::Button)
            .aria_label(format!("发音类型：{label}"))
            .cursor_pointer()
            .flex_1()
            .h(px(44.))
            .text_color(design::c(if selected {
                ColorRole::SURFACE_CONTAINER_LOWEST
            } else {
                ColorRole::ON_SURFACE_VARIANT
            }))
            .text_size(px(15.))
            .font_weight(gpui_kit::FontWeight(if selected { 700. } else { 600. }))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.review_accent = accent;
                this.request(Command::SetPronunciationAccent { accent }, cx);
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
                        row.child(design::icon_role(Icon::Check, 18., ColorRole::ON_PRIMARY))
                    })
                    .child(label),
            );
            accents = accents.child(cell);
        }
        card = card.child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .mt(px(4.))
                .child(design::text(15., 20., 650.).child("发音类型"))
                .child(design::muted_text(12., 16.).child("复习卡的音标与朗读默认使用它"))
                .child(accents.mt(px(8.))),
        );
        let auto = self.review_auto_play;
        card = card.child(
            div()
                .id("review-auto-play")
                .test_support()
                .role(gpui_kit::Role::Button)
                .aria_label(if auto {
                    "自动发音：已开启"
                } else {
                    "自动发音：已关闭"
                })
                .cursor_pointer()
                .rounded(px(20.))
                .p(px(12.))
                .px(px(14.))
                .flex()
                .items_center()
                .gap(px(12.))
                .when(auto, |row| {
                    row.bg(design::c(ColorRole::SECONDARY_CONTAINER))
                        .text_color(design::c(ColorRole::ON_SECONDARY_CONTAINER))
                })
                .when(!auto, |row| {
                    row.border_1()
                        .border_color(design::c(ColorRole::OUTLINE_VARIANT))
                })
                .on_click(cx.listener(|this, _, _, cx| {
                    let value = !this.review_auto_play;
                    this.review_auto_play = value;
                    this.request(Command::SetReviewAutoPlay { value }, cx);
                }))
                .child(
                    div()
                        .w(px(40.))
                        .h(px(22.))
                        .rounded(px(11.))
                        .flex()
                        .items_center()
                        .px(px(2.))
                        .when(auto, |track| {
                            track
                                .bg(design::c(ColorRole::PRIMARY))
                                .justify_end()
                        })
                        .when(!auto, |track| {
                            track
                                .bg(design::c(ColorRole::SURFACE_CONTAINER_HIGHEST))
                                .justify_start()
                        })
                        .child(
                            div()
                                .w(px(18.))
                                .h(px(18.))
                                .rounded(px(9.))
                                .bg(design::c(ColorRole::SURFACE_CONTAINER_LOWEST)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .child(
                            design::text(15., 20., 650.)
                                .child(if auto { "自动发音：开" } else { "自动发音：关" }),
                        )
                        .child(design::text(12., 16., 400.).opacity(0.85).child(
                            if auto {
                                "每张新词卡出现时读一遍"
                            } else {
                                "只在点音标或口音按钮时朗读"
                            },
                        )),
                ),
        );
        card.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::test::{TestQueryExt as _, TestWindowExt};
    use gpui_kit::{Bounds, TestAppContext, size};

    #[gpui_kit::test]
    fn learning_page_exposes_accent_and_auto_play_preferences(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            gpui_kit_material::init(cx);
            crate::init_component_theme(crate::theme::scheme(false), 1., cx);
            cx.set_reduce_motion(true);
        });
        let bus = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let recorder = bus.clone();
        struct Quiet(std::sync::Arc<std::sync::Mutex<Vec<Command>>>);
        impl CommandBus for Quiet {
            fn dispatch(
                &self,
                command: Command,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = cijian_ports::Result<Response>> + Send + '_>,
            > {
                self.0.lock().unwrap().push(command);
                Box::pin(async { Ok(Response::Preferences(Default::default())) })
            }
        }
        let (handle, _shell) = cx
            .update(|cx| {
                gpui_kit::open_window(
                    WindowOptions {
                        window_bounds: Some(gpui_kit::WindowBounds::Windowed(Bounds::new(
                            Default::default(),
                            size(px(1200.), px(1000.)),
                        ))),
                        ..Default::default()
                    },
                    cx,
                    move |_, cx| {
                        cx.new(|_| {
                            let mut shell = Shell::new();
                            shell.navigation.navigate(Route::Settings);
                            shell.backend = Some(std::sync::Arc::new(Quiet(recorder)));
                            shell.settings_ui.page = SettingsPage::Learning;
                            shell
                        })
                    },
                )
            })
            .unwrap();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find_by_label("发音类型：英式").is_some());
            assert!(window.try_find_by_label("发音类型：美式").is_some());
            assert!(window.try_find_by_label("自动发音：已开启").is_some());
            window.click("pronunciation-uk", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .unwrap();
        cx.update_window(handle, |_, window, cx| window.click("review-auto-play", cx))
            .unwrap();
        cx.run_until_parked();
        let commands = bus.lock().unwrap();
        assert!(
            commands.iter().any(|command| matches!(
                command,
                Command::SetPronunciationAccent {
                    accent: cijian_domain::audio::Accent::Uk
                }
            )),
            "choosing 英式 persists the accent"
        );
        assert!(
            commands
                .iter()
                .any(|command| matches!(command, Command::SetReviewAutoPlay { value: false })),
            "the switch persists auto-play"
        );
    }
}
