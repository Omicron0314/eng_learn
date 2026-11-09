use super::*;
use cijian_domain::provider::ProviderConfig;
use gpui_kit_material::components::MaterialDialog;
#[derive(Clone)]
pub(crate) enum PendingSettings {
    Navigate(Route),
    Back,
    Close,
    Reset,
    Reload,
}
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsPage {
    #[default]
    Provider,
    Learning,
    Experiment,
}
#[derive(Default)]
pub(crate) struct SettingsState {
    pub pending: Option<PendingSettings>,
    pub page: SettingsPage,
    pub advanced: bool,
    pub focus_advanced: bool,
    pub refresh_learning: bool,
    pub refresh_provider: Option<ProviderKind>,
    /// Providers still waiting for their turn: one save request at a time keeps
    /// the shared command channel and each provider's revision intact.
    pub queued_providers: std::collections::VecDeque<QueuedProviderSave>,
}

/// A validated provider write waiting for the single command slot.
pub(crate) struct QueuedProviderSave {
    pub kind: ProviderKind,
    pub config: ProviderConfig,
    pub revision: u64,
    pub secret: SecretPatch,
}
impl Shell {
    pub(super) fn settings_dirty(&self, cx: &App) -> bool {
        self.navigation.current() == &Route::Settings
            && self.settings_editor.as_ref().is_some_and(|editor| {
                editor.learning_dirty(&self.settings, cx) || editor.any_provider_dirty(cx)
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
        let llm = self.provider_config.as_ref();
        let tts = self.tts_config.as_ref();
        if let Some(editor) = &mut self.settings_editor {
            let refresh_learning = std::mem::take(&mut self.settings_ui.refresh_learning);
            let refresh_provider = self.settings_ui.refresh_provider.take();
            if refresh_learning {
                editor.reset_learning(&self.settings, window, cx);
            }
            if let Some(kind) = refresh_provider {
                editor.reset_provider(
                    kind,
                    if kind == ProviderKind::Tts { tts } else { llm },
                    window,
                    cx,
                );
            }
        } else {
            self.settings_editor = Some(Editor::new(&self.settings, [llm, tts], window, cx));
            self.settings_ui.refresh_learning = false;
            self.settings_ui.refresh_provider = None;
        }
        if self.settings_ui.focus_advanced && !self.loading {
            self.settings_ui.focus_advanced = false;
            self.settings_ui.page = SettingsPage::Experiment;
            self.page_scroll.set_offset(Default::default());
            if let Some(editor) = &self.settings_editor {
                editor.focus_advanced(window, cx);
            }
        }
    }
    fn provider_baseline(&self, kind: ProviderKind) -> Option<&ProviderConfig> {
        if kind == ProviderKind::Tts {
            self.tts_config.as_ref()
        } else {
            self.provider_config.as_ref()
        }
    }
    pub(super) fn provider_has_key(&self, kind: ProviderKind) -> bool {
        if kind == ProviderKind::Tts {
            self.has_tts_key
        } else {
            self.has_session_key
        }
    }
    pub(super) fn reset_providers(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        let llm = self.provider_config.clone();
        let tts = self.tts_config.clone();
        if let Some(editor) = &mut self.settings_editor {
            editor.reset_provider(ProviderKind::Llm, llm.as_ref(), window, cx);
            editor.reset_provider(ProviderKind::Tts, tts.as_ref(), window, cx);
        }
        cx.notify();
    }
    /// One click saves every edited provider. Each provider owns its revision
    /// and secret, so commands run one after another instead of as a single
    /// all-or-nothing write; every draft is validated before the first one is
    /// sent so a bad field cannot leave a half-applied page.
    pub(super) fn save_providers(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        let prepared = {
            let Some(editor) = &self.settings_editor else {
                return;
            };
            [ProviderKind::Llm, ProviderKind::Tts]
                .into_iter()
                .filter(|kind| editor.provider_dirty(*kind, cx))
                .map(|kind| {
                    let revision = self
                        .provider_baseline(kind)
                        .map_or(0, |config| config.revision);
                    let draft = editor.provider(kind);
                    let config = draft.config(kind, revision + 1, cx);
                    config.validate().map_err(|error| error.to_string())?;
                    let secret = draft.secret(false, cx).map_err(|error| error.message)?;
                    Ok(QueuedProviderSave {
                        kind,
                        config,
                        revision,
                        secret,
                    })
                })
                .collect::<Result<Vec<_>, String>>()
        };
        let mut saves = match prepared {
            Ok(saves) => saves,
            Err(message) => {
                self.error = Some(message);
                cx.notify();
                return;
            }
        };
        if saves.is_empty() {
            return;
        }
        let first = saves.remove(0);
        self.settings_ui.queued_providers = saves.into();
        self.dispatch_provider_save(first, cx);
    }
    pub(crate) fn next_queued_provider(&mut self) -> Option<QueuedProviderSave> {
        self.settings_ui.queued_providers.pop_front()
    }
    pub(super) fn clear_provider_key(&mut self, kind: ProviderKind, cx: &mut Context<Self>) {
        if self.loading || !self.provider_has_key(kind) {
            return;
        }
        let revision = self
            .provider_baseline(kind)
            .map_or(0, |config| config.revision);
        let prepared = {
            let Some(editor) = &self.settings_editor else {
                return;
            };
            if editor.provider_dirty(kind, cx) {
                return;
            }
            let config = editor.provider(kind).config(kind, revision + 1, cx);
            match config.validate() {
                Ok(()) => Ok(QueuedProviderSave {
                    kind,
                    config,
                    revision,
                    secret: SecretPatch::Clear,
                }),
                Err(error) => Err(error.to_string()),
            }
        };
        match prepared {
            Ok(save) => self.dispatch_provider_save(save, cx),
            Err(message) => {
                self.error = Some(message);
                cx.notify();
            }
        }
    }
    pub(crate) fn dispatch_provider_save(
        &mut self,
        save: QueuedProviderSave,
        cx: &mut Context<Self>,
    ) {
        let operation = format!(
            "provider-{:?}-{}-{}",
            save.kind, save.revision, self.request_id
        );
        self.request(
            Command::SaveProvider {
                config: save.config,
                revision: save.revision,
                secret: save.secret,
                operation,
            },
            cx,
        );
    }
    fn discard_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pending) = self.settings_ui.pending.take() else {
            return;
        };
        if let Some(editor) = &self.settings_editor {
            self.settings = editor.baseline(&self.settings);
        }
        self.settings_editor = None;
        self.error = None;
        self.settings_ui.queued_providers.clear();
        match pending {
            PendingSettings::Navigate(route) => self.navigate(route, cx),
            PendingSettings::Back => self.go_back(cx),
            PendingSettings::Close => {
                if self.close_reader_when_saved(window, cx) {
                    window.remove_window();
                }
            }
            PendingSettings::Reset => {}
            PendingSettings::Reload => self.request(Command::LoadSettings, cx),
        };
        cx.notify();
    }
    pub(super) fn settings_dialog(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let Some(pending) = &self.settings_ui.pending else {
            return div().into_any_element();
        };
        let weak = cx.entity().downgrade();
        let (headline, detail) = if matches!(pending, PendingSettings::Close) {
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
