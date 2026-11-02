use crate::ValidationError;
use serde::{Deserialize, Serialize};
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
        if ![0.75, 1., 1.25, 1.5, 2.].contains(&rate) {
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
