use crate::ValidationError;
use serde::{Deserialize, Serialize};
/// Accent of a single-word recording. Word audio is stored per accent, never mixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Accent {
    Uk,
    #[default]
    Us,
}
impl Accent {
    pub const ALL: [Self; 2] = [Self::Uk, Self::Us];
    pub fn key(self) -> &'static str {
        match self {
            Self::Uk => "uk",
            Self::Us => "us",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Uk => "英",
            Self::Us => "美",
        }
    }
    pub fn toggle(self) -> Self {
        match self {
            Self::Uk => Self::Us,
            Self::Us => Self::Uk,
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "uk" => Some(Self::Uk),
            "us" => Some(Self::Us),
            _ => None,
        }
    }
}
/// Phonetic transcription and recorded-accent availability carried by a card.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pronunciation {
    pub ipa_uk: String,
    pub ipa_us: String,
    pub audio_uk: bool,
    pub audio_us: bool,
}
impl Pronunciation {
    /// Validates the transcription strings a boundary may have supplied.
    pub fn checked(
        ipa_uk: String,
        ipa_us: String,
        audio_uk: bool,
        audio_us: bool,
    ) -> Result<Self, ValidationError> {
        let valid = |value: &str| value.chars().count() <= 100;
        if !valid(&ipa_uk) || !valid(&ipa_us) {
            return Err(ValidationError::new("phonetics", "音标过长"));
        }
        Ok(Self {
            ipa_uk,
            ipa_us,
            audio_uk,
            audio_us,
        })
    }
    /// The transcription to show for the active accent, falling back to the other one.
    pub fn ipa(&self, accent: Accent) -> &str {
        let (preferred, other) = match accent {
            Accent::Uk => (&self.ipa_uk, &self.ipa_us),
            Accent::Us => (&self.ipa_us, &self.ipa_uk),
        };
        if preferred.is_empty() {
            other
        } else {
            preferred
        }
    }
    pub fn has_audio(&self, accent: Accent) -> bool {
        match accent {
            Accent::Uk => self.audio_uk,
            Accent::Us => self.audio_us,
        }
    }
    /// Accent the player should use: the requested one when recorded, else the other.
    pub fn playable(&self, accent: Accent) -> Option<Accent> {
        if self.has_audio(accent) {
            Some(accent)
        } else {
            let other = accent.toggle();
            self.has_audio(other).then_some(other)
        }
    }
}
/// Recognises the audio containers this product stores: MP3 frames, an
/// ID3-tagged MP3, or a bare RIFF/WAVE. Shared by the source adapter and packs.
pub fn container(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() < 4 {
        return None;
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE") {
        return Some("wav");
    }
    if bytes.starts_with(b"ID3") || (bytes[0] == 0xFF && (bytes[1] & 0xE0) == 0xE0) {
        return Some("mp3");
    }
    None
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackState {
    Idle,
    Playing,
    Paused,
    Ended,
    Failed,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playback {
    pub session: Option<String>,
    pub article: Option<String>,
    pub state: PlaybackState,
    pub position: f64,
    pub duration: Option<f64>,
    pub rate: f64,
    completion_emitted: bool,
}
impl Default for Playback {
    fn default() -> Self {
        Self {
            session: None,
            article: None,
            state: PlaybackState::Idle,
            position: 0.,
            duration: None,
            rate: 1.,
            completion_emitted: false,
        }
    }
}
impl Playback {
    pub fn start(
        &mut self,
        session: String,
        article: String,
        duration: Option<f64>,
    ) -> Result<(), ValidationError> {
        if session.is_empty()
            || article.is_empty()
            || duration.is_some_and(|d| !d.is_finite() || d <= 0.)
        {
            return Err(ValidationError::new("audio", "播放参数无效"));
        }
        self.session = Some(session);
        self.article = Some(article);
        self.state = PlaybackState::Playing;
        self.position = 0.;
        self.duration = duration;
        self.completion_emitted = false;
        Ok(())
    }
    pub fn stop(&mut self) {
        self.state = PlaybackState::Idle;
        self.position = 0.;
        self.session = None;
        self.article = None;
        self.completion_emitted = false;
    }
    pub fn pause(&mut self) {
        if self.state == PlaybackState::Playing {
            self.state = PlaybackState::Paused;
        }
    }
    pub fn resume(&mut self) {
        if self.state == PlaybackState::Paused {
            self.state = PlaybackState::Playing;
        }
    }
    pub fn set_rate(&mut self, rate: f64) -> Result<(), ValidationError> {
        if !crate::settings::PLAYBACK_RATES.contains(&rate) {
            return Err(ValidationError::new("rate", "不支持的播放速度"));
        }
        self.rate = rate;
        Ok(())
    }
    pub fn seek(&mut self, position: f64) -> Result<(), ValidationError> {
        if !position.is_finite() || position < 0. || self.duration.is_some_and(|d| position > d) {
            return Err(ValidationError::new("position", "音频位置无效"));
        }
        if !matches!(self.state, PlaybackState::Playing | PlaybackState::Paused) {
            return Err(ValidationError::new("audio", "没有可定位的播放会话"));
        }
        self.position = position;
        Ok(())
    }
    /// The device adapter calls this only for a natural end, never for stop or failure.
    pub fn ended(&mut self, session: &str) -> bool {
        if self.session.as_deref() != Some(session)
            || self.state != PlaybackState::Playing
            || self.completion_emitted
        {
            return false;
        }
        self.state = PlaybackState::Ended;
        self.completion_emitted = true;
        if let Some(duration) = self.duration {
            self.position = duration;
        }
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pronunciation_falls_back_to_the_accent_that_has_data() {
        let p = Pronunciation::checked(String::new(), "/kæt/".into(), false, true).unwrap();
        assert_eq!(p.ipa(Accent::Uk), "/kæt/");
        assert_eq!(p.playable(Accent::Uk), Some(Accent::Us));
        assert_eq!(p.playable(Accent::Us), Some(Accent::Us));
        let empty = Pronunciation::default();
        assert!(empty.ipa(Accent::Us).is_empty());
        assert_eq!(empty.playable(Accent::Us), None);
        assert!(Pronunciation::checked("x".repeat(101), String::new(), false, false).is_err());
        assert_eq!(Accent::parse("uk"), Some(Accent::Uk));
        assert_eq!(Accent::parse("us"), Some(Accent::Us));
        assert_eq!(Accent::parse("au"), None);
        assert_eq!(Accent::Us.toggle(), Accent::Uk);
    }
    #[test]
    fn stopping_or_stale_end_never_completes_new_session() {
        let mut p = Playback::default();
        p.start("one".into(), "a".into(), Some(10.)).unwrap();
        p.stop();
        assert!(!p.ended("one"));
        p.start("two".into(), "b".into(), Some(20.)).unwrap();
        assert!(!p.ended("one"));
        assert!(p.ended("two"));
        assert!(!p.ended("two"));
    }
    #[test]
    fn seek_rate_and_pause_boundaries() {
        let mut p = Playback::default();
        assert!(p.seek(0.).is_err());
        p.start("s".into(), "a".into(), Some(10.)).unwrap();
        assert!(p.seek(f64::NAN).is_err());
        assert!(p.seek(11.).is_err());
        p.seek(5.).unwrap();
        p.pause();
        assert!(!p.ended("s"));
        p.resume();
        assert!(p.set_rate(1.25).is_ok());
        assert!(p.set_rate(3.).is_err());
        assert!(p.ended("s"));
    }
}
