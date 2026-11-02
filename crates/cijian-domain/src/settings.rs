use crate::ValidationError;
use serde::{Deserialize, Serialize};

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
    pub playback_rate: f64,
    pub theme: Theme,
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
impl TryFrom<Settings> for ValidatedSettings {
    type Error = ValidationError;
    fn try_from(s: Settings) -> Result<Self, Self::Error> {
        for (field, value, min, max) in [
            ("dailyNewLimit", s.daily_new_limit, 0, 200),
            ("dailyReviewLimit", s.daily_review_limit, 0, 2000),
            ("articleMinWords", s.article_min_words, 100, 800),
            ("articleMaxWords", s.article_max_words, 100, 1000),
            ("targetCount", s.target_count, 1, 12),
            ("generationMaxRetries", s.generation_max_retries, 0, 3),
            ("ttsDailyCharLimit", s.tts_daily_char_limit, 0, 200000),
            ("dayStartHour", s.day_start_hour, 0, 23),
        ] {
            if !(min..=max).contains(&value) {
                return Err(ValidationError::new(field, "数值超出范围"));
            }
        }
        for (field, value, min, max) in [
            ("requestRetention", s.request_retention, 0.70, 0.99),
            ("controlRatio", s.control_ratio, 0.0, 0.5),
        ] {
            if !value.is_finite() || !(min..=max).contains(&value) {
                return Err(ValidationError::new(field, "数值超出范围"));
            }
        }
        if s.article_min_words > s.article_max_words {
            return Err(ValidationError::new(
                "articleMinWords",
                "最小字数不能超过最大字数",
            ));
        }
        if ![0.75, 0.9, 1.0, 1.1, 1.25].contains(&s.playback_rate) {
            return Err(ValidationError::new("playbackRate", "不支持的播放速度"));
        }
        if s.timezone.parse::<chrono_tz::Tz>().is_err() {
            return Err(ValidationError::new("timezone", "无效IANA时区"));
        }
        Ok(Self(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
                playback_rate: 1.5,
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
