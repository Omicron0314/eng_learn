use super::*;
use gpui_kit::{Bounds, TestAppContext, size, test::TestWindowExt};
use std::sync::Mutex;
#[derive(Default)]
struct State {
    saved: usize,
    fail: bool,
    writes: Vec<(String, usize, u64)>,
}
#[derive(Default)]
struct Bus(Mutex<State>);
impl CommandBus for Bus {
    fn dispatch(
        &self,
        command: Command,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = cijian_ports::Result<Response>> + Send + '_>,
    > {
        let result = match command {
            Command::SaveReadingPosition {
                session,
                position,
                revision,
                ..
            } => {
                let mut state = self.0.lock().unwrap();
                state.writes.push((session.clone(), position, revision));
                if state.fail {
                    Err(AppError::new(
                        cijian_ports::ErrorCode::StorageUnavailable,
                        "原创位置保存失败",
                    ))
                } else {
                    state.saved = position;
                    Ok(Response::ReadingPositionSaved {
                        session,
                        position,
                        revision,
                    })
                }
            }
            _ => Err(AppError::new(
                cijian_ports::ErrorCode::InvalidInput,
                "原创测试只保存位置",
            )),
        };
        Box::pin(async move { result })
    }
}
fn open(
    cx: &mut TestAppContext,
    bus: Arc<Bus>,
    session: &str,
) -> (gpui_kit::AnyWindowHandle, gpui_kit::Entity<Shell>) {
    let saved = bus.0.lock().unwrap().saved;
    cx.update(|cx| {
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(gpui_kit::WindowBounds::Windowed(Bounds::new(
                    Default::default(),
                    size(px(1000.), px(1000.)),
                ))),
                ..Default::default()
            },
            cx,
            |_, cx| {
                cx.new(|_| {
                    let body = (0..160)
                        .map(|i| {
                            format!("🙂 Original line {i}: cats walk beside the quiet river.\n")
                        })
                        .collect::<String>();
                    let mut shell = Shell::new();
                    shell.backend = Some(bus);
                    shell.navigation.navigate(Route::Article {
                        id: "original-long".into(),
                        listening: false,
                    });
                    shell.article = Some(cijian_ports::articles::Article {
                        id: "original-long".into(),
                        title: "原创长文".into(),
                        body_hash: "original-long-hash".into(),
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
                        id: session.into(),
                        article_id: "original-long".into(),
                        position: saved,
                        opened_at: 0,
                        completed_at: None,
                    });
                    shell
                })
            },
        )
    })
    .unwrap()
}
#[gpui_kit::test]
fn positions_flush_on_close_restore_after_reopen_and_report_failures(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit_material::init(cx);
        init_component_theme(theme::scheme(false), 1., cx);
        cx.set_reduce_motion(true);
    });
    let bus = Arc::new(Bus::default());
    let (handle, shell) = open(cx, bus.clone(), "first");
    cx.update_window(handle, |_, window, cx| {
        for _ in 0..4 {
            window.render_frame(cx);
        }
        shell
            .read(cx)
            .reader_scroll
            .set_offset(gpui_kit::point(px(0.), px(-1400.)));
        for _ in 0..3 {
            window.render_frame(cx);
        }
        assert!(!shell.read(cx).positions.pending.is_empty());
    })
    .unwrap();
    cx.run_until_parked();
    assert!(bus.0.lock().unwrap().writes.is_empty());
    cx.background_executor
        .advance_clock(std::time::Duration::from_millis(300));
    cx.run_until_parked();
    assert_eq!(bus.0.lock().unwrap().writes.len(), 1);
    cx.update_window(handle, |_, window, cx| {
        let offset = shell.read(cx).reader_scroll.offset();
        shell
            .read(cx)
            .reader_scroll
            .set_offset(gpui_kit::point(offset.x, offset.y - px(600.)));
        for _ in 0..3 {
            window.render_frame(cx);
        }
        shell.update(cx, |shell, cx| {
            assert!(!shell.close_reader_when_saved(window, cx))
        });
    })
    .unwrap();
    cx.run_until_parked();
    let saved = bus.0.lock().unwrap().saved;
    assert!(saved > 0);
    assert_eq!(bus.0.lock().unwrap().writes.len(), 2);
    assert!(shell.read_with(cx, |shell, _| shell.positions.pending.is_empty()));
    let (handle, shell) = open(cx, bus.clone(), "second");
    cx.update_window(handle, |_, window, cx| {
        for _ in 0..6 {
            window.render_frame(cx);
        }
        let shell = shell.read(cx);
        let reader = shell.reader_text.as_ref().unwrap();
        let text = reader.input.read(cx);
        let end = saved
            + shell.article.as_ref().unwrap().body[saved..]
                .chars()
                .next()
                .unwrap()
                .len_utf8();
        let bounds = text.range_to_bounds(&(saved..end)).unwrap();
        let viewport = shell.reader_scroll.bounds();
        assert!(
            bounds.top() >= viewport.top() && bounds.bottom() <= viewport.bottom(),
            "saved source position must be visible after reopening: {bounds:?}, {viewport:?}"
        );
    })
    .unwrap();
    let reading_offset = shell.read_with(cx, |shell, _| shell.reader_scroll.offset().y);
    shell.update(cx, |shell, cx| {
        shell.navigation.navigate(Route::Article {
            id: "original-long".into(),
            listening: true,
        });
        cx.notify();
    });
    cx.update_window(handle, |_, window, cx| {
        for _ in 0..3 {
            window.render_frame(cx);
        }
    })
    .unwrap();
    shell.update(cx, |shell, cx| {
        shell.navigation.navigate(Route::Article {
            id: "original-long".into(),
            listening: false,
        });
        cx.notify();
    });
    cx.update_window(handle, |_, window, cx| {
        for _ in 0..6 {
            window.render_frame(cx);
        }
    })
    .unwrap();
    assert!(shell.read_with(cx, |shell, _| {
        f32::from(shell.reader_scroll.offset().y - reading_offset).abs() < 1.
    }));
    bus.0.lock().unwrap().fail = true;
    cx.update_window(handle, |_, window, cx| {
        let offset = shell.read(cx).reader_scroll.offset();
        shell
            .read(cx)
            .reader_scroll
            .set_offset(gpui_kit::point(offset.x, offset.y - px(800.)));
        for _ in 0..3 {
            window.render_frame(cx);
        }
        shell.update(cx, |shell, cx| {
            assert!(!shell.close_reader_when_saved(window, cx))
        });
    })
    .unwrap();
    cx.run_until_parked();
    assert!(shell.read_with(cx, |shell, _| shell.positions.close_failed
        && !shell.positions.pending.is_empty()));
    assert_eq!(bus.0.lock().unwrap().saved, saved);
    bus.0.lock().unwrap().fail = false;
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("retry-position-close", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(shell.read_with(cx, |shell, _| shell.positions.pending.is_empty()));
    let state = bus.0.lock().unwrap();
    assert!(state.saved > saved);
    assert_eq!(state.writes.len(), 4);
    assert_eq!(state.writes[2], state.writes[3]);
    drop(state);
    bus.0.lock().unwrap().fail = true;
    let (handle, shell) = open(cx, bus.clone(), "third");
    cx.update_window(handle, |_, window, cx| {
        for _ in 0..6 {
            window.render_frame(cx);
        }
        let offset = shell.read(cx).reader_scroll.offset();
        shell
            .read(cx)
            .reader_scroll
            .set_offset(gpui_kit::point(offset.x, offset.y - px(600.)));
        for _ in 0..3 {
            window.render_frame(cx);
        }
        shell.update(cx, |shell, cx| {
            assert!(!shell.close_reader_when_saved(window, cx))
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("discard-position-close", cx);
    })
    .unwrap();
    cx.background_executor
        .advance_clock(std::time::Duration::from_millis(300));
    cx.run_until_parked();
    assert!(shell.read_with(cx, |shell, _| shell.positions.pending.is_empty()));
    assert_eq!(bus.0.lock().unwrap().writes.len(), 5);
}
