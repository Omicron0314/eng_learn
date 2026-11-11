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
                            pronunciation: Default::default(),
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
                                pronunciation: Default::default(),
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
                    pronunciation: Default::default(),
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
                            pronunciation: Default::default(),
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
                            pronunciation: Default::default(),
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

/// Records the word-audio commands the review card dispatches.
#[derive(Default)]
struct WordAudioBus(std::sync::Mutex<Vec<Command>>);
impl CommandBus for WordAudioBus {
    fn dispatch(
        &self,
        command: Command,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = cijian_ports::Result<Response>> + Send + '_>,
    > {
        self.0.lock().unwrap().push(command.clone());
        let result = match command {
            Command::PlayWord { accent, .. } => Ok(Response::WordAudio { accent }),
            // Switching the accent on the card persists the preference.
            Command::SetPronunciationAccent { accent } => Ok(Response::Preferences(
                cijian_domain::settings::DevicePreferences {
                    pronunciation_accent: accent,
                    ..Default::default()
                },
            )),
            _ => Err(AppError::new(
                cijian_ports::ErrorCode::InvalidInput,
                "原创测试只播放单词发音",
            )),
        };
        Box::pin(async move { result })
    }
}

#[gpui_kit::test]
fn pronunciation_chip_switches_accent_and_ipa_replays_the_word(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit_material::init(cx);
        init_component_theme(theme::scheme(false), 1., cx);
        cx.set_reduce_motion(true);
    });
    let bus = std::sync::Arc::new(WordAudioBus::default());
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
                        shell.review = vec![cijian_ports::library::CardView {
                            id: "word-card".into(),
                            lexeme_id: "word-lexeme".into(),
                            front: "overlook".into(),
                            meaning: "忽略，忽视".into(),
                            example: String::new(),
                            text_source: "imported".into(),
                            suspended: false,
                            archived: false,
                            revision: 1,
                            created_at: 0,
                            srs: cijian_domain::srs::SrsState::new(0),
                            pronunciation: cijian_domain::audio::Pronunciation {
                                ipa_uk: "/ˌəʊvəˈlʊk/".into(),
                                ipa_us: "/ˌoʊvərˈlʊk/".into(),
                                audio_uk: true,
                                audio_us: true,
                            },
                        }];
                        shell
                    })
                },
            )
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find_by_label("播放单词发音").is_some());
        window.click("review-ipa", cx);
    })
    .unwrap();
    cx.run_until_parked();
    // The IPA replays the currently selected accent, which defaults to American.
    assert!(matches!(
        bus.0.lock().unwrap().last(),
        Some(Command::PlayWord {
            accent: cijian_domain::audio::Accent::Us,
            ..
        })
    ));
    cx.update_window(handle, |_, window, cx| {
        window.click("review-accent", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let commands = bus.0.lock().unwrap();
    assert!(matches!(
        commands.last(),
        Some(Command::PlayWord {
            accent: cijian_domain::audio::Accent::Uk,
            ..
        })
    ));
    drop(commands);
    // Pronunciation is fire-and-forget: it must not take the busy flag, blink a
    // message window, or block revealing the card.
    assert!(!shell.read_with(cx, |shell, _| shell.loading));
    cx.background_executor
        .advance_clock(std::time::Duration::from_millis(500));
    cx.run_until_parked();
    assert!(
        !shell.read_with(cx, |shell, _| shell.busy_visible),
        "playing a word never reports progress as a request"
    );
    assert!(shell.read_with(cx, |shell, _| shell.snackbar().is_none()));
    cx.update_window(handle, |_, window, cx| {
        window.press("space", cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert!(
        shell.read_with(cx, |shell, _| shell.revealed),
        "playback must not swallow the next reveal"
    );
}

/// Serves one review queue per call so a test can advance card by card.
struct QueueBus(std::sync::Mutex<(Vec<Vec<cijian_ports::library::CardView>>, Vec<Command>)>);
impl CommandBus for QueueBus {
    fn dispatch(
        &self,
        command: Command,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = cijian_ports::Result<Response>> + Send + '_>,
    > {
        let mut guard = self.0.lock().unwrap();
        let result = match command {
            Command::LoadReview => {
                let cards = if guard.0.len() > 1 {
                    guard.0.remove(0)
                } else {
                    guard.0.first().cloned().unwrap_or_default()
                };
                Ok(Response::Review {
                    cards,
                    waiting: false,
                    intervals: None,
                    stats: Box::new(cijian_ports::stats::StatsSnapshot {
                        today: Default::default(),
                        week: Default::default(),
                        reading_completion: None,
                        feedback_too_hard: None,
                        feedback_just_right: None,
                        feedback_too_easy: None,
                        heatmap: Vec::new(),
                        coverage: Vec::new(),
                        unknown_median_all: None,
                        unknown_median_week: None,
                        learned: Vec::new(),
                        learned_total: 0,
                        experiment: cijian_ports::stats::ExperimentReport {
                            version: "original".into(),
                            exposed: cijian_ports::stats::ExperimentGroup {
                                sample: 0,
                                again: 0,
                                unreviewed: 0,
                                excluded: 0,
                                again_rate: None,
                            },
                            control: cijian_ports::stats::ExperimentGroup {
                                sample: 0,
                                again: 0,
                                unreviewed: 0,
                                excluded: 0,
                                again_rate: None,
                            },
                            difference_percentage_points: None,
                        },
                    }),
                    books: Vec::new(),
                    inbox_pending: 0,
                })
            }
            other => {
                guard.1.push(other);
                Ok(Response::WordAudio {
                    accent: cijian_domain::audio::Accent::Us,
                })
            }
        };
        Box::pin(async move { result })
    }
}

fn review_card(id: &str, word: &str, audio: bool) -> cijian_ports::library::CardView {
    cijian_ports::library::CardView {
        id: id.into(),
        lexeme_id: format!("{id}-lexeme"),
        front: word.into(),
        meaning: "原创释义".into(),
        example: String::new(),
        text_source: "imported".into(),
        suspended: false,
        archived: false,
        revision: 1,
        created_at: 0,
        srs: cijian_domain::srs::SrsState::new(0),
        pronunciation: cijian_domain::audio::Pronunciation {
            ipa_uk: "/original/".into(),
            ipa_us: "/original/".into(),
            audio_uk: audio,
            audio_us: audio,
        },
    }
}

#[gpui_kit::test]
fn a_new_card_is_read_aloud_once_and_silent_cards_do_not_retry(cx: &mut TestAppContext) {
    // The preference is on by default; the switch covers the disabled case below.
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit_material::init(cx);
        init_component_theme(theme::scheme(false), 1., cx);
        cx.set_reduce_motion(true);
    });
    let bus = std::sync::Arc::new(QueueBus(std::sync::Mutex::new((
        vec![
            vec![review_card("first", "overlook", true)],
            vec![review_card("second", "abandon", true)],
            vec![review_card("third", "silent", false)],
        ],
        Vec::new(),
    ))));
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
                        shell
                    })
                },
            )
        })
        .unwrap();
    let load = |shell: &gpui_kit::Entity<Shell>, cx: &mut TestAppContext| {
        shell.update(cx, |shell, cx| shell.request(Command::LoadReview, cx));
        cx.run_until_parked();
    };
    load(&shell, cx);
    load(&shell, cx);
    load(&shell, cx);
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    let played = bus
        .0
        .lock()
        .unwrap()
        .1
        .iter()
        .filter_map(|command| match command {
            Command::PlayWord { lexeme, accent } => Some((lexeme.clone(), *accent)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        played,
        vec![
            ("first-lexeme".to_owned(), cijian_domain::audio::Accent::Us),
            ("second-lexeme".to_owned(), cijian_domain::audio::Accent::Us),
        ],
        "each new card is read once; a card without a recording stays silent"
    );
    let _ = shell;
}

#[gpui_kit::test]
fn auto_play_preference_off_keeps_new_cards_silent(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit_material::init(cx);
        init_component_theme(theme::scheme(false), 1., cx);
        cx.set_reduce_motion(true);
    });
    let bus = std::sync::Arc::new(QueueBus(std::sync::Mutex::new((
        vec![vec![review_card("first", "overlook", true)]],
        Vec::new(),
    ))));
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
                        shell.review_auto_play = false;
                        shell
                    })
                },
            )
        })
        .unwrap();
    shell.update(cx, |shell, cx| shell.request(Command::LoadReview, cx));
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    assert!(
        !bus.0
            .lock()
            .unwrap()
            .1
            .iter()
            .any(|command| matches!(command, Command::PlayWord { .. })),
        "auto-play off must not read a new card aloud"
    );
}

#[gpui_kit::test]
fn space_reveals_after_reopening_review_from_the_nav(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit_material::init(cx);
        init_component_theme(theme::scheme(false), 1., cx);
        cx.set_reduce_motion(true);
    });
    let bus = std::sync::Arc::new(QueueBus(std::sync::Mutex::new((
        vec![vec![review_card("first", "overlook", true)]],
        Vec::new(),
    ))));
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
                        shell.backend = Some(bus);
                        shell.review_auto_play = false;
                        shell
                    })
                },
            )
        })
        .unwrap();
    let review_nav = || {
        gpui_kit::ElementId::NamedChild(
            gpui_kit::ElementId::from("main-nav").into(),
            "review".into(),
        )
    };
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(review_nav(), cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("space", cx);
        assert!(
            shell.read_with(cx, |shell, _| shell.revealed),
            "space reveals the card"
        );
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    assert!(
        shell.read_with(cx, |shell, _| shell.revealed),
        "space must not re-trigger navigation and reload the card"
    );
    // The same focused nav item must not swallow the rating keys either.
    cx.update_window(handle, |_, window, cx| window.press("3", cx))
        .unwrap();
    cx.run_until_parked();
    assert!(
        bus.0
            .lock()
            .unwrap()
            .1
            .iter()
            .any(|command| matches!(command, Command::Rate { .. })),
        "a number key rates the revealed card instead of activating the nav"
    );
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.navigation.current().clone()),
        Route::Review
    );
}
