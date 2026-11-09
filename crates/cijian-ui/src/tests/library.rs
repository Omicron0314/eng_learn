use super::*;
use cijian_domain::import::BookKind;
use cijian_ports::library::BookView;
use gpui_kit::{Focusable as _, ScrollDelta, point, test::TestQueryExt as _};
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
                                    pronunciation: Default::default(),
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

#[gpui_kit::test]
fn import_overlay_stays_reachable_with_many_books(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit_material::init(cx);
        init_component_theme(theme::scheme(false), 1., cx);
        cx.set_reduce_motion(true);
    });
    for (width, height) in [(360., 600.), (360., 800.), (1000., 900.)] {
        let (handle, shell) = cx
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
                            shell.navigation.navigate(Route::Library {
                                book: None,
                                search: String::new(),
                            });
                            shell.library_ui.import_open = true;
                            shell.word_list = true;
                            shell.library_ui.books = (0..12)
                                .map(|index| BookView {
                                    id: format!("book-{index}"),
                                    name: format!("原创词书 {index}"),
                                    kind: BookKind::Study,
                                    entries: 10,
                                    revision: 1,
                                })
                                .collect();
                            shell
                        })
                    },
                )
            })
            .unwrap();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let close = window.find("close-import").bounds();
            let choose = window.find("choose-file").bounds();
            let import = window.find("import-file").bounds();
            assert!(
                close.origin.y >= px(design::TITLE_BAR_HEIGHT),
                "the import header must stay below the window chrome at {width}x{height}"
            );
            for (id, bounds) in [("choose", choose), ("import", import)] {
                assert!(
                    bounds.origin.y + bounds.size.height <= px(height),
                    "{id} action clipped at {width}x{height}: {bounds:?}"
                );
                assert!(
                    bounds.origin.x >= px(0.) && bounds.origin.x + bounds.size.width <= px(width),
                    "{id} action outside the window at {width}x{height}: {bounds:?}"
                );
            }
            window.click("close-import", cx);
            window.render_frame(cx);
        })
        .unwrap();
        assert!(
            !shell.read_with(cx, |shell, _| shell.library_ui.import_open),
            "the import overlay must receive clicks above the library page"
        );
    }
}

#[gpui_kit::test]
fn word_list_import_choice_reaches_the_backend(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit_material::init(cx);
        init_component_theme(theme::scheme(false), 1., cx);
        cx.set_reduce_motion(true);
    });
    let bus = Arc::new(RejectingBus::default());
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
                        shell.backend = Some(bus.clone());
                        shell.navigation.navigate(Route::Library {
                            book: None,
                            search: String::new(),
                        });
                        shell.library_ui.import_open = true;
                        shell.library_ui.books = (0..12)
                            .map(|index| BookView {
                                id: format!("book-{index}"),
                                name: format!("原创词书 {index}"),
                                kind: BookKind::Study,
                                entries: 10,
                                revision: 1,
                            })
                            .collect();
                        shell
                    })
                },
            )
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("mode-wordlist", cx);
        window.render_frame(cx);
        assert!(shell.read_with(cx, |shell, _| shell.word_list));
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.library_ui.suspended),
            None,
            "presses inside the import dialog must not reach the library filters"
        );
        window.click("kind-baseline", cx);
        window.render_frame(cx);
        assert!(
            shell.read_with(cx, |shell, _| shell.import_kind == BookKind::Baseline),
            "the word-list purpose must be selectable"
        );
    })
    .unwrap();
    shell.update(cx, |shell, cx| {
        shell.selected = Some(cijian_app::commands::SelectedFile {
            handle: "original".into(),
            name: "原创词表.txt".into(),
        });
        shell.library_ui.file_handle = None;
        shell.library_ui.import_name = None;
        cx.notify();
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let close = window.find("close-import").bounds();
        let import = window.find("import-file").bounds();
        assert!(
            close.origin.y >= px(design::TITLE_BAR_HEIGHT),
            "the import header must stay below the window chrome: {close:?}"
        );
        assert!(
            import.origin.y + import.size.height <= px(900.),
            "the start-import action must stay on screen: {import:?}"
        );
        window.click("import-file", cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(bus.0.lock().unwrap().iter().any(|command| matches!(
        command,
        Command::ImportSelected {
            target_book: None,
            handle,
            name,
            kind: BookKind::Baseline,
            word_list: true,
        } if handle == "original" && name == "原创词表.txt"
    )));
}

#[gpui_kit::test]
fn suspended_filter_opens_a_menu_and_selects_an_option(cx: &mut TestAppContext) {
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
                        shell.navigation.navigate(Route::Library {
                            book: None,
                            search: String::new(),
                        });
                        shell
                    })
                },
            )
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("library-suspended", cx);
        window.render_frame(cx);
        assert!(
            shell.read_with(cx, |shell, _| shell.library_ui.suspended_open),
            "the arrow must expand a menu instead of cycling the filter"
        );
        assert!(window.try_find_by_label("仅未暂停").is_some());
        window.press("down", cx);
        window.press("enter", cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert_eq!(
        shell.read_with(cx, |shell, _| shell.library_ui.suspended),
        Some(false),
        "choosing 仅未暂停 must apply that filter"
    );
    let focus = shell.read_with(cx, |shell, _| {
        shell.library_ui.suspended_focus.clone().unwrap()
    });
    cx.update_window(handle, |_, window, cx| {
        window.focus(&focus, cx);
        window.press("enter", cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert!(
        shell.read_with(cx, |shell, _| shell.library_ui.suspended_open),
        "keyboard activation must expand the menu"
    );
}

#[gpui_kit::test]
fn long_meaning_stays_inside_its_library_cell(cx: &mut TestAppContext) {
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
                        shell.navigation.navigate(Route::Library {
                            book: None,
                            search: String::new(),
                        });
                        shell.library = Some(CardPage {
                            total: 1,
                            cards: vec![CardView {
                                id: "long".into(),
                                lexeme_id: "long-lexeme".into(),
                                front: "scaffold".into(),
                                meaning: "这是一个很长的释义，用来验证词库表格不会覆盖来源列。"
                                    .repeat(20),
                                example: String::new(),
                                text_source: "原创导入".into(),
                                suspended: false,
                                archived: false,
                                revision: 0,
                                created_at: 0,
                                srs: cijian_domain::srs::SrsState::new(0),
                                pronunciation: Default::default(),
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
        let row = window.find("library-card-long").bounds();
        let meaning = window.find("library-meaning-long").bounds();
        let source = window.find("library-source-long").bounds();
        assert!(
            meaning.origin.x + meaning.size.width <= source.origin.x,
            "long meaning must truncate before the source column: meaning={meaning:?} source={source:?}"
        );
        assert!(
            row.size.height <= px(80.),
            "a long meaning must not grow the table row: {row:?}"
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn import_word_list_entry_opens_in_word_list_mode(cx: &mut TestAppContext) {
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
                        shell.navigation.navigate(Route::Library {
                            book: None,
                            search: String::new(),
                        });
                        shell
                    })
                },
            )
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("toggle-import", cx);
        window.render_frame(cx);
        assert!(shell.read_with(cx, |shell, _| {
            shell.library_ui.import_open
                && shell.word_list
                && shell.import_kind == BookKind::Baseline
        }));
        assert!(
            window.try_find_by_label("基线：作为已知词").is_some(),
            "the entry labelled 导入词表 must select word-list kinds"
        );
        window.click("mode-cards", cx);
        window.render_frame(cx);
        assert!(shell.read_with(cx, |shell, _| {
            !shell.word_list && shell.import_kind == BookKind::Study
        }));
    })
    .unwrap();
}

#[gpui_kit::test]
fn slash_focuses_library_search_without_typing(cx: &mut TestAppContext) {
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
                        shell.navigation.navigate(Route::Library {
                            book: None,
                            search: String::new(),
                        });
                        shell
                    })
                },
            )
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        for _ in 0..2 {
            window.render_frame(cx);
        }
        window.press("/", cx);
        assert!(
            shell.read_with(cx, |shell, cx| {
                shell
                    .library_ui
                    .search
                    .as_ref()
                    .is_some_and(|input| input.read(cx).focus_handle(cx).is_focused(window))
            }),
            "the slash hint must focus the search field"
        );
        assert!(
            shell.read_with(cx, |shell, cx| shell
                .library_ui
                .search
                .as_ref()
                .is_some_and(|input| input.read(cx).value().is_empty())),
            "the shortcut itself is not inserted"
        );
    })
    .unwrap();
}
