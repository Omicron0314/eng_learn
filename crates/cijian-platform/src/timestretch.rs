//! Pitch-preserving playback speed (WSOLA) on top of any decoded source.
//!
//! `Sink::set_speed` only resamples, so it changes the pitch. This adapter
//! overlap-adds Hann-windowed hops at a rate-dependent analysis stride and
//! realigns every hop against the natural continuation of the previous one, so
//! the waveform stays periodic and the perceived pitch does not move.
#[cfg(test)]
mod tests;
use rodio::Source;
use std::{collections::VecDeque, time::Duration};

const WINDOW_MS: f64 = 40.;
const SEARCH_MS: f64 = 8.;

fn identical(speed: f64) -> bool {
    (speed - 1.).abs() < 1e-9
}

/// Returns the source unchanged at 1× so ordinary playback costs nothing extra.
pub fn stretch<S: Source<Item = f32>>(inner: S, speed: f64) -> TimeStretch<S> {
    TimeStretch::new(inner, speed)
}

pub struct TimeStretch<S> {
    inner: S,
    speed: f64,
    pass_through: bool,
    channels: usize,
    sample_rate: u32,
    window: Vec<f32>,
    hop_out: usize,
    hop_in: f64,
    search: usize,
    /// Interleaved input samples; `input_start` is the absolute frame index of
    /// the first buffered frame.
    input: Vec<f32>,
    input_start: f64,
    /// Absolute frame index of the next synthesized hop.
    analysis: f64,
    /// Tail of the previous hop that must be added to the next hop's head.
    overlap: Vec<f32>,
    /// Natural continuation of the previous hop, used to pick an alignment.
    template: Vec<f32>,
    output: VecDeque<f32>,
    current_span_len: Option<usize>,
    total_duration: Option<Duration>,
    ended: bool,
}
impl<S: Source<Item = f32>> TimeStretch<S> {
    /// Explicit channel count, so tests can drive a synthetic source.
    pub fn with_format(inner: S, speed: f64, channels: usize, sample_rate: u32) -> Self {
        let mut stretch = Self::new(inner, speed);
        stretch.channels = channels;
        stretch.sample_rate = sample_rate;
        stretch
    }
    fn new(inner: S, speed: f64) -> Self {
        let channels = usize::from(inner.channels());
        let sample_rate = inner.sample_rate();
        let total_duration = inner.total_duration().map(|total| {
            if identical(speed) {
                total
            } else {
                Duration::from_secs_f64(total.as_secs_f64() / speed)
            }
        });
        let window_frames = ((WINDOW_MS * f64::from(sample_rate) / 1000.) as usize).max(2);
        let hop_out = (window_frames / 2).max(1);
        // Periodic Hann at 50% overlap sums to one, so overlap-add is unity gain.
        let window = (0..window_frames)
            .map(|index| {
                (0.5 * (1.
                    - (2. * std::f64::consts::PI * index as f64 / window_frames as f64).cos()))
                    as f32
            })
            .collect::<Vec<_>>();
        Self {
            pass_through: identical(speed),
            speed,
            channels: channels.max(1),
            sample_rate,
            window,
            hop_out,
            hop_in: hop_out as f64 * speed,
            search: ((SEARCH_MS * f64::from(sample_rate) / 1000.) as usize).max(1),
            input: Vec::new(),
            input_start: 0.,
            analysis: 0.,
            overlap: Vec::new(),
            template: Vec::new(),
            output: VecDeque::new(),
            current_span_len: Some(hop_out),
            total_duration,
            ended: false,
            inner,
        }
    }
    fn frames(&self) -> usize {
        self.window.len()
    }
    fn first_frame(&self) -> usize {
        self.input_start as usize
    }
    fn available(&self) -> usize {
        self.input_start as usize + self.input.len() / self.channels
    }
    /// Buffers every frame referenced by `analysis .. analysis + span`, or ends.
    fn fill(&mut self, span: usize) {
        while self.available() < self.analysis as usize + span + 1 {
            if self.ended {
                return;
            }
            let before = self.input.len();
            for sample in 0..self.channels {
                match self.inner.next() {
                    Some(value) => {
                        self.input.push(if value.is_finite() { value } else { 0. });
                    }
                    None => {
                        self.ended = true;
                        self.input
                            .extend(std::iter::repeat_n(0., self.channels - sample));
                        break;
                    }
                }
            }
            if self.input.len() == before {
                return;
            }
        }
    }
    fn sample_at(&self, frame: usize, channel: usize) -> f32 {
        if frame < self.first_frame() {
            return 0.;
        }
        let offset = (frame - self.first_frame()) * self.channels + channel;
        self.input.get(offset).copied().unwrap_or(0.)
    }
    /// Picks the offset whose window best continues the previous hop.
    fn aligned(&self, base: usize) -> usize {
        let template = self.template.len() / self.channels;
        if template == 0 {
            return base;
        }
        let low = base.saturating_sub(self.search).max(self.first_frame());
        let high = base + self.search;
        let mut best = low;
        let mut best_score = f64::MIN;
        for candidate in low..=high {
            let mut score = 0.;
            for frame in 0..template {
                for channel in 0..self.channels {
                    score += f64::from(self.sample_at(candidate + frame, channel))
                        * f64::from(self.template[frame * self.channels + channel]);
                }
            }
            if score > best_score {
                best_score = score;
                best = candidate;
            }
        }
        best
    }
    fn synthesize_hop(&mut self) -> bool {
        let window = self.frames();
        let hop = self.hop_out;
        let span = window + 2 * self.search;
        self.fill(span);
        if self.available() < self.analysis as usize + window {
            return false;
        }
        let offset = self.aligned(self.analysis as usize);
        let mut mixed = vec![0.; hop * self.channels];
        for (index, value) in mixed.iter_mut().enumerate() {
            *value = self.overlap.get(index).copied().unwrap_or(0.);
        }
        for frame in 0..hop {
            let gain = self.window[frame];
            for channel in 0..self.channels {
                mixed[frame * self.channels + channel] +=
                    self.sample_at(offset + frame, channel) * gain;
            }
        }
        self.overlap = (hop..window)
            .flat_map(|frame| (0..self.channels).map(move |channel| (frame, channel)))
            .map(|(frame, channel)| self.sample_at(offset + frame, channel) * self.window[frame])
            .collect();
        self.template = (0..hop)
            .flat_map(|frame| (0..self.channels).map(move |channel| (frame, channel)))
            .map(|(frame, channel)| self.sample_at(offset + hop + frame, channel))
            .collect();
        self.output.extend(mixed);
        self.current_span_len = Some(hop);
        self.analysis += self.hop_in;
        let keep = (self.analysis as usize).saturating_sub(self.search);
        while self.first_frame() < keep && self.input.len() > self.channels {
            self.input.drain(..self.channels);
            self.input_start += 1.;
        }
        true
    }
}
impl<S: Source<Item = f32>> Iterator for TimeStretch<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.pass_through {
            return self.inner.next();
        }
        while self.output.is_empty() {
            if !self.synthesize_hop() {
                return None;
            }
        }
        self.output.pop_front()
    }
}
impl<S: Source<Item = f32>> Source for TimeStretch<S> {
    fn current_span_len(&self) -> Option<usize> {
        if self.pass_through {
            return self.inner.current_span_len();
        }
        self.current_span_len
    }
    fn channels(&self) -> u16 {
        self.channels as u16
    }
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn total_duration(&self) -> Option<Duration> {
        self.total_duration
    }
    /// Seeks in the played (stretched) timeline and maps it back to the file.
    fn try_seek(&mut self, position: Duration) -> Result<(), rodio::source::SeekError> {
        let input = if self.pass_through {
            position.as_secs_f64()
        } else {
            position.as_secs_f64() * self.speed
        };
        self.inner.try_seek(Duration::from_secs_f64(input))?;
        self.input.clear();
        self.input_start = input * f64::from(self.sample_rate);
        self.analysis = self.input_start;
        self.overlap.clear();
        self.template.clear();
        self.output.clear();
        self.ended = false;
        Ok(())
    }
}
