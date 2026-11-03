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
