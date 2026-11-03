//! Product semantic colors from docs/DESIGN-SYSTEM.md.
use gpui_kit_material::color::{
    Color, ColorRole as R, Scheme,
    themes::{Contrast, ThemeId, ThemeMode},
};
pub fn scheme(dark: bool) -> Scheme {
    let mut scheme = ThemeId::Baseline.scheme(
        if dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        Contrast::Standard,
    );
    let roles = [
        (R::PRIMARY, 0x4b49c8, 0xc3c0ff),
        (R::ON_PRIMARY, 0xffffff, 0x1f1a8a),
        (R::PRIMARY_CONTAINER, 0xe2dfff, 0x3a36ab),
        (R::ON_PRIMARY_CONTAINER, 0x2c2891, 0xe2dfff),
        (R::SECONDARY, 0x5d5c72, 0xc6c4dd),
        (R::ON_SECONDARY, 0xffffff, 0x1a1a2c),
        (R::SECONDARY_CONTAINER, 0xe2e0f9, 0x45455a),
        (R::ON_SECONDARY_CONTAINER, 0x1a1a2c, 0xe2e0f9),
        (R::TERTIARY, 0x8a4a60, 0xefb8c8),
        (R::TERTIARY_CONTAINER, 0xffd8e4, 0x6b3a4b),
        (R::ON_TERTIARY_CONTAINER, 0x3e1020, 0xffd8e4),
        (R::ERROR, 0xba1a1a, 0xffb4ab),
        (R::ERROR_CONTAINER, 0xffdad6, 0x93000a),
        (R::ON_ERROR_CONTAINER, 0x410002, 0xffdad6),
        (R::SUCCESS, 0x246b3a, 0x8dd89b),
        (R::SUCCESS_BG, 0xb4f1c0, 0x0f5223),
        (R::WARNING, 0x7a5900, 0xf5bf48),
        (R::ON_WARNING, 0x261a00, 0xffdf9e),
        (R::WARNING_BG, 0xffdf9e, 0x5c4300),
        (R::ON_SUCCESS, 0x00210b, 0xb4f1c0),
        (R::SURFACE, 0xf4f1fa, 0x121218),
        (R::SURFACE_CONTAINER_LOWEST, 0xffffff, 0x0e0e14),
        (R::SURFACE_CONTAINER_LOW, 0xf9f6fe, 0x1c1b22),
        (R::SURFACE_CONTAINER, 0xf0ecf6, 0x201f27),
        (R::SURFACE_CONTAINER_HIGH, 0xeae6f0, 0x2a2931),
        (R::SURFACE_CONTAINER_HIGHEST, 0xe4e1ea, 0x35343c),
        (R::ON_SURFACE, 0x1c1b22, 0xe5e1ec),
        (R::ON_SURFACE_VARIANT, 0x47464f, 0xc8c5d0),
        (R::OUTLINE, 0x787680, 0x928f9a),
        (R::OUTLINE_VARIANT, 0xc8c5d0, 0x47464f),
        (R::INVERSE_SURFACE, 0x313038, 0xe5e1ec),
        (R::INVERSE_ON_SURFACE, 0xf3eff7, 0x313038),
    ];
    for (role, light, night) in roles {
        scheme = scheme.with_role(role, Color::hex(if dark { night } else { light }));
    }
    scheme
}
