#[cfg(test)]
mod tests;
use super::*;
use cijian_domain::settings::{Settings, numeric_rule};
use gpui_kit::{
    Entity, Focusable,
    base::{
        input::{InputEvent, InputState},
        slider::{SliderEvent, SliderState, SliderValue},
    },
};
use gpui_kit_material::components::MaterialSlider;
use std::collections::BTreeMap;
pub(super) struct Field {
    pub id: &'static str,
    pub name: &'static str,
    pub label: &'static str,
}
pub(super) const FIELDS: [Field; 10] = [
    Field {
        id: "daily-new",
        name: "dailyNewLimit",
        label: "每日新词上限",
    },
    Field {
        id: "daily-review",
        name: "dailyReviewLimit",
        label: "每日复习上限",
    },
    Field {
        id: "retention",
        name: "requestRetention",
        label: "目标记忆保留率",
    },
    Field {
        id: "min-words",
        name: "articleMinWords",
        label: "文章最少词数",
    },
    Field {
        id: "max-words",
        name: "articleMaxWords",
        label: "文章最多词数",
    },
    Field {
        id: "targets",
        name: "targetCount",
        label: "目标词数量",
    },
    Field {
        id: "control-ratio",
        name: "controlRatio",
        label: "对照词比例",
    },
    Field {
        id: "retries",
        name: "generationMaxRetries",
        label: "覆盖率重写次数",
    },
    Field {
        id: "tts-limit",
        name: "ttsDailyCharLimit",
        label: "每日远程朗读字符额度",
    },
    Field {
        id: "day-start",
        name: "dayStartHour",
        label: "学习日开始小时",
    },
];
pub(super) fn advanced(id: &str) -> bool {
    matches!(id, "targets" | "control-ratio" | "retries")
}
pub(crate) struct Editor {
    pub(super) fields: BTreeMap<&'static str, Entity<InputState>>,
    sliders: BTreeMap<&'static str, Entity<SliderState>>,
    pub(super) endpoint: Entity<InputState>,
    pub(super) model: Entity<InputState>,
    pub(super) voice: Entity<InputState>,
    pub(super) key: Entity<InputState>,
    baseline: Settings,
    provider: Option<ProviderConfig>,
    kind: ProviderKind,
}
fn values(value: &Settings) -> BTreeMap<&'static str, String> {
    [
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
    ]
    .into_iter()
    .collect()
}
impl Editor {
    pub(crate) fn new(
        value: &Settings,
        kind: ProviderKind,
        provider: Option<&ProviderConfig>,
        window: &mut Window,
        cx: &mut Context<Shell>,
    ) -> Self {
        let fields: BTreeMap<_, _> = values(value)
            .into_iter()
            .map(|(key, value)| {
                (
                    key,
                    cx.new(|cx| InputState::new(window, cx).default_value(value)),
                )
            })
            .collect();
        let mut sliders = BTreeMap::new();
        for field in &FIELDS {
            let id = field.id;
            let rule = *numeric_rule(field.name).expect("numeric setting");
            let initial = fields[id]
                .read(cx)
                .value()
                .parse::<f32>()
                .unwrap_or(rule.min as f32);
            let slider = cx.new(|_| {
                SliderState::new()
                    .min(rule.min as f32)
                    .max(rule.max as f32)
                    .step(if rule.integer { 1. } else { 0.01 })
                    .default_value(initial)
            });
            cx.subscribe_in(
                &slider,
                window,
                move |this, emitter, event: &SliderEvent, window, cx| {
                    if this.loading {
                        return;
                    }
                    let Some(editor) = &this.settings_editor else {
                        return;
                    };
                    if editor.sliders[id].entity_id() != emitter.entity_id() {
                        return;
                    }
                    if let SliderEvent::Change(SliderValue::Single(value)) = event {
                        let text = if rule.integer {
                            value.round().to_string()
                        } else {
                            format!("{value:.2}")
                        };
                        editor.fields[id].update(cx, |input, cx| input.set_value(text, window, cx));
                        cx.notify();
                    }
                },
            )
            .detach();
            cx.subscribe_in(
                &fields[id],
                window,
                move |this, emitter, event: &InputEvent, window, cx| {
                    if !matches!(event, InputEvent::Change) {
                        return;
                    }
                    let Some(editor) = &this.settings_editor else {
                        return;
                    };
                    if editor.fields[id].entity_id() != emitter.entity_id() {
                        return;
                    }
                    if let Ok(value) = emitter.read(cx).value().trim().parse::<f64>()
                        && rule.validate(value).is_ok()
                    {
                        editor.sliders[id]
                            .update(cx, |slider, cx| slider.set_value(value as f32, window, cx));
                    }
                    cx.notify();
                },
            )
            .detach();
            sliders.insert(id, slider);
        }
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
        for input in fields.values().chain([&endpoint, &model, &voice, &key]) {
            cx.observe(input, |_, _, cx| cx.notify()).detach();
        }
        Self {
            fields,
            sliders,
            endpoint,
            model,
            voice,
            key,
            baseline: value.clone(),
            provider: provider.cloned(),
            kind,
        }
    }
    pub(crate) fn draft(
        &self,
        base: &Settings,
        cx: &App,
    ) -> (Settings, BTreeMap<&'static str, String>) {
        let mut draft = base.clone();
        let mut errors = BTreeMap::new();
        for field in &FIELDS {
            let rule = numeric_rule(field.name).expect("numeric setting");
            let parsed = self.fields[field.id].read(cx).value().trim().parse::<f64>();
            let value = match parsed {
                Ok(value) => value,
                Err(_) => {
                    errors.insert(field.id, "请输入有效数值".into());
                    continue;
                }
            };
            if let Err(error) = rule.validate(value) {
                errors.insert(field.id, error.message.into());
                continue;
            }
            match field.id {
                "daily-new" => draft.daily_new_limit = value as u32,
                "daily-review" => draft.daily_review_limit = value as u32,
                "retention" => draft.request_retention = value,
                "min-words" => draft.article_min_words = value as u32,
                "max-words" => draft.article_max_words = value as u32,
                "targets" => draft.target_count = value as u32,
                "control-ratio" => draft.control_ratio = value,
                "retries" => draft.generation_max_retries = value as u32,
                "tts-limit" => draft.tts_daily_char_limit = value as u32,
                "day-start" => draft.day_start_hour = value as u32,
                _ => unreachable!(),
            }
        }
        draft.timezone = self.fields["timezone"].read(cx).value().trim().into();
        for error in draft.validation_errors() {
            if error.field == "articleMinWords"
                && (errors.contains_key("min-words") || errors.contains_key("max-words"))
            {
                continue;
            }
            let id = FIELDS
                .iter()
                .find(|field| field.name == error.field)
                .map(|field| field.id)
                .unwrap_or(if error.field == "timezone" {
                    "timezone"
                } else {
                    "device-rate"
                });
            errors.entry(id).or_insert_with(|| error.message.into());
        }
        (draft, errors)
    }
    pub(crate) fn value(&self, base: &Settings, cx: &App) -> Result<Settings, AppError> {
        let (value, errors) = self.draft(base, cx);
        if let Some(error) = errors.values().next() {
            return Err(AppError::new(
                cijian_ports::ErrorCode::InvalidInput,
                error.clone(),
            ));
        }
        Ok(value)
    }
    pub(crate) fn learning_dirty(&self, base: &Settings, cx: &App) -> bool {
        let (mut value, errors) = self.draft(base, cx);
        value.theme = self.baseline.theme;
        value.playback_rate = self.baseline.playback_rate;
        !errors.is_empty() || value != self.baseline
    }
    pub(crate) fn provider_dirty(&self, cx: &App) -> bool {
        self.endpoint.read(cx).value().trim()
            != self
                .provider
                .as_ref()
                .map(|p| p.endpoint.as_str())
                .unwrap_or("")
            || self.model.read(cx).value().trim()
                != self
                    .provider
                    .as_ref()
                    .map(|p| p.model.as_str())
                    .unwrap_or("")
            || (self.kind == ProviderKind::Tts
                && self.voice.read(cx).value().trim()
                    != self
                        .provider
                        .as_ref()
                        .and_then(|p| p.voice.as_deref())
                        .unwrap_or(""))
            || !self.key.read(cx).value().is_empty()
    }
    pub(crate) fn baseline(&self, current: &Settings) -> Settings {
        let mut value = self.baseline.clone();
        value.theme = current.theme;
        value.playback_rate = current.playback_rate;
        value
    }
    pub(crate) fn reset_learning(
        &mut self,
        value: &Settings,
        window: &mut Window,
        cx: &mut Context<Shell>,
    ) {
        self.baseline = value.clone();
        for (key, value) in values(value) {
            self.fields[key].update(cx, |input, cx| input.set_value(value, window, cx));
        }
    }
    pub(crate) fn reset_provider(
        &mut self,
        kind: ProviderKind,
        provider: Option<&ProviderConfig>,
        window: &mut Window,
        cx: &mut Context<Shell>,
    ) {
        self.kind = kind;
        self.provider = provider.cloned();
        for (input, value) in [
            (
                &self.endpoint,
                provider.map(|p| p.endpoint.clone()).unwrap_or_default(),
            ),
            (
                &self.model,
                provider.map(|p| p.model.clone()).unwrap_or_default(),
            ),
            (
                &self.voice,
                provider.and_then(|p| p.voice.clone()).unwrap_or_default(),
            ),
            (&self.key, String::new()),
        ] {
            input.update(cx, |input, cx| input.set_value(value, window, cx));
        }
    }
    pub(crate) fn focus_advanced(&self, window: &mut Window, cx: &mut App) {
        let focus = self.fields["control-ratio"].read(cx).focus_handle(cx);
        window.focus(&focus, cx);
    }
    pub(super) fn field(
        &self,
        field: &Field,
        errors: &BTreeMap<&'static str, String>,
        loading: bool,
    ) -> gpui_kit::AnyElement {
        let rule = numeric_rule(field.name).expect("numeric setting");
        let error = errors.get(field.id);
        let support = error
            .cloned()
            .unwrap_or_else(|| format!("范围 {}–{}；可用滑块或输入数值", rule.min, rule.max));
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                TextField::new(field.id, &self.fields[field.id])
                    .label(field.label)
                    .error(error.is_some())
                    .supporting_text(support)
                    .disabled(loading),
            )
            .child(
                MaterialSlider::new(&self.sliders[field.id], field.label)
                    .id(gpui_kit::SharedString::from(format!("slider-{}", field.id)))
                    .disabled(loading)
                    .value_indicator(true)
                    .format(move |value| {
                        if rule.integer {
                            format!("{value:.0}").into()
                        } else {
                            format!("{:.1}%", value * 100.).into()
                        }
                    }),
            )
            .into_any_element()
    }
}
