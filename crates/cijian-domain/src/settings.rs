use crate::ValidationError;
use serde::{Deserialize, Serialize};

pub const PLAYBACK_RATES: [f64; 7] = [0.75, 0.9, 1., 1.1, 1.25, 1.5, 2.];
#[derive(Debug, Clone, Copy)]
pub struct NumericRule {
    pub field: &'static str,
    pub min: f64,
    pub max: f64,
    pub integer: bool,
}
impl NumericRule {
    pub fn validate(self, value: f64) -> Result<(), ValidationError> {
        if !value.is_finite() || !(self.min..=self.max).contains(&value) {
            return Err(ValidationError::new(self.field, "数值超出范围"));
        }
        if self.integer && value.fract() != 0. {
            return Err(ValidationError::new(self.field, "请输入整数"));
        }
        Ok(())
    }
}
pub const NUMERIC_RULES: [NumericRule; 10] = [
    NumericRule {
        field: "dailyNewLimit",
        min: 0.,
        max: 200.,
        integer: true,
    },
    NumericRule {
        field: "dailyReviewLimit",
        min: 0.,
        max: 2000.,
        integer: true,
    },
    NumericRule {
        field: "articleMinWords",
        min: 100.,
        max: 800.,
        integer: true,
    },
    NumericRule {
        field: "articleMaxWords",
        min: 100.,
        max: 1000.,
        integer: true,
    },
    NumericRule {
        field: "targetCount",
        min: 1.,
        max: 12.,
        integer: true,
    },
    NumericRule {
        field: "generationMaxRetries",
        min: 0.,
        max: 3.,
        integer: true,
    },
    NumericRule {
        field: "ttsDailyCharLimit",
        min: 0.,
        max: 200000.,
        integer: true,
    },
    NumericRule {
        field: "dayStartHour",
        min: 0.,
        max: 23.,
        integer: true,
    },
    NumericRule {
        field: "requestRetention",
        min: 0.70,
        max: 0.99,
        integer: false,
    },
    NumericRule {
        field: "controlRatio",
        min: 0.,
        max: 0.5,
        integer: false,
    },
];
pub fn numeric_rule(field: &str) -> Option<&'static NumericRule> {
    NUMERIC_RULES.iter().find(|rule| rule.field == field)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Level {
    A2,
    B1,
    B2,
    C1,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Tier {
    Easy,
    Medium,
    Hard,
}
impl Tier {
    pub fn minimum_coverage(self) -> f64 {
        match self {
            Self::Easy => 0.98,
            Self::Medium => 0.95,
            Self::Hard => 0.90,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Genre {
    Auto,
    Story,
    Dialogue,
    Monologue,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LookupMode {
    MarkOnly,
    AutoAdd,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub level: Level,
    pub daily_new_limit: u32,
    pub daily_review_limit: u32,
    pub request_retention: f64,
    pub tier: Tier,
    pub article_min_words: u32,
    pub article_max_words: u32,
    pub genre: Genre,
    pub target_count: u32,
    pub control_ratio: f64,
    pub generation_max_retries: u32,
    pub lookup_mode: LookupMode,
    pub tts_daily_char_limit: u32,
    pub timezone: String,
    pub day_start_hour: u32,
    #[serde(default = "default_playback_rate")]
    pub playback_rate: f64,
    #[serde(default = "default_theme")]
    pub theme: Theme,
}
fn default_playback_rate() -> f64 {
    1.
}
fn default_theme() -> Theme {
    Theme::System
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DevicePreferences {
    pub theme: Theme,
    pub playback_rate: f64,
    pub system_voice: Option<String>,
}
impl Default for DevicePreferences {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            playback_rate: 1.,
            system_voice: None,
        }
    }
}
impl DevicePreferences {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if !PLAYBACK_RATES.contains(&self.playback_rate) {
            return Err(ValidationError::new("playbackRate", "不支持的播放速度"));
        }
        if self.system_voice.as_ref().is_some_and(|voice| {
            voice.is_empty() || voice.len() > 256 || voice.chars().any(char::is_control)
        }) {
            return Err(ValidationError::new("systemVoice", "本机语音标识无效"));
        }
        Ok(())
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            level: Level::B1,
            daily_new_limit: 20,
            daily_review_limit: 200,
            request_retention: 0.90,
            tier: Tier::Medium,
            article_min_words: 220,
            article_max_words: 420,
            genre: Genre::Auto,
            target_count: 8,
            control_ratio: 0.20,
            generation_max_retries: 2,
            lookup_mode: LookupMode::MarkOnly,
            tts_daily_char_limit: 20000,
            timezone: "Asia/Shanghai".into(),
            day_start_hour: 4,
            playback_rate: 1.0,
            theme: Theme::System,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValidatedSettings(Settings);
impl ValidatedSettings {
    pub fn get(&self) -> &Settings {
        &self.0
    }
}
impl Settings {
    pub fn validation_errors(&self) -> Vec<ValidationError> {
        let mut errors = Vec::new();
        for (field, value) in [
            ("dailyNewLimit", self.daily_new_limit as f64),
            ("dailyReviewLimit", self.daily_review_limit as f64),
            ("articleMinWords", self.article_min_words as f64),
            ("articleMaxWords", self.article_max_words as f64),
            ("targetCount", self.target_count as f64),
            ("generationMaxRetries", self.generation_max_retries as f64),
            ("ttsDailyCharLimit", self.tts_daily_char_limit as f64),
            ("dayStartHour", self.day_start_hour as f64),
            ("requestRetention", self.request_retention),
            ("controlRatio", self.control_ratio),
        ] {
            if let Err(error) = numeric_rule(field)
                .expect("known numeric setting")
                .validate(value)
            {
                errors.push(error);
            }
        }
        if self.article_min_words > self.article_max_words {
            errors.push(ValidationError::new(
                "articleMinWords",
                "最少词数不能超过最多词数",
            ));
        }
        if !PLAYBACK_RATES.contains(&self.playback_rate) {
            errors.push(ValidationError::new("playbackRate", "不支持的播放速度"));
        }
        if self.timezone.parse::<chrono_tz::Tz>().is_err() {
            errors.push(ValidationError::new("timezone", "无效IANA时区"));
        }
        errors
    }
}
impl TryFrom<Settings> for ValidatedSettings {
    type Error = ValidationError;
    fn try_from(s: Settings) -> Result<Self, Self::Error> {
        if let Some(error) = s.validation_errors().into_iter().next() {
            return Err(error);
        }
        Ok(Self(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_fields_report_errors_and_device_rates_remain_compatible() {
        let draft = Settings {
            daily_new_limit: 201,
            daily_review_limit: 2001,
            request_retention: f64::NAN,
            control_ratio: 0.8,
            article_min_words: 500,
            article_max_words: 400,
            timezone: "invalid/timezone".into(),
            ..Settings::default()
        };
        let fields = draft
            .validation_errors()
            .into_iter()
            .map(|error| error.field)
            .collect::<Vec<_>>();
        for field in [
            "dailyNewLimit",
            "dailyReviewLimit",
            "requestRetention",
            "controlRatio",
            "articleMinWords",
            "timezone",
        ] {
            assert!(fields.contains(&field), "{field}");
        }
        assert!(
            numeric_rule("dailyNewLimit")
                .unwrap()
                .validate(1.5)
                .is_err()
        );
        for rate in PLAYBACK_RATES {
            assert!(
                ValidatedSettings::try_from(Settings {
                    playback_rate: rate,
                    ..Settings::default()
                })
                .is_ok()
            );
            assert!(
                DevicePreferences {
                    playback_rate: rate,
                    ..Default::default()
                }
                .validate()
                .is_ok()
            );
            assert!(crate::audio::Playback::default().set_rate(rate).is_ok());
        }
    }
    #[test]
    fn defaults_and_saved_values_are_distinct() {
        assert!(ValidatedSettings::try_from(Settings::default()).is_ok());
        let saved = Settings {
            article_min_words: 100,
            article_max_words: 150,
            ..Settings::default()
        };
        assert_eq!(
            ValidatedSettings::try_from(saved)
                .unwrap()
                .get()
                .article_max_words,
            150
        );
    }
    #[test]
    fn rejects_non_finite_and_cross_field_values() {
        for value in [f64::NAN, f64::INFINITY, 0.69, 1.0] {
            assert!(
                ValidatedSettings::try_from(Settings {
                    request_retention: value,
                    ..Settings::default()
                })
                .is_err()
            );
        }
        assert!(
            ValidatedSettings::try_from(Settings {
                article_min_words: 500,
                article_max_words: 400,
                ..Settings::default()
            })
            .is_err()
        );
        assert!(
            ValidatedSettings::try_from(Settings {
                timezone: "Mars/City".into(),
                ..Settings::default()
            })
            .is_err()
        );
        assert!(
            ValidatedSettings::try_from(Settings {
                playback_rate: 1.3,
                ..Settings::default()
            })
            .is_err()
        );
    }
    #[test]
    fn rejects_each_integer_upper_bound() {
        let variants = [
            Settings {
                daily_new_limit: 201,
                ..Settings::default()
            },
            Settings {
                daily_review_limit: 2001,
                ..Settings::default()
            },
            Settings {
                article_min_words: 801,
                ..Settings::default()
            },
            Settings {
                article_max_words: 1001,
                ..Settings::default()
            },
            Settings {
                target_count: 13,
                ..Settings::default()
            },
            Settings {
                generation_max_retries: 4,
                ..Settings::default()
            },
            Settings {
                tts_daily_char_limit: 200001,
                ..Settings::default()
            },
            Settings {
                day_start_hour: 24,
                ..Settings::default()
            },
        ];
        for value in variants {
            assert!(ValidatedSettings::try_from(value).is_err());
        }
        assert!(
            ValidatedSettings::try_from(Settings {
                daily_new_limit: 0,
                daily_review_limit: 0,
                tts_daily_char_limit: 0,
                ..Settings::default()
            })
            .is_ok()
        );
    }
}
