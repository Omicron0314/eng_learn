use super::*;
use gpui_kit::{ScrollDelta, point};
#[derive(Default)]
struct RejectingBus(std::sync::Mutex<Vec<Command>>);
impl CommandBus for RejectingBus {
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
                "原创测试冲突",
            ))
        })
    }
}
#[gpui_kit::test]
fn card_editor_preserves_drafts_and_search_enter_works(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit_material::init(cx);
        init_component_theme(theme::scheme(false), 1., cx);
        cx.set_reduce_motion(true);
    });
    for width in [360., 1000.] {
        let bus = Arc::new(RejectingBus::default());
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
                            let mut shell = Shell::new();
                            shell.backend = Some(bus.clone());
                            shell.navigation.navigate(Route::Library {
                                book: None,
                                search: String::new(),
                            });
                            shell.library = Some(CardPage {
                                total: 1,
                                cards: vec![CardView {
                                    id: "original".into(),
                                    lexeme_id: "lexeme".into(),
                                    front: "apple".into(),
                                    meaning: "original meaning".into(),
                                    example: "An original example.".into(),
                                    text_source: "imported".into(),
                                    suspended: false,
                                    archived: false,
                                    revision: 1,
                                    created_at: 0,
                                    srs: cijian_domain::srs::SrsState::new(0),
                                }],
                            });
                            shell
                        })
                    },
                )
            })
            .unwrap();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.scroll(
                "search-library",
                ScrollDelta::Pixels(point(px(0.), px(-500.))),
                cx,
            );
            window.render_frame(cx);
            window.click("edit-card-original", cx);
            window.render_frame(cx);
            window.within("card-meaning").click("input-state", cx);
            window.press("ctrl-a", cx);
            window.input("edited meaning", cx);
            window.render_frame(cx);
            window.click("save-card-edit", cx);
            window.render_frame(cx);
        })
        .unwrap();
        cx.run_until_parked();
        assert!(bus.0.lock().unwrap().iter().any(|command|matches!(command,Command::EditLibraryCard{meaning,revision:1,suspended:false,..} if meaning=="edited meaning")));
        assert!(shell.read_with(cx, |shell, cx| {
            shell
                .library_ui
                .editor
                .as_ref()
                .is_some_and(|editor| editor.dirty(cx))
        }));
        assert!(
            shell.read_with(cx, |shell, _| shell.error.is_some()),
            "a failed save must expose the error and retain the draft"
        );
        cx.update_window(handle, |_, window, cx| {
            window.press("escape", cx);
            window.render_frame(cx);
        })
        .unwrap();
        assert!(shell.read_with(cx, |shell, _| shell.library_ui.discard));
        cx.update_window(handle, |_, window, cx| {
            window.click("keep-card-draft", cx);
            window.render_frame(cx);
        })
        .unwrap();
        assert!(shell.read_with(cx, |shell, cx| {
            shell.library_ui.editor.as_ref().unwrap().dirty(cx)
        }));
        shell.update(cx, |shell, cx| shell.go_back(cx));
        assert!(shell.read_with(cx, |shell, _| shell.library_ui.pending_back
            && shell.navigation.current().section() == "library"));
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.click("discard-card-draft-confirm", cx);
            window.render_frame(cx);
        })
        .unwrap();
        cx.run_until_parked();
        assert!(
            shell.read_with(cx, |shell, _| shell.library_ui.editor.is_none()
                && shell.navigation.current() == &Route::Today)
        );
        shell.update(cx, |shell, cx| {
            shell.navigate(
                Route::Library {
                    book: None,
                    search: String::new(),
                },
                cx,
            );
        });
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.scroll(
                "search-library",
                ScrollDelta::Pixels(point(px(0.), px(2000.))),
                cx,
            );
            window.render_frame(cx);
            window.within("library-search").click("input-state", cx);
            window.input("pear", cx);
            window.press("enter", cx);
            window.render_frame(cx);
        })
        .unwrap();
        assert!(shell.read_with(cx,|shell,_|matches!(shell.navigation.current(),Route::Library{search,..} if search=="pear")));
    }
}
