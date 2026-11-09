mod inbox;
mod library;
mod review;
use super::*;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{Bounds, TestAppContext, size};

#[gpui_kit::test]
fn separate_windows_render_their_saved_theme_without_changing_global_paint(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit_material::init(cx);
        init_component_theme(theme::scheme(false), 1., cx);
        cx.set_reduce_motion(true);
    });
    let windows = [true, false].map(|dark| {
        cx.update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| {
                cx.new(|_| {
                    let mut shell = Shell::new();
                    shell.settings.theme = if dark {
                        cijian_domain::settings::Theme::Dark
                    } else {
                        cijian_domain::settings::Theme::Light
                    };
                    shell
                })
            })
        })
        .unwrap()
    });
    for _ in 0..2 {
        for (i, (handle, shell)) in windows.iter().enumerate() {
            cx.update_window(*handle, |_, window, cx| window.render_frame(cx))
                .unwrap();
            assert_eq!(shell.read_with(cx, |shell, _| shell.rendered_dark), i == 0);
            assert!(
                !gpui_kit_material::theme::is_dark(),
                "a window must not replace application paint"
            );
        }
    }
}

#[gpui_kit::test]
fn navigation_routes_and_compact_targets(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit_material::init(cx);
        init_component_theme(theme::scheme(false), 1.0, cx);
        cx.set_reduce_motion(true);
    });
    for width in [360., 1000.] {
        let (handle, shell) = cx
            .update(|cx| {
                gpui_kit::open_window(
                    WindowOptions {
                        window_bounds: Some(gpui_kit::WindowBounds::Windowed(Bounds::new(
                            Default::default(),
                            size(px(width), px(800.)),
                        ))),
                        ..Default::default()
                    },
                    cx,
                    |_, cx| cx.new(|_| Shell::new()),
                )
            })
            .unwrap();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let chrome = window.find("window-chrome").bounds();
            assert_eq!(chrome.origin.y, px(0.));
            assert_eq!(chrome.size.width, px(width));
            assert_eq!(chrome.size.height, px(design::TITLE_BAR_HEIGHT));
            let chrome_bottom = chrome.origin.y + chrome.size.height;
            let title = window.find("window-title").bounds();
            assert!(title.origin.y >= chrome.origin.y);
            assert!(
                title.origin.y + title.size.height <= chrome_bottom,
                "window title must stay inside the title bar"
            );
            let nav = window.find(gpui_kit::ElementId::NamedChild(
                gpui_kit::ElementId::from("main-nav").into(),
                "today".into(),
            ));
            assert!(
                nav.bounds().origin.y >= chrome_bottom,
                "navigation must start below the window chrome"
            );
            let initial_focus = window.focused(cx);
            window.press("tab", cx);
            window.render_frame(cx);
            assert_ne!(
                window.focused(cx),
                initial_focus,
                "Tab must move focus from the application root"
            );
            let mut previous_right = px(0.);
            for destination in ["today", "review", "articles", "library", "inbox", "stats"] {
                let id = gpui_kit::ElementId::NamedChild(
                    gpui_kit::ElementId::from("main-nav").into(),
                    destination.into(),
                );
                let bounds = window.find(id).bounds();
                assert!(bounds.size.width >= px(48.));
                assert!(bounds.origin.x + bounds.size.width <= px(width));
                if width < 600. {
                    assert!(
                        bounds.origin.x >= previous_right,
                        "navigation targets overlap"
                    );
                    previous_right = bounds.origin.x + bounds.size.width;
                }
            }
            let settings = window.find("settings").bounds();
            assert!(settings.origin.y >= px(0.));
            assert!(
                settings.origin.y + settings.size.height <= px(800.),
                "settings outside viewport at width {width}: {settings:?}"
            );
            window.click(
                gpui_kit::ElementId::NamedChild(
                    gpui_kit::ElementId::from("main-nav").into(),
                    "articles".into(),
                ),
                cx,
            );
            window.render_frame(cx);
        })
        .unwrap();
        assert_eq!(
            shell.read_with(cx, |s, _| s.navigation.current().section()),
            "articles"
        );
        cx.update_window(handle, |_, window, cx| {
            window.click("settings", cx);
            window.render_frame(cx);
            window.click("theme", cx);
            window.render_frame(cx);
        })
        .unwrap();
        assert!(shell.read_with(cx, |s, _| s.dark));
    }
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
#[gpui_kit::test]
fn linux_window_chrome_offers_minimize_fullscreen_and_close(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit_material::init(cx);
        init_component_theme(theme::scheme(false), 1.0, cx);
        cx.set_reduce_motion(true);
    });
    for width in [360., 1000.] {
        let (handle, _shell) = cx
            .update(|cx| {
                gpui_kit::open_window(
                    WindowOptions {
                        window_bounds: Some(gpui_kit::WindowBounds::Windowed(Bounds::new(
                            Default::default(),
                            size(px(width), px(800.)),
                        ))),
                        ..Default::default()
                    },
                    cx,
                    |_, cx| {
                        cx.new(|_| {
                            let mut shell = Shell::new();
                            shell.caption = Some(gpui_kit_material::components::Caption::Linux);
                            shell
                        })
                    },
                )
            })
            .unwrap();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let minimize = window.find("window-caption-minimize").bounds();
            let fullscreen = window.find("window-caption-fullscreen").bounds();
            let close = window.find("window-caption-close").bounds();
            for (id, bounds) in [
                ("minimize", minimize),
                ("fullscreen", fullscreen),
                ("close", close),
            ] {
                assert_eq!(bounds.size, size(px(32.), px(32.)), "{id} target");
                assert!(bounds.origin.y >= px(0.));
                assert!(
                    bounds.origin.y + bounds.size.height <= px(design::TITLE_BAR_HEIGHT),
                    "{id} outside the title bar"
                );
                assert!(
                    bounds.origin.x + bounds.size.width <= px(width),
                    "{id} outside the {width}px window"
                );
            }
            let title = window.find("window-title").bounds();
            assert!(
                title.origin.x + title.size.width <= minimize.origin.x,
                "window title overlaps the caption targets at {width}px"
            );
            assert!(minimize.origin.x < fullscreen.origin.x);
            assert!(fullscreen.origin.x < close.origin.x);

            window.click("window-caption-fullscreen", cx);
            window.render_frame(cx);
            assert!(window.is_fullscreen());
            window.click("window-caption-fullscreen", cx);
            window.render_frame(cx);
            assert!(!window.is_fullscreen());
        })
        .unwrap();
    }
}

#[gpui_kit::test]
fn saved_notice_dismisses_without_another_action(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit_material::init(cx);
        init_component_theme(theme::scheme(false), 1.0, cx);
        cx.set_reduce_motion(true);
    });
    let (handle, shell) = cx
        .update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| {
                cx.new(|_| {
                    let mut shell = Shell::new();
                    shell.notice = Some("词卡已保存".into());
                    shell
                })
            })
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    assert!(shell.read_with(cx, |shell, _| shell.notice.is_some()));
    cx.background_executor
        .advance_clock(std::time::Duration::from_secs(5));
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    assert!(
        shell.read_with(cx, |shell, _| shell.notice.is_none()),
        "a saved notice must not persist across later clicks"
    );
}

#[derive(Default)]
struct GenerationBus(std::sync::Mutex<Vec<Command>>);
impl CommandBus for GenerationBus {
    fn dispatch(
        &self,
        command: Command,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = cijian_ports::Result<Response>> + Send + '_>,
    > {
        self.0.lock().unwrap().push(command);
        Box::pin(async {
            Err(AppError::new(
                cijian_ports::ErrorCode::StaleState,
                "原创测试观察失败",
            ))
        })
    }
}

#[gpui_kit::test]
fn generation_poll_resumes_after_a_busy_tick_and_observes_the_job(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit_material::init(cx);
        init_component_theme(theme::scheme(false), 1., cx);
        cx.set_reduce_motion(true);
    });
    let bus = Arc::new(GenerationBus::default());
    let (handle, shell) = cx
        .update(|cx| {
            let bus = bus.clone();
            gpui_kit::open_window(WindowOptions::default(), cx, move |_, cx| {
                cx.new(|_| {
                    let mut shell = Shell::new();
                    shell.backend = Some(bus);
                    shell.navigation.navigate(Route::Articles);
                    shell.loading = true;
                    shell.generation_job = Some(cijian_ports::articles::GenerationJob {
                        id: "original-job".into(),
                        operation_id: "original-operation".into(),
                        status: cijian_ports::articles::JobStatus::Running,
                        result_ref: None,
                        error: None,
                        config_revision: 1,
                    });
                    shell
                })
            })
        })
        .unwrap();
    shell.update(cx, |shell, cx| shell.poll_generation(cx));
    cx.background_executor
        .advance_clock(std::time::Duration::from_millis(750));
    cx.run_until_parked();
    assert!(
        !bus.0
            .lock()
            .unwrap()
            .iter()
            .any(|command| matches!(command, Command::ObserveJob { .. })),
        "a busy UI does not issue a second request"
    );
    shell.update(cx, |shell, _| shell.loading = false);
    cx.background_executor
        .advance_clock(std::time::Duration::from_millis(750));
    cx.run_until_parked();
    assert!(
        bus.0
            .lock()
            .unwrap()
            .iter()
            .any(|command| matches!(command, Command::ObserveJob { job } if job=="original-job")),
        "the poll must reschedule instead of stopping while a request is in flight"
    );
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
}

#[gpui_kit::test]
fn long_single_paragraph_article_grows_to_its_wrapped_height(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit_material::init(cx);
        init_component_theme(theme::scheme(false), 1., cx);
        cx.set_reduce_motion(true);
    });
    let body = (0..18)
        .map(|index| format!("original sentence {index} with ordinary words"))
        .collect::<Vec<_>>()
        .join(" ");
    let article = cijian_ports::articles::Article {
        id: "original-article".into(),
        title: "Original long paragraph".into(),
        body_hash: "original-long-hash".into(),
        analysis: cijian_domain::analysis::analyze(
            &body,
            &Default::default(),
            &Default::default(),
            &Default::default(),
            &Default::default(),
        ),
        body: body.clone(),
        genre: cijian_domain::settings::Genre::Story,
        tier: cijian_domain::settings::Tier::Easy,
        diagnostics: Vec::new(),
        created_at: 0,
        read_at: None,
    };
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
                move |_, cx| {
                    cx.new(|_| {
                        let mut shell = Shell::new();
                        shell.navigation.navigate(Route::Article {
                            id: article.id.clone(),
                            listening: false,
                        });
                        shell.reading_session = Some(cijian_ports::reading::ReadingSession {
                            id: "original-session".into(),
                            article_id: article.id.clone(),
                            position: 0,
                            opened_at: 0,
                            completed_at: None,
                        });
                        shell.article = Some(article);
                        shell
                    })
                },
            )
        })
        .unwrap();
    for _ in 0..3 {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .unwrap();
    }
    cx.update_window(handle, |_, window, _| {
        let bounds = window.find("reader-text").bounds();
        assert!(
            bounds.size.height >= px(200.),
            "a 500-character single paragraph must not be clipped to a few rows: {bounds:?}"
        );
    })
    .unwrap();
}

#[derive(Default)]
struct ArticleBus(std::sync::Mutex<Vec<Command>>);
impl CommandBus for ArticleBus {
    fn dispatch(
        &self,
        command: Command,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = cijian_ports::Result<Response>> + Send + '_>,
    > {
        self.0.lock().unwrap().push(command);
        Box::pin(async { Ok(Response::Articles(Vec::new())) })
    }
}

#[gpui_kit::test]
fn article_history_and_today_tabs_request_distinct_study_day_ranges(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit_material::init(cx);
        init_component_theme(theme::scheme(false), 1., cx);
        cx.set_reduce_motion(true);
    });
    let bus = Arc::new(ArticleBus::default());
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
                        shell.navigation.navigate(Route::Articles);
                        shell
                    })
                },
            )
        })
        .unwrap();
    for _ in 0..2 {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .unwrap();
    }
    cx.update_window(handle, |_, window, cx| {
        window.click("articles-history", cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(shell.read_with(cx, |shell, _| shell.articles_history));
    assert!(bus.0.lock().unwrap().iter().any(|command| matches!(
        command,
        Command::LoadArticles {
            from: None,
            until: Some(_),
            page: 0
        }
    )));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("articles-today", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(!shell.read_with(cx, |shell, _| shell.articles_history));
    assert!(bus.0.lock().unwrap().iter().any(|command| matches!(
        command,
        Command::LoadArticles {
            from: Some(_),
            until: Some(_),
            page: 0
        }
    )));
}

#[derive(Default)]
struct ReadingBus(std::sync::Mutex<Vec<Command>>);
impl CommandBus for ReadingBus {
    fn dispatch(
        &self,
        command: Command,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = cijian_ports::Result<Response>> + Send + '_>,
    > {
        self.0.lock().unwrap().push(command.clone());
        let result = match command {
            Command::CompleteReading {
                article,
                body_hash: _,
                session,
            } => Ok(Response::Completed(cijian_ports::reading::ReadingSession {
                id: session,
                article_id: article,
                position: 0,
                opened_at: 0,
                completed_at: Some(1),
            })),
            _ => Err(AppError::new(
                cijian_ports::ErrorCode::InvalidInput,
                "原创测试只完成阅读",
            )),
        };
        Box::pin(async move { result })
    }
}

#[gpui_kit::test]
fn complete_reading_reaches_the_command_bus(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit_material::init(cx);
        init_component_theme(theme::scheme(false), 1., cx);
        cx.set_reduce_motion(true);
    });
    let bus = Arc::new(ReadingBus::default());
    let body = (0..18)
        .map(|index| format!("original sentence {index} with ordinary words"))
        .collect::<Vec<_>>()
        .join(" ");
    let (handle, shell) = cx
        .update(|cx| {
            let bus = bus.clone();
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(gpui_kit::WindowBounds::Windowed(Bounds::new(
                        Default::default(),
                        size(px(1440.), px(900.)),
                    ))),
                    ..Default::default()
                },
                cx,
                move |_, cx| {
                    cx.new(|_| {
                        let mut shell = Shell::new();
                        shell.backend = Some(bus);
                        shell.navigation.navigate(Route::Article {
                            id: "original-article".into(),
                            listening: false,
                        });
                        shell.article = Some(cijian_ports::articles::Article {
                            id: "original-article".into(),
                            title: "Original article".into(),
                            body_hash: "original-hash".into(),
                            analysis: cijian_domain::analysis::analyze(
                                &body,
                                &Default::default(),
                                &Default::default(),
                                &Default::default(),
                                &Default::default(),
                            ),
                            body,
                            genre: cijian_domain::settings::Genre::Story,
                            tier: cijian_domain::settings::Tier::Easy,
                            diagnostics: Vec::new(),
                            created_at: 0,
                            read_at: None,
                        });
                        shell.reading_session = Some(cijian_ports::reading::ReadingSession {
                            id: "original-session".into(),
                            article_id: "original-article".into(),
                            position: 0,
                            opened_at: 0,
                            completed_at: None,
                        });
                        shell
                    })
                },
            )
        })
        .unwrap();
    for _ in 0..2 {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .unwrap();
    }
    cx.update_window(handle, |_, window, cx| {
        window.click("complete-reading", cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(bus.0.lock().unwrap().iter().any(|command| matches!(
        command,
        Command::CompleteReading { article, session, .. }
            if article == "original-article" && session == "original-session"
    )));
    assert!(shell.read_with(cx, |shell, _| {
        shell
            .reading_session
            .as_ref()
            .is_some_and(|session| session.completed_at.is_some())
    }));
}

/// Answers the polling commands and keeps every dispatch for assertions.
#[derive(Default)]
struct QuietBus(std::sync::Mutex<Vec<Command>>);
impl CommandBus for QuietBus {
    fn dispatch(
        &self,
        command: Command,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = cijian_ports::Result<Response>> + Send + '_>,
    > {
        self.0.lock().unwrap().push(command.clone());
        let response = match command {
            Command::LoadArticles { .. } => Ok(Response::Articles(Vec::new())),
            Command::ObserveJob { job } => {
                Ok(Response::Job(cijian_ports::articles::GenerationJob {
                    id: job,
                    operation_id: "original-operation".into(),
                    status: cijian_ports::articles::JobStatus::Running,
                    result_ref: None,
                    error: None,
                    config_revision: 1,
                }))
            }
            Command::AudioStatus => {
                let mut playback = cijian_domain::audio::Playback::default();
                let _ = playback.start(
                    "original-session".into(),
                    "original-article".into(),
                    Some(12.),
                );
                Ok(Response::Audio(playback))
            }
            _ => Err(AppError::new(
                cijian_ports::ErrorCode::UnsupportedCapability,
                "原创测试未提供此操作",
            )),
        };
        Box::pin(async move { response })
    }
}

/// Never answers, so a request stays in flight while the test advances time.
struct PendingBus;
impl CommandBus for PendingBus {
    fn dispatch(
        &self,
        _: Command,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = cijian_ports::Result<Response>> + Send + '_>,
    > {
        Box::pin(std::future::pending())
    }
}

fn quiet_window(
    cx: &mut TestAppContext,
    route: Route,
    bus: Arc<dyn CommandBus>,
) -> (gpui_kit::AnyWindowHandle, Entity<Shell>) {
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
                        size(px(1200.), px(900.)),
                    ))),
                    ..Default::default()
                },
                cx,
                move |_, cx| {
                    cx.new(|_| {
                        let mut shell = Shell::new();
                        shell.backend = Some(bus);
                        shell.navigation.navigate(route);
                        shell
                    })
                },
            )
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    (handle, shell)
}

#[gpui_kit::test]
fn a_generation_poll_never_flashes_the_busy_window(cx: &mut TestAppContext) {
    let bus = Arc::new(QuietBus::default());
    let (handle, shell) = quiet_window(cx, Route::Articles, bus.clone());
    shell.update(cx, |shell, _| {
        shell.notice = Some("已记录本次阅读完成".into());
        shell.generation_job = Some(cijian_ports::articles::GenerationJob {
            id: "original-job".into(),
            operation_id: "original-operation".into(),
            status: cijian_ports::articles::JobStatus::Running,
            result_ref: None,
            error: None,
            config_revision: 1,
        });
    });
    shell.update(cx, |shell, cx| shell.poll_generation(cx));
    cx.background_executor
        .advance_clock(std::time::Duration::from_millis(750));
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    assert!(
        bus.0
            .lock()
            .unwrap()
            .iter()
            .any(|command| matches!(command, Command::ObserveJob { .. })),
        "the poll still refreshes the job"
    );
    let (loading, busy, notice) = shell.read_with(cx, |shell, _| {
        (shell.loading, shell.busy_visible, shell.notice.clone())
    });
    assert!(!loading, "a poll must not take the busy flag");
    assert!(!busy, "a poll must not show the busy message");
    assert_eq!(
        notice.as_deref(),
        Some("已记录本次阅读完成"),
        "a poll must not replace the visible notice"
    );
}

#[gpui_kit::test]
fn an_audio_poll_while_listening_keeps_the_message_window_still(cx: &mut TestAppContext) {
    let bus = Arc::new(QuietBus::default());
    let (handle, shell) = quiet_window(
        cx,
        Route::Article {
            id: "original-article".into(),
            listening: true,
        },
        bus.clone(),
    );
    shell.update(cx, |shell, _| {
        shell.playback = cijian_domain::audio::Playback::default();
        let _ = shell.playback.start(
            "original-session".into(),
            "original-article".into(),
            Some(12.),
        );
        shell.notice = Some("已听完，可以填写听力难度反馈".into());
    });
    shell.update(cx, |shell, cx| shell.poll_audio(cx));
    cx.background_executor
        .advance_clock(std::time::Duration::from_millis(750));
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    assert!(
        bus.0
            .lock()
            .unwrap()
            .iter()
            .any(|command| matches!(command, Command::AudioStatus)),
        "playback is still observed"
    );
    assert!(
        !shell.read_with(cx, |shell, _| shell.loading)
            && !shell.read_with(cx, |shell, _| shell.busy_visible)
    );
    assert!(
        shell.read_with(cx, |shell, _| shell.snackbar().is_some()),
        "the listening notice stays on screen instead of blinking"
    );
    assert_eq!(
        shell
            .read_with(cx, |shell, _| shell.notice.clone())
            .as_deref(),
        Some("已听完，可以填写听力难度反馈")
    );
}

#[gpui_kit::test]
fn only_slow_requests_show_the_busy_window(cx: &mut TestAppContext) {
    let (handle, shell) = quiet_window(cx, Route::Articles, Arc::new(PendingBus));
    shell.update(cx, |shell, cx| {
        shell.request(Command::LoadStats { page: 0 }, cx)
    });
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    assert!(!shell.read_with(cx, |shell, _| shell.busy_visible));
    assert!(
        shell.read_with(cx, |shell, _| shell.snackbar().is_none()),
        "a fresh request must not flash a message before it is slow"
    );
    cx.background_executor
        .advance_clock(std::time::Duration::from_millis(500));
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    assert!(shell.read_with(cx, |shell, _| shell.busy_visible));
    assert!(
        shell.read_with(cx, |shell, _| shell.snackbar().is_some()),
        "a request that really takes time still reports progress"
    );
}

#[gpui_kit::test]
fn an_instant_request_never_shows_the_busy_window(cx: &mut TestAppContext) {
    let (handle, shell) = quiet_window(cx, Route::Articles, Arc::new(QuietBus::default()));
    shell.update(cx, |shell, cx| shell.load_articles(0, cx));
    cx.run_until_parked();
    cx.background_executor
        .advance_clock(std::time::Duration::from_millis(500));
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    assert!(!shell.read_with(cx, |shell, _| shell.busy_visible));
    assert!(shell.read_with(cx, |shell, _| shell.snackbar().is_none()));
}
