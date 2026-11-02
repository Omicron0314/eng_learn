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
    pub(super) fn audio_body(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let listening = matches!(
            self.navigation.current(),
            Route::Article {
                listening: true,
                ..
            }
        );
        let mut body = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                material_button("reading-listening-mode", ButtonVariant::Tonal)
                    .child(if listening {
                        "切到阅读"
                    } else {
                        "先听后读"
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Route::Article { id, listening } = this.navigation.current().clone()
                        {
                            this.navigation.navigate(Route::Article {
                                id,
                                listening: !listening,
                            });
                            this.show_listening_text = false;
                            cx.notify();
                        }
                    })),
            )
            .child(
                material_button("remote-audio", ButtonVariant::Filled)
                    .child("远程朗读 / 播放缓存")
                    .disabled(self.loading)
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(article) = &this.article {
                            this.request(
                                Command::EnsureAudio {
                                    article: article.id.clone(),
                                    operation: format!(
                                        "audio-{}-{}",
                                        this.request_id,
                                        this.review_started.elapsed().as_nanos()
                                    ),
                                },
                                cx,
                            );
                        }
                    })),
            )
            .child("已有缓存直接播放；首次合成会使用已配置的朗读服务及字符额度。")
            .child(if self.system_voices.is_empty() {
                "没有可用的本机英语朗读引擎"
            } else {
                "本机英语朗读不联网、不占远程字符额度"
            });
        if let Some(voice) = self.system_voices.get(self.system_voice_index) {
            body = body
                .child(
                    material_button("system-voice", ButtonVariant::Text)
                        .child(format!("本机语音：{}", voice.label))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.system_voice_index =
                                (this.system_voice_index + 1) % this.system_voices.len().max(1);
                            if let Some(voice) = this.system_voices.get(this.system_voice_index) {
                                this.request(
                                    Command::SetSystemVoice {
                                        voice: voice.id.clone(),
                                    },
                                    cx,
                                );
                            }
                            cx.notify();
                        })),
                )
                .child(
                    material_button("system-audio", ButtonVariant::Tonal)
                        .child("本机朗读")
                        .disabled(self.loading)
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let (Some(article), Some(voice)) = (
                                &this.article,
                                this.system_voices.get(this.system_voice_index),
                            ) {
                                this.request(
                                    Command::EnsureSystemAudio {
                                        article: article.id.clone(),
                                        voice: voice.id.clone(),
                                    },
                                    cx,
                                );
                            }
                        })),
                );
        }
        if let Some(article) = &self.article {
            for (index, asset) in self.stored_audio.iter().enumerate() {
                let article = article.id.clone();
                let audio = asset.id.clone();
                body = body.child(
                    material_button(
                        gpui_kit::SharedString::from(format!("stored-audio-{index}")),
                        ButtonVariant::Tonal,
                    )
                    .child(format!(
                        "播放{}音频 · {}",
                        if asset.provider_fingerprint.starts_with("legacy:") {
                            "旧版"
                        } else {
                            "已有"
                        },
                        asset.voice
                    ))
                    .disabled(self.loading)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.request(
                            Command::PlayStoredAudio {
                                article: article.clone(),
                                audio: audio.clone(),
                            },
                            cx,
                        )
                    })),
                );
            }
        }
        if listening {
            body = body.child(
                material_button("show-listening-text", ButtonVariant::Text)
                    .child(if self.show_listening_text {
                        "收起原文"
                    } else {
                        "显示原文"
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_listening_text = !this.show_listening_text;
                        cx.notify();
                    })),
            );
        }
        if self.playback.session.is_some() {
            body = body.child(format!(
                "播放位置 {:.0} 秒{}",
                self.playback.position,
                self.playback
                    .duration
                    .map(|d| format!(" / {:.0} 秒", d))
                    .unwrap_or_default()
            ));
            let mut controls = div().flex().flex_wrap().gap_2();
            controls = controls
                .child(
                    material_button("audio-pause-resume", ButtonVariant::Tonal)
                        .child(if self.playback.state == PlaybackState::Paused {
                            "继续"
                        } else {
                            "暂停"
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.request(
                                if this.playback.state == PlaybackState::Paused {
                                    Command::AudioResume
                                } else {
                                    Command::AudioPause
                                },
                                cx,
                            );
                        })),
                )
                .child(
                    material_button("audio-back-five", ButtonVariant::Tonal)
                        .child("后退 5 秒")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.request(
                                Command::AudioSeek {
                                    position: (this.playback.position - 5.).max(0.),
                                },
                                cx,
                            )
                        })),
                )
                .child(
                    material_button("audio-speed", ButtonVariant::Tonal)
                        .child(format!("{}×", self.playback.rate))
                        .on_click(cx.listener(|this, _, _, cx| {
                            let rates = cijian_domain::settings::PLAYBACK_RATES;
                            let index = rates
                                .iter()
                                .position(|r| *r == this.playback.rate)
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
                    material_button("audio-stop", ButtonVariant::Text)
                        .child("停止")
                        .on_click(
                            cx.listener(|this, _, _, cx| this.request(Command::AudioStop, cx)),
                        ),
                );
            body = body.child(controls);
        }
        if self.loading {
            body = body.child(
                material_button("cancel-audio", ButtonVariant::Text)
                    .child("取消朗读请求")
                    .on_click(cx.listener(|this, _, _, cx| this.request(Command::AudioStop, cx))),
            );
        }
        body.into_any_element()
    }
}
