use crate::{
    articles::Recorder,
    backend::{DesktopBackend, id, now},
};
use cijian_app::commands::Response;
use cijian_domain::provider::ProviderKind;
use cijian_platform::audio::AudioCommand;
use cijian_ports::{AppError, ErrorCode, Result, audio::*, provider::*};
use cijian_providers::{NativeHttp, OpenAiCompatible};
use sha2::{Digest, Sha256};
use std::io::Write;
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
impl DesktopBackend {
    pub(super) fn ensure_system_audio(&self, article_id: &str, voice: &str) -> Result<Response> {
        let ticket = self
            .audio_epoch
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1;
        let _guard = self
            .audio_lock
            .lock()
            .map_err(|_| AppError::new(ErrorCode::StorageUnavailable, "音频任务暂不可用"))?;
        let article = self.store.article(&self.profile, article_id)?;
        let path = tempfile::Builder::new()
            .suffix(".wav")
            .tempfile()
            .map_err(|_| AppError::new(ErrorCode::StorageUnavailable, "无法创建本机朗读文件"))?
            .into_temp_path();
        cijian_platform::speech::synthesize(&article.body, voice, &path)?;
        let _gate = self
            .playback_gate
            .lock()
            .map_err(|_| AppError::new(ErrorCode::StorageUnavailable, "播放器暂不可用"))?;
        if self.audio_epoch.load(std::sync::atomic::Ordering::SeqCst) != ticket {
            return Err(AppError::new(ErrorCode::Cancelled, "本机朗读已取消"));
        }
        let session = id();
        let (settings, _) = self.store.device_preferences(&self.profile, &self.device)?;
        self.store.open_audio_session(AudioSessionCommand {
            profile: self.profile.clone(),
            operation: format!("play-{session}"),
            event: id(),
            session: session.clone(),
            article: article.id.clone(),
            audio: None,
            source: "system".into(),
            rate: settings.playback_rate,
            now: now(),
        })?;
        let result = self.audio_command(AudioCommand::Play {
            path: path.to_path_buf(),
            session,
            article: article.id,
            rate: settings.playback_rate,
        })?;
        *self
            .system_audio
            .lock()
            .map_err(|_| AppError::new(ErrorCode::StorageUnavailable, "本机朗读暂不可用"))? =
            Some(path);
        Ok(result)
    }
    pub(super) fn audio_command(&self, command: AudioCommand) -> Result<Response> {
        let _gate = if matches!(command, AudioCommand::Stop) {
            Some(
                self.playback_gate
                    .lock()
                    .map_err(|_| AppError::new(ErrorCode::StorageUnavailable, "播放器暂不可用"))?,
            )
        } else {
            None
        };
        if matches!(command, AudioCommand::Stop) {
            self.audio_epoch
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        let state = self.audio.command(command)?;
        Ok(Response::Audio(state))
    }
    pub(super) fn ensure_audio(&self, article_id: &str, operation: &str) -> Result<Response> {
        let ticket = self
            .audio_epoch
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            + 1;
        let _audio_guard = self
            .audio_lock
            .lock()
            .map_err(|_| AppError::new(ErrorCode::StorageUnavailable, "音频任务暂不可用"))?;
        let article = self.store.article(&self.profile, article_id)?;
        let snapshot = {
            let _guard = self
                .config_lock
                .lock()
                .map_err(|_| AppError::new(ErrorCode::StorageUnavailable, "配置暂不可用"))?;
            let config = self
                .store
                .provider_config(&self.profile, ProviderKind::Tts)?
                .ok_or(AppError::new(
                    ErrorCode::ConfigMissing,
                    "请先配置远程朗读服务",
                ))?;
            let secret =
                futures::executor::block_on(self.secrets.read(&format!("{}/tts", self.profile)))?;
            ProviderSnapshot { config, secret }
        };
        let voice = snapshot
            .config
            .voice
            .clone()
            .ok_or(AppError::new(ErrorCode::ConfigMissing, "请配置语音名称"))?;
        let fingerprint = digest(
            format!(
                "{}\n{}\n{}\nwav",
                snapshot.config.endpoint, snapshot.config.model, voice
            )
            .as_bytes(),
        );
        let cache_key =
            digest(format!("{}\n{}\n{}", self.profile, article.body_hash, fingerprint).as_bytes());
        let cache = self.directory.join("audio");
        std::fs::create_dir_all(&cache)
            .map_err(|_| AppError::new(ErrorCode::StorageUnavailable, "无法创建音频缓存"))?;
        let existing = self.store.audio_asset(&self.profile, &cache_key)?;
        let cached = existing.filter(|asset| {
            let Some(stem) = asset.blob_key.strip_suffix(".wav") else {
                return false;
            };
            if stem.len() != 64 || !stem.bytes().all(|b| b.is_ascii_hexdigit()) {
                return false;
            }
            std::fs::read(cache.join(&asset.blob_key)).is_ok_and(|bytes| digest(&bytes) == stem)
        });
        let asset = if let Some(asset) = cached {
            self.store
                .link_cached_audio(&self.profile, &article.id, &asset.id)?;
            asset
        } else {
            let reservation = self.store.reserve_audio(ReserveAudio {
                profile: self.profile.clone(),
                operation: operation.into(),
                event: id(),
                reservation: id(),
                article: article.id.clone(),
                body_hash: article.body_hash.clone(),
                now: now(),
            })?;
            if !self.store.claim_audio(&self.profile, &reservation.id)? {
                return Err(AppError::new(
                    ErrorCode::StaleState,
                    "该音频请求已处理，未自动再次调用服务",
                ));
            }
            let outcome = (|| {
                let provider = OpenAiCompatible {
                    transport: NativeHttp::new()?,
                    recorder: Recorder(self.store.clone()),
                };
                let bytes = self.runtime.block_on(provider.speech(
                    &snapshot,
                    &article.body,
                    CallStart {
                        profile_id: self.profile.clone(),
                        call_id: id(),
                        purpose: "tts".into(),
                        model: snapshot.config.model.clone(),
                        prompt_version: "tts.request.v1".into(),
                        provider_fingerprint: fingerprint.clone(),
                        now: now(),
                    },
                ))?;
                let duration = cijian_platform::audio::validate_audio(bytes.clone())?;
                let blob_key = format!("{}.wav", digest(&bytes));
                let path = cache.join(&blob_key);
                let mut temporary = tempfile::NamedTempFile::new_in(&cache).map_err(|_| {
                    AppError::new(ErrorCode::StorageUnavailable, "无法写入音频缓存")
                })?;
                temporary.write_all(&bytes).map_err(|_| {
                    AppError::new(ErrorCode::StorageUnavailable, "音频缓存写入失败")
                })?;
                temporary.as_file().sync_all().map_err(|_| {
                    AppError::new(ErrorCode::StorageUnavailable, "音频缓存同步失败")
                })?;
                temporary.persist(&path).map_err(|_| {
                    AppError::new(ErrorCode::StorageUnavailable, "无法保存音频缓存")
                })?;
                self.store.save_audio(SaveAudio {
                    profile: self.profile.clone(),
                    operation: format!("audio-save-{operation}"),
                    event: id(),
                    reservation: reservation.id.clone(),
                    asset: AudioAsset {
                        id: id(),
                        cache_key,
                        text_hash: article.body_hash.clone(),
                        provider_fingerprint: fingerprint,
                        model: snapshot.config.model.clone(),
                        voice,
                        format: "wav".into(),
                        blob_key,
                        duration,
                        chars: reservation.chars,
                    },
                    now: now(),
                })
            })();
            match outcome {
                Ok(asset) => asset,
                Err(error) => {
                    self.store.fail_audio(&self.profile, &reservation.id)?;
                    return Err(error);
                }
            }
        };
        let _gate = self
            .playback_gate
            .lock()
            .map_err(|_| AppError::new(ErrorCode::StorageUnavailable, "播放器暂不可用"))?;
        if self.audio_epoch.load(std::sync::atomic::Ordering::SeqCst) != ticket {
            return Err(AppError::new(
                ErrorCode::Cancelled,
                "朗读已取消，缓存已保留",
            ));
        }
        let session = id();
        let (settings, _) = self.store.device_preferences(&self.profile, &self.device)?;
        self.store.open_audio_session(AudioSessionCommand {
            profile: self.profile.clone(),
            operation: format!("play-{session}"),
            event: id(),
            session: session.clone(),
            article: article.id.clone(),
            audio: Some(asset.id),
            source: "remote".into(),
            rate: settings.playback_rate,
            now: now(),
        })?;
        self.audio_command(AudioCommand::Play {
            path: cache.join(asset.blob_key),
            session,
            article: article.id,
            rate: settings.playback_rate,
        })
    }
}
