use super::*;
use gpui_kit::{
    Bounds, InputEvent as _, ScrollDelta, TestAppContext, point, size, test::TestWindowExt,
};
use std::sync::Mutex;
struct State {
    value: Settings,
    revision: u64,
    providers: [Option<ProviderConfig>; 2],
    keys: [bool; 2],
    commands: Vec<Command>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            value: Settings::default(),
            revision: 1,
            providers: [None, None],
            keys: [false, false],
            commands: Vec::new(),
        }
    }
}
#[derive(Default)]
struct Bus(Mutex<State>);
fn index(kind: ProviderKind) -> usize {
    if kind == ProviderKind::Llm { 0 } else { 1 }
}
impl CommandBus for Bus {
    fn dispatch(
        &self,
        command: Command,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = cijian_ports::Result<Response>> + Send + '_>,
    > {
        let result = {
            let mut state = self.0.lock().unwrap();
            state.commands.push(command.clone());
            match command {
                Command::LoadSettings => Ok(Response::Settings {
                    value: state.value.clone(),
                    revision: state.revision,
                    provider: state.providers[0].clone(),
                    tts: state.providers[1].clone(),
                    has_session_key: state.keys[0],
                    has_tts_key: state.keys[1],
                    voices: Vec::new(),
                    system_voice: None,
                }),
                Command::SaveSettings {
                    value, revision, ..
                } => {
                    if revision != state.revision {
                        Err(AppError::new(
                            cijian_ports::ErrorCode::StaleState,
                            "原创冲突",
                        ))
                    } else {
                        state.value = value.clone();
                        state.revision += 1;
                        Ok(Response::LearningSettingsSaved {
                            value,
                            revision: state.revision,
                        })
                    }
                }
                Command::SaveProvider { config, secret, .. } => {
                    let i = index(config.kind);
                    state.providers[i] = Some(config.clone());
                    match secret {
                        SecretPatch::Keep => {}
                        SecretPatch::Clear => state.keys[i] = false,
                        SecretPatch::Replace(_) => state.keys[i] = true,
                    };
                    Ok(Response::ProviderSaved {
                        config,
                        has_key: state.keys[i],
                    })
                }
                _ => Err(AppError::new(
                    cijian_ports::ErrorCode::UnsupportedCapability,
                    "原创测试未提供此操作",
                )),
            }
        };
        Box::pin(async move { result })
    }
}
fn reveal(window: &mut Window, bounds: Bounds<gpui_kit::Pixels>, cx: &mut App) {
    if bounds.origin.y < px(70.)
        || bounds.origin.y + bounds.size.height > window.viewport_size().height - px(80.)
    {
        let dy = window.viewport_size().height / 2. - bounds.origin.y - bounds.size.height / 2.;
        window.dispatch_event(
            gpui_kit::ScrollEvent {
                position: point(
                    window.viewport_size().width - px(20.),
                    window.viewport_size().height / 2.,
                ),
                delta: ScrollDelta::Pixels(point(px(0.), dy)),
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
        window.render_frame(cx);
    }
}
fn input(window: &mut Window, id: &str, value: &str, cx: &mut App) {
    let id = gpui_kit::SharedString::from(id.to_owned());
    window.render_frame(cx);
    let bounds = window.within(id.clone()).find("input-state").bounds();
    reveal(window, bounds, cx);
    window.within(id).click("input-state", cx);
    window.press("ctrl-a", cx);
    window.input(value, cx);
    window.render_frame(cx);
}
fn button(window: &mut Window, id: &str, cx: &mut App) {
    let id = gpui_kit::SharedString::from(id.to_owned());
    window.render_frame(cx);
    let bounds = window.find(id.clone()).bounds();
    reveal(window, bounds, cx);
    window.click(id, cx);
    window.render_frame(cx);
}
#[gpui_kit::test]
fn independent_saves_preserve_other_drafts_and_validate_every_field(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit_material::init(cx);
        init_component_theme(theme::scheme(false), 1., cx);
        cx.set_reduce_motion(true);
    });
    let bus = Arc::new(Bus::default());
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
                        shell.navigation.navigate(Route::Settings);
                        shell
                    })
                },
            )
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        input(window, "daily-new", "27", cx);
    })
    .unwrap();
    let slider = shell.read_with(cx, |shell, _| {
        shell.settings_editor.as_ref().unwrap().sliders["daily-new"].clone()
    });
    assert_eq!(
        slider.read_with(cx, |slider, _| slider.value()),
        SliderValue::Single(27.)
    );
    cx.update_window(handle, |_, window, cx| {
        let id = gpui_kit::ElementId::from(("slider", slider.entity_id()));
        let bounds = window.find(id.clone()).bounds();
        reveal(window, bounds, cx);
        window.click(id, cx);
        window.press("right", cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert_eq!(
        shell.read_with(cx, |shell, cx| shell
            .settings_editor
            .as_ref()
            .unwrap()
            .fields["daily-new"]
            .read(cx)
            .value()
            .parse::<f32>()
            .unwrap()),
        slider.read_with(cx, |slider, _| slider.value().end())
    );
    cx.update_window(handle, |_, window, cx| {
        input(window, "daily-new", "27", cx);
        button(window, "provider-kind", cx);
    })
    .unwrap();
    assert!(shell.read_with(cx, |shell, cx| {
        shell.provider_kind == ProviderKind::Tts
            && shell.settings_editor.as_ref().unwrap().fields["daily-new"]
                .read(cx)
                .value()
                .as_ref()
                == "27"
    }));
    cx.update_window(handle, |_, window, cx| {
        button(window, "provider-kind", cx);
        input(
            window,
            "provider-endpoint",
            "https://example.invalid/v1",
            cx,
        );
        input(window, "provider-model", "original-model", cx);
        input(window, "provider-key", "original-only-test-key", cx);
        button(window, "save-settings", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    assert!(shell.read_with(cx, |shell, cx| {
        let editor = shell.settings_editor.as_ref().unwrap();
        !editor.learning_dirty(&shell.settings, cx)
            && editor.provider_dirty(cx)
            && !editor.key.read(cx).value().is_empty()
    }));
    assert_eq!(bus.0.lock().unwrap().value.daily_new_limit, 27);
    cx.update_window(handle, |_, window, cx| {
        input(window, "daily-new", "31", cx);
        button(window, "save-provider", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    assert!(shell.read_with(cx, |shell, cx| {
        let editor = shell.settings_editor.as_ref().unwrap();
        editor.learning_dirty(&shell.settings, cx)
            && !editor.provider_dirty(cx)
            && editor.key.read(cx).value().is_empty()
            && editor.fields["daily-new"].read(cx).value().as_ref() == "31"
    }));
    let before = bus.0.lock().unwrap().commands.len();
    cx.update_window(handle, |_, window, cx| {
        input(window, "provider-model", "unsaved-model", cx);
        button(window, "provider-kind", cx);
        window.click("keep-settings-draft", cx);
        window.render_frame(cx);
        button(window, "clear-provider-key", cx);
        button(window, "provider-kind", cx);
        window.click("discard-settings-confirm", cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert_eq!(bus.0.lock().unwrap().commands.len(), before);
    assert!(shell.read_with(cx, |shell, cx| {
        shell.provider_kind == ProviderKind::Tts
            && shell.settings_editor.as_ref().unwrap().fields["daily-new"]
                .read(cx)
                .value()
                .as_ref()
                == "31"
    }));
    cx.update_window(handle, |_, window, cx| button(window, "provider-kind", cx))
        .unwrap();
    {
        let mut state = bus.0.lock().unwrap();
        state.revision += 1;
        state.value.daily_new_limit = 29;
    }
    cx.update_window(handle, |_, window, cx| button(window, "save-settings", cx))
        .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    assert!(shell.read_with(cx, |shell, cx| {
        shell.error.is_some()
            && shell.settings_editor.as_ref().unwrap().fields["daily-new"]
                .read(cx)
                .value()
                .as_ref()
                == "31"
    }));
    cx.update_window(handle, |_, window, cx| {
        button(window, "reload-settings", cx);
        window.click("keep-settings-draft", cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert!(shell.read_with(cx, |shell, cx| {
        shell
            .settings_editor
            .as_ref()
            .unwrap()
            .learning_dirty(&shell.settings, cx)
    }));
    cx.update_window(handle, |_, window, cx| {
        button(window, "reload-settings", cx);
        window.click("discard-settings-confirm", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    assert!(shell.read_with(cx, |shell, cx| {
        shell.settings.daily_new_limit == 29
            && !shell
                .settings_editor
                .as_ref()
                .unwrap()
                .learning_dirty(&shell.settings, cx)
    }));
    cx.update_window(handle, |_, window, cx| input(window, "daily-new", "31", cx))
        .unwrap();
    shell.update(cx, |shell, cx| shell.navigate(Route::Articles, cx));
    assert!(shell.read_with(cx, |shell, _| shell.navigation.current()
        == &Route::Settings
        && shell.settings_ui.pending.is_some()));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("keep-settings-draft", cx);
        window.render_frame(cx);
    })
    .unwrap();
    shell.update(cx, |shell, cx| shell.go_back(cx));
    assert!(shell.read_with(cx, |shell, _| matches!(
        shell.settings_ui.pending,
        Some(PendingSettings::Back)
    ) && shell.navigation.can_back()));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("keep-settings-draft", cx);
        window.render_frame(cx);
    })
    .unwrap();
    shell.update(cx, |shell, cx| assert!(!shell.may_close(cx)));
    assert!(shell.read_with(cx, |shell, _| matches!(
        shell.settings_ui.pending,
        Some(PendingSettings::Close)
    )));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("escape", cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert!(shell.read_with(cx, |shell, _| shell.settings_ui.pending.is_none()));
    let commands = bus.0.lock().unwrap().commands.len();
    cx.update_window(handle, |_, window, cx| {
        button(window, "reset-settings-draft", cx);
        window.click("discard-settings-confirm", cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert_eq!(bus.0.lock().unwrap().commands.len(), commands);
    assert!(shell.read_with(cx, |shell, cx| {
        let editor = shell.settings_editor.as_ref().unwrap();
        !editor.learning_dirty(&shell.settings, cx)
            && editor.fields["daily-new"].read(cx).value().as_ref() == "29"
    }));
    cx.update_window(handle, |_, window, cx| {
        input(window, "daily-new", "201", cx);
        input(window, "retention", "NaN", cx);
        input(window, "min-words", "600", cx);
        input(window, "max-words", "500", cx);
        input(window, "timezone", "not/a-zone", cx);
        button(window, "save-settings", cx);
    })
    .unwrap();
    assert_eq!(bus.0.lock().unwrap().commands.len(), commands);
    let errors = shell.read_with(cx, |shell, cx| {
        shell
            .settings_editor
            .as_ref()
            .unwrap()
            .draft(&shell.settings, cx)
            .1
    });
    for field in ["daily-new", "retention", "min-words", "timezone"] {
        assert!(errors.contains_key(field), "{field}");
    }
    assert!(
        bus.0
            .lock()
            .unwrap()
            .commands
            .iter()
            .all(|command| matches!(
                command,
                Command::SaveSettings { .. } | Command::SaveProvider { .. } | Command::LoadSettings
            ))
    );
}
#[gpui_kit::test]
fn experiment_navigation_focuses_advanced_input_at_compact_and_wide_widths(
    cx: &mut TestAppContext,
) {
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
                            let mut shell = Shell::new();
                            shell.backend = Some(bus.clone());
                            shell.navigation.navigate(Route::Stats);
                            shell.stats = Some(cijian_ports::stats::StatsSnapshot {
                                today: Default::default(),
                                week: Default::default(),
                                reading_completion: None,
                                feedback_just_right: None,
                                heatmap: Vec::new(),
                                coverage: Vec::new(),
                                unknown_median_all: None,
                                unknown_median_week: None,
                                learned: Vec::new(),
                                learned_total: 0,
                                experiment: cijian_ports::stats::ExperimentReport {
                                    version: "original-empty".into(),
                                    exposed: Default::default(),
                                    control: Default::default(),
                                    difference_percentage_points: None,
                                },
                            });
                            shell
                        })
                    },
                )
            })
            .unwrap();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            button(window, "experiment-settings", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let shell = shell.read(cx);
            assert_eq!(shell.navigation.current(), &Route::Settings);
            let input = &shell.settings_editor.as_ref().unwrap().fields["control-ratio"];
            assert!(input.read(cx).focus_handle(cx).is_focused(window));
            let bounds = window.within("control-ratio").find("input-state").bounds();
            assert!(
                bounds.origin.y >= px(0.)
                    && bounds.origin.y + bounds.size.height
                        < window.viewport_size().height - px(80.)
            );
            window.remove_window();
        })
        .unwrap();
        assert!(
            bus.0
                .lock()
                .unwrap()
                .commands
                .iter()
                .all(|command| matches!(command, Command::LoadSettings))
        );
    }
}
#[gpui_kit::test]
fn returning_from_reader_stops_audio_before_popping_history(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_kit_material::init(cx);
        init_component_theme(theme::scheme(false), 1., cx);
    });
    let bus = Arc::new(Bus::default());
    let (handle, shell) = cx
        .update(|cx| {
            gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| {
                cx.new(|_| {
                    let mut shell = Shell::new();
                    shell.backend = Some(bus.clone());
                    shell.navigation.navigate(Route::Article {
                        id: "original-article".into(),
                        listening: true,
                    });
                    shell
                })
            })
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("back", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(
        bus.0
            .lock()
            .unwrap()
            .commands
            .iter()
            .any(|command| matches!(command, Command::AudioStop))
    );
    assert!(shell.read_with(cx, |shell, _| shell.navigation.current() == &Route::Today));
}
