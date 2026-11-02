use super::*;
use gpui_kit_material::components::MaterialDialog;
#[derive(Clone)]
pub(crate) enum PendingSettings {
    Navigate(Route),
    Back,
    Close,
    Reset,
    Reload,
    Provider(ProviderKind),
}
#[derive(Default)]
pub(crate) struct SettingsState {
    pub pending: Option<PendingSettings>,
    pub advanced: bool,
    pub focus_advanced: bool,
    pub refresh_learning: bool,
    pub refresh_provider: bool,
}
impl Shell {
    pub(super) fn settings_dirty(&self, cx: &App) -> bool {
        self.navigation.current() == &Route::Settings
            && self.settings_editor.as_ref().is_some_and(|editor| {
                editor.learning_dirty(&self.settings, cx) || editor.provider_dirty(cx)
            })
    }
    pub(crate) fn guard_settings_exit(
        &mut self,
        pending: PendingSettings,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.navigation.current() != &Route::Settings {
            return false;
        }
        if self.loading {
            self.notice = Some("正在处理设置，请完成后再离开".into());
            cx.notify();
            return true;
        }
        if self.settings_dirty(cx) {
            self.settings_ui.pending = Some(pending);
            cx.notify();
            return true;
        }
        false
    }
    pub(crate) fn may_close(&mut self, cx: &mut Context<Self>) -> bool {
        if self.guard_settings_exit(PendingSettings::Close, cx) {
            return false;
        }
        if self.library_ui.editor.is_some() {
            if self.loading {
                self.notice = Some("正在保存词卡，请完成后再关闭".into());
                cx.notify();
                return false;
            }
            if self
                .library_ui
                .editor
                .as_ref()
                .is_some_and(|editor| editor.dirty(cx))
            {
                self.library_ui.discard = true;
                self.library_ui.pending_close = true;
                self.library_ui.pending_back = false;
                self.library_ui.pending_route = None;
                cx.notify();
                return false;
            }
        }
        true
    }
    pub(crate) fn prepare_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.navigation.current() != &Route::Settings {
            return;
        }
        let provider = if self.provider_kind == ProviderKind::Tts {
            self.tts_config.as_ref()
        } else {
            self.provider_config.as_ref()
        };
        if self.settings_editor.is_none() {
            self.settings_editor = Some(Editor::new(
                &self.settings,
                self.provider_kind,
                provider,
                window,
                cx,
            ));
            self.settings_ui.refresh_learning = false;
            self.settings_ui.refresh_provider = false;
        } else if let Some(editor) = &mut self.settings_editor {
            if std::mem::take(&mut self.settings_ui.refresh_learning) {
                editor.reset_learning(&self.settings, window, cx);
            }
            if std::mem::take(&mut self.settings_ui.refresh_provider) {
                editor.reset_provider(self.provider_kind, provider, window, cx);
            }
        }
        if self.settings_ui.focus_advanced && !self.loading {
            self.settings_ui.focus_advanced = false;
            self.page_scroll.set_offset(Default::default());
            if let Some(editor) = &self.settings_editor {
                editor.focus_advanced(window, cx);
            }
        }
    }
    pub(super) fn switch_provider(
        &mut self,
        kind: ProviderKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.loading {
            return;
        }
        if self
            .settings_editor
            .as_ref()
            .is_some_and(|editor| editor.provider_dirty(cx))
        {
            self.settings_ui.pending = Some(PendingSettings::Provider(kind));
            cx.notify();
            return;
        }
        self.apply_provider_kind(kind, window, cx);
    }
    fn apply_provider_kind(
        &mut self,
        kind: ProviderKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.provider_kind = kind;
        let provider = if kind == ProviderKind::Tts {
            self.tts_config.as_ref()
        } else {
            self.provider_config.as_ref()
        };
        if let Some(editor) = &mut self.settings_editor {
            editor.reset_provider(kind, provider, window, cx);
        }
        cx.notify();
    }
    fn discard_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pending) = self.settings_ui.pending.take() else {
            return;
        };
        if let PendingSettings::Provider(kind) = pending {
            self.apply_provider_kind(kind, window, cx);
            return;
        }
        if let Some(editor) = &self.settings_editor {
            self.settings = editor.baseline(&self.settings);
        }
        self.settings_editor = None;
        self.error = None;
        match pending {
            PendingSettings::Navigate(route) => self.navigate(route, cx),
            PendingSettings::Back => self.go_back(cx),
            PendingSettings::Close => window.remove_window(),
            PendingSettings::Reset => {}
            PendingSettings::Reload => self.request(Command::LoadSettings, cx),
            PendingSettings::Provider(_) => unreachable!(),
        };
        cx.notify();
    }
    pub(super) fn settings_dialog(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let Some(pending) = &self.settings_ui.pending else {
            return div().into_any_element();
        };
        let weak = cx.entity().downgrade();
        let (headline, detail) = if matches!(pending, PendingSettings::Provider(_)) {
            (
                "放弃当前 AI 配置草稿？",
                "切换服务类型会清空尚未保存的地址、模型、语音和密钥输入。学习设置草稿会保留。",
            )
        } else if matches!(pending, PendingSettings::Close) {
            (
                "放弃未保存的设置并关闭窗口？",
                "未保存的学习设置和 AI 配置输入将被丢弃。",
            )
        } else {
            (
                "放弃未保存的设置？",
                "学习设置和 AI 配置分别保存。放弃将清空尚未保存的内容。",
            )
        };
        MaterialDialog::new("discard-settings-dialog")
            .headline(headline)
            .supporting_text(detail)
            .modal(true)
            .dismiss_on_scrim(false)
            .on_dismiss(move |_, cx| {
                let _ = weak.update(cx, |this, cx| {
                    this.settings_ui.pending = None;
                    cx.notify();
                });
            })
            .action(
                material_button("keep-settings-draft", ButtonVariant::Tonal)
                    .child("继续编辑")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.settings_ui.pending = None;
                        cx.notify();
                    })),
            )
            .action(
                material_button("discard-settings-confirm", ButtonVariant::Text)
                    .child("放弃修改")
                    .on_click(cx.listener(|this, _, window, cx| this.discard_settings(window, cx))),
            )
            .into_any_element()
    }
}
