use super::*;
use gpui_kit::{
    Bounds, InputEvent as _, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    TestAppContext, point, size, test::TestWindowExt,
};
use std::sync::Mutex;
const BODY: &str = "🙂 Cats run.\nDogs rest.";
#[derive(Default)]
struct Gate(Mutex<(bool, Option<std::task::Waker>)>);
#[derive(Default)]
struct Bus(Mutex<Vec<Command>>, Mutex<Option<Arc<Gate>>>);
impl CommandBus for Bus {
    fn dispatch(
        &self,
        command: Command,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = cijian_ports::Result<Response>> + Send + '_>,
    > {
        self.0.lock().unwrap().push(command.clone());
        let response = match command {
            Command::Lookup {
                article,
                body_hash,
                range,
            } => Ok(Response::Lookup {
                context: cijian_ports::reading::LookupContext {
                    article_id: article,
                    body_hash,
                    selection: cijian_domain::reading::select(BODY, range).unwrap(),
                },
                revision: 0,
                meaning: "原创释义".into(),
            }),
            _ => Err(AppError::new(
                cijian_ports::ErrorCode::InvalidInput,
                "仅允许点词",
            )),
        };
        let gate = self.1.lock().unwrap().take();
        Box::pin(async move {
            if let Some(gate) = gate {
                std::future::poll_fn(|cx| {
                    let mut state = gate.0.lock().unwrap();
                    if state.0 {
                        std::task::Poll::Ready(())
                    } else {
                        state.1 = Some(cx.waker().clone());
                        std::task::Poll::Pending
                    }
                })
                .await;
            }
            response
        })
    }
}
fn point_at(
    shell: &Entity<Shell>,
    range: std::ops::Range<usize>,
    cx: &App,
) -> gpui_kit::Point<gpui_kit::Pixels> {
    let bounds = shell
        .read(cx)
        .reader_text
        .as_ref()
        .unwrap()
        .input
        .read(cx)
        .range_to_bounds(&range)
        .unwrap();
    bounds.origin + (bounds.size / 2.).into()
}
#[gpui_kit::test]
fn reader_preserves_source_selection_and_returns_focus_after_lookup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit_material::init(cx);
        init_component_theme(theme::scheme(false), 1., cx);
        cx.set_reduce_motion(true);
    });
    for width in [360., 1000.] {
        let bus = Arc::new(Bus::default());
        let (handle, shell) = cx
            .update(|cx| {
                gpui_kit::open_window(
                    WindowOptions {
                        window_bounds: Some(gpui_kit::WindowBounds::Windowed(Bounds::new(
                            Default::default(),
                            size(px(width), px(1000.)),
                        ))),
                        ..Default::default()
                    },
                    cx,
                    |_, cx| {
                        cx.new(|_| {
                            let mut shell = Shell::new();
                            shell.backend = Some(bus.clone());
                            shell.navigation.navigate(Route::Article {
                                id: "original".into(),
                                listening: false,
                            });
                            shell.article = Some(cijian_ports::articles::Article {
                                id: "original".into(),
                                title: "原创阅读".into(),
                                body: BODY.into(),
                                body_hash: "original-hash".into(),
                                genre: cijian_domain::settings::Genre::Story,
                                tier: cijian_domain::settings::Tier::Easy,
                                analysis: cijian_domain::analysis::analyze(
                                    BODY,
                                    &Default::default(),
                                    &Default::default(),
                                    &Default::default(),
                                    &Default::default(),
                                ),
                                diagnostics: Vec::new(),
                                created_at: 0,
                                read_at: None,
                            });
                            shell
                        })
                    },
                )
            })
            .unwrap();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let focus = shell
                .read(cx)
                .reader_text
                .as_ref()
                .unwrap()
                .input
                .read(cx)
                .focus_handle(cx);
            window.focus(&focus, cx);
            window.press("ctrl-a", cx);
            window.press("ctrl-c", cx);
            assert_eq!(
                cx.read_from_clipboard().unwrap().text().as_deref(),
                Some(BODY)
            );
            window.input("must not mutate", cx);
            window.render_frame(cx);
            assert_eq!(
                shell
                    .read(cx)
                    .reader_text
                    .as_ref()
                    .unwrap()
                    .input
                    .read(cx)
                    .value()
                    .as_ref(),
                BODY
            );
            let anchor = point_at(&shell, 5..9, cx);
            let offset = shell.read(cx).reader_scroll.offset();
            shell
                .read(cx)
                .reader_scroll
                .set_offset(point(offset.x, offset.y + px(400.) - anchor.y));
            window.render_frame(cx);
            let position = point_at(&shell, 5..9, cx);
            window.dispatch_event(
                MouseDownEvent {
                    button: MouseButton::Left,
                    position,
                    modifiers: Default::default(),
                    click_count: 1,
                    first_mouse: false,
                }
                .to_platform_input(),
                cx,
            );
            window.dispatch_event(
                MouseUpEvent {
                    button: MouseButton::Left,
                    position,
                    modifiers: Default::default(),
                    click_count: 1,
                }
                .to_platform_input(),
                cx,
            );
        })
        .unwrap();
        cx.run_until_parked();
        assert!(matches!(
            bus.0.lock().unwrap().as_slice(),
            [Command::Lookup {
                range: TextRange { start: 5, end: 9 },
                ..
            }]
        ));
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            if !shell.read(cx).compact_reader {
                let text = shell
                    .read(cx)
                    .reader_text
                    .as_ref()
                    .unwrap()
                    .input
                    .read(cx)
                    .input_bounds();
                assert!(
                    text.right() <= window.find("mark-known").bounds().left(),
                    "expanded word panel must reserve space beside the text"
                );
            }
            window.press("escape", cx);
            window.render_frame(cx);
            assert!(
                shell
                    .read(cx)
                    .reader_text
                    .as_ref()
                    .unwrap()
                    .input
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window)
            );
            let start = point_at(&shell, 5..6, cx);
            let end = point_at(&shell, 16..17, cx);
            window.dispatch_event(
                MouseDownEvent {
                    button: MouseButton::Left,
                    position: start,
                    modifiers: Default::default(),
                    click_count: 1,
                    first_mouse: false,
                }
                .to_platform_input(),
                cx,
            );
            window.dispatch_event(
                MouseMoveEvent {
                    position: end,
                    pressed_button: Some(MouseButton::Left),
                    modifiers: Default::default(),
                }
                .to_platform_input(),
                cx,
            );
            window.dispatch_event(
                MouseUpEvent {
                    button: MouseButton::Left,
                    position: end,
                    modifiers: Default::default(),
                    click_count: 1,
                }
                .to_platform_input(),
                cx,
            );
            window.render_frame(cx);
            assert!(
                !shell
                    .read(cx)
                    .reader_text
                    .as_ref()
                    .unwrap()
                    .input
                    .read(cx)
                    .selected_range()
                    .is_empty()
            );
        })
        .unwrap();
        assert_eq!(bus.0.lock().unwrap().len(), 1);
        cx.update_window(handle, |_, window, cx| {
            window.press("left", cx);
            window.press("alt-enter", cx);
        })
        .unwrap();
        cx.run_until_parked();
        assert_eq!(bus.0.lock().unwrap().len(), 2);
        assert!(
            shell.read_with(cx, |shell, _| shell.lookup.as_ref().is_some_and(
                |(context, _, _)| context.selection.range == TextRange { start: 5, end: 9 }
            ))
        );
        let gate = Arc::new(Gate::default());
        *bus.1.lock().unwrap() = Some(gate.clone());
        shell.update(cx, |shell, cx| {
            shell.request(
                Command::Lookup {
                    article: "original".into(),
                    body_hash: "original-hash".into(),
                    range: TextRange { start: 15, end: 19 },
                },
                cx,
            )
        });
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.press("escape", cx);
        })
        .unwrap();
        {
            let mut state = gate.0.lock().unwrap();
            state.0 = true;
            if let Some(waker) = state.1.take() {
                waker.wake();
            }
        }
        cx.run_until_parked();
        assert!(shell.read_with(cx, |shell, _| shell.lookup.is_none() && !shell.loading));
        cx.update_window(handle, |_, window, _| window.remove_window())
            .unwrap();
    }
}
