use super::*;
use crate::design::Icon;
use cijian_domain::{
    provider::{ProviderConfig, ProviderKind},
    settings::{Level, LookupMode, Theme, Tier},
};
use cijian_ports::provider::{Secret, SecretPatch};
use gpui_kit_material::components::TextField;
mod drafts;
mod editor;
mod pronunciation;
pub(super) use drafts::{PendingSettings, SettingsPage, SettingsState};
pub(super) use editor::Editor;
/// Copy must follow the store that actually accepted the secret.
pub(crate) fn key_source_copy(
    source: cijian_ports::provider::SecretSource,
    has_key: bool,
) -> (&'static str, &'static str) {
    match (source, has_key) {
        (cijian_ports::provider::SecretSource::System, true) => (
            "保存配置不会调用模型。密钥写入系统凭据库，重启后仍可使用。",
            "系统凭据库中已有密钥；留空表示保留，可显式清除",
        ),
        (cijian_ports::provider::SecretSource::System, false) => (
            "保存配置不会调用模型。密钥写入系统凭据库，重启后仍可使用。",
            "系统凭据库可用，当前没有已保存密钥",
        ),
        (cijian_ports::provider::SecretSource::Session, true) => (
            "保存配置不会调用模型。系统凭据库不可用，密钥只保存在本次会话，退出后需重新输入。",
            "本次会话已设置密钥；留空表示保留",
        ),
        (cijian_ports::provider::SecretSource::Session, false) => (
            "保存配置不会调用模型。系统凭据库不可用，密钥只保存在本次会话，退出后需重新输入。",
            "本次会话未设置密钥",
        ),
    }
}
impl Shell {
    pub(super) fn apply_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dark = match self.settings.theme {
            Theme::Dark => true,
            Theme::Light => false,
            Theme::System => matches!(
                window.appearance(),
                gpui_kit::WindowAppearance::Dark | gpui_kit::WindowAppearance::VibrantDark
            ),
        };
        if self.dark != dark {
            self.dark = dark;
            cx.notify();
        }
    }
    fn settings_nav(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let page = self.settings_ui.page;
        let mut nav = design::pane()
            .w(px(280.))
            .flex_shrink_0()
            .h_full()
            .p(px(24.))
            .px(px(12.))
            .gap(px(4.));
        nav = nav.child(
            div()
                .px(px(12.))
                .mb(px(14.))
                .child(design::title(36., 44., 780.).child("设置")),
        );
        for (target, icon, label) in [
            (SettingsPage::Provider, Icon::Server, "AI 服务"),
            (SettingsPage::Learning, Icon::Cards, "学习与阅读"),
            (SettingsPage::Experiment, Icon::Beaker, "实验参数"),
        ] {
            let selected = page == target;
            nav = nav.child(
                div()
                    .id(gpui_kit::SharedString::from(match target {
                        SettingsPage::Provider => "settings-ai",
                        SettingsPage::Learning => "settings-learning",
                        SettingsPage::Experiment => "settings-experiment",
                    }))
                    .test_support()
                    .role(gpui_kit::Role::Button)
                    .aria_label(label)
                    .cursor_pointer()
                    .h(px(56.))
                    .px(px(16.))
                    .rounded(px(28.))
                    .flex()
                    .items_center()
                    .gap(px(14.))
                    .text_size(px(15.))
                    .when(selected, |row| {
                        row.bg(design::c(ColorRole::SECONDARY_CONTAINER))
                            .text_color(design::c(ColorRole::ON_SECONDARY_CONTAINER))
                            .font_weight(gpui_kit::FontWeight(700.))
                    })
                    .when(!selected, |row| {
                        row.text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                            .font_weight(gpui_kit::FontWeight(500.))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.settings_ui.page = target;
                        this.settings_ui.advanced = target == SettingsPage::Experiment;
                        cx.notify();
                    }))
                    .child(design::icon_role(
                        icon,
                        24.,
                        if selected {
                            ColorRole::ON_SECONDARY_CONTAINER
                        } else {
                            ColorRole::ON_SURFACE_VARIANT
                        },
                    ))
                    .child(label),
            );
        }
        nav = nav.child(div().flex_1());
        nav = nav.child(
            div()
                .m(px(8.))
                .p(px(14.))
                .px(px(16.))
                .rounded(px(20.))
                .bg(design::c(ColorRole::SURFACE_CONTAINER_LOWEST))
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .gap(px(2.))
                        .child(design::text(15., 20., 650.).child("词间 MVP"))
                        .child(design::muted_text(12., 16.).child("自用 · 本地数据")),
                )
                .child(self.settings_theme_button(cx)),
        );
        nav.into_any_element()
    }

    fn settings_theme_button(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        div()
            .id("theme")
            .test_support()
            .role(gpui_kit::Role::Button)
            .aria_label(if self.dark {
                "切换浅色"
            } else {
                "切换深色"
            })
            .cursor_pointer()
            .w(px(40.))
            .h(px(40.))
            .rounded(px(20.))
            .flex()
            .items_center()
            .justify_center()
            .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
            .on_click(cx.listener(|this, _, window, cx| {
                this.settings.theme = if this.dark { Theme::Light } else { Theme::Dark };
                this.apply_theme(window, cx);
                if this.backend.is_some() {
                    this.request(
                        Command::SetTheme {
                            value: this.settings.theme,
                        },
                        cx,
                    );
                }
            }))
            .child(design::icon_role(
                Icon::Today,
                24.,
                ColorRole::ON_SURFACE_VARIANT,
            ))
            .into_any_element()
    }

    fn settings_header(
        &self,
        title: &'static str,
        hint: &'static str,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let mut row = div()
            .flex()
            .items_center()
            .child(design::title(30., 36., 780.).flex_1().child(title))
            .child(design::muted_text(13., 18.).child(hint));
        if self.compact {
            row = row.child(self.settings_theme_button(cx));
        }
        row.into_any_element()
    }

    fn provider_summary(&self) -> gpui_kit::AnyElement {
        let configured = self.provider_config.as_ref();
        let (icon, title, detail, color) = (
            Icon::Spark,
            "LLM · 写作与释义",
            configured
                .map(|config| config.endpoint.clone())
                .unwrap_or_else(|| "尚未配置".into()),
            (
                ColorRole::PRIMARY_CONTAINER,
                ColorRole::ON_PRIMARY_CONTAINER,
            ),
        );
        let mut row = div().flex().gap(px(14.));
        {
            let mut card = design::card()
                .flex_1()
                .min_w_0()
                .rounded(px(28.))
                .p(px(18.))
                .gap(px(10.))
                .flex()
                .flex_col();
            card = card.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(
                        design::shaped([14., 14., 14., 14.], design::c(color.0))
                            .w(px(44.))
                            .h(px(44.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(design::c(color.1))
                            .child(design::icon_color(icon, 18., design::c(color.1))),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(design::text(16., 22., 650.).child(title))
                            .child(design::muted_text(12., 16.).text_ellipsis().child(detail)),
                    )
                    .child(if configured.is_some() {
                        design::tag(ColorRole::SUCCESS_BG, ColorRole::ON_SUCCESS)
                            .child(design::dot(ColorRole::SUCCESS, 8.))
                            .child("已配置")
                    } else {
                        design::tag(
                            ColorRole::SURFACE_CONTAINER_HIGH,
                            ColorRole::ON_SURFACE_VARIANT,
                        )
                        .child("未配置")
                    }),
            );
            let model = configured.map(|config| config.model.clone());
            card = card.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .rounded(px(12.))
                    .bg(design::c(ColorRole::SURFACE_CONTAINER))
                    .p(px(8.))
                    .px(px(12.))
                    .child(
                        design::mono(13., 18., 400.)
                            .flex_1()
                            .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                            .text_ellipsis()
                            .child(model.unwrap_or_else(|| "未设置模型".into())),
                    ),
            );
            row = row.child(card);
        }
        row.into_any_element()
    }

    /// One provider's card. LLM and TTS are configured at the same time, so both
    /// forms always stay editable instead of a mode switch that hides one.
    fn provider_form(
        &self,
        kind: ProviderKind,
        editor: &Editor,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let llm = kind == ProviderKind::Llm;
        let prefix = if llm { "llm" } else { "tts" };
        let draft = editor.provider(kind);
        let configured = if llm {
            self.provider_config.as_ref()
        } else {
            self.tts_config.as_ref()
        };
        let has_key = self.provider_has_key(kind);
        let clearable = has_key && !draft.dirty(kind, cx);
        let id = |name: &str| gpui_kit::SharedString::from(format!("{prefix}-{name}"));
        let mut form = design::card()
            .w_full()
            .rounded(px(28.))
            .p(px(22.))
            .px(px(24.))
            .gap(px(16.))
            .flex()
            .flex_col();
        form = form.child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(design::text(16., 22., 650.).flex_1().child(if llm {
                    "LLM · 写作与释义"
                } else {
                    "TTS · 整篇音频"
                }))
                .child(design::muted_text(12., 16.).child(if llm {
                    "全部必填 · 无默认模型"
                } else {
                    "留空则用本机朗读"
                }))
                .child(if configured.is_some() {
                    design::tag(ColorRole::SUCCESS_BG, ColorRole::ON_SUCCESS)
                        .child(design::dot(ColorRole::SUCCESS, 8.))
                        .child("已配置")
                } else {
                    design::tag(
                        ColorRole::SURFACE_CONTAINER_HIGH,
                        ColorRole::ON_SURFACE_VARIANT,
                    )
                    .child("未配置")
                }),
        );
        let mut fields = div()
            .flex()
            .gap(px(12.))
            .child(
                div().flex_grow(1.4).flex_basis(px(0.)).min_w_0().child(
                    TextField::new(id("endpoint"), &draft.endpoint)
                        .outlined()
                        .label("端点 Base URL")
                        .disabled(self.loading)
                        .w_full(),
                ),
            )
            .child(
                div().flex_grow(1.).flex_basis(px(0.)).min_w_0().child(
                    TextField::new(id("model"), &draft.model)
                        .outlined()
                        .label("模型")
                        .disabled(self.loading)
                        .w_full(),
                ),
            );
        if !llm {
            fields = fields.child(
                div().flex_grow(0.8).flex_basis(px(0.)).min_w_0().child(
                    TextField::new(id("voice"), &draft.voice)
                        .outlined()
                        .label("声音")
                        .disabled(self.loading)
                        .w_full(),
                ),
            );
        }
        form = form.child(fields);
        form = form.child(
            TextField::new(id("key"), &draft.key)
                .outlined()
                .label("API 密钥")
                .disabled(self.loading)
                .w_full(),
        );
        form = form.child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(
                    design::text(12., 16., 400.)
                        .flex_1()
                        .text_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                        .child(key_source_copy(self.key_source, has_key).1),
                )
                .child(
                    div()
                        .id(id("clear-key"))
                        .test_support()
                        .role(gpui_kit::Role::Button)
                        .aria_label(if llm {
                            "清除已保存的 LLM 密钥"
                        } else {
                            "清除已保存的 TTS 密钥"
                        })
                        .cursor_pointer()
                        .h(px(32.))
                        .px(px(12.))
                        .rounded(px(16.))
                        .flex()
                        .items_center()
                        .text_color(design::c(ColorRole::ERROR))
                        .text_size(px(12.))
                        .when(self.loading || !clearable, |button| button.opacity(0.38))
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.clear_provider_key(kind, cx)),
                        )
                        .child("清除密钥"),
                ),
        );
        form.into_any_element()
    }

    fn provider_page(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let Some(editor) = &self.settings_editor else {
            return design::muted_text(16., 24.)
                .child("正在读取设置…")
                .into_any_element();
        };
        let provider_dirty = editor.any_provider_dirty(cx);
        let mut page = div()
            .id("provider-page")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap(px(18.));
        page = page.child(self.settings_header(
            "AI 服务",
            "保存后立即生效，无需重启 · 此页不会发起测试调用",
            cx,
        ));
        // Both provider cards scroll; the actions stay reachable at any height.
        let mut content = div()
            .id("provider-scroll")
            .track_scroll(&self.page_scroll)
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap(px(18.))
            .overflow_y_scroll()
            .when(self.compact, |content| content.pb(px(90.)));
        content = content.child(self.provider_summary());
        content = content.child(self.provider_form(ProviderKind::Llm, editor, cx));
        page = page.child(content);
        page = page.child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    design::muted_text(12., 16.)
                        .flex_1()
                        .child(key_source_copy(self.key_source, self.has_session_key).0),
                )
                .child(
                    div()
                        .id("reset-provider-draft")
                        .test_support()
                        .role(gpui_kit::Role::Button)
                        .aria_label("撤销 AI 配置修改")
                        .cursor_pointer()
                        .h(px(40.))
                        .px(px(12.))
                        .rounded(px(20.))
                        .flex()
                        .items_center()
                        .text_color(design::c(ColorRole::PRIMARY))
                        .when(self.loading || !provider_dirty, |button| {
                            button.opacity(0.38)
                        })
                        .on_click(
                            cx.listener(|this, _, window, cx| this.reset_providers(window, cx)),
                        )
                        .child("撤销修改"),
                )
                .child(
                    div()
                        .id("save-provider")
                        .test_support()
                        .role(gpui_kit::Role::Button)
                        .aria_label("保存 AI 配置")
                        .cursor_pointer()
                        .h(px(56.))
                        .px(px(28.))
                        .rounded(px(28.))
                        .bg(design::c(ColorRole::PRIMARY))
                        .text_color(design::c(ColorRole::ON_PRIMARY))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .text_size(px(16.))
                        .font_weight(gpui_kit::FontWeight(650.))
                        .when(self.loading || !provider_dirty, |button| {
                            button.opacity(0.38)
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            if !this.loading {
                                this.save_providers(cx);
                            }
                        }))
                        .child(design::icon_role(Icon::Check, 18., ColorRole::ON_PRIMARY))
                        .child("保存并生效"),
                ),
        );
        page.into_any_element()
    }

    fn learning_page(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let Some(editor) = &self.settings_editor else {
            return design::muted_text(16., 24.)
                .child("正在读取设置…")
                .into_any_element();
        };
        let (_, errors) = editor.draft(&self.settings, cx);
        let learning_dirty = editor.learning_dirty(&self.settings, cx);
        let mut page = div()
            .id("learning-page")
            .track_scroll(&self.page_scroll)
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap(px(18.))
            .overflow_y_scroll();
        page = page.child(self.settings_header("学习与阅读", "合理的上限，让学习更轻一点", cx));
        page = page.child(self.pronunciation_preferences(cx));
        let mut level = design::card()
            .rounded(px(28.))
            .p(px(20.))
            .px(px(22.))
            .gap(px(14.))
            .flex()
            .flex_col();
        level = level.child(
            div()
                .flex()
                .flex_col()
                .child(design::text(16., 22., 650.).child("英语等级"))
                .child(design::muted_text(13., 18.).child("用于估计已知词与选择生成难度")),
        );
        let mut levels = div().flex().gap(px(10.));
        for (value, label, hint) in [
            (Level::A2, "A2", "约初中水平"),
            (Level::B1, "B1", "约大学英语四级"),
            (Level::B2, "B2", "约六级水平"),
            (Level::C1, "C1", "高级"),
        ] {
            let selected = self.settings.level == value;
            levels = levels.child(
                div()
                    .id(gpui_kit::SharedString::from(format!(
                        "level-{}",
                        label.to_lowercase()
                    )))
                    .role(gpui_kit::Role::Button)
                    .aria_label(label)
                    .cursor_pointer()
                    .flex_1()
                    .rounded(px(20.))
                    .p(px(14.))
                    .px(px(16.))
                    .gap(px(2.))
                    .flex()
                    .flex_col()
                    .when(selected, |card| {
                        card.bg(design::c(ColorRole::SECONDARY))
                            .text_color(design::c(ColorRole::SURFACE_CONTAINER_LOWEST))
                    })
                    .when(!selected, |card| {
                        card.border_1()
                            .border_color(design::c(ColorRole::OUTLINE_VARIANT))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.settings.level = value;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .child(design::title(22., 28., 750.).flex_1().child(label))
                            .when(selected, |row| {
                                row.child(design::icon_role(
                                    Icon::Check,
                                    18.,
                                    ColorRole::ON_PRIMARY,
                                ))
                            }),
                    )
                    .child(
                        design::text(12., 16., 400.)
                            .opacity(if selected { 0.85 } else { 1. })
                            .text_color(if selected {
                                design::c(ColorRole::SURFACE_CONTAINER_LOW)
                            } else {
                                design::c(ColorRole::ON_SURFACE_VARIANT)
                            })
                            .child(hint),
                    ),
            );
        }
        level = level.child(levels);
        page = page.child(level);

        let mut two = div().flex().gap(px(14.)).items_stretch();
        let mut preferences = design::card()
            .flex_1()
            .min_w_0()
            .rounded(px(28.))
            .p(px(20.))
            .px(px(22.))
            .gap(px(12.))
            .flex()
            .flex_col();
        preferences = preferences.child(
            div()
                .flex()
                .flex_col()
                .child(design::text(16., 22., 650.).child("默认难度"))
                .child(design::muted_text(13., 18.).child("文章页生成时预选")),
        );
        let mut tiers = div().flex().gap(px(2.));
        for (index, tier) in [Tier::Easy, Tier::Medium, Tier::Hard]
            .into_iter()
            .enumerate()
        {
            let selected = self.settings.tier == tier;
            let label = match tier {
                Tier::Easy => "轻松",
                Tier::Medium => "适中",
                Tier::Hard => "挑战",
            };
            let radii = match index {
                0 => [20., 6., 6., 20.],
                2 => [6., 20., 20., 6.],
                _ => [6., 6., 6., 6.],
            };
            let mut cell = design::shaped(
                radii,
                design::c(if selected {
                    ColorRole::SECONDARY
                } else {
                    ColorRole::SURFACE_CONTAINER_HIGH
                }),
            )
            .id(gpui_kit::SharedString::from(format!(
                "default-tier-{}",
                index
            )))
            .role(gpui_kit::Role::Button)
            .aria_label(label)
            .cursor_pointer()
            .flex_1()
            .h(px(40.))
            .text_color(design::c(if selected {
                ColorRole::SURFACE_CONTAINER_LOWEST
            } else {
                ColorRole::ON_SURFACE_VARIANT
            }))
            .text_size(px(14.))
            .font_weight(gpui_kit::FontWeight(if selected { 700. } else { 600. }))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.settings.tier = tier;
                cx.notify();
            }));
            cell = cell.child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(6.))
                    .when(selected, |row| {
                        row.child(design::icon_role(Icon::Check, 18., ColorRole::ON_PRIMARY))
                    })
                    .child(label),
            );
            tiers = tiers.child(cell);
        }
        preferences = preferences.child(tiers);
        preferences = preferences
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .mt(px(6.))
                    .child(design::text(16., 22., 650.).child("点词模式"))
                    .child(design::muted_text(13., 18.).child("阅读时点“不认识”会发生什么")),
            )
            .child(
                div()
                    .id("lookup-mode")
                    .role(gpui_kit::Role::Button)
                    .aria_label("点词模式：只标记")
                    .cursor_pointer()
                    .rounded(px(20.))
                    .p(px(12.))
                    .px(px(14.))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .when(self.settings.lookup_mode == LookupMode::MarkOnly, |row| {
                        row.bg(design::c(ColorRole::SECONDARY_CONTAINER))
                            .text_color(design::c(ColorRole::ON_SECONDARY_CONTAINER))
                    })
                    .when(self.settings.lookup_mode != LookupMode::MarkOnly, |row| {
                        row.border_1()
                            .border_color(design::c(ColorRole::OUTLINE_VARIANT))
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.settings.lookup_mode = LookupMode::MarkOnly;
                        cx.notify();
                    }))
                    .child(if self.settings.lookup_mode == LookupMode::MarkOnly {
                        design::dot(ColorRole::PRIMARY, 20.)
                    } else {
                        div()
                            .w(px(20.))
                            .h(px(20.))
                            .rounded(px(10.))
                            .border_2()
                            .border_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .child(design::text(15., 20., 650.).child("只标记 → 生词箱"))
                            .child(
                                design::text(12., 16., 400.)
                                    .opacity(0.85)
                                    .child("之后在生词箱里批量决定"),
                            ),
                    ),
            )
            .child(
                div()
                    .id("lookup-mode-auto")
                    .role(gpui_kit::Role::Button)
                    .aria_label("点词模式：自动建卡")
                    .cursor_pointer()
                    .rounded(px(20.))
                    .p(px(12.))
                    .px(px(14.))
                    .flex()
                    .items_start()
                    .gap(px(12.))
                    .when(self.settings.lookup_mode == LookupMode::AutoAdd, |row| {
                        row.bg(design::c(ColorRole::SECONDARY_CONTAINER))
                            .text_color(design::c(ColorRole::ON_SECONDARY_CONTAINER))
                    })
                    .when(self.settings.lookup_mode != LookupMode::AutoAdd, |row| {
                        row.border_1()
                            .border_color(design::c(ColorRole::OUTLINE_VARIANT))
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.settings.lookup_mode = LookupMode::AutoAdd;
                        cx.notify();
                    }))
                    .child(div().mt(px(2.)).child(
                        if self.settings.lookup_mode == LookupMode::AutoAdd {
                            design::dot(ColorRole::PRIMARY, 20.)
                        } else {
                            div()
                                .w(px(20.))
                                .h(px(20.))
                                .rounded(px(10.))
                                .border_2()
                                .border_color(design::c(ColorRole::ON_SURFACE_VARIANT))
                        },
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .gap(px(4.))
                            .child(design::text(15., 20., 650.).child("自动建卡"))
                            .child(design::muted_text(12., 16.).child("直接加入复习"))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .text_color(design::c(ColorRole::WARNING))
                                    .text_size(px(12.))
                                    .child(design::icon_role(Icon::Alert, 16., ColorRole::WARNING))
                                    .child("点错也会建卡，并计入每日新词上限"),
                            ),
                    ),
            );
        two = two.child(preferences);

        let mut limits = design::card()
            .flex_1()
            .min_w_0()
            .rounded(px(28.))
            .p(px(4.))
            .px(px(22.))
            .flex()
            .flex_col();
        let mut first_limit = true;
        for field in editor::FIELDS
            .iter()
            .filter(|field| matches!(field.id, "daily-new" | "daily-review" | "retention"))
        {
            if !first_limit {
                limits = limits.child(design::divider());
            }
            first_limit = false;
            limits = limits.child(editor.design_field(field, &errors, self.loading, cx));
        }
        limits = limits
            .child(design::divider())
            .child(editor.design_range_field(self.loading, cx));
        two = two.child(limits);
        page = page.child(two);

        let tts = design::card()
            .rounded(px(28.))
            .p(px(4.))
            .px(px(22.))
            .flex()
            .flex_col();
        let tts = tts.child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .py(px(8.))
                .child(
                    div()
                        .w(px(200.))
                        .flex_shrink_0()
                        .flex()
                        .flex_col()
                        .child(design::text(15., 20., 650.).child("学习日与时区"))
                        .child(design::muted_text(12., 16.).child("决定“今天”从几点开始")),
                )
                .child(
                    div().flex_1().child(
                        TextField::new("timezone", &editor.fields["timezone"])
                            .outlined()
                            .label("时区")
                            .disabled(self.loading)
                            .error(errors.contains_key("timezone"))
                            .supporting_text(
                                errors
                                    .get("timezone")
                                    .cloned()
                                    .unwrap_or("使用 IANA 时区名称".into()),
                            ),
                    ),
                )
                .child(
                    div().w(px(180.)).child(
                        TextField::new("day-start", &editor.fields["day-start"])
                            .outlined()
                            .label("学习日开始")
                            .disabled(self.loading),
                    ),
                ),
        );
        page = page.child(tts);

        if self.settings_ui.advanced {
            let mut advanced = design::card()
                .rounded(px(28.))
                .p(px(20.))
                .px(px(22.))
                .gap(px(12.))
                .flex()
                .flex_col();
            advanced = advanced.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(design::icon_role(
                        Icon::Beaker,
                        20.,
                        ColorRole::ON_SURFACE_VARIANT,
                    ))
                    .child(design::text(16., 22., 650.).child("高级 · 实验参数"))
                    .child(div().flex_1())
                    .child(design::muted_text(12., 16.).child("影响选词与对照实验")),
            );
            for field in editor::FIELDS
                .iter()
                .filter(|field| editor::advanced(field.id))
            {
                advanced = advanced.child(editor.field(field, &errors, self.loading));
            }
            page = page.child(advanced);
        }

        let footer = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .mt(px(4.))
            .child(
                div()
                    .id("advanced-settings")
                    .role(gpui_kit::Role::Button)
                    .aria_label("高级实验设置")
                    .cursor_pointer()
                    .h(px(40.))
                    .px(px(16.))
                    .rounded(px(20.))
                    .bg(design::c(ColorRole::SECONDARY_CONTAINER))
                    .text_color(design::c(ColorRole::ON_SECONDARY_CONTAINER))
                    .flex()
                    .items_center()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.settings_ui.advanced = !this.settings_ui.advanced;
                        cx.notify();
                    }))
                    .child(if self.settings_ui.advanced {
                        "收起高级实验设置"
                    } else {
                        "展开高级实验设置"
                    }),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id("reload-settings")
                    .test_support()
                    .role(gpui_kit::Role::Button)
                    .aria_label("重新载入已保存设置")
                    .cursor_pointer()
                    .h(px(40.))
                    .px(px(12.))
                    .rounded(px(20.))
                    .flex()
                    .items_center()
                    .text_color(design::c(ColorRole::PRIMARY))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if !this.guard_settings_exit(PendingSettings::Reload, cx) {
                            this.request(Command::LoadSettings, cx);
                        }
                    }))
                    .child("重新载入"),
            )
            .child(
                div()
                    .id("reset-settings-draft")
                    .test_support()
                    .role(gpui_kit::Role::Button)
                    .aria_label("撤销未保存修改")
                    .cursor_pointer()
                    .h(px(40.))
                    .px(px(12.))
                    .rounded(px(20.))
                    .flex()
                    .items_center()
                    .text_color(design::c(ColorRole::PRIMARY))
                    .when(self.loading || !learning_dirty, |button| {
                        button.opacity(0.38)
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.loading || !this.settings_dirty(cx) {
                            return;
                        }
                        this.settings_ui.pending = Some(PendingSettings::Reset);
                        cx.notify();
                    }))
                    .child("撤销修改"),
            )
            .child(
                div()
                    .id("save-settings")
                    .test_support()
                    .role(gpui_kit::Role::Button)
                    .aria_label("保存学习设置")
                    .cursor_pointer()
                    .h(px(56.))
                    .px(px(28.))
                    .rounded(px(28.))
                    .bg(design::c(ColorRole::PRIMARY))
                    .text_color(design::c(ColorRole::ON_PRIMARY))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_size(px(16.))
                    .font_weight(gpui_kit::FontWeight(650.))
                    .when(
                        self.loading || !learning_dirty || !errors.is_empty(),
                        |button| button.opacity(0.38),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.loading {
                            return;
                        }
                        if let Some(editor) = &this.settings_editor {
                            match editor.value(&this.settings, cx) {
                                Ok(value) => this.request(
                                    Command::SaveSettings {
                                        value,
                                        revision: this.settings_revision,
                                        operation: format!(
                                            "settings-{}-{}",
                                            this.settings_revision, this.request_id
                                        ),
                                    },
                                    cx,
                                ),
                                Err(error) => {
                                    this.error = Some(error.message);
                                    cx.notify();
                                }
                            }
                        }
                    }))
                    .child(design::icon_role(Icon::Check, 18., ColorRole::ON_PRIMARY))
                    .child("保存并生效"),
            );
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(page)
            .child(footer)
            .into_any_element()
    }

    fn experiment_page(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let Some(editor) = &self.settings_editor else {
            return design::muted_text(16., 24.)
                .child("正在读取设置…")
                .into_any_element();
        };
        let (_, errors) = editor.draft(&self.settings, cx);
        let mut page = div()
            .id("experiment-page")
            .track_scroll(&self.page_scroll)
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap(px(18.))
            .overflow_y_scroll();
        page = page.child(self.settings_header("实验参数", "影响选词与对照实验，一般无需修改", cx));
        let mut experiment = design::card()
            .rounded(px(28.))
            .p(px(20.))
            .px(px(22.))
            .gap(px(12.))
            .flex()
            .flex_col();
        experiment = experiment.child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(design::icon_role(
                    Icon::Beaker,
                    20.,
                    ColorRole::ON_SURFACE_VARIANT,
                ))
                .child(design::text(16., 22., 650.).child("高级实验参数"))
                .child(div().flex_1())
                .child(design::muted_text(12., 16.).child("对照组用于观察，不代表因果")),
        );
        for field in editor::FIELDS
            .iter()
            .filter(|field| editor::advanced(field.id))
        {
            experiment = experiment.child(editor.field(field, &errors, self.loading));
        }
        page = page.child(experiment);
        let mut data = design::card()
            .rounded(px(28.))
            .p(px(20.))
            .px(px(22.))
            .gap(px(12.))
            .flex()
            .flex_col();
        data = data
            .child(design::text(16., 22., 650.).child("学习数据"))
            .child(
                design::muted_text(12., 16.)
                    .child("备份包含学习记录和文章，始终不包含密钥。恢复会建立并打开新的学习库。"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .flex_wrap()
                    .child(
                        div()
                            .id("backup-audio")
                            .role(gpui_kit::Role::Button)
                            .aria_label("备份是否包含音频")
                            .cursor_pointer()
                            .h(px(40.))
                            .px(px(16.))
                            .rounded(px(20.))
                            .bg(design::c(ColorRole::SECONDARY_CONTAINER))
                            .text_color(design::c(ColorRole::ON_SECONDARY_CONTAINER))
                            .flex()
                            .items_center()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.backup_audio = !this.backup_audio;
                                cx.notify();
                            }))
                            .child(if self.backup_audio {
                                "备份包含音频：是"
                            } else {
                                "备份包含音频：否"
                            }),
                    )
                    .child(
                        div()
                            .id("export-backup")
                            .role(gpui_kit::Role::Button)
                            .aria_label("导出学习备份")
                            .cursor_pointer()
                            .h(px(40.))
                            .px(px(16.))
                            .rounded(px(20.))
                            .border_1()
                            .border_color(design::c(ColorRole::OUTLINE_VARIANT))
                            .flex()
                            .items_center()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.request(
                                    Command::ExportBackup {
                                        include_audio: this.backup_audio,
                                    },
                                    cx,
                                )
                            }))
                            .child("导出学习备份"),
                    )
                    .child(
                        div()
                            .id("restore-backup")
                            .role(gpui_kit::Role::Button)
                            .aria_label("恢复备份")
                            .cursor_pointer()
                            .h(px(40.))
                            .px(px(16.))
                            .rounded(px(20.))
                            .border_1()
                            .border_color(design::c(ColorRole::OUTLINE_VARIANT))
                            .flex()
                            .items_center()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.request(Command::RestoreBackup, cx)
                            }))
                            .child("恢复到新学习库"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .flex_wrap()
                    .child(
                        div()
                            .id("inspect-legacy")
                            .role(gpui_kit::Role::Button)
                            .aria_label("选择旧库并预检")
                            .cursor_pointer()
                            .h(px(40.))
                            .px(px(16.))
                            .rounded(px(20.))
                            .bg(design::c(ColorRole::SECONDARY_CONTAINER))
                            .text_color(design::c(ColorRole::ON_SECONDARY_CONTAINER))
                            .flex()
                            .items_center()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.request(Command::InspectLegacy, cx)
                            }))
                            .child("选择旧库并预检"),
                    )
                    .child(
                        design::muted_text(12., 16.).child("先只读预检，再确认迁入新的学习库。"),
                    ),
            );
        if let Some(selection) = &self.legacy_selection {
            for (index, profile) in self.legacy_profiles.iter().enumerate() {
                let selection = selection.clone();
                let profile_id = profile.id.clone();
                data = data.child(
                    div()
                        .id(gpui_kit::SharedString::from(format!(
                            "legacy-profile-{index}"
                        )))
                        .role(gpui_kit::Role::Button)
                        .aria_label(format!("档案 {}", index + 1))
                        .cursor_pointer()
                        .h(px(40.))
                        .px(px(16.))
                        .rounded(px(20.))
                        .bg(design::c(ColorRole::SURFACE_CONTAINER))
                        .flex()
                        .items_center()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.request(
                                Command::SelectLegacyProfile {
                                    selection: selection.clone(),
                                    profile: profile_id.clone(),
                                },
                                cx,
                            )
                        }))
                        .child(format!(
                            "档案 {}：{} 张卡 / {} 篇文章",
                            index + 1,
                            profile.cards,
                            profile.articles
                        )),
                );
            }
        }
        if let Some(report) = &self.legacy_report {
            data = data.child(design::muted_text(12., 16.).child(format!(
                    "预检：{} 张旧词卡，{} 条旧评分，{} 篇旧文章，{} 字节音频",
                    report.source_counts.get("cards").copied().unwrap_or(0),
                    report
                        .source_counts
                        .get("review_logs")
                        .copied()
                        .unwrap_or(0),
                    report
                        .source_counts
                        .get("articles")
                        .copied()
                        .unwrap_or(0),
                    report.audio_bytes
                )));
            for message in report.blockers.iter().chain(&report.warnings).take(12) {
                data = data.child(design::muted_text(12., 16.).child(message.clone()));
            }
            data = data
                .child(
                    div()
                        .id("copy-legacy-report")
                        .role(gpui_kit::Role::Button)
                        .aria_label("复制预检报告")
                        .cursor_pointer()
                        .h(px(40.))
                        .px(px(16.))
                        .rounded(px(20.))
                        .bg(design::c(ColorRole::SECONDARY_CONTAINER))
                        .flex()
                        .items_center()
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(report) = &this.legacy_report
                                && let Ok(text) = serde_json::to_string_pretty(report)
                            {
                                cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(text));
                                this.notice = Some("预检报告已复制，不包含密钥".into());
                                cx.notify();
                            }
                        }))
                        .child("复制完整预检报告"),
                )
                .child(
                    div()
                        .id("confirm-legacy-import")
                        .role(gpui_kit::Role::Button)
                        .aria_label("确认迁入")
                        .cursor_pointer()
                        .h(px(56.))
                        .px(px(28.))
                        .rounded(px(28.))
                        .bg(design::c(ColorRole::PRIMARY))
                        .text_color(design::c(ColorRole::ON_PRIMARY))
                        .flex()
                        .items_center()
                        .when(self.loading || !report.blockers.is_empty(), |button| {
                            button.opacity(0.38)
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(selection) = this.legacy_selection.clone() {
                                this.request(Command::ImportLegacy { selection }, cx);
                            }
                        }))
                        .child("确认迁入预览快照并打开新库"),
                );
        }
        page = page.child(data);
        let footer = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .mt(px(4.))
            .child(div().flex_1())
            .child(
                div()
                    .id("reload-settings")
                    .role(gpui_kit::Role::Button)
                    .aria_label("重新载入已保存设置")
                    .cursor_pointer()
                    .h(px(40.))
                    .px(px(12.))
                    .rounded(px(20.))
                    .flex()
                    .items_center()
                    .text_color(design::c(ColorRole::PRIMARY))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if !this.guard_settings_exit(PendingSettings::Reload, cx) {
                            this.request(Command::LoadSettings, cx);
                        }
                    }))
                    .child("重新载入"),
            )
            .child(
                div()
                    .id("save-settings")
                    .test_support()
                    .role(gpui_kit::Role::Button)
                    .aria_label("保存实验参数")
                    .cursor_pointer()
                    .h(px(56.))
                    .px(px(28.))
                    .rounded(px(28.))
                    .bg(design::c(ColorRole::PRIMARY))
                    .text_color(design::c(ColorRole::ON_PRIMARY))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(editor) = &this.settings_editor {
                            match editor.value(&this.settings, cx) {
                                Ok(value) => this.request(
                                    Command::SaveSettings {
                                        value,
                                        revision: this.settings_revision,
                                        operation: format!(
                                            "settings-{}-{}",
                                            this.settings_revision, this.request_id
                                        ),
                                    },
                                    cx,
                                ),
                                Err(error) => {
                                    this.error = Some(error.message);
                                    cx.notify();
                                }
                            }
                        }
                    }))
                    .child(design::icon_role(Icon::Check, 18., ColorRole::ON_PRIMARY))
                    .child("保存并生效"),
            );
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(page)
            .child(footer)
            .into_any_element()
    }

    pub(super) fn settings_body(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        if self.settings_editor.is_none() {
            return design::pane()
                .flex_1()
                .h_full()
                .p(px(28.))
                .child(design::muted_text(16., 24.).child("正在读取设置…"))
                .into_any_element();
        }
        let content = match self.settings_ui.page {
            SettingsPage::Provider => self.provider_page(cx),
            SettingsPage::Learning => self.learning_page(cx),
            SettingsPage::Experiment => self.experiment_page(cx),
        };
        let content = design::pane()
            .flex_1()
            .min_w_0()
            .h_full()
            .p(px(28.))
            .px(px(if self.compact { 18. } else { 36. }))
            .gap(px(18.))
            .child(content);
        if self.compact {
            let mut tabs = div().flex().gap(px(8.)).w_full().px(px(16.)).pt(px(12.));
            for (page, label) in [
                (SettingsPage::Provider, "AI 服务"),
                (SettingsPage::Learning, "学习与阅读"),
                (SettingsPage::Experiment, "实验参数"),
            ] {
                let selected = self.settings_ui.page == page;
                tabs = tabs.child(
                    div()
                        .id(gpui_kit::SharedString::from(match page {
                            SettingsPage::Provider => "settings-ai",
                            SettingsPage::Learning => "settings-learning",
                            SettingsPage::Experiment => "settings-experiment",
                        }))
                        .test_support()
                        .role(gpui_kit::Role::Button)
                        .aria_label(label)
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.settings_ui.page = page;
                            this.settings_ui.advanced = page == SettingsPage::Experiment;
                            cx.notify();
                        }))
                        .child(design::chip(selected, label, None)),
                );
            }
            return div()
                .flex()
                .flex_col()
                .w_full()
                .h_full()
                .child(tabs)
                .child(div().flex_1().min_h_0().overflow_hidden().child(content))
                .into_any_element();
        }
        let main = div()
            .flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .gap(px(12.))
            .child(self.settings_nav(cx))
            .child(content);
        div()
            .flex()
            .w_full()
            .h_full()
            .child(main)
            .child(self.settings_dialog(cx))
            .into_any_element()
    }
}
