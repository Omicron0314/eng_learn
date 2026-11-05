use super::*;
use gpui_kit::{
    Bounds, InputEvent as _, ScrollDelta, TestAppContext, point, size, test::TestWindowExt,
};
use std::sync::Mutex;
#[derive(Default)]
struct Bus(Mutex<Vec<Command>>);
impl CommandBus for Bus {
    fn dispatch(
        &self,
        command: Command,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = cijian_ports::Result<Response>> + Send + '_>,
    > {
        let result = {
            let mut calls = self.0.lock().unwrap();
            calls.push(command.clone());
            match command {
                Command::FlagSentences {
                    article, body_hash, ..
                } if calls.len() > 1 => Ok(Response::SentencesFlagged { article, body_hash }),
                _ => Err(AppError::new(
                    cijian_ports::ErrorCode::StorageUnavailable,
                    "原创事务失败",
                )),
            }
        };
        Box::pin(async move { result })
    }
}
fn open(window: &mut Window, cx: &mut App) {
    window.render_frame(cx);
    let bounds = window.find("flag-sentences").bounds();
    window.dispatch_event(
        gpui_kit::ScrollEvent {
            position: point(
                window.viewport_size().width * 0.6,
                window.viewport_size().height / 2.,
            ),
            delta: ScrollDelta::Pixels(point(
                px(0.),
                window.viewport_size().height / 2. - bounds.origin.y,
            )),
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
    window.render_frame(cx);
    window.click("flag-sentences", cx);
    window.render_frame(cx);
}
#[gpui_kit::test]
fn flag_failure_keeps_multisentence_draft_and_retry_operation(cx: &mut TestAppContext) {
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
                            size(px(width), px(900.)),
                        ))),
                        ..Default::default()
                    },
                    cx,
                    |_, cx| {
                        cx.new(|_| {
                            let body = "Cats run. Dogs rest.".to_owned();
                            let mut shell = Shell::new();
                            shell.backend = Some(bus.clone());
                            shell.navigation.navigate(Route::Article {
                                id: "original".into(),
                                listening: false,
                            });
                            shell.article = Some(cijian_ports::articles::Article {
                                id: "original".into(),
                                title: "原创短文".into(),
                                body_hash: "original-test-hash".into(),
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
                            shell
                        })
                    },
                )
            })
            .unwrap();
        cx.update_window(handle, |_, window, cx| {
            open(window, cx);
            window.click("flag-sentence-0", cx);
            window.click("flag-sentence-1", cx);
            window.within("sentence-note").click("input-state", cx);
            window.input(&"好".repeat(201), cx);
            window.render_frame(cx);
            window.click("submit-sentence-feedback", cx);
        })
        .unwrap();
        assert!(bus.0.lock().unwrap().is_empty());
        cx.update_window(handle, |_, window, cx| {
            window.within("sentence-note").click("input-state", cx);
            window.press("ctrl-a", cx);
            window.input(&"好".repeat(200), cx);
            window.render_frame(cx);
            window.click("submit-sentence-feedback", cx);
        })
        .unwrap();
        cx.run_until_parked();
        assert!(shell.read_with(cx, |shell, cx| {
            let draft = shell.reader_flags.drafts.values().next().unwrap();
            shell.error.is_some()
                && draft.note.read(cx).value().chars().count() == 200
                && draft.ranges.len() == 2
        }));
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.click("close-sentence-feedback", cx);
            window.render_frame(cx);
            open(window, cx);
            window.click("submit-sentence-feedback", cx);
        })
        .unwrap();
        cx.run_until_parked();
        assert!(shell.read_with(cx, |shell, _| !shell.reader_flags.open
            && shell.reader_flags.drafts.is_empty()));
        let calls = bus.0.lock().unwrap();
        assert_eq!(calls.len(), 2);
        let [
            Command::FlagSentences {
                operation: first,
                flag,
                ..
            },
            Command::FlagSentences {
                operation: second, ..
            },
        ] = calls.as_slice()
        else {
            panic!("only feedback commands expected")
        };
        assert_eq!(first, second);
        assert_eq!(flag.ranges.len(), 2);
        assert_eq!(flag.note.chars().count(), 200);
        cx.update_window(handle, |_, window, _| window.remove_window())
            .unwrap();
    }
}
