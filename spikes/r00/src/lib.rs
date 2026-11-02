use gpui_kit::base::input::InputState;
use gpui_kit::{App, Context, Entity, Window, WindowOptions, div, prelude::*};
use gpui_kit_material::{
    color::{
        ColorRole,
        themes::{Contrast, ThemeId, ThemeMode},
    },
    components::{ButtonVariant, MaterialSideSheet, TextField, material_button},
    icons::Assets,
    theme::{self, color},
};

struct PlatformProbe {
    input: Entity<InputState>,
    dark: bool,
    scale: f32,
    sheet: bool,
}

impl Render for PlatformProbe {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .size_full()
            .p_8()
            .bg(color(ColorRole::SURFACE))
            .text_color(color(ColorRole::ON_SURFACE))
            .child("词间 · 平台验证")
            .child("输入中文，使用 Tab 切换焦点，选择并复制输入框内的文字。")
            .child(TextField::new("chinese-input", &self.input).label("中文输入与选词"))
            .child(
                material_button("theme", ButtonVariant::Tonal)
                    .child("切换浅色／深色")
                    .on_click(_cx.listener(|this, _, _, cx| {
                        this.dark = !this.dark;
                        this.apply_theme(cx);
                    })),
            )
            .child(
                material_button("sheet-open", ButtonVariant::Tonal)
                    .child("打开释义面板")
                    .on_click(_cx.listener(|this, _, _, cx| {
                        this.sheet = true;
                        cx.notify();
                    })),
            )
            .child(
                MaterialSideSheet::new("lookup-sheet")
                    .open(self.sheet)
                    .modal(true)
                    .headline("语境释义")
                    .child("example：示例。Escape 关闭并检查焦点恢复。")
                    .on_dismiss({
                        let entity = _cx.entity().downgrade();
                        move |_, cx| {
                            let _ = entity.update(cx, |this, cx| {
                                this.sheet = false;
                                cx.notify();
                            });
                        }
                    }),
            )
            .child(
                material_button("scale", ButtonVariant::Tonal)
                    .child("字体 100%／150%／200%")
                    .on_click(_cx.listener(|this, _, _, cx| {
                        this.scale = if this.scale >= 2.0 {
                            1.0
                        } else {
                            this.scale + 0.5
                        };
                        this.apply_theme(cx);
                    })),
            )
    }
}

impl PlatformProbe {
    fn apply_theme(&self, cx: &mut Context<Self>) {
        let mode = if self.dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        };
        theme::init_component_theme(
            ThemeId::Baseline.scheme(mode, Contrast::Standard),
            self.scale,
            cx,
        );
        cx.notify();
    }
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn run_web() {
    gpui_kit::platform::web_init();
    let platform = std::rc::Rc::new(
        gpui_kit::web::WebPlatform::new_with_backend_and_font_fallback(
            false,
            gpui_kit::web::WebBackendPreference::Auto,
            gpui_kit::web::CanvasFontFallback::EmojiAndCjk,
        ),
    );
    let http = std::sync::Arc::new(platform.fetch_http_client());
    run(gpui_kit::Application::with_platform(platform).with_http_client(http));
}

pub fn run(application: gpui_kit::Application) {
    application.with_assets(Assets::new()).run(|cx: &mut App| {
        #[cfg(target_family = "wasm")]
        cx.text_system()
            .add_fonts(vec![
                std::borrow::Cow::Borrowed(include_bytes!("../assets/DejaVuSans.ttf")),
                std::borrow::Cow::Borrowed(include_bytes!("../assets/CijianProbePunctuation.otf")),
            ])
            .expect("加载原型字体");
        gpui_kit::init(cx);
        gpui_kit_material::init(cx);
        theme::init_component_theme(
            ThemeId::Baseline.scheme(ThemeMode::Light, Contrast::Standard),
            1.0,
            cx,
        );
        gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| PlatformProbe {
                input: cx.new(|cx| {
                    let input = InputState::new(window, cx)
                        .default_value("你好，词间。Select and copy words.");
                    input.focus(window, cx);
                    input
                }),
                dark: false,
                scale: 1.0,
                sheet: false,
            })
        })
        .expect("无法打开平台验证窗口");
        cx.activate(true);
    });
}
