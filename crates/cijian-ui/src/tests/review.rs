use super::*;
use gpui_kit::{
    Bounds, TestAppContext, size,
    test::{TestQueryExt as _, TestWindowExt},
};
#[gpui_kit::test]
fn revealed_review_shows_meaning_example_and_source(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit_material::init(cx);
        init_component_theme(theme::scheme(false), 1., cx);
        cx.set_reduce_motion(true);
    });
    let (handle, _shell) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(gpui_kit::WindowBounds::Windowed(Bounds::new(
                        Default::default(),
                        size(px(1000.), px(900.)),
                    ))),
                    ..Default::default()
                },
                cx,
                |_, cx| {
                    cx.new(|_| {
                        let mut shell = Shell::new();
                        shell.navigation.navigate(Route::Review);
                        shell.revealed = true;
                        shell.review = vec![cijian_ports::library::CardView {
                            id: "original-card".into(),
                            lexeme_id: "original-lexeme".into(),
                            front: "original".into(),
                            meaning: "原创释义".into(),
                            example: "An original example.".into(),
                            text_source: "原创导入".into(),
                            suspended: false,
                            archived: false,
                            revision: 0,
                            created_at: 0,
                            srs: cijian_domain::srs::SrsState::new(0),
                        }];
                        shell
                    })
                },
            )
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find_by_label("来源：原创导入").is_some());
        window.press("1", cx);
    })
    .unwrap();
}

#[gpui_kit::test]
fn long_meaning_wraps_without_overlapping_the_source_or_ratings(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit_material::init(cx);
        init_component_theme(theme::scheme(false), 1., cx);
        cx.set_reduce_motion(true);
    });
    for (width, height) in [(360., 800.), (1000., 900.)] {
        let (handle, _shell) = cx
            .update(|cx| {
                gpui_kit::open_window(
                    WindowOptions {
                        window_bounds: Some(gpui_kit::WindowBounds::Windowed(Bounds::new(
                            Default::default(),
                            size(px(width), px(height)),
                        ))),
                        ..Default::default()
                    },
                    cx,
                    |_, cx| {
                        cx.new(|_| {
                            let mut shell = Shell::new();
                            shell.navigation.navigate(Route::Review);
                            shell.revealed = true;
                            shell.review = vec![cijian_ports::library::CardView {
                                id: "long-card".into(),
                                lexeme_id: "long-lexeme".into(),
                                front: "scaffold".into(),
                                meaning: "这是一个很长的释义，用来验证换行不会覆盖来源标签，也不会把评分按钮挤出窗口。"
                                    .repeat(12),
                                example: "An original example long enough to exercise wrapping in the card."
                                    .into(),
                                text_source: "原创导入".into(),
                                suspended: false,
                                archived: false,
                                revision: 0,
                                created_at: 0,
                                srs: cijian_domain::srs::SrsState::new(0),
                            }];
                            shell
                        })
                    },
                )
            })
            .unwrap();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let card = window.find("review-card").bounds();
            let source = window.find("review-source").bounds();
            let rating = window.find("rating-0").bounds();
            assert!(
                source.origin.x >= card.origin.x
                    && source.origin.x + source.size.width <= card.origin.x + card.size.width,
                "the source tag must stay inside the card at {width}x{height}: card={card:?} source={source:?}"
            );
            assert!(
                rating.origin.y + rating.size.height <= px(height),
                "the rating row must stay inside {width}x{height}: {rating:?}"
            );
        })
        .unwrap();
    }
}

#[derive(Default)]
struct GlossBus(std::sync::Mutex<Vec<Command>>);
impl CommandBus for GlossBus {
    fn dispatch(
        &self,
        command: Command,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = cijian_ports::Result<Response>> + Send + '_>,
    > {
        self.0.lock().unwrap().push(command.clone());
        let result = match command {
            Command::ExplainCard { card, revision, .. } => {
                Ok(Response::CardExplained(cijian_ports::library::CardView {
                    id: card,
                    lexeme_id: "original-lexeme".into(),
                    front: "scaffold".into(),
                    meaning: "n. /ˈskæfəld/\n脚手架\nan original AI meaning".into(),
                    example: "An original example.".into(),
                    text_source: "generated".into(),
                    suspended: false,
                    archived: false,
                    revision: revision + 1,
                    created_at: 0,
                    srs: cijian_domain::srs::SrsState::new(0),
                }))
            }
            _ => Err(AppError::new(
                cijian_ports::ErrorCode::InvalidInput,
                "原创测试只生成本卡释义",
            )),
        };
        Box::pin(async move { result })
    }
}

#[gpui_kit::test]
fn missing_meaning_is_explained_once_without_blocking_rating(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit_material::init(cx);
        init_component_theme(theme::scheme(false), 1., cx);
        cx.set_reduce_motion(true);
    });
    let bus = std::sync::Arc::new(GlossBus::default());
    let (handle, shell) = cx
        .update(|cx| {
            let bus = bus.clone();
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(gpui_kit::WindowBounds::Windowed(Bounds::new(
                        Default::default(),
                        size(px(1000.), px(900.)),
                    ))),
                    ..Default::default()
                },
                cx,
                move |_, cx| {
                    cx.new(|_| {
                        let mut shell = Shell::new();
                        shell.navigation.navigate(Route::Review);
                        shell.backend = Some(bus);
                        shell.provider_config = Some(cijian_domain::provider::ProviderConfig {
                            kind: cijian_domain::provider::ProviderKind::Llm,
                            endpoint: "http://127.0.0.1:1/v1".into(),
                            model: "original".into(),
                            voice: None,
                            revision: 1,
                        });
                        shell.review = vec![cijian_ports::library::CardView {
                            id: "missing-card".into(),
                            lexeme_id: "original-lexeme".into(),
                            front: "scaffold".into(),
                            meaning: String::new(),
                            example: "An original example.".into(),
                            text_source: "missing".into(),
                            suspended: false,
                            archived: false,
                            revision: 3,
                            created_at: 0,
                            srs: cijian_domain::srs::SrsState::new(0),
                        }];
                        shell
                    })
                },
            )
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("space", cx);
        assert!(
            shell.read_with(cx, |shell, _| shell.revealed
                && shell.gloss_pending.as_deref() == Some("missing-card")),
            "reveal is immediate and starts one background gloss"
        );
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    let commands = bus.0.lock().unwrap();
    assert_eq!(
        commands
            .iter()
            .filter(|command| matches!(command, Command::ExplainCard { .. }))
            .count(),
        1
    );
    drop(commands);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("space", cx);
        window.press("3", cx);
        assert!(
            shell.read_with(cx, |shell, _| {
                shell.review[0].meaning.contains("脚手架")
                    && shell.review[0].text_source == "generated"
                    && shell.review[0].revision == 4
            }),
            "the generated meaning and revision reach the card"
        );
    })
    .unwrap();
    cx.run_until_parked();
    let commands = bus.0.lock().unwrap();
    assert_eq!(
        commands
            .iter()
            .filter(|command| matches!(command, Command::ExplainCard { .. }))
            .count(),
        1,
        "an already revealed card is not explained twice"
    );
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, Command::Rate { .. })),
        "rating is dispatched with the updated revision"
    );
}

#[gpui_kit::test]
fn clicking_the_card_reveals_it(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit_material::init(cx);
        init_component_theme(theme::scheme(false), 1., cx);
        cx.set_reduce_motion(true);
    });
    let (handle, shell) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(gpui_kit::WindowBounds::Windowed(Bounds::new(
                        Default::default(),
                        size(px(1000.), px(900.)),
                    ))),
                    ..Default::default()
                },
                cx,
                |_, cx| {
                    cx.new(|_| {
                        let mut shell = Shell::new();
                        shell.navigation.navigate(Route::Review);
                        shell.review = vec![cijian_ports::library::CardView {
                            id: "original-card".into(),
                            lexeme_id: "original-lexeme".into(),
                            front: "meadow".into(),
                            meaning: "原创释义".into(),
                            example: "An original example.".into(),
                            text_source: "imported".into(),
                            suspended: false,
                            archived: false,
                            revision: 1,
                            created_at: 0,
                            srs: cijian_domain::srs::SrsState::new(0),
                        }];
                        shell
                    })
                },
            )
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("review-card", cx);
        window.render_frame(cx);
        assert!(shell.read_with(cx, |shell, _| shell.revealed));
    })
    .unwrap();
}

#[gpui_kit::test]
fn entering_review_starts_a_new_progress_session(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit_material::init(cx);
        init_component_theme(theme::scheme(false), 1., cx);
        cx.set_reduce_motion(true);
    });
    let (_handle, shell) = cx
        .update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| {
                cx.new(|_| {
                    let mut shell = Shell::new();
                    shell.review_done = 7;
                    shell
                })
            })
        })
        .unwrap();
    shell.update(cx, |shell, cx| shell.navigate(Route::Review, cx));
    assert_eq!(shell.read_with(cx, |shell, _| shell.review_done), 0);
}

#[test]
fn review_intervals_show_anki_defaults() {
    assert_eq!(crate::format_interval(60_000), "1分钟");
    assert_eq!(crate::format_interval(330_000), "6分钟");
    assert_eq!(crate::format_interval(600_000), "10分钟");
    assert_eq!(crate::format_interval(4 * 86_400_000), "4天");
}
