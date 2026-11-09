use super::*;
use gpui_kit::{
    Bounds, TestAppContext, size,
    test::{TestQueryExt as _, TestWindowExt},
};
fn snapshot() -> StatsSnapshot {
    StatsSnapshot {
        today: Default::default(),
        week: Default::default(),
        reading_completion: None,
        feedback_too_hard: None,
        feedback_just_right: None,
        feedback_too_easy: None,
        heatmap: (0..91)
            .map(|index| cijian_ports::stats::ActivityDay {
                date: format!("day-{index}"),
                count: index as u64 % 5,
                future: index > 88,
            })
            .collect(),
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
    }
}
#[gpui_kit::test]
fn heatmap_cells_show_their_day_by_pointer_keyboard_and_reject_future_days(
    cx: &mut TestAppContext,
) {
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
                        size(px(360.), px(900.)),
                    ))),
                    ..Default::default()
                },
                cx,
                |_, cx| {
                    cx.new(|_| {
                        let mut shell = Shell::new();
                        shell.navigation.navigate(Route::Today);
                        shell.stats = Some(snapshot());
                        shell
                    })
                },
            )
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find_by_label("day-10：0 次活动").is_some());
        let first = window.find("activity-0").bounds();
        let last = window.find("activity-90").bounds();
        assert!(
            last.origin.y > first.origin.y,
            "weeks must stack vertically"
        );
        assert!(
            first.origin.x >= px(0.) && last.origin.x + last.size.width <= px(360.),
            "heatmap must fit a 360dp viewport: first={first:?} last={last:?}"
        );
        window.click("activity-10", cx);
    })
    .unwrap();
    assert_eq!(shell.read_with(cx, |shell, _| shell.heatmap_day), Some(10));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("right", cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert_eq!(shell.read_with(cx, |shell, _| shell.heatmap_day), Some(11));
    cx.update_window(handle, |_, window, cx| {
        window.press("down", cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert_eq!(shell.read_with(cx, |shell, _| shell.heatmap_day), Some(18));
    cx.update_window(handle, |_, window, cx| {
        window.press("end", cx);
        window.render_frame(cx);
        assert!(window.try_find_by_label("day-88：3 次活动").is_some());
        assert!(
            window
                .try_find_by_label("day-90：0 次活动，未来日期不可选择")
                .is_some()
        );
        window.press("right", cx);
        window.render_frame(cx);
    })
    .unwrap();
    assert_eq!(shell.read_with(cx, |shell, _| shell.heatmap_day), Some(88));
}
