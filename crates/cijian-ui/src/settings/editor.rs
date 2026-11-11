#[cfg(test)]
mod tests;
use super::*;
use crate::design;
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
pub(super) const FIELDS: [Field; 9] = [
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
        label: "记忆评估保留率",
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
        id: "day-start",
        name: "dayStartHour",
        label: "学习日开始小时",
    },
];
pub(super) fn advanced(id: &str) -> bool {
    matches!(id, "targets" | "control-ratio" | "retries")
}
/// One provider's editable draft. LLM and TTS are configured independently and
/// stay editable at the same time, so each keeps its own inputs.
pub(super) struct ProviderDraft {
    pub(super) endpoint: Entity<InputState>,
    pub(super) model: Entity<InputState>,
    pub(super) voice: Entity<InputState>,
    pub(super) key: Entity<InputState>,
    baseline: Option<ProviderConfig>,
}
pub(super) fn provider_index(kind: ProviderKind) -> usize {
    if kind == ProviderKind::Llm { 0 } else { 1 }
}
impl ProviderDraft {
    fn new(
        provider: Option<&ProviderConfig>,
        window: &mut Window,
        cx: &mut Context<Shell>,
    ) -> Self {
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
        for input in [&endpoint, &model, &voice, &key] {
            cx.observe(input, |_, _, cx| cx.notify()).detach();
        }
        Self {
            endpoint,
            model,
            voice,
            key,
            baseline: provider.cloned(),
        }
    }
    fn reset(
        &mut self,
        provider: Option<&ProviderConfig>,
        window: &mut Window,
        cx: &mut Context<Shell>,
    ) {
        self.baseline = provider.cloned();
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
    pub(super) fn dirty(&self, kind: ProviderKind, cx: &App) -> bool {
        let baseline = self.baseline.as_ref();
        self.endpoint.read(cx).value().trim() != baseline.map_or("", |p| p.endpoint.as_str())
            || self.model.read(cx).value().trim() != baseline.map_or("", |p| p.model.as_str())
            || (kind == ProviderKind::Tts
                && self.voice.read(cx).value().trim()
                    != baseline.and_then(|p| p.voice.as_deref()).unwrap_or(""))
            || !self.key.read(cx).value().is_empty()
    }
    pub(super) fn config(&self, kind: ProviderKind, revision: u64, cx: &App) -> ProviderConfig {
        ProviderConfig {
            kind,
            endpoint: self.endpoint.read(cx).value().trim().into(),
            model: self.model.read(cx).value().trim().into(),
            voice: (kind == ProviderKind::Tts).then(|| self.voice.read(cx).value().trim().into()),
            revision,
        }
    }
    pub(super) fn secret(&self, clear: bool, cx: &App) -> Result<SecretPatch, AppError> {
        if clear {
            return Ok(SecretPatch::Clear);
        }
        let text = self.key.read(cx).value().to_string();
        if text.is_empty() {
            return Ok(SecretPatch::Keep);
        }
        Secret::new(text)
            .map(SecretPatch::Replace)
            .map_err(|error| AppError::new(cijian_ports::ErrorCode::InvalidInput, error.message))
    }
}
pub(crate) struct Editor {
    pub(super) fields: BTreeMap<&'static str, Entity<InputState>>,
    sliders: BTreeMap<&'static str, Entity<SliderState>>,
    providers: [ProviderDraft; 2],
    baseline: Settings,
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
        ("timezone", value.timezone.clone()),
        ("day-start", value.day_start_hour.to_string()),
    ]
    .into_iter()
    .collect()
}
impl Editor {
    pub(crate) fn new(
        value: &Settings,
        profile_providers: [Option<&ProviderConfig>; 2],
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
        let providers = [
            ProviderDraft::new(profile_providers[0], window, cx),
            ProviderDraft::new(profile_providers[1], window, cx),
        ];
        for input in fields.values() {
            cx.observe(input, |_, _, cx| cx.notify()).detach();
        }
        Self {
            fields,
            sliders,
            providers,
            baseline: value.clone(),
        }
    }
    pub(super) fn provider(&self, kind: ProviderKind) -> &ProviderDraft {
        &self.providers[provider_index(kind)]
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
    pub(crate) fn provider_dirty(&self, kind: ProviderKind, cx: &App) -> bool {
        self.provider(kind).dirty(kind, cx)
    }
    pub(crate) fn any_provider_dirty(&self, cx: &App) -> bool {
        self.provider_dirty(ProviderKind::Llm, cx)
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
        self.providers[provider_index(kind)].reset(provider, window, cx);
    }
    pub(crate) fn focus_advanced(&self, window: &mut Window, cx: &mut App) {
        let focus = self.fields["control-ratio"].read(cx).focus_handle(cx);
        window.focus(&focus, cx);
    }
    fn format_value(rule: &cijian_domain::settings::NumericRule, value: f64) -> String {
        if rule.integer {
            format!("{value:.0}")
        } else {
            format!("{value:.2}")
        }
    }

    fn slider_visual(
        &self,
        rule: &cijian_domain::settings::NumericRule,
        field_id: &'static str,
        cx: &App,
    ) -> gpui_kit::AnyElement {
        let value = self.fields[field_id]
            .read(cx)
            .value()
            .trim()
            .parse::<f64>()
            .unwrap_or(rule.min);
        let fraction = if rule.max > rule.min {
            ((value - rule.min) / (rule.max - rule.min)).clamp(0., 1.) as f32
        } else {
            0.
        };
        div()
            .relative()
            .h(px(44.))
            .flex_1()
            .min_w(px(80.))
            .child(
                div()
                    .absolute()
                    .top(px(14.))
                    .left(px(0.))
                    .right(px(0.))
                    .h(px(16.))
                    .rounded(px(8.))
                    .bg(design::c(ColorRole::SECONDARY_CONTAINER)),
            )
            .child(
                div()
                    .absolute()
                    .top(px(14.))
                    .left(px(0.))
                    .w(gpui_kit::relative(fraction))
                    .h(px(16.))
                    .rounded(px(8.))
                    .bg(design::c(ColorRole::PRIMARY)),
            )
            .child(
                div()
                    .absolute()
                    .top(px(0.))
                    .left(gpui_kit::relative(fraction))
                    .ml(px(-2.))
                    .w(px(4.))
                    .h(px(44.))
                    .rounded(px(2.))
                    .bg(design::c(ColorRole::PRIMARY)),
            )
            .child(
                div()
                    .absolute()
                    .top(px(21.))
                    .right(px(4.))
                    .w(px(4.))
                    .h(px(4.))
                    .rounded(px(2.))
                    .bg(design::c(ColorRole::ON_SECONDARY_CONTAINER)),
            )
            .into_any_element()
    }

    fn hidden_input(&self, field: &Field, loading: bool) -> gpui_kit::AnyElement {
        div()
            .absolute()
            .right(px(0.))
            .top(px(14.))
            .w(px(200.))
            .h(px(44.))
            .overflow_hidden()
            .opacity(0.)
            .child(
                TextField::new(field.id, &self.fields[field.id])
                    .label(field.label)
                    .disabled(loading),
            )
            .into_any_element()
    }

    pub(super) fn design_range_field(&self, loading: bool, cx: &App) -> gpui_kit::AnyElement {
        let min_rule = numeric_rule("articleMinWords").expect("numeric setting");
        let max_rule = numeric_rule("articleMaxWords").expect("numeric setting");
        let min = self.fields["min-words"]
            .read(cx)
            .value()
            .trim()
            .parse::<f64>()
            .unwrap_or(min_rule.min);
        let max = self.fields["max-words"]
            .read(cx)
            .value()
            .trim()
            .parse::<f64>()
            .unwrap_or(max_rule.max);
        let min_fraction = ((min - 100.) / (1000. - 100.)).clamp(0., 1.) as f32;
        let max_fraction = ((max - 100.) / (1000. - 100.)).clamp(0., 1.) as f32;
        let row = div()
            .relative()
            .flex()
            .items_center()
            .gap(px(16.))
            .min_h(px(72.))
            .py(px(8.))
            .child(
                div()
                    .w(px(150.))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .child(design::text(15., 20., 650.).child("文章长度"))
                    .child(design::muted_text(12., 16.).child("最少 / 最多词数")),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_w(px(80.))
                    .h(px(44.))
                    .child(
                        div()
                            .absolute()
                            .top(px(14.))
                            .left(px(0.))
                            .right(px(0.))
                            .h(px(16.))
                            .rounded(px(8.))
                            .bg(design::c(ColorRole::SECONDARY_CONTAINER)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(14.))
                            .left(gpui_kit::relative(min_fraction))
                            .w(gpui_kit::relative((max_fraction - min_fraction).max(0.01)))
                            .h(px(16.))
                            .bg(design::c(ColorRole::PRIMARY)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(0.))
                            .left(gpui_kit::relative(min_fraction))
                            .ml(px(-2.))
                            .w(px(4.))
                            .h(px(44.))
                            .rounded(px(2.))
                            .bg(design::c(ColorRole::PRIMARY)),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(0.))
                            .left(gpui_kit::relative(max_fraction))
                            .ml(px(-2.))
                            .w(px(4.))
                            .h(px(44.))
                            .rounded(px(2.))
                            .bg(design::c(ColorRole::PRIMARY)),
                    )
                    .child(
                        div().absolute().inset_0().opacity(0.).child(
                            MaterialSlider::new(&self.sliders["min-words"], "文章最少词数")
                                .id(gpui_kit::SharedString::from("slider-min-words"))
                                .disabled(loading),
                        ),
                    )
                    .child(
                        div().absolute().inset_0().opacity(0.).child(
                            MaterialSlider::new(&self.sliders["max-words"], "文章最多词数")
                                .id(gpui_kit::SharedString::from("slider-max-words"))
                                .disabled(loading),
                        ),
                    ),
            )
            .child(
                design::mono(13., 18., 400.)
                    .w(px(76.))
                    .flex_shrink_0()
                    .text_align(gpui_kit::TextAlign::Right)
                    .child(format!("{min:.0}–{max:.0}")),
            )
            .child(
                div()
                    .absolute()
                    .right(px(0.))
                    .top(px(0.))
                    .w(px(200.))
                    .h(px(44.))
                    .overflow_hidden()
                    .opacity(0.)
                    .child(
                        TextField::new("min-words", &self.fields["min-words"])
                            .label("文章最少词数")
                            .disabled(loading),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .right(px(0.))
                    .top(px(28.))
                    .w(px(200.))
                    .h(px(44.))
                    .overflow_hidden()
                    .opacity(0.)
                    .child(
                        TextField::new("max-words", &self.fields["max-words"])
                            .label("文章最多词数")
                            .disabled(loading),
                    ),
            );
        let _ = min_rule;
        let _ = max_rule;
        row.into_any_element()
    }

    pub(super) fn design_field(
        &self,
        field: &Field,
        errors: &BTreeMap<&'static str, String>,
        loading: bool,
        cx: &App,
    ) -> gpui_kit::AnyElement {
        let rule = *numeric_rule(field.name).expect("numeric setting");
        let value = self.fields[field.id]
            .read(cx)
            .value()
            .trim()
            .parse::<f64>()
            .unwrap_or(rule.min);
        let mut row = div()
            .relative()
            .flex()
            .items_center()
            .gap(px(16.))
            .min_h(px(72.))
            .py(px(8.))
            .child(
                div()
                    .w(px(150.))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .child(design::text(15., 20., 650.).child(field.label))
                    .child(design::muted_text(12., 16.).child(format!(
                        "{}–{}",
                        if rule.integer {
                            format!("{:.0}", rule.min)
                        } else {
                            format!("{:.2}", rule.min)
                        },
                        if rule.integer {
                            format!("{:.0}", rule.max)
                        } else {
                            format!("{:.2}", rule.max)
                        }
                    ))),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_w(px(80.))
                    .h(px(44.))
                    .child(self.slider_visual(&rule, field.id, cx))
                    .child(
                        div().absolute().inset_0().opacity(0.).child(
                            MaterialSlider::new(&self.sliders[field.id], field.label)
                                .id(gpui_kit::SharedString::from(format!("slider-{}", field.id)))
                                .disabled(loading),
                        ),
                    ),
            )
            .child(
                design::mono(13., 18., 400.)
                    .w(px(64.))
                    .flex_shrink_0()
                    .text_align(gpui_kit::TextAlign::Right)
                    .child(Self::format_value(&rule, value)),
            )
            .child(self.hidden_input(field, loading));
        if let Some(error) = errors.get(field.id) {
            row = row.child(
                design::text(11., 14., 400.)
                    .text_color(design::c(ColorRole::ERROR))
                    .child(error.clone()),
            );
        }
        row.into_any_element()
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
