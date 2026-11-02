use super::*;
use cijian_domain::{
    provider::{ProviderConfig, ProviderKind},
    settings::{Level, LookupMode, Settings, Theme},
};
use cijian_ports::provider::{Secret, SecretPatch};
use gpui_kit::{Entity, base::input::InputState};
use gpui_kit_material::components::TextField;
use std::collections::BTreeMap;
pub(super) struct Editor {
    fields: BTreeMap<&'static str, Entity<InputState>>,
    endpoint: Entity<InputState>,
    model: Entity<InputState>,
    voice: Entity<InputState>,
    key: Entity<InputState>,
}
impl Editor {
    pub fn new(
        value: &Settings,
        provider: Option<&ProviderConfig>,
        window: &mut Window,
        cx: &mut Context<Shell>,
    ) -> Self {
        let values = [
            ("daily-new", value.daily_new_limit.to_string()),
            ("daily-review", value.daily_review_limit.to_string()),
            ("retention", value.request_retention.to_string()),
            ("min-words", value.article_min_words.to_string()),
            ("max-words", value.article_max_words.to_string()),
            ("targets", value.target_count.to_string()),
            ("control-ratio", value.control_ratio.to_string()),
            ("retries", value.generation_max_retries.to_string()),
            ("tts-limit", value.tts_daily_char_limit.to_string()),
            ("timezone", value.timezone.clone()),
            ("day-start", value.day_start_hour.to_string()),
        ];
        let fields = values
            .into_iter()
            .map(|(key, value)| {
                (
                    key,
                    cx.new(|cx| InputState::new(window, cx).default_value(value)),
                )
            })
            .collect();
        let endpoint = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(provider.map(|p| p.endpoint.clone()).unwrap_or_default())
        });
        let model = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(provider.map(|p| p.model.clone()).unwrap_or_default())
        });
        let voice = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(provider.and_then(|p| p.voice.clone()).unwrap_or_default())
        });
        let key = cx.new(|cx| InputState::new(window, cx).masked(true));
        Self {
            voice,
            fields,
            endpoint,
            model,
            key,
        }
    }
    fn value(&self, base: &Settings, cx: &App) -> Result<Settings, AppError> {
        let text = |key: &str| self.fields[key].read(cx).value().to_string();
        let integer = |key: &str| {
            text(key)
                .trim()
                .parse::<u32>()
                .map_err(|_| AppError::new(cijian_ports::ErrorCode::InvalidInput, "请输入有效整数"))
        };
        let number = |key: &str| {
            text(key)
                .trim()
                .parse::<f64>()
                .map_err(|_| AppError::new(cijian_ports::ErrorCode::InvalidInput, "请输入有效数值"))
        };
        let mut value = base.clone();
        value.daily_new_limit = integer("daily-new")?;
        value.daily_review_limit = integer("daily-review")?;
        value.request_retention = number("retention")?;
        value.article_min_words = integer("min-words")?;
        value.article_max_words = integer("max-words")?;
        value.target_count = integer("targets")?;
        value.control_ratio = number("control-ratio")?;
        value.generation_max_retries = integer("retries")?;
        value.tts_daily_char_limit = integer("tts-limit")?;
        value.timezone = text("timezone");
        value.day_start_hour = integer("day-start")?;
        cijian_domain::settings::ValidatedSettings::try_from(value.clone())
            .map_err(|e| AppError::new(cijian_ports::ErrorCode::InvalidInput, e.to_string()))?;
        Ok(value)
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
            init_component_theme(theme::scheme(dark), 1., cx);
            cx.notify();
        }
    }
    pub(super) fn settings_body(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let mut body = div().flex().flex_col().gap_4();
        let Some(editor) = &self.settings_editor else {
            return body.child("正在读取设置…").into_any_element();
        };
        body = body
            .child("外观与学习")
            .child(
                material_button("theme", ButtonVariant::Tonal)
                    .child(if self.dark {
                        "切换浅色"
                    } else {
                        "切换深色"
                    })
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
                    .on_click(cx.listener(|this, _, _, cx| {
                        let rates = [0.75, 1., 1.25, 1.5, 2.];
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
        for (key, label) in [
            ("daily-new", "每日新词上限"),
            ("daily-review", "每日复习上限"),
            ("retention", "目标记忆保留率"),
            ("min-words", "文章最少词数"),
            ("max-words", "文章最多词数"),
            ("targets", "目标词数量"),
            ("control-ratio", "对照词比例"),
            ("retries", "覆盖率重写次数"),
            ("tts-limit", "每日远程朗读字符额度"),
            ("timezone", "学习时区"),
            ("day-start", "学习日开始小时"),
        ] {
            body = body.child(TextField::new(key, &editor.fields[key]).label(label));
        }
        body = body.child(
            material_button("save-settings", ButtonVariant::Filled)
                .child("保存学习设置")
                .disabled(self.loading)
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
        body = body
            .child("AI 服务")
            .child(
                material_button("provider-kind", ButtonVariant::Tonal)
                    .child(if self.provider_kind == ProviderKind::Llm {
                        "配置类型：文章与释义"
                    } else {
                        "配置类型：远程朗读"
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.provider_kind = if this.provider_kind == ProviderKind::Llm {
                            ProviderKind::Tts
                        } else {
                            ProviderKind::Llm
                        };
                        this.settings_editor = None;
                        cx.notify();
                    })),
            )
            .child("保存配置不会调用模型。密钥仅保存在本次会话，退出后需重新输入。")
            .child(
                if (self.provider_kind == ProviderKind::Llm && self.has_session_key)
                    || (self.provider_kind == ProviderKind::Tts && self.has_tts_key)
                {
                    "本次会话已设置密钥；留空表示保留"
                } else {
                    "本次会话未设置密钥"
                },
            )
            .child(TextField::new("provider-endpoint", &editor.endpoint).label("服务地址"))
            .child(TextField::new("provider-model", &editor.model).label("模型名称"))
            .child(
                TextField::new("provider-voice", &editor.voice).label("语音名称（远程朗读时必填）"),
            )
            .child(TextField::new("provider-key", &editor.key).label("会话密钥"))
            .child(
                material_button("save-provider", ButtonVariant::Filled)
                    .child("保存 AI 配置")
                    .disabled(self.loading)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.save_provider(false, cx);
                    })),
            )
            .child(
                material_button("clear-provider-key", ButtonVariant::Text)
                    .child("清除会话密钥并保存配置")
                    .disabled(self.loading)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.save_provider(true, cx);
                    })),
            );
        body = body
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
        body.into_any_element()
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
