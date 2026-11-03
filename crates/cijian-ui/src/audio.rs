use super::*;
use cijian_domain::audio::{Playback, PlaybackState};
impl Shell {
    pub(super) fn receive_audio(&mut self, state: Playback, cx: &mut Context<Self>) {
        let active = matches!(state.state, PlaybackState::Playing | PlaybackState::Paused);
        self.playback = state;
        if active {
            self.poll_audio(cx);
        } else if self.playback.state == PlaybackState::Ended {
            self.notice = Some("已听完，可以填写听力难度反馈".into());
        }
    }
    pub(super) fn poll_audio(&mut self, cx: &mut Context<Self>) {
        self.audio_poll_epoch += 1;
        let epoch = self.audio_poll_epoch;
        let session = self.playback.session.clone();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(750))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.audio_poll_epoch == epoch
                    && matches!(this.navigation.current(), Route::Article { .. })
                    && this.playback.session == session
                    && matches!(
                        this.playback.state,
                        PlaybackState::Playing | PlaybackState::Paused
                    )
                {
                    if this.loading {
                        this.poll_audio(cx);
                    } else {
                        this.request(Command::AudioStatus, cx);
                    }
                }
            });
        })
        .detach();
    }
}
