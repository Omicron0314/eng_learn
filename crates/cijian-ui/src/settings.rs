use super::*;
use cijian_domain::{
    provider::{ProviderConfig, ProviderKind},
    settings::{Genre, Level, LookupMode, Theme, Tier},
};
use cijian_ports::provider::{Secret, SecretPatch};
use gpui_kit_material::components::TextField;
mod drafts;
mod editor;
pub(super) use drafts::{PendingSettings, SettingsState};
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
    pub(super) fn settings_body(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let mut body = div().flex().flex_col().gap_4();
        let Some(editor) = &self.settings_editor else {
            return body.child("正在读取设置…").into_any_element();
        };
        let (draft, errors) = editor.draft(&self.settings, cx);
        let learning_dirty = editor.learning_dirty(&self.settings, cx);
        let dirty = learning_dirty || editor.provider_dirty(cx);
        body = body
            .child(if dirty {
                "有未保存的修改"
            } else {
                "设置已保存"
            })
            .child(
                material_button("reset-settings-draft", ButtonVariant::Text)
                    .child("撤销未保存修改")
                    .disabled(self.loading || !dirty)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.settings_ui.pending = Some(PendingSettings::Reset);
                        cx.notify();
                    })),
            )
            .child(
                material_button("reload-settings", ButtonVariant::Text)
                    .child("重新载入已保存设置")
                    .disabled(self.loading)
                    .on_click(cx.listener(|this, _, _, cx| {
                        if !this.guard_settings_exit(PendingSettings::Reload, cx) {
                            this.request(Command::LoadSettings, cx);
                        }
                    })),
            )
            .child(
                material_button("advanced-settings", ButtonVariant::Tonal)
                    .child(if errors.keys().any(|id| editor::advanced(id)) {
                        "高级实验设置 · 有待修正项"
                    } else if self.settings_ui.advanced {
                        "收起高级实验设置"
                    } else {
                        "展开高级实验设置"
                    })
                    .disabled(self.loading)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.settings_ui.advanced = !this.settings_ui.advanced;
                        cx.notify();
                    })),
            );
        if self.settings_ui.advanced {
            body = body
                .child("高级实验设置")
                .child("对照词比例用于生成时划分出现组与对照组，修改后用于后续生成。");
            for field in editor::FIELDS
                .iter()
                .filter(|field| editor::advanced(field.id))
            {
                body = body.child(editor.field(field, &errors, self.loading));
            }
        }
        body = body
            .child("外观与学习")
            .child(
                material_button("theme", ButtonVariant::Tonal)
                    .child(if self.dark {
                        "切换浅色"
                    } else {
                        "切换深色"
                    })
                    .disabled(self.loading)
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
                    })),
            )
            .child(
                material_button("system-theme", ButtonVariant::Text)
                    .child("主题跟随系统")
                    .disabled(self.loading)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.settings.theme = Theme::System;
                        this.apply_theme(window, cx);
                        if this.backend.is_some() {
                            this.request(
                                Command::SetTheme {
                                    value: Theme::System,
                                },
                                cx,
                            );
                        }
                    })),
            )
            .child(
                material_button("device-rate", ButtonVariant::Tonal)
                    .child(format!("本机播放速度：{}×", self.settings.playback_rate))
                    .disabled(self.loading)
                    .on_click(cx.listener(|this, _, _, cx| {
                        let rates = cijian_domain::settings::PLAYBACK_RATES;
                        let index = rates
                            .iter()
                            .position(|r| *r == this.settings.playback_rate)
                            .unwrap_or(1);
                        this.request(
                            Command::SetPlaybackRate {
                                rate: rates[(index + 1) % rates.len()],
                            },
                            cx,
                        );
                    })),
            )
            .child(
                material_button("level", ButtonVariant::Tonal)
                    .child(format!("英语等级：{:?}", self.settings.level))
                    .disabled(self.loading)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.settings.level = match this.settings.level {
                            Level::A2 => Level::B1,
                            Level::B1 => Level::B2,
                            Level::B2 => Level::C1,
                            Level::C1 => Level::A2,
                        };
                        cx.notify();
                    })),
            )
            .child(
                material_button("lookup-mode", ButtonVariant::Tonal)
                    .child(if self.settings.lookup_mode == LookupMode::MarkOnly {
                        "不认识时放入生词箱"
                    } else {
                        "不认识时自动加入复习"
                    })
                    .disabled(self.loading)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.settings.lookup_mode =
                            if this.settings.lookup_mode == LookupMode::MarkOnly {
                                LookupMode::AutoAdd
                            } else {
                                LookupMode::MarkOnly
                            };
                        cx.notify();
                    })),
            );
        if self.settings.lookup_mode == LookupMode::AutoAdd {
            body = body.child("自动加入复习会把标为“不认识”的词直接加入学习流程，可能增加复习量。");
        }
        body = body
            .child(
                material_button("default-tier", ButtonVariant::Tonal)
                    .child(format!(
                        "默认难度：{}",
                        match self.settings.tier {
                            Tier::Easy => "轻松",
                            Tier::Medium => "适中",
                            Tier::Hard => "挑战",
                        }
                    ))
                    .disabled(self.loading)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.settings.tier = match this.settings.tier {
                            Tier::Easy => Tier::Medium,
                            Tier::Medium => Tier::Hard,
                            Tier::Hard => Tier::Easy,
                        };
                        cx.notify();
                    })),
            )
            .child(
                material_button("default-genre", ButtonVariant::Tonal)
                    .child(format!(
                        "默认体裁：{}",
                        match self.settings.genre {
                            Genre::Auto => "自动",
                            Genre::Story => "故事",
                            Genre::Dialogue => "对话",
                            Genre::Monologue => "独白",
                        }
                    ))
                    .disabled(self.loading)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.settings.genre = match this.settings.genre {
                            Genre::Auto => Genre::Story,
                            Genre::Story => Genre::Dialogue,
                            Genre::Dialogue => Genre::Monologue,
                            Genre::Monologue => Genre::Auto,
                        };
                        cx.notify();
                    })),
            );
        for field in editor::FIELDS
            .iter()
            .filter(|field| !editor::advanced(field.id))
        {
            body = body.child(editor.field(field, &errors, self.loading));
        }
        body = body.child(
            TextField::new("timezone", &editor.fields["timezone"])
                .label("学习时区")
                .disabled(self.loading)
                .error(errors.contains_key("timezone"))
                .supporting_text(
                    errors
                        .get("timezone")
                        .cloned()
                        .unwrap_or("使用 IANA 时区名称".into()),
                ),
        );
        let saved = editor.baseline(&self.settings);
        if draft.timezone != saved.timezone || draft.day_start_hour != saved.day_start_hour {
            body = body
                .child("改变时区或日界会按新边界重新统计活动与今日配额，已有复习时间保持不变。");
        }
        body = body.child(
            material_button("save-settings", ButtonVariant::Filled)
                .child("保存学习设置")
                .disabled(self.loading || !learning_dirty || !errors.is_empty())
                .on_click(cx.listener(|this, _, _, cx| {
                    let Some(editor) = &this.settings_editor else {
                        return;
                    };
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
                })),
        );
        let has_key = if self.provider_kind == ProviderKind::Llm {
            self.has_session_key
        } else {
            self.has_tts_key
        };
        body = body
            .child("AI 服务")
            .child(
                material_button("provider-kind", ButtonVariant::Tonal)
                    .child(if self.provider_kind == ProviderKind::Llm {
                        "配置类型：文章与释义"
                    } else {
                        "配置类型：远程朗读"
                    })
                    .disabled(self.loading)
                    .on_click(cx.listener(|this, _, window, cx| {
                        let kind = if this.provider_kind == ProviderKind::Llm {
                            ProviderKind::Tts
                        } else {
                            ProviderKind::Llm
                        };
                        this.switch_provider(kind, window, cx);
                    })),
            )
            .child(key_source_copy(self.key_source, has_key).0)
            .child(key_source_copy(self.key_source, has_key).1)
            .child(
                TextField::new("provider-endpoint", &editor.endpoint)
                    .label("服务地址")
                    .disabled(self.loading),
            )
            .child(
                TextField::new("provider-model", &editor.model)
                    .label("模型名称")
                    .disabled(self.loading),
            )
            .when(self.provider_kind == ProviderKind::Tts, |body| {
                body.child(
                    TextField::new("provider-voice", &editor.voice)
                        .label("语音名称")
                        .disabled(self.loading),
                )
            })
            .child(
                TextField::new("provider-key", &editor.key)
                    .label("会话密钥")
                    .disabled(self.loading),
            )
            .child(
                material_button("save-provider", ButtonVariant::Filled)
                    .child("保存 AI 配置")
                    .disabled(self.loading || !editor.provider_dirty(cx))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.save_provider(false, cx);
                    })),
            )
            .child(
                material_button("clear-provider-key", ButtonVariant::Text)
                    .child("清除已保存的会话密钥")
                    .disabled(self.loading || !has_key || editor.provider_dirty(cx))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.save_provider(true, cx);
                    })),
            );
        body = body
            .when(has_key && editor.provider_dirty(cx), |body| {
                body.child("请先保存或撤销 AI 草稿，再清除已保存密钥。")
            })
            .child("学习数据")
            .child("备份包含学习记录和文章，不包含密钥或音频缓存。恢复会建立并打开新的学习库。")
            .child(
                material_button("export-backup", ButtonVariant::Tonal)
                    .child("导出学习备份")
                    .disabled(self.loading)
                    .on_click(
                        cx.listener(|this, _, _, cx| this.request(Command::ExportBackup, cx)),
                    ),
            )
            .child(
                material_button("restore-backup", ButtonVariant::Tonal)
                    .child("恢复到新学习库并打开")
                    .disabled(self.loading)
                    .on_click(
                        cx.listener(|this, _, _, cx| this.request(Command::RestoreBackup, cx)),
                    ),
            );
        body = body
            .child("旧版学习库迁入")
            .child("先只读预检，再确认迁入新的学习库。密钥不会迁入，新库增量不会回写旧 Web。")
            .child(
                material_button("inspect-legacy", ButtonVariant::Tonal)
                    .child("选择旧库并预检")
                    .disabled(self.loading)
                    .on_click(
                        cx.listener(|this, _, _, cx| this.request(Command::InspectLegacy, cx)),
                    ),
            );
        if let Some(selection) = &self.legacy_selection {
            for (index, profile) in self.legacy_profiles.iter().enumerate() {
                let selection = selection.clone();
                let profile_id = profile.id.clone();
                body = body.child(
                    material_button(
                        gpui_kit::SharedString::from(format!("legacy-profile-{index}")),
                        ButtonVariant::Tonal,
                    )
                    .child(format!(
                        "档案 {}：{} 张卡 / {} 篇文章",
                        index + 1,
                        profile.cards,
                        profile.articles
                    ))
                    .disabled(self.loading)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.request(
                            Command::SelectLegacyProfile {
                                selection: selection.clone(),
                                profile: profile_id.clone(),
                            },
                            cx,
                        )
                    })),
                );
            }
        }
        if let Some(report) = &self.legacy_report {
            body = body.child(format!(
                "预检：{} 张旧词卡，{} 条旧评分，{} 篇旧文章，{} 字节音频",
                report.source_counts.get("cards").copied().unwrap_or(0),
                report
                    .source_counts
                    .get("review_logs")
                    .copied()
                    .unwrap_or(0),
                report.source_counts.get("articles").copied().unwrap_or(0),
                report.audio_bytes
            ));
            for message in report.blockers.iter().chain(&report.warnings).take(20) {
                body = body.child(message.clone());
            }
            if report.blockers.len() + report.warnings.len() > 20 {
                body = body.child("更多提示可复制完整预检报告查看。");
            }
            body = body
                .child(
                    material_button("copy-legacy-report", ButtonVariant::Text)
                        .child("复制完整预检报告")
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(report) = &this.legacy_report
                                && let Ok(text) = serde_json::to_string_pretty(report)
                            {
                                cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(text));
                                this.notice = Some("预检报告已复制，不包含密钥".into());
                                cx.notify();
                            }
                        })),
                )
                .child(
                    material_button("confirm-legacy-import", ButtonVariant::Filled)
                        .child("确认迁入预览快照并打开新库")
                        .disabled(self.loading || !report.blockers.is_empty())
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(selection) = this.legacy_selection.clone() {
                                this.request(Command::ImportLegacy { selection }, cx);
                            }
                        })),
                );
        }
        body.child(self.settings_dialog(cx)).into_any_element()
    }
    fn save_provider(&mut self, clear: bool, cx: &mut Context<Self>) {
        let Some(editor) = &self.settings_editor else {
            return;
        };
        let revision = (if self.provider_kind == ProviderKind::Tts {
            self.tts_config.as_ref()
        } else {
            self.provider_config.as_ref()
        })
        .map_or(0, |p| p.revision);
        let config = ProviderConfig {
            kind: self.provider_kind,
            endpoint: editor.endpoint.read(cx).value().trim().into(),
            model: editor.model.read(cx).value().trim().into(),
            voice: if self.provider_kind == ProviderKind::Tts {
                Some(editor.voice.read(cx).value().trim().into())
            } else {
                None
            },
            revision: revision + 1,
        };
        if let Err(error) = config.validate() {
            self.error = Some(error.to_string());
            cx.notify();
            return;
        }
        let text = editor.key.read(cx).value().to_string();
        let secret = if clear {
            SecretPatch::Clear
        } else if text.is_empty() {
            SecretPatch::Keep
        } else {
            match Secret::new(text) {
                Ok(secret) => SecretPatch::Replace(secret),
                Err(error) => {
                    self.error = Some(error.message);
                    cx.notify();
                    return;
                }
            }
        };
        self.request(
            Command::SaveProvider {
                config,
                revision,
                secret,
                operation: format!("provider-{revision}-{}", self.request_id),
            },
            cx,
        );
    }
}
