//! Owns the output stream and the only active sink on one dedicated thread.
#[cfg(test)]
mod tests;
use crate::timestretch::stretch;
use cijian_domain::audio::{Playback, PlaybackState};
use cijian_ports::{AppError, ErrorCode, Result};
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};
use std::{io::BufReader, path::PathBuf, sync::mpsc, time::Duration};
#[derive(Debug, Clone)]
pub enum AudioCommand {
    Play {
        path: PathBuf,
        session: String,
        article: String,
        rate: f64,
    },
    /// Plays one short word recording on its own sink so article state is untouched.
    /// Words play at their recorded speed; only whole articles follow the rate.
    PlayWord {
        path: PathBuf,
    },
    Pause,
    Resume,
    Stop,
    Seek(f64),
    Rate(f64),
    Status,
}
pub fn validate_audio(bytes: Vec<u8>) -> Result<Option<f64>> {
    let source = Decoder::try_from(std::io::Cursor::new(bytes))
        .map_err(|_| AppError::new(ErrorCode::LlmOutputInvalid, "服务返回的音频无法解码"))?;
    Ok(source
        .total_duration()
        .map(|duration| duration.as_secs_f64()))
}

pub struct NativeAudio {
    sender: Option<mpsc::Sender<(AudioCommand, mpsc::SyncSender<Result<Playback>>)>>,
    worker: Option<std::thread::JoinHandle<()>>,
}
/// Decodes one file and wraps it so `rate` changes speed without moving the pitch.
fn open_stretched(
    path: &std::path::Path,
    rate: f64,
) -> Result<(impl Source<Item = f32> + use<>, Option<f64>)> {
    let file = std::fs::File::open(path).map_err(|_| failure("音频缓存文件不可用"))?;
    let decoder =
        Decoder::try_from(BufReader::new(file)).map_err(|_| failure("音频格式无法解码"))?;
    let total = decoder.total_duration().map(|total| total.as_secs_f64());
    Ok((stretch(decoder, rate), total))
}
fn failure(message: &str) -> AppError {
    AppError::new(ErrorCode::UnsupportedCapability, message)
}
impl NativeAudio {
    pub fn new() -> Self {
        Self::with_completion(|_, _| Ok(()))
    }
    pub fn with_completion(
        mut on_complete: impl FnMut(Playback, i64) -> Result<()> + Send + 'static,
    ) -> Self {
        let (sender, receiver) =
            mpsc::channel::<(AudioCommand, mpsc::SyncSender<Result<Playback>>)>();
        let worker = std::thread::spawn(move || {
            let mut stream: Option<OutputStream> = None;
            let mut sink: Option<Sink> = None;
            let mut word: Option<Sink> = None;
            let mut state = Playback::default();
            // Path and original (unscaled) duration of the article in the sink.
            let mut current: Option<(PathBuf, Option<f64>)> = None;
            // Offset of the current sink start in the played timeline, so a
            // rebuilt sink still reports the real position.
            let mut position_base = 0.;
            let mut completions = std::collections::VecDeque::new();
            let mut completion_error = None;
            loop {
                if let Some(sink) = &sink
                    && state.state == PlaybackState::Playing
                {
                    state.position = position_base + sink.get_pos().as_secs_f64();
                    if sink.empty()
                        && let Some(session) = state.session.clone()
                    {
                        state.ended(&session);
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|time| time.as_millis().try_into().unwrap_or(i64::MAX))
                            .unwrap_or(0);
                        completions.push_back((state.clone(), now));
                    }
                }
                if let Some((completed, at)) = completions.front() {
                    match on_complete(completed.clone(), *at) {
                        Ok(()) => {
                            completions.pop_front();
                            completion_error = None;
                        }
                        Err(error) => completion_error = Some(error),
                    }
                }
                let (command, response) = match receiver.recv_timeout(Duration::from_millis(100)) {
                    Ok(command) => command,
                    Err(mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                };
                let result = (|| {
                    match command {
                        AudioCommand::Play {
                            path,
                            session,
                            article,
                            rate,
                        } => {
                            if let Some(old) = sink.take() {
                                old.stop();
                            }
                            state.stop();
                            if stream.is_none() {
                                stream = Some(
                                    OutputStreamBuilder::open_default_stream()
                                        .map_err(|_| failure("没有可用的音频输出设备"))?,
                                );
                            }
                            let (source, original) = open_stretched(&path, rate)?;
                            let next =
                                Sink::connect_new(stream.as_ref().expect("opened output").mixer());
                            state.set_rate(rate).map_err(|e| failure(&e.to_string()))?;
                            let duration = original.map(|total| total / rate);
                            position_base = 0.;
                            current = Some((path, original));
                            next.append(source);
                            state
                                .start(session, article, duration)
                                .map_err(|e| failure(&e.to_string()))?;
                            sink = Some(next);
                        }
                        AudioCommand::PlayWord { path } => {
                            if let Some(old) = word.take() {
                                old.stop();
                            }
                            if stream.is_none() {
                                stream = Some(
                                    OutputStreamBuilder::open_default_stream()
                                        .map_err(|_| failure("没有可用的音频输出设备"))?,
                                );
                            }
                            let file = std::fs::File::open(path)
                                .map_err(|_| failure("单词发音文件不可用"))?;
                            let decoder = Decoder::try_from(BufReader::new(file))
                                .map_err(|_| failure("单词发音无法解码"))?;
                            let next =
                                Sink::connect_new(stream.as_ref().expect("opened output").mixer());
                            next.append(decoder);
                            word = Some(next);
                        }
                        AudioCommand::Pause => {
                            if let Some(sink) = &sink {
                                sink.pause();
                                state.pause();
                            }
                        }
                        AudioCommand::Resume => {
                            if let Some(sink) = &sink {
                                sink.play();
                                state.resume();
                            }
                        }
                        AudioCommand::Stop => {
                            current = None;
                            position_base = 0.;
                            if let Some(sink) = sink.take() {
                                sink.stop();
                            }
                            if let Some(word) = word.take() {
                                word.stop();
                            }
                            state.stop();
                        }
                        AudioCommand::Seek(position) => {
                            if !position.is_finite() || position < 0. {
                                return Err(failure("音频位置无效"));
                            }
                            let sink =
                                sink.as_ref().ok_or_else(|| failure("没有正在播放的音频"))?;
                            let mut next = state.clone();
                            next.seek(position).map_err(|e| failure(&e.to_string()))?;
                            sink.try_seek(
                                Duration::try_from_secs_f64(next.position)
                                    .map_err(|_| failure("音频位置超出范围"))?,
                            )
                            .map_err(|_| failure("当前音频不支持定位"))?;
                            position_base = 0.;
                            state = next;
                        }
                        AudioCommand::Rate(rate) => {
                            // Resampling would move the pitch, so rebuild the stretched
                            // source at the new rate and keep the same place in the file.
                            let previous = state.rate;
                            state.set_rate(rate).map_err(|e| failure(&e.to_string()))?;
                            if let Some((path, original)) = current.clone() {
                                let played = state.position * previous / rate;
                                let (mut source, _) = open_stretched(&path, rate)?;
                                if played > 0. {
                                    source
                                        .try_seek(Duration::from_secs_f64(played))
                                        .map_err(|_| failure("当前音频不支持定位"))?;
                                }
                                if let Some(old) = sink.take() {
                                    old.stop();
                                }
                                let next = Sink::connect_new(
                                    stream.as_ref().expect("opened output").mixer(),
                                );
                                next.append(source);
                                if state.state == PlaybackState::Paused {
                                    next.pause();
                                }
                                position_base = played;
                                state.position = played;
                                state.duration = original.map(|total| total / rate);
                                sink = Some(next);
                            }
                        }
                        AudioCommand::Status => {
                            if let Some(error) = &completion_error {
                                return Err(error.clone());
                            }
                        }
                    }
                    Ok(state.clone())
                })();
                let _ = response.send(result);
            }
        });
        Self {
            sender: Some(sender),
            worker: Some(worker),
        }
    }
    pub fn command(&self, command: AudioCommand) -> Result<Playback> {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.sender
            .as_ref()
            .ok_or_else(|| failure("播放器已停止"))?
            .send((command, sender))
            .map_err(|_| failure("播放器已停止"))?;
        receiver.recv().map_err(|_| failure("播放器已停止"))?
    }
}
impl Drop for NativeAudio {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
impl Default for NativeAudio {
    fn default() -> Self {
        Self::new()
    }
}
