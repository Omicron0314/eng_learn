//! Visual primitives copied from the rendered design board.
//!
//! The board is `docs/cijian-m3.html`; the values here follow its `.m3`
//! tokens and the post-script screen markup rather than the obsolete initial
//! Today source. Colors stay on the Material role palette so both themes work.
mod icons;
pub(crate) use icons::Icon;

use gpui_kit::{
    AnimationExt, App, BorderStyle, BoxShadow, Corners, Edges, Font, FontFallbacks, FontWeight,
    Hsla, SharedString, Styled, div, hsla, prelude::*, px, transparent_black,
};
use gpui_kit_material::{color::ColorRole, theme::color};

pub(crate) const RAIL_WIDTH: f32 = 96.;

/// Track height of the wave progress bar, matching the review header in the
/// design board.
pub(crate) const WAVE_HEIGHT: f32 = 12.;
/// Stroke width of both the flowing wave and the remaining flat track.
const WAVE_STROKE: f32 = 4.;
/// Crest offset above and below the track centre.
const WAVE_AMPLITUDE: f32 = 5.;
/// Distance between crests. One animation cycle travels exactly this far, so
/// the repeat loops without a jump.
const WAVE_WAVELENGTH: f32 = 30.;
/// Sampling step of the wave polyline; small enough to read as a curve at 4dp.
const WAVE_SAMPLE: f32 = 3.;
/// Cycle length of the flowing wave: slow enough to read as motion, not noise.
const WAVE_CYCLE: std::time::Duration = std::time::Duration::from_millis(2600);
/// Decorative motion is capped so an idle-looking screen does not ask for
/// every frame the compositor can produce.
const WAVE_MAX_FPS: f32 = 30.;
pub(crate) const TITLE_BAR_HEIGHT: f32 = 48.;
pub(crate) const PANE_RADIUS: f32 = 28.;
pub(crate) const CARD_RADIUS: f32 = 24.;

const FONT_FILES: &[&[u8]] = &[
    include_bytes!("../../assets/fonts/CijianSans-Regular.ttf"),
    include_bytes!("../../assets/fonts/CijianSans-SemiBold.ttf"),
    include_bytes!("../../assets/fonts/CijianSans-Bold.ttf"),
    include_bytes!("../../assets/fonts/CijianSans-ExtraBold.ttf"),
    include_bytes!("../../assets/fonts/CijianDisplay-Bold.ttf"),
    include_bytes!("../../assets/fonts/CijianMono-Regular.ttf"),
    include_bytes!("../../assets/fonts/CijianMono-SemiBold.ttf"),
];

pub(crate) fn install_fonts(cx: &mut App) {
    let fonts = FONT_FILES
        .iter()
        .map(|data| std::borrow::Cow::Borrowed(*data))
        .collect();
    if let Err(error) = cx.text_system().add_fonts(fonts) {
        eprintln!("无法加载界面字体：{error}");
    }
}

pub(crate) fn base_font() -> Font {
    Font {
        family: "Cijian Sans".into(),
        features: Default::default(),
        fallbacks: Some(FontFallbacks::from_fonts(vec![
            "Noto Sans CJK SC".into(),
            "Noto Sans SC".into(),
            "Noto Sans CJK JP".into(),
            "Noto Sans".into(),
            "DejaVu Sans".into(),
        ])),
        weight: FontWeight(400.),
        style: gpui_kit::FontStyle::Normal,
    }
}

pub(crate) fn c(role: ColorRole) -> Hsla {
    color(role).into()
}

pub(crate) fn alpha(color: Hsla, value: f32) -> Hsla {
    color.opacity(value)
}

pub(crate) fn glass(dark: bool) -> Hsla {
    if dark {
        hsla(0., 0., 1., 0.08)
    } else {
        hsla(0., 0., 1., 0.55)
    }
}

pub(crate) fn icon(icon: Icon, size: f32) -> impl IntoElement {
    icon_role(icon, size, ColorRole::ON_SURFACE)
}

pub(crate) fn icon_role(icon: Icon, size: f32, role: ColorRole) -> impl IntoElement {
    icon_color(icon, size, c(role))
}

pub(crate) fn icon_color(icon: Icon, size: f32, color: Hsla) -> impl IntoElement {
    gpui_kit::svg()
        .data(icon.svg().as_bytes())
        .w(px(size))
        .h(px(size))
        .flex_none()
        .text_color(color)
}

pub(crate) fn shaped(radii: [f32; 4], background: Hsla) -> gpui_kit::Div {
    shaped_border(radii, background, 0., transparent_black())
}

pub(crate) fn shaped_border(
    radii: [f32; 4],
    background: Hsla,
    border: f32,
    border_color: Hsla,
) -> gpui_kit::Div {
    let corners = Corners {
        top_left: px(radii[0]),
        top_right: px(radii[1]),
        bottom_right: px(radii[2]),
        bottom_left: px(radii[3]),
    };
    div().relative().child(
        gpui_kit::canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                window.paint_quad(gpui_kit::quad(
                    bounds,
                    corners,
                    background,
                    Edges::all(px(border)),
                    border_color,
                    BorderStyle::Solid,
                ));
            },
        )
        .absolute()
        .inset_0(),
    )
}

pub(crate) fn disp(size: f32, line: f32) -> gpui_kit::Div {
    div()
        .font_family("Cijian Display")
        .font_weight(FontWeight(750.))
        .text_size(px(size))
        .line_height(px(line))
}

pub(crate) fn title(size: f32, line: f32, weight: f32) -> gpui_kit::Div {
    div()
        .font_family("Cijian Display")
        .font_weight(FontWeight(weight))
        .text_size(px(size))
        .line_height(px(line))
}

pub(crate) fn text(size: f32, line: f32, weight: f32) -> gpui_kit::Div {
    div()
        .text_size(px(size))
        .line_height(px(line))
        .font_weight(FontWeight(weight))
}

pub(crate) fn mono(size: f32, line: f32, weight: f32) -> gpui_kit::Div {
    div()
        .font_family("Cijian Mono")
        .text_size(px(size))
        .line_height(px(line))
        .font_weight(FontWeight(weight))
}

pub(crate) fn muted_text(size: f32, line: f32) -> gpui_kit::Div {
    text(size, line, 400.).text_color(c(ColorRole::ON_SURFACE_VARIANT))
}

pub(crate) fn shadow_card() -> Vec<BoxShadow> {
    vec![BoxShadow::new(px(0.), px(1.), hsla(20. / 360., 0.38, 0.13, 0.06)).blur_radius(px(2.))]
}

pub(crate) fn shadow_float() -> Vec<BoxShadow> {
    vec![BoxShadow::new(px(0.), px(6.), hsla(20. / 360., 0.38, 0.13, 0.16)).blur_radius(px(20.))]
}

pub(crate) fn card() -> gpui_kit::Div {
    div()
        .rounded(px(CARD_RADIUS))
        .bg(c(ColorRole::SURFACE_CONTAINER_LOWEST))
        .shadow(shadow_card())
}

pub(crate) fn pane() -> gpui_kit::Div {
    div()
        .rounded(px(PANE_RADIUS))
        .bg(c(ColorRole::SURFACE_CONTAINER_LOW))
        .flex()
        .flex_col()
        .min_w_0()
}

pub(crate) fn tag(bg: ColorRole, fg: ColorRole) -> gpui_kit::Div {
    div()
        .h(px(24.))
        .px(px(10.))
        .rounded(px(12.))
        .flex()
        .items_center()
        .gap(px(6.))
        .flex_none()
        .bg(c(bg))
        .text_color(c(fg))
        .text_size(px(12.))
        .line_height(px(16.))
        .font_weight(FontWeight(650.))
}

pub(crate) fn dot(role: ColorRole, size: f32) -> gpui_kit::Div {
    div()
        .w(px(size))
        .h(px(size))
        .rounded(px(size / 2.))
        .flex_none()
        .bg(c(role))
}

pub(crate) fn kbd(label: impl Into<SharedString>) -> gpui_kit::Div {
    mono(12., 16., 600.)
        .min_w(px(24.))
        .h(px(24.))
        .px(px(6.))
        .rounded(px(7.))
        .flex()
        .items_center()
        .justify_center()
        .bg(c(ColorRole::SURFACE_CONTAINER_HIGH))
        .text_color(c(ColorRole::ON_SURFACE_VARIANT))
        .child(label.into())
}

pub(crate) fn chip(
    on: bool,
    label: impl Into<SharedString>,
    leading: Option<Icon>,
) -> gpui_kit::Div {
    let mut chip = div()
        .h(px(32.))
        .px(px(12.))
        .rounded(px(8.))
        .text_size(px(13.))
        .line_height(px(18.))
        .font_weight(FontWeight(600.))
        .flex()
        .items_center()
        .gap(px(6.))
        .flex_none();
    let foreground = if on {
        chip = chip
            .bg(c(ColorRole::SECONDARY_CONTAINER))
            .text_color(c(ColorRole::ON_SECONDARY_CONTAINER));
        ColorRole::ON_SECONDARY_CONTAINER
    } else {
        chip = chip
            .border_1()
            .border_color(c(ColorRole::OUTLINE_VARIANT))
            .text_color(c(ColorRole::ON_SURFACE_VARIANT));
        ColorRole::ON_SURFACE_VARIANT
    };
    if let Some(symbol) = leading {
        chip = chip.child(icon_role(symbol, 16., foreground));
    }
    chip.child(label.into())
}

pub(crate) fn bar(fraction: f32, fill: ColorRole) -> gpui_kit::Div {
    div()
        .h(px(8.))
        .flex_1()
        .rounded(px(4.))
        .overflow_hidden()
        .bg(c(ColorRole::SURFACE_CONTAINER_HIGH))
        .child(
            div()
                .h_full()
                .w(gpui_kit::relative(fraction.clamp(0., 1.)))
                .rounded(px(4.))
                .bg(c(fill)),
        )
}

pub(crate) fn divider() -> gpui_kit::Div {
    div().h(px(1.)).w_full().bg(c(ColorRole::OUTLINE_VARIANT))
}

pub(crate) fn empty_state(message: impl Into<SharedString>) -> gpui_kit::Div {
    div()
        .flex_1()
        .min_h(px(120.))
        .rounded(px(CARD_RADIUS))
        .border_1()
        .border_dashed()
        .border_color(c(ColorRole::OUTLINE_VARIANT))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(10.))
        .text_size(px(14.))
        .text_color(c(ColorRole::ON_SURFACE_VARIANT))
        .child(icon(Icon::Spark, 18.))
        .child(message.into())
}

/// Painter for a line/area chart. Values are 0..=1.
pub(crate) fn chart(
    values: Vec<f32>,
    width: f32,
    height: f32,
    line: Hsla,
    band: Option<(f32, f32)>,
    dashed_end: bool,
) -> gpui_kit::Div {
    let n = values.len().max(2);
    let points: Vec<(f32, f32)> = values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let x = 40. + (width - 40.) * index as f32 / (n - 1) as f32;
            let y = 20. + (height - 40.) * (1. - value.clamp(0., 1.));
            (x, y)
        })
        .collect();
    let points_for_paint = points.clone();
    div().relative().w_full().h(px(height)).child(
        gpui_kit::canvas(
            move |_, _, _| (),
            move |bounds, _, window, _| {
                if let Some((low, high)) = band {
                    let top = bounds.origin.y + px(20. + (height - 40.) * (1. - high));
                    let bottom = bounds.origin.y + px(20. + (height - 40.) * (1. - low));
                    window.paint_quad(gpui_kit::quad(
                        gpui_kit::Bounds {
                            origin: gpui_kit::point(bounds.origin.x + px(40.), top),
                            size: gpui_kit::size(bounds.size.width - px(40.), bottom - top),
                        },
                        Corners::all(px(10.)),
                        alpha(c(ColorRole::SECONDARY_CONTAINER), 0.7),
                        Edges::all(px(0.)),
                        transparent_black(),
                        BorderStyle::Solid,
                    ));
                }
                for value in [0.0f32, 0.5, 1.0] {
                    let y = bounds.origin.y + px(20. + (height - 40.) * value);
                    let mut path = gpui_kit::PathBuilder::stroke(px(1.));
                    path.move_to(gpui_kit::point(bounds.origin.x + px(40.), y));
                    path.line_to(gpui_kit::point(bounds.origin.x + bounds.size.width, y));
                    if let Ok(path) = path.build() {
                        window.paint_path(path, c(ColorRole::SURFACE_CONTAINER_HIGH));
                    }
                }
                let mut path = gpui_kit::PathBuilder::stroke(px(3.5));
                if dashed_end {
                    path = path.dash_array(&[px(5.), px(5.)]);
                }
                let mut first = true;
                for (index, (x, y)) in points_for_paint.iter().enumerate() {
                    let point = gpui_kit::point(bounds.origin.x + px(*x), bounds.origin.y + px(*y));
                    if first {
                        path.move_to(point);
                        first = false;
                    } else {
                        path.line_to(point);
                    }
                    let _ = index;
                }
                if let Ok(path) = path.build() {
                    window.paint_path(path, line);
                }
                for (index, (x, y)) in points_for_paint.iter().enumerate() {
                    let size = if index + 1 == points_for_paint.len() {
                        7.
                    } else {
                        4.5
                    };
                    let center =
                        gpui_kit::point(bounds.origin.x + px(*x), bounds.origin.y + px(*y));
                    window.paint_quad(gpui_kit::quad(
                        gpui_kit::Bounds {
                            origin: gpui_kit::point(center.x - px(size), center.y - px(size)),
                            size: gpui_kit::size(px(size * 2.), px(size * 2.)),
                        },
                        Corners::all(px(size)),
                        if index + 1 == points_for_paint.len() {
                            line
                        } else {
                            c(ColorRole::SURFACE_CONTAINER_LOWEST)
                        },
                        Edges::all(px(2.5)),
                        line,
                        BorderStyle::Solid,
                    ));
                }
            },
        )
        .absolute()
        .inset_0(),
    )
}

/// Vertical offset of the wave centreline at `x` dp from the track start.
///
/// The offset is zero at the start, peaks upward after a quarter wavelength
/// and repeats every [`WAVE_WAVELENGTH`]; shifting by one wavelength is
/// therefore indistinguishable from not shifting, which is what makes the
/// repeating animation seamless.
fn wave_offset(x: f32, shift: f32) -> f32 {
    -WAVE_AMPLITUDE * ((x + shift) / WAVE_WAVELENGTH * std::f32::consts::TAU).sin()
}

/// An M3 linear-progress track whose filled part flows as a wave.
///
/// The filled span follows `fraction` and the remainder stays a flat 4dp line,
/// the same shape as the review header in the design board. `shift` slides the
/// wave along the track in dp and is decoration only: it never changes how much
/// progress is shown or where the track ends.
pub(crate) fn wave_progress(fraction: f32, shift: f32) -> gpui_kit::Div {
    let fraction = fraction.clamp(0., 1.);
    let done = c(ColorRole::PRIMARY);
    let idle = c(ColorRole::SECONDARY_CONTAINER);
    div().relative().w_full().h(px(WAVE_HEIGHT)).child(
        gpui_kit::canvas(
            move |_, _, _| (),
            move |bounds, _, window, _| {
                let inset = px(WAVE_STROKE / 2.);
                let span = (bounds.size.width - px(WAVE_STROKE)).as_f32();
                if span <= 0. {
                    return;
                }
                let start = bounds.origin.x + inset;
                let mid = bounds.origin.y + px(WAVE_HEIGHT / 2.);
                let end = bounds.origin.x + bounds.size.width - inset;
                // A round cap needs a full stroke of length to be visible, so
                // the wave never collapses to nothing at zero progress.
                let filled = (span * fraction).min(span).max(WAVE_STROKE);
                let split = start + px(filled);
                let wave_y = move |x: f32| mid + px(wave_offset(x, shift));

                let mut path = gpui_kit::PathBuilder::stroke(px(WAVE_STROKE));
                path.move_to(gpui_kit::point(start, wave_y(0.)));
                let mut x = 0.;
                while x + WAVE_SAMPLE < filled {
                    x += WAVE_SAMPLE;
                    path.line_to(gpui_kit::point(start + px(x), wave_y(x)));
                }
                path.line_to(gpui_kit::point(split, wave_y(filled)));
                if let Ok(path) = path.build() {
                    window.paint_path(path, done);
                }

                if split < end {
                    let mut path = gpui_kit::PathBuilder::stroke(px(WAVE_STROKE));
                    path.move_to(gpui_kit::point(split, mid));
                    path.line_to(gpui_kit::point(end, mid));
                    if let Ok(path) = path.build() {
                        window.paint_path(path, idle);
                    }
                }

                window.paint_quad(gpui_kit::quad(
                    gpui_kit::Bounds {
                        origin: gpui_kit::point(
                            bounds.origin.x + bounds.size.width - px(2.),
                            mid - px(2.),
                        ),
                        size: gpui_kit::size(px(4.), px(4.)),
                    },
                    Corners::all(px(2.)),
                    done,
                    Edges::all(px(0.)),
                    transparent_black(),
                    BorderStyle::Solid,
                ));
            },
        )
        .absolute()
        .inset_0(),
    )
}

/// [`wave_progress`] with the wave flowing while `moving`.
///
/// A still track is used whenever nothing is happening (a finished review
/// queue, paused audio) so the window stops asking for frames; reduced motion
/// is honoured by the animation element itself, which then renders the same
/// still frame.
pub(crate) fn wave_progress_moving(
    id: impl Into<gpui_kit::ElementId>,
    fraction: f32,
    moving: bool,
) -> gpui_kit::AnyElement {
    if !moving {
        return wave_progress(fraction, 0.).into_any_element();
    }
    div()
        .w_full()
        .with_animation(
            id,
            gpui_kit::Animation::new(WAVE_CYCLE)
                .repeat()
                .with_max_fps(WAVE_MAX_FPS),
            move |_, delta| wave_progress(fraction, delta * WAVE_WAVELENGTH),
        )
        .into_any_element()
}

/// A rounded ring with `fraction` filled, used by the stats cards.
pub(crate) fn ring(fraction: f32, size: f32, fill: ColorRole) -> impl IntoElement {
    let fraction = fraction.clamp(0., 1.);
    let color = c(fill);
    let track = c(ColorRole::SURFACE_CONTAINER_HIGH);
    div().w(px(size)).h(px(size)).flex_none().child(
        gpui_kit::canvas(
            move |_, _, _| (),
            move |bounds, _, window, _| {
                let center = bounds.center();
                let radius = size / 2. - 5.;
                let width = 10.;
                let mut draw_arc = |start: f32, end: f32, color: Hsla| {
                    let mut path = gpui_kit::PathBuilder::stroke(px(width));
                    let steps = 48;
                    for step in 0..=steps {
                        let angle = start + (end - start) * step as f32 / steps as f32;
                        let point = gpui_kit::point(
                            center.x + px(radius * angle.cos()),
                            center.y + px(radius * angle.sin()),
                        );
                        if step == 0 {
                            path.move_to(point);
                        } else {
                            path.line_to(point);
                        }
                    }
                    if let Ok(path) = path.build() {
                        window.paint_path(path, color);
                    }
                };
                let top = -std::f32::consts::FRAC_PI_2;
                draw_arc(top, top + std::f32::consts::TAU, track);
                if fraction > 0.001 {
                    draw_arc(top, top + std::f32::consts::TAU * fraction, color);
                }
            },
        )
        .absolute()
        .inset_0(),
    )
}

#[cfg(test)]
mod wave_progress_tests {
    use super::*;

    #[test]
    fn one_wavelength_shift_is_invisible() {
        // The repeat loop is only seamless if travelling a wavelength lands on
        // the same shape it started with.
        for x in [0., 7.5, 15., 22.5, 30., 61.] {
            assert!((wave_offset(x, 0.) - wave_offset(x, WAVE_WAVELENGTH)).abs() < 1e-4);
        }
    }

    #[test]
    fn wave_starts_flat_and_alternates_crests() {
        assert!(wave_offset(0., 0.).abs() < 1e-6);
        assert!((wave_offset(WAVE_WAVELENGTH / 4., 0.) + WAVE_AMPLITUDE).abs() < 1e-4);
        assert!((wave_offset(WAVE_WAVELENGTH * 3. / 4., 0.) - WAVE_AMPLITUDE).abs() < 1e-4);
    }
}
