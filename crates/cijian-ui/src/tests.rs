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
