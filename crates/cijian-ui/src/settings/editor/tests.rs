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
                    source: cijian_ports::provider::SecretSource::Session,
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
                        source: cijian_ports::provider::SecretSource::Session,
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
                // Scroll over the target itself: the content pane may end
                // before the window's right edge once the rail is shown.
                position: point(
                    bounds.origin.x + bounds.size.width / 2.,
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
        window.click("settings-learning", cx);
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
        window.click("settings-ai", cx);
        window.render_frame(cx);
        // Both provider forms are on screen; neither needs a mode switch.
        input(window, "llm-endpoint", "https://example.invalid/v1", cx);
        input(window, "llm-model", "original-model", cx);
        input(window, "llm-key", "original-only-test-key", cx);
        window.click("settings-learning", cx);
        window.render_frame(cx);
        button(window, "save-settings", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    assert!(shell.read_with(cx, |shell, cx| {
        let editor = shell.settings_editor.as_ref().unwrap();
        !editor.learning_dirty(&shell.settings, cx)
            && editor.provider_dirty(ProviderKind::Llm, cx)
            && !editor
                .provider(ProviderKind::Llm)
                .key
                .read(cx)
                .value()
                .is_empty()
    }));
    assert_eq!(bus.0.lock().unwrap().value.daily_new_limit, 27);
    cx.update_window(handle, |_, window, cx| {
        input(window, "daily-new", "31", cx);
        window.click("settings-ai", cx);
        window.render_frame(cx);
        button(window, "save-provider", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    assert!(shell.read_with(cx, |shell, cx| {
        let editor = shell.settings_editor.as_ref().unwrap();
        editor.learning_dirty(&shell.settings, cx)
            && !editor.any_provider_dirty(cx)
            && editor
                .provider(ProviderKind::Llm)
                .key
                .read(cx)
                .value()
                .is_empty()
            && editor.fields["daily-new"].read(cx).value().as_ref() == "31"
    }));
    let before = bus.0.lock().unwrap().commands.len();
    cx.update_window(handle, |_, window, cx| {
        input(window, "llm-model", "unsaved-model", cx);
        button(window, "llm-clear-key", cx);
        window.click("reset-provider-draft", cx);
        window.render_frame(cx);
    })
    .unwrap();
    let commands = bus.0.lock().unwrap().commands.len();
    if commands != before {
        println!(
            "extra commands: {:?}",
            &bus.0.lock().unwrap().commands[before..]
        );
    }
    assert_eq!(commands, before);
    assert!(shell.read_with(cx, |shell, cx| {
        let editor = shell.settings_editor.as_ref().unwrap();
        !editor.any_provider_dirty(cx)
            && editor.fields["daily-new"].read(cx).value().as_ref() == "31"
    }));
    {
        let mut state = bus.0.lock().unwrap();
        state.revision += 1;
        state.value.daily_new_limit = 29;
    }
    cx.update_window(handle, |_, window, cx| {
        window.click("settings-learning", cx);
        window.render_frame(cx);
        button(window, "save-settings", cx);
    })
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

#[test]
fn credential_copy_never_claims_restart_persistence_for_session_memory() {
    use cijian_ports::provider::SecretSource;
    for has_key in [true, false] {
        let (system, _) = crate::settings::key_source_copy(SecretSource::System, has_key);
        assert!(system.contains("系统凭据库"));
        assert!(system.contains("重启后仍可使用"));
        let (session, state) = crate::settings::key_source_copy(SecretSource::Session, has_key);
        assert!(session.contains("只保存在本次会话"));
        assert!(session.contains("退出后需重新输入"));
        assert!(!session.contains("重启后仍可使用"));
        assert_eq!(state.contains("已设置密钥"), has_key);
    }
}

#[gpui_kit::test]
fn both_provider_forms_save_together_and_keep_their_keys(cx: &mut TestAppContext) {
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
        // No mode switch: both cards accept text in the same visit.
        input(window, "llm-endpoint", "https://llm.example.invalid/v1", cx);
        input(window, "llm-model", "llm-model", cx);
        input(window, "llm-key", "llm-test-key", cx);
        input(window, "tts-endpoint", "https://tts.example.invalid/v1", cx);
        input(window, "tts-model", "tts-model", cx);
        input(window, "tts-voice", "tts-voice", cx);
        input(window, "tts-key", "tts-test-key", cx);
    })
    .unwrap();
    assert!(shell.read_with(cx, |shell, cx| {
        let editor = shell.settings_editor.as_ref().unwrap();
        editor.provider_dirty(ProviderKind::Llm, cx) && editor.provider_dirty(ProviderKind::Tts, cx)
    }));
    cx.update_window(handle, |_, window, cx| {
        button(window, "save-provider", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    {
        let state = bus.0.lock().unwrap();
        let saves = state
            .commands
            .iter()
            .filter_map(|command| match command {
                Command::SaveProvider {
                    config,
                    revision,
                    secret,
                    ..
                } => Some((config.clone(), *revision, secret)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(saves.len(), 2, "one command per edited provider");
        assert_eq!(saves[0].0.kind, ProviderKind::Llm);
        assert_eq!(saves[0].0.endpoint, "https://llm.example.invalid/v1");
        assert_eq!(saves[0].0.model, "llm-model");
        assert_eq!(saves[0].0.voice, None);
        assert_eq!(saves[0].1, 0, "a first save starts from revision zero");
        assert!(matches!(saves[0].2, SecretPatch::Replace(_)));
        assert_eq!(saves[1].0.kind, ProviderKind::Tts);
        assert_eq!(saves[1].0.endpoint, "https://tts.example.invalid/v1");
        assert_eq!(saves[1].0.voice.as_deref(), Some("tts-voice"));
        assert!(matches!(saves[1].2, SecretPatch::Replace(_)));
        assert_eq!(state.keys, [true, true]);
        assert_eq!(
            state.providers[1]
                .as_ref()
                .map(|config| config.model.as_str()),
            Some("tts-model")
        );
    }
    assert!(shell.read_with(cx, |shell, cx| {
        let editor = shell.settings_editor.as_ref().unwrap();
        !editor.any_provider_dirty(cx)
            && editor
                .provider(ProviderKind::Llm)
                .key
                .read(cx)
                .value()
                .is_empty()
            && editor
                .provider(ProviderKind::Tts)
                .key
                .read(cx)
                .value()
                .is_empty()
    }));
    cx.update_window(handle, |_, window, cx| {
        button(window, "tts-clear-key", cx);
    })
    .unwrap();
    cx.run_until_parked();
    {
        let state = bus.0.lock().unwrap();
        assert!(matches!(
            state.commands.last(),
            Some(Command::SaveProvider { config, secret, .. })
                if config.kind == ProviderKind::Tts && matches!(secret, SecretPatch::Clear)
        ));
        assert_eq!(state.keys, [true, false], "clearing TTS keeps the LLM key");
    }
}

#[gpui_kit::test]
fn an_invalid_provider_blocks_both_saves_before_anything_is_sent(cx: &mut TestAppContext) {
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
        input(window, "llm-endpoint", "https://llm.example.invalid/v1", cx);
        input(window, "llm-model", "llm-model", cx);
        input(window, "tts-endpoint", "not-a-url", cx);
        input(window, "tts-model", "tts-model", cx);
        button(window, "save-provider", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(
        !bus.0
            .lock()
            .unwrap()
            .commands
            .iter()
            .any(|command| matches!(command, Command::SaveProvider { .. })),
        "a bad TTS endpoint must not save the valid LLM draft first"
    );
    assert!(shell.read_with(cx, |shell, cx| {
        shell.error.is_some()
            && shell
                .settings_editor
                .as_ref()
                .unwrap()
                .provider_dirty(ProviderKind::Llm, cx)
    }));
}
